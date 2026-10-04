#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD targets"]
fn ordinary_neutral_collectives_reach_both_target_llvm_backends() {
    let example = workspace().join("examples/workgroup_sync_v1");
    let sources = [
        "src/kernel.rs",
        "src/kernel_u32.rs",
        "src/kernel_f32.rs",
        "src/kernel_scan_u32.rs",
        "src/kernel_scan_u32_exclusive.rs",
        "src/kernel_scan_i32.rs",
        "src/kernel_scan_i32_inclusive.rs",
        "src/kernel_scan_f32.rs",
        "src/kernel_scan_f32_exclusive.rs",
    ]
    .map(|relative| {
        let path = example.join(relative);
        let bytes = std::fs::read(&path).expect("read immutable ordinary Rust source");
        (path, bytes)
    });

    for (cpu, target) in [("gfx942", "gfx942:xnack-"), ("gfx950", "gfx950:xnack-")] {
        for (feature, symbol, arithmetic, barriers) in [
            (
                "lds-kernel",
                "lds_publish_read_reduce_i32_v1",
                "call { i32, i1 } @llvm.sadd.with.overflow.i32(",
                14,
            ),
            (
                "lds-u32-kernel",
                "lds_publish_read_reduce_u32_v1",
                "call { i32, i1 } @llvm.uadd.with.overflow.i32(",
                14,
            ),
            (
                "lds-f32-kernel",
                "lds_publish_read_reduce_f32_v1",
                "fadd float",
                14,
            ),
            (
                "lds-scan-u32-kernel",
                "lds_inclusive_scan_u32_v1",
                "call { i32, i1 } @llvm.uadd.with.overflow.i32(",
                6,
            ),
            (
                "lds-scan-u32-exclusive-kernel",
                "lds_exclusive_scan_u32_v1",
                "call { i32, i1 } @llvm.uadd.with.overflow.i32(",
                18,
            ),
            (
                "lds-scan-i32-kernel",
                "lds_exclusive_scan_i32_v1",
                "call { i32, i1 } @llvm.sadd.with.overflow.i32(",
                16,
            ),
            (
                "lds-scan-i32-inclusive-kernel",
                "lds_inclusive_scan_i32_v1",
                "call { i32, i1 } @llvm.sadd.with.overflow.i32(",
                6,
            ),
            (
                "lds-scan-f32-kernel",
                "lds_inclusive_scan_f32_v1",
                "fadd float",
                18,
            ),
            (
                "lds-scan-f32-exclusive-kernel",
                "lds_exclusive_scan_f32_v1",
                "fadd float",
                16,
            ),
        ] {
            let scratch = ScratchDirectory::new(&format!("{cpu}-{feature}"));
            let llvm_path = scratch.0.join("neutral-reduction.ll");
            let binding_path = scratch.0.join("crate-binding-v1");
            let output = Command::new(env!("CARGO"))
                .current_dir(&example)
                .env(
                    "RUSTC_WORKSPACE_WRAPPER",
                    env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
                )
                .env("FE2O3_EXTRACT_CRATE_V1", "fe2o3_workgroup_sync_v1")
                .env("FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1", &llvm_path)
                .env("FE2O3_EXTRACT_CRATE_BINDING_PATH_V1", &binding_path)
                .env(
                    "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2",
                    "55".repeat(32),
                )
                .env("FE2O3_CRATE_BINDING_ID_V1", "77".repeat(32))
                .env_remove("RUSTFLAGS")
                .env_remove("CARGO_ENCODED_RUSTFLAGS")
                .env(
                    "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                    format!(
                        "-Zalways-encode-mir -Ctarget-cpu={cpu} -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
                    ),
                )
                .args([
                    "check",
                    "--release",
                    "--locked",
                    "--no-default-features",
                    "--features",
                    feature,
                    "-Zbuild-std=core",
                    "--target",
                    "amdgcn-amd-amdhsa",
                    "--target-dir",
                ])
                .arg(scratch.0.join("cargo"))
                .arg("--lib")
                .output()
                .expect("run neutral workgroup production extraction");
            let stderr = String::from_utf8(output.stderr).expect("rustc diagnostic is UTF-8");
            assert!(
                output.status.success(),
                "{feature} did not reach {target} production LLVM:\n{stderr}"
            );
            assert!(
                stderr.contains("Rust -> semantic MIR -> ranked PLIRON -> Kernel IR")
                    && stderr.contains(
                        "composed formal/ranked memory -> target-KIR optimizer (7 pass(es)",
                    )
                    && stderr.contains(&format!("-> {target} LLVM"))
                    && stderr.contains("artifact/launch authority false"),
                "{feature} omitted its successful {target} lowering receipt:\n{stderr}",
            );
            for forbidden in ["error[FE2O3-RACE", "lowering stopped", "panic"] {
                assert!(
                    !stderr.contains(forbidden),
                    "{feature} emitted forbidden diagnostic {forbidden:?}:\n{stderr}"
                );
            }
            let llvm = std::fs::read_to_string(&llvm_path)
                .expect("production extraction emitted neutral collective LLVM");
            for required in [
                "target triple = \"amdgcn-amd-amdhsa\"",
                symbol,
                "addrspace(3)",
                "llvm.amdgcn.workitem.id.x",
                "call void @llvm.trap()",
                arithmetic,
            ] {
                assert!(
                    llvm.contains(required),
                    "{feature} {target} LLVM omitted {required:?}:\n{llvm}"
                );
            }
            assert_eq!(
                llvm.matches("call void asm sideeffect \"s_barrier\", \"\"()")
                    .count(),
                barriers,
                "{feature} {target} LLVM changed the exact collective barrier recipe:\n{llvm}",
            );
            assert_eq!(
                llvm.matches("fence syncscope(\"workgroup\") release")
                    .count(),
                barriers,
                "{feature} {target} LLVM changed the release side of the barrier recipe",
            );
            assert_eq!(
                llvm.matches("fence syncscope(\"workgroup\") acquire")
                    .count(),
                barriers,
                "{feature} {target} LLVM changed the acquire side of the barrier recipe",
            );
            let binding = std::fs::read_to_string(&binding_path).expect("crate binding handoff");
            assert_eq!(binding.trim().len(), 64);
            assert!(binding.trim().bytes().all(|byte| byte.is_ascii_hexdigit()));
            for (source_path, source_before) in &sources {
                assert_eq!(
                    std::fs::read(source_path).expect("re-read ordinary Rust source"),
                    source_before.as_slice(),
                    "production extraction changed its source input",
                );
            }
        }
    }
}
