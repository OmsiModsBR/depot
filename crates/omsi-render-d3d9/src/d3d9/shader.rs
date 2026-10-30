//! HLSL compilation through the system's `d3dcompiler_47.dll`, which Windows 10 and later
//! ship. The DLL is loaded from System32 the first time a shader is compiled, so a game that
//! never opens this renderer does not need it to start. Compiling needs no GPU, so these
//! tests run on CI machines without one.

use std::ffi::c_void;
use std::sync::OnceLock;

use windows::core::{Interface, HRESULT, PCSTR};
use windows::Win32::Graphics::Direct3D::ID3DBlob;
use windows::Win32::System::LibraryLoader::{
    GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32,
};

use crate::{BackendError, Result};

/// The sources of the renderer's shaders, built into the binary.
pub(crate) const BASIC: &str = include_str!("shaders/basic.hlsl");
pub(crate) const SCENE: &str = include_str!("shaders/scene.hlsl");
pub(crate) const UI: &str = include_str!("shaders/ui.hlsl");
pub(crate) const CORONA: &str = include_str!("shaders/corona.hlsl");
pub(crate) const SKY: &str = include_str!("shaders/sky.hlsl");
pub(crate) const SUN_GLOW: &str = include_str!("shaders/sunglow.hlsl");

/// The stage a shader is compiled for. Shader Model 3.0 is the renderer's floor.
#[derive(Debug, Clone, Copy)]
pub enum Stage {
    Vertex,
    Pixel,
}

impl Stage {
    fn target(self) -> &'static [u8] {
        match self {
            Stage::Vertex => b"vs_3_0\0",
            Stage::Pixel => b"ps_3_0\0",
        }
    }
}

/// `D3DCOMPILE_OPTIMIZATION_LEVEL3` of `d3dcompiler.h`.
const OPTIMIZATION_LEVEL3: u32 = 1 << 15;

/// `D3DCompile` of `d3dcompiler.h` (no defines and no include handler are passed here).
type D3DCompileFn = unsafe extern "system" fn(
    src: *const c_void,
    src_size: usize,
    source_name: PCSTR,
    defines: *const c_void,
    include: *mut c_void,
    entry: PCSTR,
    target: PCSTR,
    flags1: u32,
    flags2: u32,
    code: *mut *mut c_void,
    errors: *mut *mut c_void,
) -> HRESULT;

/// `D3DDisassemble` of `d3dcompiler.h` (no comment string is passed here).
type D3DDisassembleFn = unsafe extern "system" fn(
    src: *const c_void,
    src_size: usize,
    flags: u32,
    comments: PCSTR,
    disassembly: *mut *mut c_void,
) -> HRESULT;

