mod core_scalar_enum_payload_tests {
    use super::{ScratchTarget, materialize_source_safety_fixture};
    use fe2o3_kernel_ir::{
        AddressSpace, FunctionRole, MemoryEffect, ScalarType, Type, VerifiedSimulationBundleV1,
        analyze_interprocedural_effects_v1, decode_module_v7,
    };
    use serde_json::json;
    use std::io::Read;
    use std::path::Path;
    use std::process::Command;

    const CRATE: &str = "fe2o3_production_source_safety_fixture";
    const FINAL_MIR_FILE: &str =
        "fe2o3_production_source_safety_fixture.choose_scalar_payload.runtime-optimized.after.mir";
    const MAX_FINAL_MIR_BYTES: u64 = 64 * 1024;
    const SOURCE: &str = r#"#![no_std]
use fe2o3_device::{kernel, thread, DisjointSlice};

#[repr(u8)]
enum Payload {
    Absent = 3,
    Present(u64) = 11,
}

#[inline(never)]
fn choose_scalar_payload(flag: u64, value: u64) -> (u64, u8) {
    let selected = if flag == 0 {
        Payload::Absent
    } else if flag == 1 {
        Payload::Present(value)
    } else {
        Payload::Present(value)
    };
    match selected {
        Payload::Absent => (0_u64, 0_u8),
        Payload::Present(payload) => (payload, 1_u8),
    }
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn scalar_enum_payload(flag: u64, value: u64, mut output: DisjointSlice<u64>,
                           mut present: DisjointSlice<u8>) {
    let (result, flag) = choose_scalar_payload(flag, value);
    if let core::option::Option::Some(slot) = output.get_mut(thread::index_1d()) {
        *slot = result;
    }
    if let core::option::Option::Some(slot) = present.get_mut(thread::index_1d()) {
        *slot = flag;
    }
}
"#;

    fn clean_command(fixture: &Path) -> Command {
        let mut command = Command::new(env!("CARGO"));
        command.current_dir(fixture);
        for (name, _) in std::env::vars_os() {
            let text = name.to_string_lossy();
            if text.starts_with("FE2O3_EXTRACT_")
                || text.starts_with("FE2O3_DIAGNOSTIC_")
                || matches!(
                    text.as_ref(),
                    "RUSTFLAGS"
                        | "CARGO_ENCODED_RUSTFLAGS"
                        | "CARGO_BUILD_TARGET"
                        | "RUSTC_WRAPPER"
                        | "RUSTC_WORKSPACE_WRAPPER"
                        | "CARGO_BUILD_RUSTC_WRAPPER"
                        | "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"
                        | "FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2"
                        | "FE2O3_CRATE_BINDING_ID_V1"
                )
            {
                command.env_remove(name);
            }
        }
        command
            .env("CARGO_INCREMENTAL", "0")
            .env("CARGO_TERM_COLOR", "never");
        command
    }

    fn final_runtime_mir(target: &ScratchTarget, directory: &Path) -> String {
        let read = || -> Result<String, String> {
            if directory.parent() != Some(target.path()) {
                return Err("dump directory is not a direct ScratchTarget child".into());
            }
            let metadata = std::fs::symlink_metadata(directory).map_err(|e| e.to_string())?;
            if !metadata.file_type().is_dir() {
                return Err("dump directory is not a regular non-symlink directory".into());
            }
            let owner = target.path().canonicalize().map_err(|e| e.to_string())?;
            if directory
                .canonicalize()
                .map_err(|e| e.to_string())?
                .parent()
                != Some(owner.as_path())
            {
                return Err("dump directory escaped ScratchTarget".into());
            }
            let path = directory.join(FINAL_MIR_FILE);
            if !std::fs::symlink_metadata(&path)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_file()
            {
                return Err("final MIR is not a regular non-symlink file".into());
            }
            let mut options = std::fs::OpenOptions::new();
            options.read(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.custom_flags(libc::O_NOFOLLOW);
            }
            let file = options.open(path).map_err(|e| e.to_string())?;
            let metadata = file.metadata().map_err(|e| e.to_string())?;
            if !metadata.is_file() || metadata.len() > MAX_FINAL_MIR_BYTES {
                return Err("final MIR is not regular or exceeds the 64 KiB byte cap".into());
            }
            let mut bytes = Vec::new();
            file.take(MAX_FINAL_MIR_BYTES + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() as u64 > MAX_FINAL_MIR_BYTES {
                return Err("final MIR exceeded the 64 KiB byte cap while reading".into());
            }
            let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
            Ok(format!(
                "actual {FINAL_MIR_FILE} ({} bytes):\n{text}",
                text.len()
            ))
        };
        read().unwrap_or_else(|error| format!("final runtime-optimized MIR unavailable: {error}"))
    }

    fn export(
        fixture: &Path,
        target: &ScratchTarget,
        cpu: &str,
        opt: u8,
        case: &str,
    ) -> std::path::PathBuf {
        let path = target.path().join(format!("{case}.fe2sim"));
        assert!(
            !path.exists(),
            "actual export must not reuse an earlier bundle"
        );
        let mir_directory = target.path().join(format!("{case}-mir"));
        std::fs::create_dir(&mir_directory).expect("create this export's fresh MIR dump directory");
        let output = clean_command(fixture)
            .env(
                "RUSTC_WORKSPACE_WRAPPER",
                env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
            )
            .env("FE2O3_EXTRACT_CRATE_V1", CRATE)
            .env("FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1", &path)
            .env(
                "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
                format!(
                    "-Zalways-encode-mir -Zinline-mir=no -Zmir-opt-level={} -Copt-level={opt} \
                     -Ctarget-cpu={cpu} -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
                    if opt == 0 { 0 } else { 2 },
                ),
            )
            .args([
                "rustc",
                "--release",
                "--offline",
                "--locked",
                "-Zbuild-std=core",
                "--lib",
                "--target",
                "amdgcn-amd-amdhsa",
                "--target-dir",
            ])
            .arg(target.path().join("cargo"))
            .args(["--", "--cfg"])
            .arg(format!("scalar_enum_payload_case=\"{case}\""))
            .args([
                "-Zdump-mir=choose_scalar_payload&runtime-optimized",
                "-Zdump-mir-exclude-pass-number",
            ])
            .arg(format!("-Zdump-mir-dir={}", mir_directory.display()))
            .output()
            .expect("run genuine scalar enum source extraction");
        assert_eq!(
            std::fs::read_to_string(fixture.join("src/lib.rs")).unwrap(),
            SOURCE
        );
        assert!(
            output.status.success(),
            "{case}: {}\n{}",
            String::from_utf8_lossy(&output.stderr),
            final_runtime_mir(target, &mir_directory),
        );
        let bundle =
            VerifiedSimulationBundleV1::from_canonical_bytes(std::fs::read(&path).unwrap())
                .expect("admit this run's actual compiler-produced bundle");
        assert_eq!(bundle.kernel_count(), 1);
        assert_eq!(bundle.target(), format!("{cpu}:xnack-"));
        assert!(
            bundle
                .require_canonical_compiler_execution_association()
                .is_err()
        );
        let lineage = bundle.source_lineage();
        assert!(lineage.rustc_identity_inventory_receipt_bytes() > 0);
        assert_ne!(lineage.rustc_identity_inventory_receipt_sha256(), [0; 32]);
        assert!(lineage.rustc_preflight_plan_receipt_bytes() > 0);
        assert_ne!(lineage.rustc_preflight_plan_receipt_sha256(), [0; 32]);
        inspect_pure_helper(&bundle, case);
        path
    }

    fn inspect_pure_helper(bundle: &VerifiedSimulationBundleV1, case: &str) {
        let module = decode_module_v7(bundle.canonical_kir_v7())
            .expect("decode the actual source-produced KIR");
        let helpers: Vec<_> = module
            .functions
            .iter()
            .filter(|function| {
                function.role == FunctionRole::InternalHelper
                    && function.signature.parameters
                        == [Type::Scalar(ScalarType::U64), Type::Scalar(ScalarType::U64)]
                    && function.signature.results
                        == [Type::Scalar(ScalarType::U64), Type::Scalar(ScalarType::U8)]
            })
            .collect();
        assert_eq!(
            helpers.len(),
            1,
            "{case}: source helper was not retained uniquely"
        );
        let helper = helpers[0];
        let body = helper
            .body
            .as_ref()
            .expect("source helper has an actual body");
        assert!(!body.blocks.is_empty());
        for operation in body.blocks.iter().flat_map(|block| &block.operations) {
            assert!(
                !operation.memory_effects().iter().any(|effect| matches!(
                    effect,
                    MemoryEffect::Allocate(AddressSpace::Private)
                        | MemoryEffect::Read(AddressSpace::Private)
                        | MemoryEffect::Write(AddressSpace::Private)
                )),
                "{case}: retained synthetic Private effect in {:?}",
                operation.kind,
            );
        }
        let effects = analyze_interprocedural_effects_v1(&module)
            .expect("analyze the actual exported graph without modifying it");
        assert!(
            effects
                .function(&helper.id)
                .is_some_and(|decision| decision.is_complete_and_pure()),
            "{case}: retained helper is not complete and pure",
        );
    }

    fn simulate(target: &ScratchTarget, bundle: &Path, case: &str, flag: u64, value: u64) {
        let request = target.path().join("scalar-enum-request.json");
        std::fs::write(
            &request,
            serde_json::to_vec(&json!({
                "schema": "fe2o3-simulation-request-v1",
                "kernel": "scalar_enum_payload",
                "grid": [64, 1, 1],
                "workgroup": [64, 1, 1],
                "arguments": [
                    {"kind": "scalar", "type": "u64", "bits": format!("0x{flag:016x}")},
                    {"kind": "scalar", "type": "u64", "bits": format!("0x{value:016x}")},
                    {"kind": "buffer", "element": "u64", "access": "read_write",
                     "alignment": 8, "bytes": format!("0x{}", "a5".repeat(64 * 8))},
                    {"kind": "buffer", "element": "u8", "access": "read_write",
                     "alignment": 1, "bytes": format!("0x{}", "a5".repeat(64))}
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v1(bundle, &request)
            .expect("admit the current source bundle and request");
        let execution = admitted
            .input()
            .module
            .simulate(
                &admitted.input().request,
                admitted.input().simulation_target(),
                admitted.input().simulation_limits,
            )
            .unwrap_or_else(|error| panic!("{case}/{flag}/{value}: {error:?}"));
        assert_eq!(execution.invocations_executed(), 64);
        // The oracle does not evaluate an enum or reuse the source helper.
        let expected_value = (if flag == 0 { 0 } else { value }).to_le_bytes();
        let output = execution.buffer(2).unwrap().bytes();
        assert_eq!(output.len(), 64 * 8);
        assert!(
            output
                .chunks_exact(8)
                .all(|actual| actual == expected_value),
            "{case}/{flag}/{value}: scalar payload changed",
        );
        assert_eq!(
            execution.buffer(3).unwrap().bytes(),
            vec![u8::from(flag != 0); 64],
            "{case}/{flag}/{value}: absent variant or tag changed",
        );
    }

    #[test]
    #[ignore = "requires pinned nightly rust-src and AMD target; CPU execution only"]
    fn genuine_scalar_enum_payload_elision_preserves_source_semantics() {
        let target = ScratchTarget::new();
        let fixture = materialize_source_safety_fixture(&target, SOURCE);
        let metadata = clean_command(&fixture)
            .args(["metadata", "--offline", "--format-version", "1"])
            .output()
            .expect("resolve only the generated fixture lockfile");
        assert!(
            metadata.status.success(),
            "fixture resolution: {}",
            String::from_utf8_lossy(&metadata.stderr),
        );
        for cpu in ["gfx942", "gfx950"] {
            for opt in [0, 3] {
                let case = format!("scalar-enum-{cpu}-opt{opt}");
                let bundle = export(&fixture, &target, cpu, opt, &case);
                for flag in [0, 1, 2, u64::MAX] {
                    for value in [0, 1, 1_u64 << 32, 1_u64 << 63, u64::MAX] {
                        simulate(&target, &bundle, &case, flag, value);
                    }
                }
            }
        }
    }
}
