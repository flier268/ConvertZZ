use super::super::error::CoreError;
use super::super::types::Direction;
use cjk_convert_rs::ConvertOptions;
use novel_segment::POSTAG;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const BUNDLED_RULES: &str = include_str!("../../../resources/conversion-specials/rules.txt");
const BUNDLED_PLACE_NAMES: &str =
    include_str!("../../../resources/conversion-specials/place-names.txt");

static SPECIALS: OnceLock<Specials> = OnceLock::new();

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dir {
    S2t,
    T2s,
    Both,
}

impl Dir {
    fn matches(self, direction: Direction) -> bool {
        match (self, direction) {
            (_, Direction::None) => false,
            (Dir::Both, _) => true,
            (Dir::S2t, Direction::S2t) => true,
            (Dir::T2s, Direction::T2s) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Skip,
    S2t,
    T2s,
    Variant,
    S2tMulti,
    T2sMulti,
    Pin,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct When {
    word_eq: Vec<String>,
    word_prefix: Vec<String>,
    next_eq: Vec<String>,
    next_prefix: Vec<String>,
    ch0: Vec<char>,
    ch1: Vec<char>,
    pos: u32,
    prev_pos: u32,
}

impl When {
    fn is_empty(&self) -> bool {
        self.word_eq.is_empty()
            && self.word_prefix.is_empty()
            && self.next_eq.is_empty()
            && self.next_prefix.is_empty()
            && self.ch0.is_empty()
            && self.ch1.is_empty()
            && self.pos == 0
            && self.prev_pos == 0
    }

    fn matches(&self, word: &str, next: Option<&str>, pos: u32, prev_pos: u32) -> bool {
        if !self.word_eq.is_empty() && !self.word_eq.iter().any(|item| item == word) {
            return false;
        }
        if !self.word_prefix.is_empty()
            && !self
                .word_prefix
                .iter()
                .any(|item| word.starts_with(item.as_str()))
        {
            return false;
        }
        if !self.next_eq.is_empty() {
            let Some(next) = next else {
                return false;
            };
            if !self.next_eq.iter().any(|item| item == next) {
                return false;
            }
        }
        if !self.next_prefix.is_empty() {
            let Some(next) = next else {
                return false;
            };
            if !self
                .next_prefix
                .iter()
                .any(|item| next.starts_with(item.as_str()))
            {
                return false;
            }
        }
        if !self.ch0.is_empty() {
            let Some(first) = word.chars().next() else {
                return false;
            };
            if !self.ch0.contains(&first) {
                return false;
            }
        }
        if !self.ch1.is_empty() {
            let mut chars = word.chars();
            let Some(_) = chars.next() else {
                return false;
            };
            let Some(second) = chars.next() else {
                return false;
            };
            if !self.ch1.contains(&second) {
                return false;
            }
        }
        if self.pos != 0 && pos & self.pos == 0 {
            return false;
        }
        if self.prev_pos != 0 && prev_pos & self.prev_pos == 0 {
            return false;
        }
        true
    }
}

#[derive(Clone, Debug)]
struct Rewrite {
    from: Vec<String>,
    to: String,
    dir: Dir,
    when: When,
}

#[derive(Clone, Debug)]
pub(crate) struct Specials {
    skip_s2t: String,
    skip_t2s: String,
    s2t_table: HashMap<char, char>,
    t2s_table: HashMap<char, char>,
    rewrites: Vec<Rewrite>,
    pinned: Vec<String>,
    /// 完整「xx鄉／xx里」及其簡體、裡、台 詞形 → 正名。
    place_canonical: HashMap<String, String>,
}

pub(crate) const PIN_POS: u32 = POSTAG::D_N;
pub(crate) const PIN_FREQ: u64 = 1000;

impl Specials {
    pub(crate) fn s2t_options(&self) -> ConvertOptions<'_> {
        ConvertOptions {
            safe: false,
            skip: &self.skip_s2t,
            table: nonempty_table(&self.s2t_table),
            ..ConvertOptions::DEFAULT
        }
    }

    pub(crate) fn t2s_options(&self) -> ConvertOptions<'_> {
        ConvertOptions {
            safe: false,
            skip: &self.skip_t2s,
            table: nonempty_table(&self.t2s_table),
            ..ConvertOptions::DEFAULT
        }
    }

    pub(crate) fn apply_token(
        &self,
        word: &str,
        next: Option<&str>,
        direction: Direction,
        pos: u32,
        prev_pos: u32,
    ) -> String {
        if direction == Direction::S2t {
            if let Some(canonical) = self.place_canonical.get(word) {
                return canonical.clone();
            }
        }
        for rule in &self.rewrites {
            if !rule.dir.matches(direction) {
                continue;
            }
            if !rule.from.iter().any(|item| word.contains(item.as_str())) {
                continue;
            }
            if !rule.when.matches(word, next, pos, prev_pos) {
                continue;
            }
            let mut output = word.to_string();
            for item in &rule.from {
                if output.contains(item.as_str()) {
                    output = output.replace(item, &rule.to);
                }
            }
            return output;
        }
        word.to_string()
    }

    pub(crate) fn pinned_words(&self) -> &[String] {
        &self.pinned
    }
}

fn nonempty_table(table: &HashMap<char, char>) -> Option<&HashMap<char, char>> {
    if table.is_empty() {
        None
    } else {
        Some(table)
    }
}

pub(crate) fn current() -> &'static Specials {
    SPECIALS.get_or_init(|| {
        load_specials().unwrap_or_else(|error| {
            panic!("無法載入 conversion-specials：{error}");
        })
    })
}

