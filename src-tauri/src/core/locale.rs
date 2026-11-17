//! 介面語言。只由 GUI 主視窗透過 `set_ui_language` 切換；無頭 CLI 維持繁體中文。

#[cfg(not(test))]
static ENGLISH: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// 測試平行執行，語系改為每執行緒各自設定，避免英文測試影響其他斷言繁中訊息的測試。
#[cfg(test)]
thread_local! {
    static ENGLISH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub fn set_ui_locale(locale: &str) {
    #[cfg(not(test))]
    ENGLISH.store(locale == "en", std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    ENGLISH.with(|english| english.set(locale == "en"));
}

pub fn is_english() -> bool {
    #[cfg(not(test))]
    return ENGLISH.load(std::sync::atomic::Ordering::Relaxed);
    #[cfg(test)]
    return ENGLISH.with(std::cell::Cell::get);
}

/// 依介面語言格式化訊息：`tr!("繁中 {x}", "English {x}", ...)`，回傳 `String`。
macro_rules! tr {
    ($zh:literal, $en:literal $(,)?) => {
        if $crate::core::locale::is_english() {
            format!($en)
        } else {
            format!($zh)
        }
    };
    ($zh:literal, $en:literal, $($arg:tt)+) => {
        if $crate::core::locale::is_english() {
            format!($en, $($arg)+)
        } else {
            format!($zh, $($arg)+)
        }
    };
}

/// 不需格式化的固定訊息：`tr_static!("繁中", "English")`，回傳 `&'static str`。
macro_rules! tr_static {
    ($zh:literal, $en:literal $(,)?) => {
        if $crate::core::locale::is_english() {
            $en
        } else {
            $zh
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_locale_selects_english_text() {
        set_ui_locale("en");
        let name = "a.txt";
        let english = tr!("正在掃描：{name}", "Scanning: {name}");
        let fixed = tr_static!("取消", "Cancel");
        set_ui_locale("zh-TW");
        assert_eq!(english, "Scanning: a.txt");
        assert_eq!(fixed, "Cancel");
        assert_eq!(tr!("正在掃描：{}", "Scanning: {}", name), "正在掃描：a.txt");
    }

    #[test]
    fn english_locale_translates_user_dictionary_problem() {
        use crate::core::user_dictionary::{
            entry_problem, UserDictionaryDirection, UserDictionaryEntry,
        };
        let entry = UserDictionaryEntry {
            simplified: String::new(),
            traditional: "軟體".into(),
            direction: UserDictionaryDirection::Both,
            imported_at: None,
            source_file: None,
        };
        set_ui_locale("en");
        let english = entry_problem(&entry);
        set_ui_locale("zh-TW");
        assert_eq!(
            english.as_deref(),
            Some("The simplified word cannot be empty.")
        );
        assert_eq!(entry_problem(&entry).as_deref(), Some("簡體詞不可空白。"));
    }

    #[test]
    fn english_locale_translates_cli_parse_error() {
        let args = vec!["--engine".to_string(), "legacy".to_string()];
        set_ui_locale("en");
        let english = crate::core::cli::parse_cli(&args, None).parse_errors;
        set_ui_locale("zh-TW");
        assert_eq!(
            english,
            vec![
                "The legacy dictionary engine was removed in 2.0. Import Dictionary.csv as a user dictionary on the User dictionary page, then use the segmented engine."
                    .to_string()
            ]
        );
    }
}
