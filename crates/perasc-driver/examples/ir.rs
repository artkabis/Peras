//! Prints the IR of a file, optimized: a tool of decision 0043's prototypes.
#![allow(clippy::expect_used, clippy::print_stdout, clippy::print_stderr)]
fn main() {
    let path = std::env::args().nth(1).expect("a file");
    let source = std::fs::read_to_string(&path).expect("readable");
    match perasc_driver::compile_ir(&source) {
        Ok(mut m) => {
            perasc_ir::optimize(&mut m);
            if std::env::var_os("ARENA").is_some() {
                for (name, t) in perasc_ir::arena::analyse(&m) {
                    println!(
                        "{name}: {} of {} allocations taken, regions {:?}",
                        t.taken, t.allocations, t.regions
                    );
                }
            } else {
                println!("{m}");
            }
        }
        Err(e) => eprintln!("{e:?}"),
    }
}