/// The compiler's DLL, loaded once from System32 (it then stays loaded).
fn compiler_dll() -> std::result::Result<usize, String> {
    static DLL: OnceLock<std::result::Result<usize, String>> = OnceLock::new();
    DLL.get_or_init(|| {
        // SAFETY: a NUL-terminated wide literal; System32 only, so no DLL from the game's
        // folder or the current one is picked up.
        let dll = unsafe {
            LoadLibraryExW(
                windows::core::w!("d3dcompiler_47.dll"),
                None,
                LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        }
        .map_err(|e| format!("d3dcompiler_47.dll could not be loaded: {e}"))?;
        Ok(dll.0 as usize)
    })
    .clone()
}

/// The compiler's entry point, looked up once.
fn d3d_compile() -> Result<D3DCompileFn> {
    static COMPILE: OnceLock<std::result::Result<usize, String>> = OnceLock::new();
    let found = COMPILE.get_or_init(|| {
        let dll = windows::Win32::Foundation::HMODULE(compiler_dll()? as *mut c_void);
        // SAFETY: a live module and a NUL-terminated name.
        unsafe { GetProcAddress(dll, windows::core::s!("D3DCompile")) }
            .map(|f| f as usize)
            .ok_or_else(|| "d3dcompiler_47.dll has no D3DCompile".to_owned())
    });
    match found {
        // SAFETY: the address is `D3DCompile`'s, whose signature `D3DCompileFn` is.
        Ok(f) => Ok(unsafe { std::mem::transmute::<usize, D3DCompileFn>(*f) }),
        Err(e) => Err(BackendError::Unsupported(e.clone())),
    }
}

/// The instruction slots the compiler itself counts for `code` (its disassembly's
/// "approximately N instruction slots used"), the figure a device's limit is held to.
pub fn reported_slots(code: &[u32]) -> Result<u32> {
    let dll = windows::Win32::Foundation::HMODULE(
        compiler_dll().map_err(BackendError::Unsupported)? as *mut c_void,
    );
    // SAFETY: a live module and a NUL-terminated name.
    let f = unsafe { GetProcAddress(dll, windows::core::s!("D3DDisassemble")) }
        .map(|f| f as usize)
        .ok_or_else(|| BackendError::Unsupported("no D3DDisassemble".into()))?;
    // SAFETY: the address is `D3DDisassemble`'s, whose signature `D3DDisassembleFn` is.
    let disassemble = unsafe { std::mem::transmute::<usize, D3DDisassembleFn>(f) };
    let mut out: *mut c_void = std::ptr::null_mut();
    // SAFETY: the code outlives the call; the blob it returns is owned here.
    let text = unsafe {
        disassemble(
            code.as_ptr().cast(),
            code.len() * 4,
            0,
            PCSTR::null(),
            &mut out,
        )
        .ok()
        .map_err(|e| BackendError::Backend(e.to_string()))?;
        if out.is_null() {
            return Err(BackendError::Backend("no disassembly".into()));
        }
        let blob = ID3DBlob::from_raw(out);
        String::from_utf8_lossy(blob_bytes(&blob)).into_owned()
    };
    text.lines()
        .find_map(|l| {
            let rest = l.split("approximately ").nth(1)?;
            rest.split_whitespace().next()?.parse().ok()
        })
        .ok_or_else(|| BackendError::Backend("no slot count in the disassembly".into()))
}

fn blob_bytes(blob: &ID3DBlob) -> &[u8] {
    // SAFETY: the blob owns `GetBufferSize` bytes at `GetBufferPointer` for its lifetime.
    unsafe {
        std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())
    }
}

