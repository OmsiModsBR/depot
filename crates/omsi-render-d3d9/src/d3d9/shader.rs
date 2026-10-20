//! HLSL compilation through the system's `d3dcompiler_47.dll`, which Windows 10 and later
//! ship. Compiling needs no GPU, so these tests run on CI machines without one.

use windows::core::PCSTR;
use windows::Win32::Graphics::Direct3D::Fxc::{D3DCompile, D3DCOMPILE_OPTIMIZATION_LEVEL3};
use windows::Win32::Graphics::Direct3D::ID3DBlob;

use crate::{BackendError, Result};

/// The sources of the renderer's shaders, built into the binary.
pub(crate) const BASIC: &str = include_str!("shaders/basic.hlsl");

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
    let mut code: Option<ID3DBlob> = None;
    let mut errors: Option<ID3DBlob> = None;
    // SAFETY: every pointer refers to a live local for the duration of the call.
    let result = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            PCSTR(name.as_ptr().cast()),
            None,
            None,
            PCSTR(entry.as_ptr().cast()),
            PCSTR(stage.target().as_ptr()),
            D3DCOMPILE_OPTIMIZATION_LEVEL3,
            0,
            &mut code,
            Some(&mut errors),
        )
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
