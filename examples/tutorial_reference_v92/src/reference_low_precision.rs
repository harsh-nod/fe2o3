use crate::protocol::{Output, Request, buffer};
use crate::references::{READ, output};
use fe2o3_gfx950_low_precision::reference::{self as cpu, LowPrecisionFormat as Format};

pub const KERNELS: [&str; 4] = [
    "gfx950_fp4_gemm_rust",
    "gfx950_fp8_gemm_rust",
    "gfx950_fp4_attention_rust",
    "gfx950_fp8_attention_rust",
];
pub fn contains(kernel: &str) -> bool {
    KERNELS.contains(&kernel)
}
fn format(kernel: &str) -> Format {
    if kernel.starts_with("gfx950_fp4_") {
        Format::Fp4E2M1
    } else {
        Format::Fp8E4M3
    }
}
pub fn generate(kernel: &str) -> Result<Request, String> {
    if !contains(kernel) {
        return Err("unknown low precision kernel".to_owned());
    }
    let fmt = format(kernel);
    let matrix = |rows, columns, salt| {
        buffer(
            "u8",
            READ,
            &cpu::deterministic_batched_matrix(fmt, 16, rows, columns, salt),
        )
    };
    let args = if kernel.contains("_gemm_") {
        vec![matrix(16, 128, 1), matrix(128, 16, 3), output(16 * 256)]
    } else {
        vec![
            matrix(16, 128, 1),
            matrix(16, 128, 3),
            matrix(16, 16, 4),
            output(16 * 256),
        ]
    };
    Ok(Request::new(kernel, 256, 4, args))
}
pub fn evaluate(request: &Request) -> Result<Vec<Output>, String> {
    let gemm = request.kernel.contains("_gemm_");
    request.launch(256, 4, if gemm { 3 } else { 4 })?;
    let result = if gemm {
        cpu::batched_gemm_reference(
            format(&request.kernel),
            16,
            &request.data(0, "u8")?,
            &request.data(1, "u8")?,
        )
    } else {
        cpu::batched_attention_reference(
            format(&request.kernel),
            16,
            &request.data(0, "u8")?,
            &request.data(1, "u8")?,
            &request.data(2, "u8")?,
        )
    }
    .map_err(|e| e.to_string())?;
    Ok(vec![Output::f32s(
        if gemm { 2 } else { 3 },
        &result,
        if gemm { 0.0 } else { 0.0001 },
    )])
}