/// Compiles `entry` of `source` for `stage` and returns the bytecode as `u32` words, the form
/// `CreateVertexShader`/`CreatePixelShader` take. Compiler messages come back in the error.
pub fn compile(source: &str, name: &str, entry: &str, stage: Stage) -> Result<Vec<u32>> {
    let name =
        std::ffi::CString::new(name).map_err(|_| BackendError::InvalidArgument("name".into()))?;
    let entry =
        std::ffi::CString::new(entry).map_err(|_| BackendError::InvalidArgument("entry".into()))?;
    let d3d_compile = d3d_compile()?;
    let mut code: *mut c_void = std::ptr::null_mut();
    let mut errors: *mut c_void = std::ptr::null_mut();
    // SAFETY: every pointer refers to a live local for the duration of the call; the blobs
    // it returns are owned here (each taken by `from_raw` once).
    let (result, code, errors) = unsafe {
        let hr = d3d_compile(
            source.as_ptr().cast(),
            source.len(),
            PCSTR(name.as_ptr().cast()),
            std::ptr::null(),
            std::ptr::null_mut(),
            PCSTR(entry.as_ptr().cast()),
            PCSTR(stage.target().as_ptr()),
            OPTIMIZATION_LEVEL3,
            0,
            &mut code,
            &mut errors,
        );
        let blob = |p: *mut c_void| (!p.is_null()).then(|| ID3DBlob::from_raw(p));
        (hr.ok(), blob(code), blob(errors))
    };
    if let Err(e) = result {
        let msg = errors
            .as_ref()
            .map(|b| {
                String::from_utf8_lossy(blob_bytes(b))
                    .trim_end_matches('\0')
                    .trim()
                    .to_owned()
            })
            .unwrap_or_else(|| e.to_string());
        return Err(BackendError::Backend(format!("shader {entry:?}: {msg}")));
    }
    let code = code.ok_or_else(|| BackendError::Backend("compiler returned no code".into()))?;
    let bytes = blob_bytes(&code);
    Ok(bytes
        .chunks_exact(4)
        .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_shaders_compile_for_sm3() {
        let vs = compile(BASIC, "basic.hlsl", "vs_main", Stage::Vertex).unwrap();
        let ps = compile(BASIC, "basic.hlsl", "ps_main", Stage::Pixel).unwrap();
        // The first token of SM3 bytecode is the version: 0xFFFE0300 (vs), 0xFFFF0300 (ps).
        assert_eq!(vs[0], 0xFFFE_0300);
        assert_eq!(ps[0], 0xFFFF_0300);
    }

    #[test]
    fn scene_shaders_compile_for_sm3() {
        let vs = compile(SCENE, "scene.hlsl", "vs_main", Stage::Vertex).unwrap();
        let ps = compile(SCENE, "scene.hlsl", "ps_main", Stage::Pixel).unwrap();
        assert_eq!(vs[0], 0xFFFE_0300);
        assert_eq!(ps[0], 0xFFFF_0300);
    }

    #[test]
    fn the_scene_pixel_shader_fits_every_sm3_card() {
        // (512 slots is what every Shader Model 3.0 card gives a pixel shader; the shader
        // keeps a tenth of it free, for the compiler's count being approximate and for what
        // is added next)
        let ps = compile(SCENE, "scene.hlsl", "ps_main", Stage::Pixel).unwrap();
        let slots = reported_slots(&ps).unwrap();
        println!("scene pixel shader: {slots} instruction slots");
        assert!(slots <= 460, "the scene pixel shader takes {slots} slots");
        // the device's guard counts the bytecode itself (`caps::instruction_slots`): it
        // must not stray far from the compiler's count
        let counted = crate::caps::instruction_slots(&ps);
        assert!(
            counted.abs_diff(slots) <= slots / 10,
            "counted {counted}, the compiler {slots}"
        );
    }

    #[test]
    fn corona_shaders_compile_for_sm3() {
        let vs = compile(CORONA, "corona.hlsl", "vs_main", Stage::Vertex).unwrap();
        let ps = compile(CORONA, "corona.hlsl", "ps_main", Stage::Pixel).unwrap();
        assert_eq!(vs[0], 0xFFFE_0300);
        assert_eq!(ps[0], 0xFFFF_0300);
        // (a separate pass: its pixel shader has its own 512 slots, with room to spare)
        let slots = reported_slots(&ps).unwrap();
        println!("corona pixel shader: {slots} instruction slots");
        assert!(slots <= 460, "the corona pixel shader takes {slots} slots");
    }

    #[test]
    fn sky_shaders_compile_for_sm3() {
        let vs = compile(SKY, "sky.hlsl", "vs_main", Stage::Vertex).unwrap();
        let ps = compile(SKY, "sky.hlsl", "ps_main", Stage::Pixel).unwrap();
        assert_eq!(vs[0], 0xFFFE_0300);
        assert_eq!(ps[0], 0xFFFF_0300);
        // (a separate pass: its pixel shader has its own 512 slots, with room to spare)
        let slots = reported_slots(&ps).unwrap();
        println!("sky pixel shader: {slots} instruction slots");
        assert!(slots <= 460, "the sky pixel shader takes {slots} slots");
    }

    #[test]
    fn sun_glow_shaders_compile_for_sm3() {
        let vs = compile(SUN_GLOW, "sunglow.hlsl", "vs_main", Stage::Vertex).unwrap();
        let ps = compile(SUN_GLOW, "sunglow.hlsl", "ps_main", Stage::Pixel).unwrap();
        assert_eq!(vs[0], 0xFFFE_0300);
        assert_eq!(ps[0], 0xFFFF_0300);
        let slots = reported_slots(&ps).unwrap();
        println!("sun glow pixel shader: {slots} instruction slots");
        assert!(slots <= 460, "the sun glow pixel shader takes {slots} slots");
    }

    #[test]
    fn ui_shaders_compile_for_sm3() {
        let vs = compile(UI, "ui.hlsl", "vs_main", Stage::Vertex).unwrap();
        let ps = compile(UI, "ui.hlsl", "ps_main", Stage::Pixel).unwrap();
        assert_eq!(vs[0], 0xFFFE_0300);
        assert_eq!(ps[0], 0xFFFF_0300);
    }

    #[test]
    fn compiler_errors_are_reported() {
        let err = compile(
            "float4 ps_main() : COLOR0 { return nope; }",
            "bad.hlsl",
            "ps_main",
            Stage::Pixel,
        )
        .unwrap_err();
        assert!(err.to_string().contains("nope"), "{err}");
    }
}
