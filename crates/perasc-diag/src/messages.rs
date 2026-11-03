//! The Fluent message files, embedded in the compiler binary.
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
}
