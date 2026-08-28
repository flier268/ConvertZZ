use super::aggregator::EST_BYTES_PER_PAIR;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

pub const LCS_MATRIX_BYTES: u64 = 801 * 801 * 4;
pub const EST_LINE_BYTES: u64 = 256;
const CGROUP_UNLIMITED: u64 = 0x7FFF_FFFF_FFFF_F000;

#[derive(Clone, Debug, Default)]
pub struct MemorySample {
    pub rss_bytes: Option<u64>,
    pub cgroup_current: Option<u64>,
    pub cgroup_max: Option<u64>,
    pub aggregator_est: u64,
    pub inflight_est: u64,
    pub mem_available: Option<u64>,
    pub mem_total: Option<u64>,
}

pub trait MemorySampler: Send + Sync {
    fn sample(&self) -> MemorySample;
}

#[derive(Clone, Debug)]
pub struct MemoryPolicy {
    pub soft_bytes: Option<u64>,
    pub hard_bytes: Option<u64>,
    pub lcs_inflight: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct ResolvedMemory {
    pub soft: Option<u64>,
    pub hard: Option<u64>,
    pub kind: UsageKind,
    pub warned_estimate: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsageKind {
    Cgroup,
    Rss,
    Estimate,
}

pub struct HostMemorySampler;

impl MemorySampler for HostMemorySampler {
    fn sample(&self) -> MemorySample {
        #[cfg(target_os = "linux")]
        {
            linux_sample()
        }
        #[cfg(not(target_os = "linux"))]
        {
            MemorySample::default()
        }
    }
}

pub fn default_sampler() -> Arc<dyn MemorySampler> {
    Arc::new(HostMemorySampler)
}

pub fn usage(sample: &MemorySample) -> (u64, UsageKind) {
    if sample.cgroup_current.is_some() && sample.cgroup_max.is_some() {
        return (sample.cgroup_current.unwrap_or(0), UsageKind::Cgroup);
    }
    if let Some(rss) = sample.rss_bytes {
        return (rss, UsageKind::Rss);
    }
    (
        sample.aggregator_est.saturating_add(sample.inflight_est),
        UsageKind::Estimate,
    )
}

pub fn resolve_thresholds(
    policy: &MemoryPolicy,
    sample: &MemorySample,
    linux: bool,
) -> Result<ResolvedMemory, String> {
    let auto_budget = auto_budget(sample, linux);
    let wanted_soft = match policy.soft_bytes {
        Some(0) => None,
        Some(value) => Some(value),
        None if linux => Some(proportion(auto_budget, 0.70)),
        None => Some(1536 * 1024 * 1024),
    };
    let wanted_hard = match policy.hard_bytes {
        Some(0) => None,
        Some(value) => Some(value),
        None if linux => Some(proportion(auto_budget, 0.90)),
        None => Some(2048 * 1024 * 1024),
    };
    let clip = sample.cgroup_max;
    let soft = wanted_soft.map(|value| match clip {
        Some(max) => value.min(proportion(max, 0.70)),
        None => value,
    });
    let hard = wanted_hard.map(|value| match clip {
        Some(max) => value.min(proportion(max, 0.90)),
        None => value,
    });
    if let (Some(soft), Some(hard)) = (soft, hard) {
        if soft > hard {
            return Err("軟水位不可大於硬水位。".into());
        }
    }
    let (_, kind) = usage(sample);
    Ok(ResolvedMemory {
        soft,
        hard,
        kind,
        warned_estimate: kind == UsageKind::Estimate,
    })
}

pub fn auto_lcs_inflight(jobs: usize, soft: Option<u64>) -> usize {
    let from_soft = soft
        .map(|bytes| ((bytes as f64 * 0.20) / LCS_MATRIX_BYTES as f64).floor() as usize)
        .unwrap_or(jobs);
    from_soft.max(1).min(jobs.max(1))
}

fn auto_budget(sample: &MemorySample, linux: bool) -> u64 {
    const CAP: u64 = 32 * 1024 * 1024 * 1024;
    if let Some(max) = sample.cgroup_max {
        return max.min(CAP);
    }
    if !linux {
        return 2048 * 1024 * 1024;
    }
    if let Some(available) = sample.mem_available {
        return available.min(CAP);
    }
    if let Some(total) = sample.mem_total {
        let reserved = (2 * 1024 * 1024 * 1024).max((total as f64 * 0.20) as u64);
        return total.saturating_sub(reserved).min(CAP);
    }
    2048 * 1024 * 1024
}

fn proportion(value: u64, ratio: f64) -> u64 {
    (value as f64 * ratio) as u64
}

pub fn inflight_estimate(lcs_inflight: usize, batch_size: usize) -> u64 {
    lcs_inflight as u64 * LCS_MATRIX_BYTES + batch_size as u64 * EST_LINE_BYTES
}

pub fn aggregator_estimate(unique_pairs: usize) -> u64 {
    unique_pairs as u64 * EST_BYTES_PER_PAIR
}

#[cfg(target_os = "linux")]
fn linux_sample() -> MemorySample {
    let (cgroup_current, cgroup_max) = read_cgroup();
    let (mem_available, mem_total) = read_meminfo();
    MemorySample {
        rss_bytes: read_rss(),
        cgroup_current,
        cgroup_max,
        aggregator_est: 0,
        inflight_est: 0,
        mem_available,
        mem_total,
    }
}

#[cfg(target_os = "linux")]
fn read_rss() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kb.saturating_mul(1024));
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn read_meminfo() -> (Option<u64>, Option<u64>) {
    let text = match std::fs::read_to_string("/proc/meminfo") {
        Ok(text) => text,
        Err(_) => return (None, None),
    };
    let mut available = None;
    let mut total = None;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemAvailable:") {
            available = rest
                .split_whitespace()
                .next()
                .and_then(|item| item.parse::<u64>().ok())
                .map(|kb| kb.saturating_mul(1024));
        } else if let Some(rest) = line.strip_prefix("MemTotal:") {
            total = rest
                .split_whitespace()
                .next()
                .and_then(|item| item.parse::<u64>().ok())
                .map(|kb| kb.saturating_mul(1024));
        }
    }
    (available, total)
}

