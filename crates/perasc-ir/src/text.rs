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

/// Whether `s` is a decimal number as `parseF64` reads it: a sign or none,
/// digits with at most one point among them and at least one digit, then
/// `e` or `E`, a sign or none and digits, or nothing else. No spaces, no
/// `Infinity`, no `0x`, no `_`: stricter than JavaScript's `Number`, which
/// reads the same value from every text this accepts.
#[must_use]
pub fn is_decimal(s: &[u8]) -> bool {
    let body = match s.first() {
        Some(b'+' | b'-') => &s[1..],
        _ => s,
    };
    let (mantissa, exponent) = match body.iter().position(|&c| c == b'e' || c == b'E') {
        Some(at) => (&body[..at], Some(&body[at + 1..])),
        None => (body, None),
    };
    let digits = mantissa.iter().filter(|c| c.is_ascii_digit()).count();
    let others = mantissa.len() - digits;
    let mantissa_ok = digits >= 1 && (others == 0 || (others == 1 && mantissa.contains(&b'.')));
    let exponent_ok = exponent.is_none_or(|e| {
        let e = match e.first() {
            Some(b'+' | b'-') => &e[1..],
            _ => e,
        };
        !e.is_empty() && e.iter().all(u8::is_ascii_digit)
    });
    mantissa_ok && exponent_ok
}

/// The number `s` writes, correctly rounded, when [`is_decimal`] accepts it:
/// what `parseF64` gives, and JavaScript's `Number(s)`.
#[must_use]
pub fn parse_decimal(s: &[u8]) -> Option<f64> {
    if !is_decimal(s) {
        return None;
    }
    std::str::from_utf8(s).ok()?.parse::<f64>().ok()
}

/// What the loader's `float_text` writes for `mode`: 0 `String(x)`, 1 the
/// same for an `f32`, `2 + d` `x.toFixed(d)`.
#[must_use]
pub fn float_text(x: f64, mode: i32) -> String {
    match u32::try_from(mode - 2) {
        Ok(digits) => fixed_text(x, digits),
        Err(_) => number_text(x, mode == 1),
    }
}

/// `x.toFixed(digits)`, as JavaScript writes it: the exact value of `x`
/// rounded to `digits` decimals, a tie away from zero; `String(x)` from
/// 1e21 up, and for NaN and the infinities; `0` for `-0`, `-0.00` for a
/// negative number that rounds to zero.
#[must_use]
pub fn fixed_text(x: f64, digits: u32) -> String {
    if !x.is_finite() || x.abs() >= 1e21 {
        return number_text(x, false);
    }
    let digits = usize::try_from(digits).unwrap_or(0);
    // Every digit of the exact value: a float is a fraction of a power of
    // two, whose decimals end before the 1,075th.
    let exact = format!("{:.1100}", x.abs());
    let (whole, decimals) = exact.split_once('.').unwrap_or((&exact, ""));
    let mut kept: Vec<u8> = whole.bytes().chain(decimals.bytes().take(digits)).collect();
    // A tie or more rounds away from zero, as JavaScript picks the larger n.
    if decimals.as_bytes().get(digits).is_some_and(|&d| d >= b'5') {
        let mut i = kept.len();
        loop {
            if i == 0 {
                kept.insert(0, b'1');
                break;
            }
            i -= 1;
            if let Some(d) = kept.get_mut(i) {
                if *d == b'9' {
                    *d = b'0';
                } else {
                    *d += 1;
                    break;
                }
            }
        }
    }
    let split = kept.len() - digits;
    let mut out = String::from(if x < 0.0 { "-" } else { "" });
    out.push_str(std::str::from_utf8(kept.get(..split).unwrap_or_default()).unwrap_or("0"));
    if digits > 0 {
        out.push('.');
        out.push_str(std::str::from_utf8(kept.get(split..).unwrap_or_default()).unwrap_or(""));
    }
    out
}

/// A float as text, as JavaScript's `String()` writes it: the fewest
/// significant digits that read back as the same number, in plain notation
/// from 1e-6 up to 1e21 and in exponent notation outside; `NaN`,
/// `Infinity`, and `0` for both zeros. With `single`, `x` is an `f32`, which
/// JavaScript does not have: the fewest digits, correctly rounded, that read
/// back as the same `f32`, written the same way, so `0.1` and not
/// `0.10000000149011612`.
#[must_use]
pub fn number_text(x: f64, single: bool) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if x == 0.0 {
        return "0".into();
    }
    // Shortest digits and exponent, as `{:e}` writes them: `d.ddde-7`.
    let scientific = if single {
        shortest_single(x.abs())
    } else {
        shortest_double(x.abs())
    };
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };
    let point = exponent.parse::<i32>().unwrap_or(0) + 1;
    let mut out = String::from(if x < 0.0 { "-" } else { "" });
    let k = i32::try_from(digits.len()).unwrap_or(i32::MAX);
    if k <= point && point <= 21 {
        out.push_str(digits);
        out.extend(std::iter::repeat_n(
            '0',
            usize::try_from(point - k).unwrap_or(0),
        ));
    } else if 0 < point && point <= 21 {
        let (whole, fraction) = digits.split_at(usize::try_from(point).unwrap_or(0));
        out.push_str(whole);
        out.push('.');
        out.push_str(fraction);
    } else if -6 < point && point <= 0 {
        out.push_str("0.");
        out.extend(std::iter::repeat_n(
            '0',
            usize::try_from(-point).unwrap_or(0),
        ));
        out.push_str(digits);
    } else {
        let (first, rest) = digits.split_at(1);
        out.push_str(first);
        if !rest.is_empty() {
            out.push('.');
            out.push_str(rest);
        }
        out.push('e');
        out.push(if point > 0 { '+' } else { '-' });
        out.push_str(&(point - 1).abs().to_string());
    }
    out
}

