#![forbid(unsafe_code)]

//! Authority-free execution of compiler-produced low-precision Bundle V8 files.

use std::path::Path;

/// Executes one exact Bundle V8 request and returns an FP32 output buffer.
pub fn simulate_bundle_v8(
    bundle: impl AsRef<Path>,
    request: impl AsRef<Path>,
    output_buffer: usize,
) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let admitted =
        fe2o3_kir_sim_cli::load_debug_simulation_bundle_v8(bundle.as_ref(), request.as_ref())?;
    if admitted.bundle().grants_compiler_authority()
        || admitted.bundle().grants_proof_authority()
        || admitted.bundle().grants_artifact_authority()
        || admitted.bundle().grants_hardware_authority()
        || admitted.bundle().grants_load_authority()
        || admitted.bundle().grants_launch_authority()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "simulation Bundle V8 unexpectedly granted production authority",
        )
        .into());
    }
    let evidence = admitted
        .input()
        .simulation_bundle_evidence()
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Bundle V8 admission omitted exact bundle evidence",
            )
        })?;
    if evidence.envelope_version != 8 || evidence.production_kir_version != 13 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "low-precision simulation requires exact Bundle V8 / KIR V13 custody",
        )
        .into());
    }

    let input = admitted.input();
    let execution = input.module.simulate(
        &input.request,
        input.simulation_target(),
        input.simulation_limits,
    )?;
    let output = execution.buffer(output_buffer).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("simulation omitted output buffer {output_buffer}"),
        )
    })?;
    let chunks = output.bytes().chunks_exact(core::mem::size_of::<f32>());
    if !chunks.remainder().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "simulation output is not an exact FP32 buffer",
        )
        .into());
    }
    Ok(chunks
        .map(|bytes| f32::from_bits(u32::from_le_bytes(bytes.try_into().unwrap())))
        .collect())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NONCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn forged_bundle_fails_before_output_observation() {
        let root = std::env::var_os("FE2O3_GFX950_LOW_PRECISION_TEST_SCRATCH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join(format!(
                "fe2o3-gfx950-low-precision-forged-v8-{}-{}",
                std::process::id(),
                NONCE.fetch_add(1, Ordering::Relaxed),
            ));
        std::fs::create_dir(&root).unwrap();
        let bundle = root.join("forged.bundle-v8");
        let request = root.join("request.json");
        std::fs::write(&bundle, b"not-a-fe2o3-bundle-v8").unwrap();
        std::fs::write(&request, b"{}").unwrap();
        let sentinel = [f32::from_bits(0x7f7f_ffff); 4];

        let error = super::simulate_bundle_v8(&bundle, &request, 2)
            .expect_err("forged Bundle V8 must fail closed");

        assert_eq!(sentinel, [f32::from_bits(0x7f7f_ffff); 4]);
        assert!(!error.to_string().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
}