#[cfg(target_os = "linux")]
fn read_cgroup() -> (Option<u64>, Option<u64>) {
    let text = match std::fs::read_to_string("/proc/self/cgroup") {
        Ok(text) => text,
        Err(_) => return (None, None),
    };
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("0::") {
            let dir = format!("/sys/fs/cgroup{path}");
            let current = read_u64_file(&format!("{dir}/memory.current"));
            if current.is_some() {
                return (current, read_cgroup_max(&format!("{dir}/memory.max")));
            }
        }
        if let Some(path) = line.splitn(3, ':').nth(2) {
            if line.contains(":memory:") {
                let dir = format!("/sys/fs/cgroup/memory{path}");
                let current = read_u64_file(&format!("{dir}/memory.usage_in_bytes"));
                if current.is_some() {
                    return (
                        current,
                        read_cgroup_max(&format!("{dir}/memory.limit_in_bytes")),
                    );
                }
            }
        }
    }
    (None, None)
}

#[cfg(target_os = "linux")]
fn read_u64_file(path: &str) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

#[cfg(target_os = "linux")]
fn read_cgroup_max(path: &str) -> Option<u64> {
    let text = std::fs::read_to_string(path).ok()?;
    let trimmed = text.trim();
    if trimmed == "max" {
        return None;
    }
    let value: u64 = trimmed.parse().ok()?;
    if value >= CGROUP_UNLIMITED {
        None
    } else {
        Some(value)
    }
}

pub struct CountingSemaphore {
    state: Mutex<(usize, usize)>,
    cvar: Condvar,
}

pub struct Permit<'a> {
    sem: &'a CountingSemaphore,
}

