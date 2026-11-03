//! The Fluent message files, embedded in the compiler binary, and the languages they cover.
//!
//! English (`en.ftl`) is the source of truth; `fr.ftl` must hold every English key.

/// A language the compiler speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// English, the source of truth.
    English,
    /// French.
    French,
}

impl Language {
    /// Every supported language.
    pub const ALL: [Self; 2] = [Self::English, Self::French];

    /// The Fluent source for this language.
    #[must_use]
    pub const fn source(self) -> &'static str {
        match self {
            Self::English => include_str!("../messages/en.ftl"),
            Self::French => include_str!("../messages/fr.ftl"),
        }
    }

    /// The BCP 47 tag: `en` or `fr`.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::French => "fr",
        }
    }

    /// Parses a tag or a POSIX locale: `fr`, `fr-CA`, `fr_FR.UTF-8`, `EN`.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        let primary = tag.split(['-', '_', '.', '@']).next()?.to_ascii_lowercase();
        match primary.as_str() {
            "en" => Some(Self::English),
            "fr" => Some(Self::French),
            _ => None,
        }
    }

    /// The language from the environment: `PERAS_LANG`, then the POSIX locale
    /// variables in priority order (`LC_ALL`, `LC_MESSAGES`, `LANG`); English otherwise.
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_vars(|name| std::env::var(name).ok())
    }

    /// [`Language::from_env`] with an injectable lookup, for tests.
    fn from_vars(get: impl Fn(&str) -> Option<String>) -> Self {
        ["PERAS_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .filter_map(|name| get(name).filter(|value| !value.is_empty()))
            .find_map(|value| Self::from_tag(&value))
            .unwrap_or(Self::English)
    }
}

/// Whether `id` is defined (`id = …` at the start of a line) in that language's file.
#[must_use]
pub fn has_message(language: Language, id: &str) -> bool {
    language.source().lines().any(|line| {
        line.strip_prefix(id)
            .is_some_and(|rest| rest.trim_start().starts_with('='))
    })
}

#[cfg(test)]
mod tests {
    use super::{Language, has_message};

    #[test]
    fn finds_defined_messages_only() {
        assert!(has_message(Language::English, "cli-usage"));
        assert!(has_message(Language::French, "cli-usage"));
        assert!(!has_message(Language::English, "cli"));
        assert!(!has_message(Language::English, "no-such-message"));
    }

    #[test]
    fn tags_and_locales_are_recognised() {
        assert_eq!(Language::from_tag("fr"), Some(Language::French));
        assert_eq!(Language::from_tag("fr_FR.UTF-8"), Some(Language::French));
        assert_eq!(Language::from_tag("EN-us"), Some(Language::English));
        assert_eq!(Language::from_tag("de"), None);
        assert_eq!(Language::from_tag(""), None);
    }

    #[test]
    fn the_environment_picks_the_first_supported_language() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(k, _)| *k == name)
                    .map(|(_, v)| (*v).to_owned())
            }
        };
        assert_eq!(Language::from_vars(env(&[])), Language::English);
        assert_eq!(
            Language::from_vars(env(&[("LANG", "fr_FR.UTF-8")])),
            Language::French
        );
        // PERAS_LANG wins over the locale; unsupported values are skipped.
        assert_eq!(
            Language::from_vars(env(&[("PERAS_LANG", "en"), ("LANG", "fr_FR")])),
            Language::English
        );
        assert_eq!(
            Language::from_vars(env(&[("LC_ALL", "de_DE"), ("LANG", "fr_FR")])),
            Language::French
        );
    }
}
