//! Tokens produced by the lexer.

use perasc_diag::Span;

/// A token: its kind and where it sits in the source.
///
/// Tokens carry no text; slice the source with [`Token::text`] when needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// What the token is.
    pub kind: TokenKind,
    /// Its bytes in the source.
    pub span: Span,
}

impl Token {
    /// The source text of the token, or `""` if the span does not fit the source.
    #[must_use]
    pub fn text(self, source: &str) -> &str {
        let (Ok(start), Ok(end)) = (
            usize::try_from(self.span.start),
            usize::try_from(self.span.end),
        ) else {
            return "";
        };
        source.get(start..end).unwrap_or("")
    }
}

/// The kinds of token.
///
/// `<<` and `>>` are never single tokens: the parser joins adjacent `<` or `>`,
/// so that nested generics such as `Array<Option<f32>>` close cleanly.
/// Contextual words (`from`, `of`, `as`, `test`) and type names (`i32`, `void`)
/// are identifiers. Words that TypeScript has and Peras removed (`var`, `null`,
/// `this`…) are identifiers too, so the parser can explain what replaces them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // Names and literals.
    /// `count`, `Buffer`, `i32`.
    Ident,
    /// `42`, `0xFF`, `1_000`.
    Int,
    /// `0.5`, `1e3`, `2.5e-4`.
    Float,
    /// `"text"` or `'text'`, quotes included.
    String,

    // Keywords.
    /// `async`
    Async,
    /// `await`
    Await,
    /// `break`
    Break,
    /// `class`
    Class,
    /// `const`
    Const,
    /// `continue`
    Continue,
    /// `else`
    Else,
    /// `enum`
    Enum,
    /// `export`
    Export,
    /// `false`
    False,
    /// `fn`
    Fn,
    /// `for`
    For,
    /// `if`
    If,
    /// `impl`
    Impl,
    /// `import`
    Import,
    /// `inout`
    Inout,
    /// `interface`
    Interface,
    /// `let`
    Let,
    /// `match`
    Match,
    /// `return`
    Return,
    /// `sink`
    Sink,
    /// `struct`
    Struct,
    /// `trait`
    Trait,
    /// `true`
    True,
    /// `type`
    Type,
    /// `while`
    While,

    // Delimiters and punctuation.
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `,`
    Comma,
    /// `;`
    Semicolon,
    /// `:`
    Colon,
    /// `.`
    Dot,
    /// `?`
    Question,
    /// `@`
    At,
    /// `=>`
    FatArrow,

    // Operators.
    /// `=`
    Eq,
    /// `==`
    EqEq,
    /// `!`
    Bang,
    /// `!=`
    BangEq,
    /// `<`
    Lt,
    /// `<=`
    LtEq,
    /// `>`
    Gt,
    /// `>=`
    GtEq,
    /// `+`
    Plus,
    /// `+=`
    PlusEq,
    /// `++`
    PlusPlus,
    /// `-`
    Minus,
    /// `-=`
    MinusEq,
    /// `--`
    MinusMinus,
    /// `*`
    Star,
    /// `*=`
    StarEq,
    /// `/`
    Slash,
    /// `/=`
    SlashEq,
    /// `%`
    Percent,
    /// `%=`
    PercentEq,
    /// `&`
    Amp,
    /// `&&`
    AmpAmp,
    /// `|`
    Pipe,
    /// `||`
    PipePipe,
    /// `^`
    Caret,
    /// `~`
    Tilde,

    /// Source the lexer already reported as an error. The parser accepts it
    /// wherever a token is expected, so one mistake gives one diagnostic.
    Error,
    /// End of the file; always the last token.
    Eof,
}

impl TokenKind {
    /// Whether this kind is a keyword.
    #[must_use]
    pub const fn is_keyword(self) -> bool {
        matches!(
            self,
            Self::Async
                | Self::Await
                | Self::Break
                | Self::Class
                | Self::Const
                | Self::Continue
                | Self::Else
                | Self::Enum
                | Self::Export
                | Self::False
                | Self::Fn
                | Self::For
                | Self::If
                | Self::Impl
                | Self::Import
                | Self::Inout
                | Self::Interface
                | Self::Let
                | Self::Match
                | Self::Return
                | Self::Sink
                | Self::Struct
                | Self::Trait
                | Self::True
                | Self::Type
                | Self::While
        )
    }
}
