use std::{
    ops::{Deref, DerefMut},
    os::raw::c_int,
    ptr, slice,
};

use hotg_rune_runtime::Runtime as RustRuntime;

use crate::{Error, InputTensors, Metadata, OutputTensors};

/// A loaded Rune.
pub struct Runtime {
    inner: RustRuntime,
}

impl Deref for Runtime {
    type Target = RustRuntime;

    fn deref(&self) -> &Self::Target { &self.inner }
}

impl DerefMut for Runtime {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.inner }
}

/// Data used when loading a Rune.
#[repr(C)]
pub struct Config {
    pub rune: *const u8,
    pub rune_len: c_int,
    pub engine: Engine,
}

#[no_mangle]
pub unsafe extern "C" fn rune_runtime_free(runtime: *mut Runtime) {
    if runtime.is_null() {
        return;
    }

    let rt = Box::from_raw(runtime);
    drop(rt);
}

/// Execute the rune, reading from the input tensors that were provided and
/// writing to the output tensors.
#[no_mangle]
#[must_use]
pub unsafe extern "C" fn rune_runtime_predict(
    runtime: *mut Runtime,
) -> *mut Error {
    expect!(!runtime.is_null());
    let runtime = &mut *runtime;

    match runtime.inner.predict() {
        Ok(_) => ptr::null_mut(),
        Err(e) => Error::boxed(e),
    }
}

/// Get a set of all the input nodes in this Rune.
#[no_mangle]
#[must_use]
pub unsafe extern "C" fn rune_runtime_inputs(
    runtime: *const Runtime,
    metadata_out: *mut *mut Metadata,
) -> *mut Error {
    expect!(!runtime.is_null());
    expect!(!metadata_out.is_null());
    let runtime = &*runtime;

    metadata_out.write(Box::into_raw(Box::new(Metadata::from(
        runtime.capabilities(),
    ))));

    ptr::null_mut()
}

/// Get a set of all the output nodes in this Rune.
#[no_mangle]
#[must_use]
pub unsafe extern "C" fn rune_runtime_outputs(
    runtime: *const Runtime,
    metadata_out: *mut *mut Metadata,
) -> *mut Error {
    expect!(!runtime.is_null());
    expect!(!metadata_out.is_null());
    let runtime = &*runtime;

    metadata_out
        .write(Box::into_raw(Box::new(Metadata::from(runtime.outputs()))));

    ptr::null_mut()
}

#[no_mangle]
#[must_use]
pub unsafe extern "C" fn rune_runtime_input_tensors(
    runtime: *mut Runtime,
    tensors_out: *mut *mut InputTensors,
) -> *mut Error {
    expect!(!runtime.is_null());
    expect!(!tensors_out.is_null());
    let runtime = &mut *runtime;

    tensors_out.write(Box::into_raw(Box::new(runtime.input_tensors().into())));

    ptr::null_mut()
}

/// Get a reference to the tensors associated with each output node.
///
/// This will return `null` if `runtime` is `null`.
///
/// # Safety
///
/// This reference points directly into the runtime's internals, so any use of
/// the runtime while this reference is alive may invalidate it.
#[no_mangle]
#[must_use]
pub unsafe extern "C" fn rune_runtime_output_tensors<'rt>(
    runtime: *mut Runtime,
) -> *mut OutputTensors<'rt> {
    if runtime.is_null() {
        return ptr::null_mut();
    }

    let runtime = &*runtime;
    let output_tensors = OutputTensors::from(runtime.output_tensors());

    Box::into_raw(Box::new(output_tensors))
}

#[no_mangle]
#[must_use]
pub unsafe extern "C" fn rune_runtime_load(
    cfg: &Config,
    runtime_out: *mut *mut Runtime,
) -> *mut Error {
    expect!(!cfg.rune.is_null());
    expect!(cfg.rune_len > 0);
    expect!(!runtime_out.is_null());

    let wasm = slice::from_raw_parts(cfg.rune, cfg.rune_len as usize);

    let load_result = match cfg.engine {
        Engine::Wasm3 => load_wasm3(wasm),
        Engine::Wasmer => load_wasmer(wasm),
    };

    match load_result {
        Ok(inner) => {
            runtime_out.write(Box::into_raw(Box::new(Runtime { inner })));
            std::ptr::null_mut()
        },
        Err(e) => Error::boxed(e),
    }
}

fn load_wasm3(wasm: &[u8]) -> Result<RustRuntime, anyhow::Error> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "wasm3")] {
            RustRuntime::wasm3(wasm)
        } else {
            let _ = wasm;
            unsupported_engine(Engine::Wasm3)
        }
    }
}

fn load_wasmer(wasm: &[u8]) -> Result<RustRuntime, anyhow::Error> {
    cfg_if::cfg_if! {
        if #[cfg(feature = "wasmer")] {
            RustRuntime::wasmer(wasm)
        } else {
            let _ = wasm;
            unsupported_engine(Engine::Wasmer)
        }
    }
}

/// The WebAssembly edngine to use when running a Rune.
#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(u32)]
pub enum Engine {
    Wasm3 = 0,
    Wasmer = 1,
}

#[allow(dead_code)]
fn unsupported_engine(engine: Engine) -> Result<RustRuntime, anyhow::Error> {
    Err(anyhow::anyhow!(
        "Not compiled with support for the {:?} engine",
        engine
    ))
}