pub(crate) fn init() -> Result<(), CoreError> {
    if SPECIALS.get().is_some() {
        return Ok(());
    }
    let parsed = load_specials().map_err(|error| CoreError::new("CONVERSION_SPECIALS", error))?;
    let _ = SPECIALS.set(parsed);
    Ok(())
}

fn load_specials() -> Result<Specials, String> {
    let mut specials = parse(&load_first(&file_candidates(), BUNDLED_RULES))?;
    absorb_place_names(
        &mut specials,
        &load_first(&place_name_candidates(), BUNDLED_PLACE_NAMES),
    )?;
    Ok(specials)
}

fn load_first(candidates: &[PathBuf], bundled: &str) -> String {
    for path in candidates {
        if path.is_file() {
            if let Ok(text) = std::fs::read_to_string(path) {
                return text;
            }
        }
    }
    bundled.to_string()
}

fn file_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("CONVERTZZ_CONVERSION_SPECIALS") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            candidates.push(path.join("rules.txt"));
        } else {
            candidates.push(path);
        }
    }
    candidates.extend([
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/conversion-specials/rules.txt"),
        PathBuf::from("src-tauri/resources/conversion-specials/rules.txt"),
    ]);
    if let Some(directory) = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(Path::parent)
    {
        candidates.push(directory.join("conversion-specials/rules.txt"));
        candidates.push(directory.join("resources/conversion-specials/rules.txt"));
        candidates.push(directory.join("../lib/ConvertZZ/conversion-specials/rules.txt"));
    }
    if let Some(appdir) = std::env::var_os("APPDIR").map(PathBuf::from) {
        candidates.push(appdir.join("usr/lib/ConvertZZ/conversion-specials/rules.txt"));
        candidates.push(appdir.join("conversion-specials/rules.txt"));
    }
    candidates
}

fn place_name_candidates() -> Vec<PathBuf> {
    file_candidates()
        .into_iter()
        .map(|path| path.with_file_name("place-names.txt"))
        .collect()
}