/// The fewest significant digits that read back as `x`, as `{:e}` writes
/// them. Of two candidates as near, JavaScript takes the even one, which
/// `{:.N$e}` gives; Rust's shortest form may take the other.
fn shortest_double(x: f64) -> String {
    let shortest = format!("{x:e}");
    let length = shortest
        .split('e')
        .next()
        .map_or(1, |m| m.chars().filter(char::is_ascii_digit).count());
    let precision = length.saturating_sub(1);
    let nearest = format!("{x:.precision$e}");
    if nearest.parse::<f64>() == Ok(x) {
        nearest
    } else {
        shortest
    }
}

/// The fewest significant digits that read back as the `f32` `x`, as `{:e}`
/// writes them: what the loader finds with `toPrecision`, one more digit at a
/// time, which rounds a tie away from zero.
fn shortest_single(x: f64) -> String {
    // Every `f32` has an exact decimal form of at most 112 digits.
    let exact = format!("{x:.120e}");
    let (mantissa, exponent) = exact.split_once('e').unwrap_or((&exact, "0"));
    let digits: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    let exponent = exponent.parse::<i32>().unwrap_or(0);
    for precision in 1..=9 {
        let (head, exponent) = rounded_up_on_ties(&digits, exponent, precision);
        let (first, rest) = head.split_at(1);
        let text = format!(
            "{}.{}e{exponent}",
            String::from_utf8_lossy(first),
            String::from_utf8_lossy(rest)
        );
        // The same `f32`, bit for bit: `x` is neither zero nor `NaN` here.
        #[expect(clippy::cast_possible_truncation, reason = "an f32 read back")]
        if text
            .parse::<f64>()
            .is_ok_and(|y| (y as f32).to_bits() == (x as f32).to_bits())
        {
            return text;
        }
    }
    format!("{x:e}")
}

/// The first `precision` of `digits`, rounded to nearest, a tie away from
/// zero; the exponent grows when `9…9` becomes `10…0`.
fn rounded_up_on_ties(digits: &[u8], exponent: i32, precision: usize) -> (Vec<u8>, i32) {
    let mut head = digits.get(..precision).unwrap_or(digits).to_vec();
    if digits.get(precision).is_some_and(|&d| d >= b'5') {
        let mut at = head.len();
        loop {
            if at == 0 {
                head.insert(0, b'1');
                head.pop();
                return (head, exponent + 1);
            }
            at -= 1;
            if let Some(d) = head.get_mut(at) {
                if *d == b'9' {
                    *d = b'0';
                } else {
                    *d += 1;
                    break;
                }
            }
        }
    }
    (head, exponent)
}

#[cfg(test)]
mod tests {
    use super::{index_of, number_text, parse_decimal, replace, slice, split, trim};

    #[test]
    fn decimals_follow_a_strict_grammar() {
        for (text, want) in [
            ("12.5", 12.5),
            (".5", 0.5),
            ("5.", 5.0),
            ("-1e3", -1000.0),
            ("1E-2", 0.01),
        ] {
            assert_eq!(parse_decimal(text.as_bytes()), Some(want), "{text}");
        }
        assert!(parse_decimal(b"-0").is_some_and(|x| x == 0.0 && x.is_sign_negative()));
        for text in [
            "", ".", "+", "1e", "e5", " 1", "1 ", "0x10", "1_0", "inf", "Infinity", "NaN", "1..2",
        ] {
            assert_eq!(parse_decimal(text.as_bytes()), None, "{text}");
        }
    }

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
    /// Expected texts written by JavaScript's `String()`, and for `f32` by
    /// the loader's `toPrecision` search.
    #[test]
    fn floats_are_written_as_javascript_writes_them() {
        let doubles = [
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.0, "-1"),
            (0.1, "0.1"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1.0 / 3.0, "0.3333333333333333"),
            (-2.5, "-2.5"),
            (100.0, "100"),
            (1e21, "1e+21"),
            (1e20, "100000000000000000000"),
            (123_456_789_012_345_680_000.0, "123456789012345680000"),
            (1.5e21, "1.5e+21"),
            (1e-6, "0.000001"),
            (1e-7, "1e-7"),
            (1.234e-7, "1.234e-7"),
            (0.000_001_234, "0.000001234"),
            (5e-324, "5e-324"),
            (1.5e-323, "1.5e-323"),
            (f64::MAX, "1.7976931348623157e+308"),
            (9_007_199_254_740_992.0, "9007199254740992"),
            (9_007_199_254_740_994.0, "9007199254740994"),
            (-1e-300, "-1e-300"),
            (f64::NAN, "NaN"),
            (f64::INFINITY, "Infinity"),
            (f64::NEG_INFINITY, "-Infinity"),
            (123.456, "123.456"),
            (4.35, "4.35"),
            (5e-7, "5e-7"),
            (1e100, "1e+100"),
        ];
        for (x, text) in doubles {
            assert_eq!(number_text(x, false), text, "{x:e}");
        }
        let singles: [(f32, &str); 13] = [
            (0.1, "0.1"),
            (1.0 / 3.0, "0.33333334"),
            (16_777_216.0, "16777216"),
            (16_777_218.0, "16777218"),
            (f32::MAX, "3.4028235e+38"),
            (1e-45, "1e-45"),
            (1e-7, "1e-7"),
            (0.3, "0.3"),
            (2.5, "2.5"),
            (1e21, "1e+21"),
            (f32::MIN_POSITIVE, "1.1754944e-38"),
            (-7.5, "-7.5"),
            (100.0, "100"),
        ];
        for (x, text) in singles {
            assert_eq!(number_text(f64::from(x), true), text, "{x:e}");
        }
    }
}
