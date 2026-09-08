use std::path::Path;

pub fn vecadd_cpu_oracle(a: &[f32], b: &[f32], output: &mut [f32], launched: usize) {
    let updated = launched.min(a.len()).min(b.len()).min(output.len());
    for index in 0..updated {
        super::vecadd_cpu_reference(index, a, b, &mut output[index]);
    }
}

/// Executes an authority-free Bundle V8 with its exact canonical KIR V13 graph.
pub fn simulate_bundle_v8(
    bundle: impl AsRef<Path>,
    request: impl AsRef<Path>,
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
    let input = admitted.input();
    let execution = input.module.simulate(
        &input.request,
        input.simulation_target(),
        input.simulation_limits,
    )?;
    let output = execution.buffer(2).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "vecadd simulation did not return output buffer 2",
        )
    })?;
    let chunks = output.bytes().chunks_exact(4);
    if !chunks.remainder().is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "vecadd simulation output is not an exact f32 buffer",
        )
        .into());
    }
    Ok(chunks
        .map(|bytes| f32::from_bits(u32::from_le_bytes(bytes.try_into().unwrap())))
        .collect())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NONCE: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn oracle_preserves_out_of_bounds_tails() {
        for output_len in [0_usize, 1, 2, 63, 64, 65, 127, 129] {
            let input_len = output_len.saturating_sub(1);
            let a = (0..input_len).map(|i| i as f32).collect::<Vec<_>>();
            let b = (0..input_len).map(|i| (i as f32) * 2.0).collect::<Vec<_>>();
            let mut output = vec![-17.0_f32; output_len];
            let launched = output_len.max(1).div_ceil(64) * 64;

            super::vecadd_cpu_oracle(&a, &b, &mut output, launched);

            for (index, value) in output.iter().copied().enumerate() {
                let expected = if index < input_len {
                    a[index] + b[index]
                } else {
                    -17.0
                };
                assert_eq!(value, expected);
            }
        }
    }

    #[test]
    fn forged_bundle_v8_is_rejected_without_mutating_caller_output() {
        let root = std::env::temp_dir().join(format!(
            "fe2o3-vecadd-v8-forgery-{}-{}",
            std::process::id(),
            NONCE.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&root).expect("create isolated vecadd simulator fixture");
        let bundle = root.join("forged.bundle-v8");
        let request = root.join("request.json");
        fs::write(&bundle, b"not-a-fe2o3-bundle-v8").expect("write forged bundle");
        fs::write(&request, b"{}").expect("write inert request");

        let caller_output = [91.0_f32, -7.0];
        let error = super::simulate_bundle_v8(&bundle, &request)
            .expect_err("forged Bundle V8 must fail closed");

        assert_eq!(caller_output, [91.0, -7.0]);
        assert!(!error.to_string().is_empty());
        fs::remove_dir_all(root).expect("remove vecadd simulator fixture");
    }
}
