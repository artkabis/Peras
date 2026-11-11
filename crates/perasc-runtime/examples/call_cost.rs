//! The cost of a call into another WebAssembly instance, against the same
//! call inside the module and the callee written inline (decision 0016). The
//! runtime of Peras is its own instance, called through imports: this
//! measures what that costs, apart from the work done.
//!
//! Writes `callee.wasm`, `import.wasm`, `internal.wasm` and `inline.wasm` to
//! the directory given; `node scripts/call-cost.mjs` builds and times them.

use std::path::PathBuf;

/// The work: from `from`, the end of the UTF-8 character there, in `len`
/// bytes of memory at 0, as the runtime's `str_next_part` walks `split("")`.
const STEP: &str = r"
  (local.set $end (i32.add (local.get $from) (i32.const 1)))
  (block $whole
    (loop $more
      (br_if $whole (i32.ge_u (local.get $end) (local.get $len)))
      (br_if $whole (i32.ne (i32.and (i32.load8_u (local.get $end)) (i32.const 192)) (i32.const 128)))
      (local.set $end (i32.add (local.get $end) (i32.const 1)))
      (br $more)))
  (local.get $end)";

/// The module that walks `len` bytes `rounds` times, one character per call
/// of the step, which it imports, defines, or writes inline.
fn caller(how: &str) -> String {
    let step = format!(
        "(func $step (param $from i32) (param $len i32) (result i32) (local $end i32) {STEP})"
    );
    let (import, def, call) = match how {
        "import" => (
            r#"(import "rt" "step" (func $step (param i32 i32) (result i32)))"#.to_owned(),
            String::new(),
            "(call $step (local.get $at) (local.get $len))".to_owned(),
        ),
        "internal" => (
            String::new(),
            step,
            "(call $step (local.get $at) (local.get $len))".to_owned(),
        ),
        _ => (
            String::new(),
            String::new(),
            format!("(block (result i32) (local.set $from (local.get $at)) {STEP})"),
        ),
    };
    format!(
        r#"(module
          (import "env" "memory" (memory 1))
          {import}
          {def}
          (func (export "walk") (param $len i32) (param $rounds i32) (result i32)
            (local $r i32) (local $at i32) (local $sum i32) (local $from i32) (local $end i32)
            (loop $round
              (local.set $at (i32.const 0))
              (loop $char
                (local.set $at {call})
                (local.set $sum (i32.add (local.get $sum) (local.get $at)))
                (br_if $char (i32.lt_u (local.get $at) (local.get $len))))
              (local.set $r (i32.add (local.get $r) (i32.const 1)))
              (br_if $round (i32.lt_u (local.get $r) (local.get $rounds))))
            (local.get $sum)))"#
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("usage: call_cost <directory>")?,
    );
    std::fs::create_dir_all(&dir)?;
    let callee = format!(
        r#"(module (import "env" "memory" (memory 1))
          (func (export "step") (param $from i32) (param $len i32) (result i32) (local $end i32) {STEP}))"#
    );
    std::fs::write(dir.join("callee.wasm"), wat::parse_str(callee)?)?;
    for how in ["import", "internal", "inline"] {
        std::fs::write(
            dir.join(format!("{how}.wasm")),
            wat::parse_str(caller(how))?,
        )?;
    }
    Ok(())
}
