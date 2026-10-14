//! Bounded convert parallelism. Default-on, capped by CPU and available memory.

use crate::core::roundtrip_dict::{default_sampler, MemorySample, MemorySampler};

/// Conservative peak working set per concurrent segmented convert worker.
const EST_BYTES_PER_WORKER: u64 = 96 * 1024 * 1024;
/// Hard ceiling so huge machines do not pin every core into DictTokenizer peaks.
const MAX_CONVERT_JOBS: usize = 8;
/// Floor for auto budget when host sample is unavailable.
const FALLBACK_BUDGET_BYTES: u64 = 1536 * 1024 * 1024;

/// Resolve convert worker count. `CONVERTZZ_CONVERT_JOBS` overrides auto (minimum 1).
pub fn default_convert_jobs() -> usize {
    if let Ok(raw) = std::env::var("CONVERTZZ_CONVERT_JOBS") {
        if let Ok(parsed) = raw.trim().parse::<usize>() {
            return parsed.max(1);
        }
    }
    let cpus = std::thread::available_parallelism()
        .map(|value| value.get())
        .unwrap_or(2)
        .max(1);
    let sample = MemorySampler::sample(default_sampler().as_ref());
    convert_jobs_for(cpus, &sample, MAX_CONVERT_JOBS)
}

pub fn convert_jobs_for(cpus: usize, sample: &MemorySample, max_jobs: usize) -> usize {
    let cpus = cpus.max(1);
    let max_jobs = max_jobs.max(1);
    let budget = convert_memory_budget(sample);
    let by_memory = (budget / EST_BYTES_PER_WORKER).max(1) as usize;
    cpus.min(by_memory).min(max_jobs).max(1)
}

fn convert_memory_budget(sample: &MemorySample) -> u64 {
    let current = sample.cgroup_current.or(sample.rss_bytes).unwrap_or(0);
    let headroom = sample
        .cgroup_max
        .map(|max| max.saturating_sub(current))
        .or(sample.mem_available)
        .or_else(|| {
            sample
                .mem_total
                .map(|total| total.saturating_sub(current).saturating_mul(80) / 100)
        })
        .unwrap_or(FALLBACK_BUDGET_BYTES);
    // Leave headroom for UI / OS; only spend half of remaining on convert workers.
    (headroom / 2).max(EST_BYTES_PER_WORKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jobs_shrink_when_memory_is_tight() {
        let sample = MemorySample {
            rss_bytes: Some(512 * 1024 * 1024),
            cgroup_current: Some(512 * 1024 * 1024),
            cgroup_max: Some(1024 * 1024 * 1024),
            aggregator_est: 0,
            inflight_est: 0,
            mem_available: Some(200 * 1024 * 1024),
            mem_total: Some(1024 * 1024 * 1024),
        };
        // headroom from cgroup: 512 MiB → half 256 MiB → 2 workers at 96 MiB
        assert_eq!(convert_jobs_for(8, &sample, 8), 2);
    }

    #[test]
    fn jobs_respect_cpu_and_cap() {
        let sample = MemorySample {
            rss_bytes: None,
            cgroup_current: None,
            cgroup_max: None,
            aggregator_est: 0,
            inflight_est: 0,
            mem_available: Some(16 * 1024 * 1024 * 1024),
            mem_total: Some(32 * 1024 * 1024 * 1024),
        };
        assert_eq!(convert_jobs_for(4, &sample, 8), 4);
        assert_eq!(convert_jobs_for(16, &sample, 8), 8);
    }

    #[test]
    fn conversion_service_and_segment_are_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<crate::core::ConversionService>();
        assert_sync::<novel_segment::Segment>();
    }
}