impl CountingSemaphore {
    pub fn new(max: usize) -> Self {
        Self {
            state: Mutex::new((0, max.max(1))),
            cvar: Condvar::new(),
        }
    }

    pub fn set_max(&self, max: usize) {
        let mut guard = self.state.lock().unwrap_or_else(|error| error.into_inner());
        guard.1 = max.max(1);
        self.cvar.notify_all();
    }

    pub fn max(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .1
    }

    pub fn acquire(&self) -> Permit<'_> {
        let mut guard = self.state.lock().unwrap_or_else(|error| error.into_inner());
        while guard.0 >= guard.1 {
            guard = self
                .cvar
                .wait(guard)
                .unwrap_or_else(|error| error.into_inner());
        }
        guard.0 += 1;
        Permit { sem: self }
    }
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        let mut guard = self
            .sem
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        guard.0 = guard.0.saturating_sub(1);
        self.sem.cvar.notify_one();
    }
}

pub struct LcsPool {
    sem: CountingSemaphore,
    buffers: Mutex<Vec<Vec<u32>>>,
    inflight: AtomicUsize,
    peak: AtomicUsize,
}

pub struct LcsBufferGuard<'a> {
    pool: &'a LcsPool,
    buf: Option<Vec<u32>>,
    _permit: Permit<'a>,
}

impl LcsPool {
    pub fn new(size: usize) -> Self {
        let size = size.max(1);
        Self {
            sem: CountingSemaphore::new(size),
            buffers: Mutex::new((0..size).map(|_| Vec::new()).collect()),
            inflight: AtomicUsize::new(0),
            peak: AtomicUsize::new(0),
        }
    }

    pub fn resize(&self, size: usize) {
        self.sem.set_max(size.max(1));
    }

    pub fn peak(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }

    pub fn acquire(&self) -> LcsBufferGuard<'_> {
        let permit = self.sem.acquire();
        let buf = self
            .buffers
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pop()
            .unwrap_or_default();
        let now = self.inflight.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(now, Ordering::SeqCst);
        LcsBufferGuard {
            pool: self,
            buf: Some(buf),
            _permit: permit,
        }
    }
}

impl std::ops::Deref for LcsBufferGuard<'_> {
    type Target = Vec<u32>;
    fn deref(&self) -> &Self::Target {
        self.buf.as_ref().expect("buffer")
    }
}

impl std::ops::DerefMut for LcsBufferGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.buf.as_mut().expect("buffer")
    }
}

impl Drop for LcsBufferGuard<'_> {
    fn drop(&mut self) {
        self.pool.inflight.fetch_sub(1, Ordering::SeqCst);
        if let Some(buf) = self.buf.take() {
            let mut buffers = self
                .pool
                .buffers
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if buffers.len() < self.pool.sem.max() {
                buffers.push(buf);
            }
        }
    }
}

#[derive(Default)]
pub struct SampleClock {
    last: Mutex<Option<Instant>>,
}

impl SampleClock {
    pub fn due(&self) -> bool {
        let mut last = self.last.lock().unwrap_or_else(|error| error.into_inner());
        match *last {
            Some(instant) if instant.elapsed().as_millis() < 500 => false,
            _ => {
                *last = Some(Instant::now());
                true
            }
        }
    }
}

#[derive(Default)]
pub struct FakeSampler {
    pub rss_bytes: AtomicU64,
    pub cgroup_current: AtomicU64,
    pub cgroup_max: AtomicU64,
}

impl MemorySampler for FakeSampler {
    fn sample(&self) -> MemorySample {
        let rss = self.rss_bytes.load(Ordering::SeqCst);
        let current = self.cgroup_current.load(Ordering::SeqCst);
        let max = self.cgroup_max.load(Ordering::SeqCst);
        MemorySample {
            rss_bytes: (rss > 0).then_some(rss),
            cgroup_current: (current > 0).then_some(current),
            cgroup_max: (max > 0).then_some(max),
            aggregator_est: 0,
            inflight_est: 0,
            mem_available: None,
            mem_total: None,
        }
    }
}
