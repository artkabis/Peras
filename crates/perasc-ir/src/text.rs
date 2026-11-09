//! The functions of the runtime on strings, in Rust: the interpreter's model
//! of `perasc-runtime/runtime/buffers.wat`, with the same semantics. Indices
//! count bytes; one that falls inside a character moves back to its start.

/// Whether byte `i` continues a character.
fn continues(s: &[u8], i: usize) -> bool {
    s.get(i).is_some_and(|b| b & 0xC0 == 0x80)
}

/// An index made valid: negative from the end, clamped, at a character start.
fn index(s: &[u8], i: i32) -> usize {
    let len = i64::try_from(s.len()).unwrap_or(i64::MAX);
    let mut i = i64::from(i);
    if i < 0 {
        i += len;
    }
    let mut i = usize::try_from(i.clamp(0, len)).unwrap_or(0);
    while i > 0 && i < s.len() && continues(s, i) {
        i -= 1;
    }
    i
}

/// The first index at or after `from` where `needle` is, or -1.
#[must_use]
pub fn index_of(s: &[u8], needle: &[u8], from: i32) -> i32 {
    let start = usize::try_from(from.max(0)).unwrap_or(0);
    let mut i = start;
    while i + needle.len() <= s.len() {
        if s.get(i..i + needle.len()) == Some(needle) {
            return i32::try_from(i).unwrap_or(-1);
        }
        i += 1;
    }
    -1
}

/// The bytes from `start` to `end`.
#[must_use]
pub fn slice(s: &[u8], start: i32, end: i32) -> Vec<u8> {
    let a = index(s, start);
    let b = index(s, end).max(a);
    s.get(a..b).unwrap_or_default().to_vec()
}

/// Without spaces, tabs and line breaks at both ends.
#[must_use]
pub fn trim(s: &[u8]) -> Vec<u8> {
    let blank = |b: &u8| matches!(b, b' ' | b'\t' | b'\n' | b'\r');
    let start = s.iter().position(|b| !blank(b)).unwrap_or(s.len());
    let end = s.iter().rposition(|b| !blank(b)).map_or(start, |e| e + 1);
    s.get(start..end.max(start)).unwrap_or_default().to_vec()
}

/// `s` with `from` replaced by `to`: the first one, or all.
#[must_use]
pub fn replace(s: &[u8], from: &[u8], to: &[u8], all: bool) -> Vec<u8> {
    if from.is_empty() {
        return s.to_vec();
    }
    let mut out = Vec::with_capacity(s.len());
    let mut i = 0;
    while let Ok(at) = usize::try_from(index_of(s, from, i32::try_from(i).unwrap_or(i32::MAX))) {
        out.extend_from_slice(s.get(i..at).unwrap_or_default());
        out.extend_from_slice(to);
        i = at + from.len();
        if !all {
            break;
        }
    }
    out.extend_from_slice(s.get(i..).unwrap_or_default());
    out
}

/// The parts between each separator; with an empty one, the characters.
#[must_use]
pub fn split(s: &[u8], separator: &[u8]) -> Vec<Vec<u8>> {
    if separator.is_empty() {
        let mut parts = Vec::new();
        let mut i = 0;
        while i < s.len() {
            let mut j = i + 1;
            while j < s.len() && continues(s, j) {
                j += 1;
            }
            parts.push(s.get(i..j).unwrap_or_default().to_vec());
            i = j;
        }
        return parts;
    }
    let mut parts = Vec::new();
    let mut i = 0;
    while let Ok(at) = usize::try_from(index_of(s, separator, i32::try_from(i).unwrap_or(i32::MAX)))
    {
        parts.push(s.get(i..at).unwrap_or_default().to_vec());
        i = at + separator.len();
    }
    parts.push(s.get(i..).unwrap_or_default().to_vec());
    parts
}

/// The integer `s` writes: a sign or none, then at least one decimal digit
/// and nothing else, in the range of a signed integer of `bits` bits.
#[must_use]
pub fn parse_int(s: &[u8], bits: i32) -> Option<i64> {
    let (negative, digits) = match s.split_first() {
        Some((b'-', rest)) => (true, rest),
        Some((b'+', rest)) => (false, rest),
        _ => (false, s),
    };
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let shift = u32::try_from(bits.clamp(1, 64) - 1).ok()?;
    let limit = (1u64 << shift) - 1 + u64::from(negative);
    let mut magnitude: u64 = 0;
    for &d in digits {
        magnitude = magnitude
            .checked_mul(10)?
            .checked_add(u64::from(d - b'0'))
            .filter(|&m| m <= limit)?;
    }
    Some(if negative {
        0i64.wrapping_sub_unsigned(magnitude)
    } else {
        i64::try_from(magnitude).ok()?
    })
}

#[cfg(test)]
mod tests {
    use super::{index_of, replace, slice, split, trim};

    #[test]
    fn indices_count_bytes_and_stop_at_characters() {
        assert_eq!(index_of(b"abcabc", b"c", 3), 5);
        assert_eq!(index_of(b"abc", b"", 1), 1);
        assert_eq!(index_of(b"abc", b"x", 0), -1);
        assert_eq!(slice(b"hello", 1, -1), b"ell");
        assert_eq!(slice(b"hello", 3, 1), b"");
        // `é` is two bytes: an index inside it moves back to its start.
        assert_eq!(slice("aé".as_bytes(), 0, 2), b"a");
        assert_eq!(trim(b" \t hi \n"), b"hi");
        assert_eq!(trim(b"   "), b"");
        assert_eq!(replace(b"a-b-c", b"-", b"+", false), b"a+b-c");
        assert_eq!(replace(b"a-b-c", b"-", b"", true), b"abc");
        assert_eq!(
            split(b"a,,b", b","),
            vec![b"a".to_vec(), Vec::new(), b"b".to_vec()]
        );
        assert_eq!(
            split("hé".as_bytes(), b""),
            vec![b"h".to_vec(), "é".as_bytes().to_vec()]
        );
        assert_eq!(split(b"", b","), vec![Vec::<u8>::new()]);
    }
}
