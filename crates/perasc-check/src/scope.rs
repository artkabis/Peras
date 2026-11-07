//! Scopes and bindings: what each name means at each point of the program.

use perasc_diag::Span;
use perasc_syntax::ast::{Convention, Symbol};

use crate::ty::{EnumId, FnId, StructId, Ty};

/// What a name is bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// A local variable; `keyword` is its `let` or `const`.
    Local {
        /// Declared with `let`.
        mutable: bool,
        /// The keyword, replaced by the `const` fix.
        keyword: Span,
    },
    /// A parameter; `type_start` is where `inout` would be inserted.
    Param {
        /// Its passing convention.
        convention: Convention,
        /// Start of its type annotation.
        type_start: u32,
    },
    /// A module-level `let` or `const`.
    Global {
        /// Declared with `let`.
        mutable: bool,
        /// The keyword.
        keyword: Span,
    },
    /// A function.
    Fn(FnId),
    /// A structure, usable as a constructor.
    Struct(StructId),
    /// An enumeration, whose variants are written `Name.Variant`.
    Enum(EnumId),
    /// A name a `match` pattern gives to a value, or a part of it.
    Pattern,
    /// `Some`, provided by the language.
    Some,
    /// `None`, provided by the language.
    None,
}

/// A declared name.
#[derive(Debug, Clone)]
pub struct Binding {
    /// The name.
    pub symbol: Symbol,
    /// Its text, for messages and suggestions.
    pub name: String,
    /// Where it is declared; `None` for names provided by the language.
    pub span: Option<Span>,
    /// What it is.
    pub kind: Kind,
    /// Its type.
    pub ty: Ty,
    /// Whether its value has been read.
    pub used: bool,
}

impl Binding {
    /// Whether an unread binding is an error: variables and parameters only, and
    /// never a name starting with `_`, which marks it unused on purpose. An
    /// `inout` parameter only written is used: the caller reads it.
    #[must_use]
    pub fn must_be_used(&self) -> bool {
        let inout = matches!(
            self.kind,
            Kind::Param {
                convention: Convention::Inout,
                ..
            }
        );
        matches!(self.kind, Kind::Local { .. } | Kind::Param { .. })
            && !inout
            && !self.name.starts_with('_')
    }
}

/// The result of declaring a name.
pub enum Declared {
    /// Declared without conflict.
    Ok,
    /// Already declared in the same scope, at this binding.
    Duplicate(usize),
    /// Hides this binding of an enclosing scope.
    Shadows(usize),
}

/// The stack of scopes. Scope 0 holds the language's names and the module's
/// declarations; each block, function and loop pushes one more.
#[derive(Debug, Default)]
pub struct Scopes {
    /// Every binding ever declared, by index.
    pub bindings: Vec<Binding>,
    /// For each open scope, the indices of its bindings.
    stack: Vec<Vec<usize>>,
}

impl Scopes {
    /// Opens a scope.
    pub fn push(&mut self) {
        self.stack.push(Vec::new());
    }

    /// Closes the innermost scope and returns its bindings.
    pub fn pop(&mut self) -> Vec<usize> {
        self.stack.pop().unwrap_or_default()
    }

    /// The innermost binding of a name.
    #[must_use]
    pub fn lookup(&self, symbol: Symbol) -> Option<usize> {
        self.stack
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .copied()
            .find(|&i| self.bindings.get(i).is_some_and(|b| b.symbol == symbol))
    }

    /// The innermost binding with this text, for suggestions.
    #[must_use]
    pub fn lookup_name(&self, name: &str) -> Option<usize> {
        self.stack
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .copied()
            .find(|&i| self.bindings.get(i).is_some_and(|b| b.name == name))
    }

    /// Declares a name in the innermost scope. A conflicting name is still
    /// declared, so later uses resolve to it without further errors.
    pub fn declare(&mut self, binding: Binding) -> Declared {
        let in_scope = self.stack.last().and_then(|scope| {
            scope.iter().copied().find(|&i| {
                self.bindings
                    .get(i)
                    .is_some_and(|b| b.symbol == binding.symbol)
            })
        });
        let outer = self.lookup(binding.symbol);
        let index = self.bindings.len();
        self.bindings.push(binding);
        if let Some(scope) = self.stack.last_mut() {
            scope.push(index);
        }
        match (in_scope, outer) {
            (Some(previous), _) => Declared::Duplicate(previous),
            (None, Some(previous)) => Declared::Shadows(previous),
            (None, None) => Declared::Ok,
        }
    }

    /// Names visible here, for "did you mean" suggestions.
    pub fn visible_names(&self) -> impl Iterator<Item = &str> {
        self.stack
            .iter()
            .flatten()
            .filter_map(|&i| self.bindings.get(i))
            .map(|b| b.name.as_str())
    }
}

/// The closest candidate to `name`, if close enough to be a likely typo.
#[must_use]
pub fn suggest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    const LONGEST: usize = 64;
    if name.len() > LONGEST {
        return None;
    }
    let limit = (name.chars().count() / 3).max(1);
    candidates
        .filter(|c| *c != name && c.len() <= LONGEST)
        .map(|c| (distance(name, c), c))
        .filter(|(d, _)| *d <= limit)
        .min_by_key(|(d, c)| (*d, *c))
        .map(|(_, c)| c)
}

/// Levenshtein distance, ignoring case differences as half a step rounded down.
fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut current = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(!ca.eq_ignore_ascii_case(cb));
            let best = (previous
                .get(j)
                .copied()
                .unwrap_or(usize::MAX)
                .saturating_add(cost))
            .min(
                previous
                    .get(j + 1)
                    .copied()
                    .unwrap_or(usize::MAX)
                    .saturating_add(1),
            )
            .min(
                current
                    .get(j)
                    .copied()
                    .unwrap_or(usize::MAX)
                    .saturating_add(1),
            );
            current.push(best);
        }
        previous = current;
    }
    previous.last().copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::suggest;

    #[test]
    fn suggests_close_names_only() {
        let names = ["total", "count", "samples", "Buffer"];
        assert_eq!(suggest("totl", names.into_iter()), Some("total"));
        assert_eq!(suggest("buffer", names.into_iter()), Some("Buffer"));
        assert_eq!(suggest("sample", names.into_iter()), Some("samples"));
        assert_eq!(suggest("xyz", names.into_iter()), None);
        assert_eq!(suggest("total", names.into_iter()), None);
    }
}
