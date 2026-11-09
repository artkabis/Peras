//! A module built by perasc, linked to the debug allocator in wasmi, the way
//! the JavaScript loader links it in a browser. Shared by the driver's tests.

use perasc_runtime::Fault;
use wasmi::{Caller, Engine, Extern, Instance, Linker, Store};

/// The first fault reported: its kind and the sites it names, by number.
pub type Reported = Option<(Option<Fault>, i32, i32)>;

/// The allocator and the module, linked.
pub struct Linked {
    pub store: Store<Reported>,
    pub allocator: Instance,
    pub module: Instance,
}

/// Validates the module, then links it to `runtime`, a build of the debug
/// allocator.
pub fn instantiate_with(wasm: &[u8], allocator_module: &[u8]) -> Result<Linked, wasmi::Error> {
    let validated = wasmparser::Validator::new().validate_all(wasm);
    assert!(validated.is_ok(), "{:?}", validated.err());
    let engine = Engine::default();
    let mut store = Store::new(&engine, None);
    let mut runtime = Linker::new(&engine);
    runtime.func_wrap(
        "peras",
        "fault",
        |mut caller: Caller<'_, Reported>, kind: i32, _ptr: i32, alloc: i32, release: i32| {
            caller
                .data_mut()
                .get_or_insert((Fault::from_kind(kind), alloc, release));
            Err::<(), _>(wasmi::Error::new("peras fault"))
        },
    )?;
    // A float as text, written as the loader writes it.
    runtime.func_wrap(
        "peras",
        "float_text",
        |mut caller: Caller<'_, Reported>, x: f64, single: i32, at: i32| {
            let text = perasc_ir::text::number_text(x, single != 0);
            let memory = caller
                .get_export("memory")
                .and_then(Extern::into_memory)
                .ok_or_else(|| wasmi::Error::new("memory"))?;
            let at = usize::try_from(at).map_err(|_| wasmi::Error::new("address"))?;
            memory
                .write(&mut caller, at, text.as_bytes())
                .map_err(|_| wasmi::Error::new("write"))?;
            i32::try_from(text.len()).map_err(|_| wasmi::Error::new("length"))
        },
    )?;
    let allocator = runtime
        .instantiate_and_start(&mut store, &wasmi::Module::new(&engine, allocator_module)?)?;
    let mut linker = Linker::new(&engine);
    let strings = perasc_ir::RuntimeFn::ALL.map(perasc_ir::RuntimeFn::name);
    let core = [
        "alloc",
        "release",
        "buf_new",
        "buf_reserve",
        "buf_release",
        "buf_clone",
    ];
    for name in core.into_iter().chain(strings) {
        let func = allocator
            .get_func(&store, name)
            .ok_or_else(|| wasmi::Error::new(name))?;
        linker.define(
            perasc_codegen_wasm::RUNTIME_MODULE,
            name,
            Extern::Func(func),
        )?;
    }
    let memory = allocator
        .get_memory(&store, "memory")
        .ok_or_else(|| wasmi::Error::new("memory"))?;
    linker.define(
        perasc_codegen_wasm::RUNTIME_MODULE,
        "memory",
        Extern::Memory(memory),
    )?;
    // An index out of bounds: JavaScript throws a `RangeError` here.
    linker.func_wrap(
        perasc_codegen_wasm::RUNTIME_MODULE,
        "bounds",
        |_: Caller<'_, Reported>, _index: i32, _len: i32| {
            Err::<(), _>(wasmi::Error::new("index out of bounds"))
        },
    )?;
    let module = linker.instantiate_and_start(&mut store, &wasmi::Module::new(&engine, wasm)?)?;
    Ok(Linked {
        store,
        allocator,
        module,
    })
}
