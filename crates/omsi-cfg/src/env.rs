//! The `OMSI_*` switches of the environment, read once: the frame asked for dozens of them
//! every time round (`getenv` takes a lock and walks the environment - 2 % of a frame's CPU
//! time in `sync_materials` alone). The environment is not changed while the game runs.

use std::collections::HashMap;
use std::ffi::OsString;
use std::sync::{OnceLock, RwLock};

fn cache() -> &'static RwLock<HashMap<String, Option<OsString>>> {
    static C: OnceLock<RwLock<HashMap<String, Option<OsString>>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

#[derive(Default)]
struct Fnv(u64);

impl std::hash::Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        let mut h = if self.0 == 0 {
            0xcbf2_9ce4_8422_2325
        } else {
            self.0
        };
        for b in bytes {
            h = (h ^ *b as u64).wrapping_mul(0x0100_0000_01b3);
        }
        self.0 = h;
    }
}

type Local =
    std::cell::RefCell<HashMap<String, Option<OsString>, std::hash::BuildHasherDefault<Fnv>>>;

thread_local! {
    static LOCAL: Local = Default::default();
}

pub fn var_os(name: &str) -> Option<OsString> {
    if let Some(v) = LOCAL.with(|l| l.borrow().get(name).cloned()) {
        return v;
    }
    let v = shared(name);
    LOCAL.with(|l| l.borrow_mut().insert(name.to_string(), v.clone()));
    v
}

fn shared(name: &str) -> Option<OsString> {
    if let Some(v) = cache().read().ok().and_then(|c| c.get(name).cloned()) {
        return v;
    }
    let v = std::env::var_os(name);
    if let Ok(mut c) = cache().write() {
        c.insert(name.to_string(), v.clone());
    }
    v
}

pub fn var(name: &str) -> Result<String, std::env::VarError> {
    match var_os(name) {
        Some(s) => s.into_string().map_err(std::env::VarError::NotUnicode),
        None => Err(std::env::VarError::NotPresent),
    }
}
