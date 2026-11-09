//! Pieces of text as views of the text they come from (decision 0004), run
//! in wasmi on the debug allocator, which catches a block released twice,
//! one left alive, and bytes read after their block was released. Each
//! result is checked against Rust's own `split`. Where the text changes while
//! its pieces are in use, they must have been copied: a view left there would
//! read bytes released or moved.

#[path = "fixtures/link.rs"]
mod link;

use perasc_diag::i18n::Localizer;
use perasc_diag::messages::Language;
use perasc_driver::{Allocator, DirectOptions, build_direct};
use wasmi::Val;

const SOURCE: &str = r#"fn sample(n: i32): String {
  return " ab  c déjà ".repeat(n) + "vu";
}

export fn words(n: i32): i32 {
  const text = sample(n);
  return text.split(" ").filter((w) => w.length > 0).length;
}

export fn joined(n: i32): i32 {
  const text = sample(n);
  return text.split(" ").filter((w) => w.length > 1).join("|").length;
}

export fn kept(n: i32): i32 {
  let text = sample(n);
  const k = text.split(" ").filter((w) => w.length > 1);
  text += "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";
  return k.length * 1000 + k.join("").length + k.join("").indexOf("à") * 7;
}

export fn loops(n: i32): i32 {
  const text = sample(n);
  let total = 0;
  for (const w of text.split(" ")) {
    total = total * 3 + w.length;
  }
  return total;
}

export fn changes(n: i32): i32 {
  let text = sample(n);
  let total = 0;
  for (const w of text.split(" ")) {
    text += "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx";
    total = total * 3 + w.length;
    if (w.startsWith("d")) total += 1000;
  }
  return total + text.length;
}

export fn characters(n: i32): i32 {
  const text = sample(n);
  return text.split("").length * 1000 + text.split("").join("-").length;
}

export fn temporary(n: i32): i32 {
  const text = sample(n);
  return (text + " end").split(" ").filter((w) => w.length > 0).length;
}

export fn mapped(n: i32): i32 {
  const text = sample(n);
  return text.split(" ").map((w) => w + "?").join("").length;
}

export fn copied(n: i32): i32 {
  let text = sample(n);
  const c = text.split(" ").copy();
  text += "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz";
  return c.join(",").length + c.join(",").indexOf("à") * 7;
}

export fn sliced(n: i32): i32 {
  let text = sample(n);
  const s = text.split(" ").slice(1, 4);
  text += "abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyz";
  return s.join(",").length + s.join(",").indexOf("à") * 7;
}

export fn counted(n: i32): i32 {
  const text = sample(n);
  return text.split(" ").length;
}
"#;

/// What each function gives, by Rust's `split`.
fn expected(name: &str, n: i32) -> i32 {
    let text = format!(
        "{}vu",
        " ab  c déjà ".repeat(usize::try_from(n).unwrap_or(0))
    );
    let parts: Vec<&str> = text.split(' ').collect();
    let len = |s: &str| i32::try_from(s.len()).unwrap_or(i32::MAX);
    let count = |v: usize| i32::try_from(v).unwrap_or(i32::MAX);
    // The byte index of the first "à", or -1: what reads the bytes themselves.
    let at = |s: &str| {
        s.find('à')
            .map_or(-1, |i| i32::try_from(i).unwrap_or(i32::MAX))
    };
    match name {
        "words" => count(parts.iter().filter(|w| !w.is_empty()).count()),
        "joined" => len(&parts
            .iter()
            .filter(|w| w.len() > 1)
            .copied()
            .collect::<Vec<_>>()
            .join("|")),
        "kept" => {
            let k: Vec<&str> = parts.iter().filter(|w| w.len() > 1).copied().collect();
            let joined = k.concat();
            count(k.len()) * 1000 + len(&joined) + at(&joined) * 7
        }
        "loops" => parts
            .iter()
            .fold(0_i32, |t, w| t.wrapping_mul(3).wrapping_add(len(w))),
        "changes" => {
            let total = parts.iter().fold(0_i32, |t, w| {
                let t = t.wrapping_mul(3).wrapping_add(len(w));
                if w.starts_with('d') {
                    t.wrapping_add(1000)
                } else {
                    t
                }
            });
            total.wrapping_add(len(&text) + 60 * count(parts.len()))
        }
        "characters" => {
            let chars: Vec<String> = text.chars().map(String::from).collect();
            count(chars.len()) * 1000 + len(&chars.join("-"))
        }
        "temporary" => {
            let t = format!("{text} end");
            count(t.split(' ').filter(|w| !w.is_empty()).count())
        }
        // Each part and a "?": one byte more each.
        "mapped" => parts.iter().fold(0, |t, w| t + len(w) + 1),
        "copied" => {
            let joined = parts.join(",");
            len(&joined) + at(&joined) * 7
        }
        "sliced" => {
            let joined = parts
                .get(1..4.min(parts.len()))
                .unwrap_or_default()
                .join(",");
            len(&joined) + at(&joined) * 7
        }
        "counted" => count(parts.len()),
        _ => unreachable!("{name}"),
    }
}

#[test]
fn views_give_what_split_gives_and_copies_where_the_text_changes() {
    let wasm = build_direct(
        SOURCE,
        "m.wasm",
        DirectOptions {
            optimize: true,
            allocator: Allocator::Debug,
        },
        &Localizer::new(Language::English),
    )
    .unwrap_or_else(|e| unreachable!("{e:?}"))
    .wasm;
    for name in [
        "words",
        "joined",
        "kept",
        "loops",
        "changes",
        "characters",
        "temporary",
        "mapped",
        "copied",
        "sliced",
        "counted",
    ] {
        // Up to 7, the text fits in its header's block (256 bytes): when it
        // grows, its old bytes stay there. From 40 on, they are released.
        for n in [0, 1, 2, 7, 40, 300] {
            let mut linked = link::instantiate(&wasm).unwrap_or_else(|e| unreachable!("{e}"));
            let func = linked
                .module
                .get_func(&linked.store, name)
                .unwrap_or_else(|| unreachable!("{name}"));
            let mut results = [Val::I32(0)];
            let called = func.call(&mut linked.store, &[Val::I32(n)], &mut results);
            assert!(
                called.is_ok(),
                "{name}({n}): {called:?} {:?}",
                linked.store.data()
            );
            assert_eq!(results[0].i32(), Some(expected(name, n)), "{name}({n})");
            let live = linked
                .allocator
                .get_typed_func::<(), i32>(&linked.store, "live_count")
                .and_then(|f| f.call(&mut linked.store, ()));
            assert!(
                matches!(live, Ok(0)),
                "{name}({n}) leaves blocks alive: {live:?}"
            );
        }
    }
}
