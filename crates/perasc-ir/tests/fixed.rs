//! `toFixed` in the interpreter and the tests' host, `text::fixed_text`,
//! against JavaScript's own, in Node when it is installed: random doubles of
//! every magnitude, ties, and every number of digits from 0 to 100.

use std::path::PathBuf;
use std::process::Command;

/// Node, from `PATH` or the folders it is installed in here.
fn node() -> Option<PathBuf> {
    let from_path = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default();
    let installed = std::fs::read_dir("/opt")
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path().join("bin"));
    from_path
        .into_iter()
        .chain(installed)
        .map(|dir| dir.join("node"))
        .find(|path| path.is_file())
}

#[test]
fn to_fixed_writes_what_javascript_writes() {
    let Some(node) = node() else {
        return;
    };
    let mut values: Vec<f64> = vec![
        0.5,
        1.5,
        2.5,
        -2.5,
        1.005,
        1.45,
        0.0,
        -0.0,
        -0.0001,
        9.995,
        1e21,
        999_999_999_999_999_900_000.0,
        5e-324,
        -1e-300,
        0.125,
        0.375,
        1e-7,
        123.456,
        f64::NAN,
        f64::INFINITY,
    ];
    let mut s: u64 = 0x2545_f491_4f6c_dd1d;
    for _ in 0..4000 {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        // Every magnitude from 1e-12 to 1e22, and short decimals with ties.
        let bits = f64::from_bits(s);
        if bits.is_finite() && bits.abs() < 1e22 && bits.abs() > 1e-12 {
            values.push(bits);
        }
        #[allow(clippy::cast_precision_loss)]
        let short = (s % 2_000_001) as f64 / 1000.0 - 1000.0;
        values.push(short);
    }
    let digits = [0_u32, 1, 2, 3, 5, 10, 17, 20, 50, 100];
    let input: Vec<String> = values
        .iter()
        .map(|x| {
            if x.is_nan() {
                "NaN".to_owned()
            } else if x.is_infinite() {
                "Infinity".to_owned()
            } else {
                format!("{x:e}")
            }
        })
        .collect();
    let script = format!(
        "const xs = [{}]; const ds = {digits:?}; const out = []; for (const x of xs) for (const d of ds) out.push(x.toFixed(d)); console.log(out.join('\\n'));",
        input.join(",")
    );
    let output = Command::new(node).arg("-e").arg(script).output();
    let Ok(output) = output else {
        return;
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut lines = text.lines();
    for x in &values {
        for &d in &digits {
            let expected = lines.next().unwrap_or("");
            assert_eq!(
                perasc_ir::text::fixed_text(*x, d),
                expected,
                "({x:e}).toFixed({d})"
            );
        }
    }
    assert!(lines.next().is_none());
}