pub(crate) fn parse(text: &str) -> Result<Specials, String> {
    let mut raw = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line_no = index + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }
        raw.push((line_no, parse_rule(line)?));
    }

    let mut context_from = HashSet::new();
    for (_, rule) in &raw {
        if !rule.when.is_empty() {
            for item in &rule.from {
                context_from.insert(item.clone());
            }
        }
    }

    let mut skip_s2t = String::new();
    let mut skip_t2s = String::new();
    let mut s2t_table = HashMap::new();
    let mut t2s_table = HashMap::new();
    let mut rewrites = Vec::new();
    let mut pinned = Vec::new();
    let mut pinned_seen = HashSet::new();
    let mut push_pin = |word: &str, pinned: &mut Vec<String>| {
        if word.chars().count() < 2 || !pinned_seen.insert(word.to_string()) {
            return;
        }
        pinned.push(word.to_string());
    };

    for (line_no, rule) in raw {
        for word in rule.when.word_eq.iter().chain(rule.when.word_prefix.iter()) {
            push_pin(word, &mut pinned);
        }
        match rule.kind {
            Kind::Pin => {
                for word in &rule.from {
                    push_pin(word, &mut pinned);
                }
            }
            Kind::Skip => {
                let chars: String = rule.from.iter().flat_map(|item| item.chars()).collect();
                push_skip(rule.dir, &chars, &mut skip_s2t, &mut skip_t2s);
            }
            Kind::S2t | Kind::T2s | Kind::Variant | Kind::S2tMulti | Kind::T2sMulti => {
                if rule.to.is_empty() {
                    return Err(format!(
                        "第 {line_no} 行：{kind} 需要 to 欄",
                        kind = kind_name(rule.kind)
                    ));
                }
                let single_char = rule.from.len() == 1
                    && rule.from[0].chars().count() == 1
                    && rule.to.chars().count() == 1
                    && rule.when.is_empty()
                    && !context_from.contains(&rule.from[0]);
                if single_char {
                    let from = rule.from[0].chars().next().expect("single char");
                    let to = rule.to.chars().next().expect("single char");
                    insert_table(rule.dir, from, to, &mut s2t_table, &mut t2s_table);
                } else {
                    rewrites.push(Rewrite {
                        from: rule.from,
                        to: rule.to,
                        dir: rule.dir,
                        when: rule.when,
                    });
                }
            }
        }
    }

    Ok(Specials {
        skip_s2t,
        skip_t2s,
        s2t_table,
        t2s_table,
        rewrites,
        pinned,
        place_canonical: HashMap::new(),
    })
}

fn absorb_place_names(specials: &mut Specials, text: &str) -> Result<(), String> {
    let mut pinned_seen: HashSet<String> = specials.pinned.iter().cloned().collect();
    let mut seen_canonical = HashSet::new();
    for (index, line) in text.lines().enumerate() {
        let line_no = index + 1;
        let name = line.trim();
        if name.is_empty() || name.starts_with('#') || name.starts_with("//") {
            continue;
        }
        if name.contains('\t') || name.contains(' ') {
            return Err(format!(
                "place-names 第 {line_no} 行：地名必須是單一完整名稱"
            ));
        }
        if name.chars().count() < 2 || !(name.ends_with('鄉') || name.ends_with('里')) {
            return Err(format!(
                "place-names 第 {line_no} 行：只收完整 xx鄉／xx里，收到「{name}」"
            ));
        }
        if !seen_canonical.insert(name.to_string()) {
            continue;
        }
        for form in place_name_forms(name) {
            specials
                .place_canonical
                .entry(form.clone())
                .or_insert_with(|| name.to_string());
            if form.chars().count() >= 2 && pinned_seen.insert(form.clone()) {
                specials.pinned.push(form);
            }
        }
    }
    Ok(())
}

fn place_name_forms(canonical: &str) -> Vec<String> {
    let mut forms = HashSet::new();
    let mut bases = vec![canonical.to_string()];
    for (from, to) in [("臺", "台"), ("台", "臺"), ("庄", "莊"), ("莊", "庄")] {
        if canonical.contains(from) {
            bases.push(canonical.replace(from, to));
        }
    }
    for base in bases {
        forms.insert(base.clone());
        if let Some(stem) = base.strip_suffix('鄉') {
            forms.insert(format!("{stem}乡"));
            if stem.contains('里') {
                let stem_li = stem.replace('里', "裡");
                forms.insert(format!("{stem_li}鄉"));
                forms.insert(format!("{stem_li}乡"));
            }
        } else if let Some(stem) = base.strip_suffix('里') {
            forms.insert(format!("{stem}裡"));
            forms.insert(format!("{stem}裏"));
        }
    }
    forms.into_iter().collect()
}

struct ParsedRule {
    kind: Kind,
    from: Vec<String>,
    to: String,
    dir: Dir,
    when: When,
}

