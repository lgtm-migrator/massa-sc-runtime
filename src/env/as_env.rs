//! Extends the env of wasmer-as

use crate::{
    env::get_memory,
    execution::{abi_bail, ABIResult},
    types::Interface,
};
use anyhow::Result;
use as_ffi_bindings::{Read, StringPtr};
use wasmer::{Global, HostEnvInitError, Instance, WasmerEnv};

use super::MassaEnv;

#[derive(Clone)]
pub struct ASEnv {
    wasm_env: as_ffi_bindings::Env,
    interface: Box<dyn Interface>,
    remaining_points: Option<Global>,
    exhausted_points: Option<Global>,
}

impl MassaEnv<as_ffi_bindings::Env> for ASEnv {
    fn new(interface: &dyn Interface) -> Self {
        Self {
            wasm_env: Default::default(),
            interface: interface.clone_box(),
            remaining_points: None,
            exhausted_points: None,
        }
    }
    fn get_exhausted_points(&self) -> Option<&Global> {
        self.exhausted_points.as_ref()
    }
    fn get_remaining_points(&self) -> Option<&Global> {
        self.remaining_points.as_ref()
    }
    fn get_interface(&self) -> Box<dyn Interface> {
        self.interface.clone()
    }
    fn get_wasm_env(&self) -> &as_ffi_bindings::Env {
        &self.wasm_env
    }
}

impl WasmerEnv for ASEnv {
    fn init_with_instance(&mut self, instance: &Instance) -> Result<(), HostEnvInitError> {
        self.wasm_env.init_with_instance(instance)?;
        self.remaining_points = Some(
            instance
                .exports
                .get_with_generics_weak("wasmer_metering_remaining_points")
                .map_err(HostEnvInitError::from)?,
        );
        self.exhausted_points = Some(
            instance
                .exports
                .get_with_generics_weak("wasmer_metering_points_exhausted")
                .map_err(HostEnvInitError::from)?,
        );
        Ok(())
    }
}

/// Called by the instance when an error popped. It print the filename where the error
/// had pop up, an error message and more stacktrace information as line and column
///
/// This function is automatically exported by AssemblyScript on build and allow assemblyscript
/// to log what happened when a smartcontract crashed inside the instance.
///
/// Because AssemblyScript require this to be imported:
/// - To create an instance, this function has to be in the ImportObject in the "env" namespace.
/// - We can take advantage of the behaviours printing the assemblyscript error
pub fn assembly_script_abort(
    env: &ASEnv,
    message: StringPtr,
    filename: StringPtr,
    line: i32,
    col: i32,
) -> ABIResult<()> {
    let memory = get_memory!(env);
    let message = message.read(memory);
    let filename = filename.read(memory);
    if message.is_err() {
        abi_bail!("abort: failed to load message")
    }
    let mut ret = message.unwrap();
    if let Ok(filename) = filename {
        ret.push_str(&format!(", {filename}"));
    }
    if line != 0 {
        ret.push_str(&format!(", line {line}"));
    }
    if col != 0 {
        ret.push_str(&format!(", col {col}"));
    }
    abi_bail!(ret);
}

/// Assembly script builtin export `seed` function
pub fn assembly_script_seed(env: &ASEnv) -> ABIResult<f64> {
    match env.interface.unsafe_random_f64() {
        Ok(ret) => Ok(ret),
        _ => abi_bail!("failed to get random from interface"),
    }
}

/// Assembly script builtin `Date.now()`.
///
/// Note for developpers: It seems that AS as updated the output of that function
/// for the newest versions. Probably the signature will be soon () -> i64
/// instead of () -> f64.
pub fn assembly_script_date(env: &ASEnv) -> ABIResult<f64> {
    let utime = match env.interface.get_time() {
        Ok(time) => time,
        _ => abi_bail!("failed to get time from interface"),
    };
    let ret = utime as f64;
    if ret as u64 != utime {
        abi_bail!("error getting time value") // will happen in a while
    }
    Ok(ret)
}

/// Assembly script builtin `trace`.
#[allow(clippy::too_many_arguments)]
pub fn assembly_script_trace(
    env: &ASEnv,
    message: StringPtr,
    n: i32,
    a0: f64,
    a1: f64,
    a2: f64,
    a3: f64,
    a4: f64,
) -> ABIResult<()> {
    let memory = get_memory!(env);
    let mut message = match message.read(memory) {
        Ok(m) => m,
        _ => abi_bail!("trace function: aborting failed to load message"),
    };
    let a = [a0, a1, a2, a3, a4];
    if n > 5 {
        abi_bail!("trace function: invalid number of arguments");
    }
    (0..(n as usize)).for_each(|i| message.push_str(&format!(", {}", a[i])));
    if env.interface.print(&message).is_err() {
        abi_bail!("interface error: print failed");
    }
    Ok(())
}
