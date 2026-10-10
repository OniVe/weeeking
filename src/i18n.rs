//! Язык пользовательских текстов: `WEEEK_LANG=ru|en` (по умолчанию `ru`).
//!
//! Все тексты, которые видит пользователь (справка CLI, сообщения `store-token`,
//! описания инструментов и промптов MCP, тексты ошибок), выбираются макросами
//! [`t!`] (статическая строка) и [`tf!`] (строка с `format!`-аргументами).

/// Язык текстов, видимых пользователю.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Ru,
    En,
}

/// Разбирает значение `WEEEK_LANG`: всё, кроме явного `en`, — русский.
pub fn lang_from(value: Option<&str>) -> Lang {
    match value {
        Some(value) if value.trim().eq_ignore_ascii_case("en") => Lang::En,
        _ => Lang::Ru,
    }
}

/// Текущий язык (`WEEEK_LANG`, по умолчанию `ru`).
pub fn lang() -> Lang {
    lang_from(std::env::var("WEEEK_LANG").ok().as_deref())
}

/// Статическая строка на текущем языке: `t!("русский", "english")`.
macro_rules! t {
    ($ru:expr, $en:expr) => {
        match $crate::i18n::lang() {
            $crate::i18n::Lang::Ru => $ru,
            $crate::i18n::Lang::En => $en,
        }
    };
}

/// Форматируемая строка на текущем языке: `tf!("ru: {}", "en: {}", value)`.
macro_rules! tf {
    ($ru:literal, $en:literal $(, $args:expr)* $(,)?) => {
        match $crate::i18n::lang() {
            $crate::i18n::Lang::Ru => format!($ru $(, $args)*),
            $crate::i18n::Lang::En => format!($en $(, $args)*),
        }
    };
}

pub(crate) use t;
pub(crate) use tf;

#[cfg(test)]
mod tests {
    use super::{Lang, lang_from};

    #[test]
    fn language_parsing() {
        assert_eq!(lang_from(None), Lang::Ru);
        assert_eq!(lang_from(Some("ru")), Lang::Ru);
        assert_eq!(lang_from(Some("RU")), Lang::Ru);
        assert_eq!(lang_from(Some("en")), Lang::En);
        assert_eq!(lang_from(Some(" EN ")), Lang::En);
        assert_eq!(lang_from(Some("de")), Lang::Ru);
    }
}