fn parse_rule(line: &str) -> Result<ParsedRule, String> {
    let line_display = line.trim();
    let fields: Vec<&str> = line.split('\t').map(str::trim).collect();
    if fields.is_empty() || fields[0].is_empty() {
        return Err(format!("無法解析規則：{line_display}"));
    }
    let kind = parse_kind(fields[0]).ok_or_else(|| {
        format!(
            "未知 kind「{}」。可用 skip / s2t / t2s / variant / s2t-multi / t2s-multi / pin。",
            fields[0]
        )
    })?;
    if fields.len() < 2 || fields[1].is_empty() {
        return Err(format!("{kind} 缺少 from", kind = kind_name(kind)));
    }
    let from = split_alts(fields[1]);
    let to = fields.get(2).copied().unwrap_or("").to_string();
    let dir = match fields.get(3).copied().unwrap_or("") {
        "" => default_dir(kind),
        value => parse_dir(value)
            .ok_or_else(|| format!("未知 dir「{value}」。可用 s2t / t2s / both。"))?,
    };
    let when = match fields.get(4).copied().unwrap_or("") {
        "" => When::default(),
        value => parse_when(value)?,
    };
    Ok(ParsedRule {
        kind,
        from,
        to,
        dir,
        when,
    })
}

fn parse_kind(value: &str) -> Option<Kind> {
    Some(match value {
        "skip" => Kind::Skip,
        "s2t" => Kind::S2t,
        "t2s" => Kind::T2s,
        "variant" => Kind::Variant,
        "s2t-multi" => Kind::S2tMulti,
        "t2s-multi" => Kind::T2sMulti,
        "pin" => Kind::Pin,
        _ => return None,
    })
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Skip => "skip",
        Kind::S2t => "s2t",
        Kind::T2s => "t2s",
        Kind::Variant => "variant",
        Kind::S2tMulti => "s2t-multi",
        Kind::T2sMulti => "t2s-multi",
        Kind::Pin => "pin",
    }
}

fn default_dir(kind: Kind) -> Dir {
    match kind {
        Kind::Skip | Kind::S2t | Kind::Variant | Kind::S2tMulti | Kind::Pin => Dir::S2t,
        Kind::T2s | Kind::T2sMulti => Dir::T2s,
    }
}

fn parse_dir(value: &str) -> Option<Dir> {
    Some(match value {
        "s2t" => Dir::S2t,
        "t2s" => Dir::T2s,
        "both" => Dir::Both,
        _ => return None,
    })
}

fn split_alts(value: &str) -> Vec<String> {
    value
        .split('|')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_when(raw: &str) -> Result<When, String> {
    let mut when = When::default();
    for part in raw.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some(value) = part.strip_prefix("word^=") {
            when.word_prefix.extend(split_alts(value));
        } else if let Some(value) = part.strip_prefix("word=") {
            when.word_eq.extend(split_alts(value));
        } else if let Some(value) = part.strip_prefix("next^=") {
            when.next_prefix.extend(split_alts(value));
        } else if let Some(value) = part.strip_prefix("next=") {
            when.next_eq.extend(split_alts(value));
        } else if let Some(value) = part.strip_prefix("ch0=") {
            when.ch0.extend(parse_chars(value, "ch0")?);
        } else if let Some(value) = part.strip_prefix("ch1=") {
            when.ch1.extend(parse_chars(value, "ch1")?);
        } else if let Some(value) = part.strip_prefix("pos=") {
            when.pos |= parse_pos_mask(value).ok_or_else(|| format!("無法解析 pos={value}"))?;
        } else if let Some(value) = part.strip_prefix("prev-pos=") {
            when.prev_pos |=
                parse_pos_mask(value).ok_or_else(|| format!("無法解析 prev-pos={value}"))?;
        } else {
            return Err(format!(
                "未知 when「{part}」。可用 word= / word^= / next= / next^= / ch0= / ch1= / pos= / prev-pos="
            ));
        }
    }
    Ok(when)
}

fn parse_chars(value: &str, field: &str) -> Result<Vec<char>, String> {
    let mut chars = Vec::new();
    for item in split_alts(value) {
        let mut it = item.chars();
        let Some(ch) = it.next() else {
            continue;
        };
        if it.next().is_some() {
            return Err(format!("{field} 必須是單字，收到「{item}」"));
        }
        chars.push(ch);
    }
    Ok(chars)
}

fn parse_pos_mask(raw: &str) -> Option<u32> {
    if raw.is_empty() {
        return None;
    }
    if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        return u32::from_str_radix(hex, 16).ok();
    }
    let mut mask = 0u32;
    for part in raw.split('+') {
        let name = part.trim();
        if name.is_empty() {
            return None;
        }
        mask |= pos_name_bit(name)?;
    }
    Some(mask)
}

