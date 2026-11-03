//! Resolves Fluent message IDs into text, in English or French.

use std::borrow::Cow;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

use crate::messages::Language;
use crate::{Arg, Message};

/// Turns [`Message`]s into text in one language, falling back to English.
pub struct Localizer {
    language: Language,
    bundle: FluentBundle<FluentResource>,
    /// English, used when the chosen language lacks a message (CI prevents it).
    fallback: Option<FluentBundle<FluentResource>>,
}

impl std::fmt::Debug for Localizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Localizer")
            .field("language", &self.language)
            .finish_non_exhaustive()
    }
}

fn bundle(language: Language) -> FluentBundle<FluentResource> {
    let id: LanguageIdentifier = language.tag().parse().unwrap_or_default();
    let mut bundle = FluentBundle::new(vec![id]);
    // Fluent wraps each argument in U+2068…U+2069 isolation marks by default.
    // Those are the bidirectional controls E0007 refuses: never emit them.
    bundle.set_use_isolating(false);
    // A syntax error in a .ftl file still yields every valid message;
    // `every_message_file_parses_cleanly` keeps the files error-free.
    let resource = FluentResource::try_new(language.source().to_owned())
        .unwrap_or_else(|(partial, _errors)| partial);
    // Ids are unique per file (tested), so adding a single resource cannot fail.
    let _ = bundle.add_resource(resource);
    bundle
}

impl Localizer {
    /// A localizer for `language`.
    #[must_use]
    pub fn new(language: Language) -> Self {
        let fallback = (language != Language::English).then(|| bundle(Language::English));
        Self {
            language,
            bundle: bundle(language),
            fallback,
        }
    }

    /// The language messages are written in.
    #[must_use]
    pub const fn language(&self) -> Language {
        self.language
    }

    /// The text of a message with its arguments.
    #[must_use]
    pub fn message(&self, message: &Message) -> String {
        let mut args = FluentArgs::new();
        for (name, value) in &message.args {
            let value = match value {
                Arg::Text(text) => FluentValue::from(text.as_str()),
                Arg::Number(n) => FluentValue::from(*n),
            };
            args.set(*name, value);
        }
        self.format(message.id, &args)
    }

    /// The text of a message without arguments.
    #[must_use]
    pub fn text(&self, id: &str) -> String {
        self.format(id, &FluentArgs::new())
    }

    fn format(&self, id: &str, args: &FluentArgs<'_>) -> String {
        [Some(&self.bundle), self.fallback.as_ref()]
            .into_iter()
            .flatten()
            .find_map(|bundle| format_in(bundle, id, args))
            .map_or_else(|| id.to_owned(), Cow::into_owned)
    }
}

fn format_in<'b>(
    bundle: &'b FluentBundle<FluentResource>,
    id: &str,
    args: &'b FluentArgs<'_>,
) -> Option<Cow<'b, str>> {
    let pattern = bundle.get_message(id)?.value()?;
    let mut errors = Vec::new();
    Some(bundle.format_pattern(pattern, Some(args), &mut errors))
}

#[cfg(test)]
mod tests {
    use super::Localizer;
    use crate::Message;
    use crate::messages::Language;
    use fluent_bundle::FluentResource;

    #[test]
    fn every_message_file_parses_cleanly() {
        for language in Language::ALL {
            let parsed = FluentResource::try_new(language.source().to_owned());
            assert!(
                parsed.is_ok(),
                "{language:?}: {:?}",
                parsed.err().map(|e| e.1)
            );
        }
    }

    #[test]
    fn messages_are_resolved_with_arguments_and_plurals() {
        let en = Localizer::new(Language::English);
        let fr = Localizer::new(Language::French);
        let one = Message::new("lex-invalid-character-fix-remove").number("count", 1);
        let two = Message::new("lex-invalid-character-fix-remove").number("count", 2);
        assert_eq!(en.message(&one), "remove this character");
        assert_eq!(en.message(&two), "remove these characters");
        assert_eq!(fr.message(&two), "supprime ces caractères");
        let op = Message::new("lex-strict-equality").text("operator", "===");
        assert_eq!(en.message(&op), "`===` does not exist in Peras");
        assert_eq!(fr.message(&op), "`===` n’existe pas en Peras");
    }

    #[test]
    fn output_never_contains_bidirectional_isolation_marks() {
        let fr = Localizer::new(Language::French);
        let text = fr.message(&Message::new("lex-unterminated-string-fix").text("quote", "\""));
        assert!(!text.contains(['\u{2068}', '\u{2069}']), "{text:?}");
    }

    #[test]
    fn unknown_ids_are_shown_as_is() {
        let en = Localizer::new(Language::English);
        assert_eq!(en.text("no-such-message"), "no-such-message");
    }
}