fn pos_name_bit(name: &str) -> Option<u32> {
    Some(match name {
        "D_A" => POSTAG::D_A,
        "D_B" => POSTAG::D_B,
        "D_C" => POSTAG::D_C,
        "D_D" => POSTAG::D_D,
        "D_E" => POSTAG::D_E,
        "D_F" => POSTAG::D_F,
        "D_I" => POSTAG::D_I,
        "D_L" => POSTAG::D_L,
        "A_M" => POSTAG::A_M,
        "D_MQ" => POSTAG::D_MQ,
        "D_N" => POSTAG::D_N,
        "D_O" => POSTAG::D_O,
        "D_P" => POSTAG::D_P,
        "A_Q" => POSTAG::A_Q,
        "D_R" => POSTAG::D_R,
        "D_S" => POSTAG::D_S,
        "D_T" => POSTAG::D_T,
        "D_U" => POSTAG::D_U,
        "D_V" => POSTAG::D_V,
        "D_W" => POSTAG::D_W,
        "D_X" => POSTAG::D_X,
        "D_Y" => POSTAG::D_Y,
        "D_Z" => POSTAG::D_Z,
        "A_NR" => POSTAG::A_NR,
        "A_NS" => POSTAG::A_NS,
        "A_NT" => POSTAG::A_NT,
        "A_NX" => POSTAG::A_NX,
        "A_NZ" => POSTAG::A_NZ,
        "UNK" => POSTAG::UNK,
        _ => return None,
    })
}

fn push_skip(dir: Dir, chars: &str, skip_s2t: &mut String, skip_t2s: &mut String) {
    match dir {
        Dir::S2t => append_unique(skip_s2t, chars),
        Dir::T2s => append_unique(skip_t2s, chars),
        Dir::Both => {
            append_unique(skip_s2t, chars);
            append_unique(skip_t2s, chars);
        }
    }
}

fn append_unique(target: &mut String, chars: &str) {
    for ch in chars.chars() {
        if !target.contains(ch) {
            target.push(ch);
        }
    }
}

fn insert_table(
    dir: Dir,
    from: char,
    to: char,
    s2t: &mut HashMap<char, char>,
    t2s: &mut HashMap<char, char>,
) {
    match dir {
        Dir::S2t => {
            s2t.entry(from).or_insert(to);
        }
        Dir::T2s => {
            t2s.entry(from).or_insert(to);
        }
        Dir::Both => {
            s2t.entry(from).or_insert(to);
            t2s.entry(from).or_insert(to);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_rules_parse() {
        let specials = parse(BUNDLED_RULES).expect("bundled rules");
        assert!(specials.skip_s2t.contains('璇'));
        assert!(specials.skip_s2t.contains('疱'));
        assert_eq!(specials.s2t_table.get(&'皰'), Some(&'疱'));
        assert_eq!(
            specials.apply_token("什么", None, Direction::S2t, 0, 0),
            "什麼"
        );
        assert_eq!(
            specials.apply_token("老么", None, Direction::S2t, 0, 0),
            "老么"
        );
        assert_eq!(
            specials.apply_token("老幺", None, Direction::S2t, 0, 0),
            "老么"
        );
        assert_eq!(
            specials.apply_token("么", Some("兒"), Direction::S2t, 0, 0),
            "么"
        );
        assert_eq!(
            specials.apply_token("么兒", None, Direction::S2t, 0, 0),
            "么兒"
        );
        assert_eq!(
            specials.apply_token("幺女", None, Direction::S2t, 0, 0),
            "么女"
        );
        assert_eq!(
            specials.apply_token("好么", None, Direction::S2t, 0, 0),
            "好麼"
        );
        for word in [
            "和牛", "胜肽", "勝肽", "本里", "本裡", "里辦", "裡辦", "里民", "裡民", "里長", "裡長",
            "里名", "裡名", "老么", "老幺",
        ] {
            assert!(
                specials.pinned_words().iter().any(|item| item == word),
                "pinned missing {word}: {:?}",
                specials.pinned_words()
            );
        }
        assert_eq!(
            specials.apply_token("勝肽", None, Direction::S2t, 0, 0),
            "胜肽"
        );
        assert_eq!(
            specials.apply_token("胜肽", None, Direction::S2t, 0, 0),
            "胜肽"
        );
        assert_eq!(
            specials.apply_token("勝利", None, Direction::S2t, 0, 0),
            "勝利"
        );
        assert_eq!(
            specials.apply_token("裡長", None, Direction::S2t, 0, 0),
            "里長"
        );
        assert_eq!(
            specials.apply_token("本裡", None, Direction::S2t, 0, 0),
            "本里"
        );
    }

    fn bundled_with_places() -> Specials {
        let mut specials = parse(BUNDLED_RULES).expect("rules");
        absorb_place_names(&mut specials, BUNDLED_PLACE_NAMES).expect("places");
        specials
    }

    #[test]
    fn bundled_place_names_pin_complete_xiang_li_only() {
        let specials = bundled_with_places();
        for word in [
            "三星鄉",
            "三星乡",
            "莊敬里",
            "莊敬裡",
            "水里鄉",
            "水裡鄉",
            "南庄鄉",
            "南莊鄉",
            "臺西鄉",
            "台西鄉",
            "太麻里鄉",
            "里港鄉",
            "夢裡里",
        ] {
            assert!(
                specials.pinned_words().iter().any(|item| item == word),
                "pinned missing {word}"
            );
        }
        for word in ["羅東鎮", "竹北市", "板橋區", "鄉", "里", "這裡", "公里"] {
            assert!(
                !specials.pinned_words().iter().any(|item| item == word),
                "must not pin incomplete or non-鄉里 name {word}"
            );
        }
        assert_eq!(
            specials.apply_token("莊敬裡", None, Direction::S2t, 0, 0),
            "莊敬里"
        );
        assert_eq!(
            specials.apply_token("三星乡", None, Direction::S2t, 0, 0),
            "三星鄉"
        );
        assert_eq!(
            specials.apply_token("水裡鄉", None, Direction::S2t, 0, 0),
            "水里鄉"
        );
        assert_eq!(
            specials.apply_token("南莊鄉", None, Direction::S2t, 0, 0),
            "南庄鄉"
        );
        assert_eq!(
            specials.apply_token("夢裡里", None, Direction::S2t, 0, 0),
            "夢裡里"
        );
        assert_eq!(
            specials.apply_token("這裡", None, Direction::S2t, 0, 0),
            "這裡"
        );
        let err = absorb_place_names(&mut parse("").expect("empty"), "羅東鎮\n").unwrap_err();
        assert!(err.contains("xx鄉"), "{err}");
    }

    #[test]
    fn protocol_covers_s2t_t2s_multi_and_variant() {
        let specials = parse(
            "\
s2t\t发\t發\ts2t
t2s\t髮\t发\tt2s
variant\t爲\t為\ts2t
skip\t乾\t\tt2s
t2s-multi\t乾\t乾\tt2s\tword^=乾隆|乾坤
t2s-multi\t乾\t干\tt2s
s2t-multi\t只\t隻\ts2t\tpos=D_MQ+A_Q
s2t-multi\t只\t只\ts2t
pin\t和牛\t\ts2t
",
        )
        .expect("parse");
        assert_eq!(specials.s2t_table.get(&'发'), Some(&'發'));
        assert_eq!(specials.t2s_table.get(&'髮'), Some(&'发'));
        assert_eq!(specials.s2t_table.get(&'爲'), Some(&'為'));
        assert!(specials.skip_t2s.contains('乾'));
        assert_eq!(
            specials.apply_token("乾隆", None, Direction::T2s, 0, 0),
            "乾隆"
        );
        assert_eq!(
            specials.apply_token("乾燥", None, Direction::T2s, 0, 0),
            "干燥"
        );
        assert_eq!(
            specials.apply_token("只", None, Direction::S2t, POSTAG::D_MQ, 0),
            "隻"
        );
        assert_eq!(
            specials.apply_token("只", None, Direction::S2t, POSTAG::D_D, 0),
            "只"
        );
        assert!(specials.pinned_words().iter().any(|item| item == "和牛"));
        assert!(specials.pinned_words().iter().any(|item| item == "乾隆"));
        assert!(specials.pinned_words().iter().any(|item| item == "乾坤"));
    }

    #[test]
    fn unknown_kind_is_rejected() {
        let err = parse("nope\t么\t麼\ts2t\n").unwrap_err();
        assert!(err.contains("未知 kind"), "{err}");
    }
}
