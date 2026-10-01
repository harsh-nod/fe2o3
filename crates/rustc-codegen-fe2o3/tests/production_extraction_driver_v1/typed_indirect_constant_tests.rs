mod typed_indirect_constant_tests {
    use super::{ScratchTarget, materialize_source_safety_fixture};
    use fe2o3_kernel_ir::VerifiedSimulationBundleV1;
    use serde_json::json;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const CRATE: &str = "fe2o3_production_source_safety_fixture";
    const SOURCE: &str = r#"#![no_std]
use fe2o3_device::{kernel, thread, DisjointSlice};

#[repr(i8)]
enum Direct {
    Empty = -7,
    Narrow(u8) = 12,
    Wide(u64) = 91,
}

enum Niche {
    Empty,
    Filled(bool, u64),
    Other,
}

#[inline(never)]
fn nested_constant(flag: u64, value: u64) -> ((u64, u8), [u16; 2]) {
    if flag == 0 {
        ((0x0123_4567_89ab_cdef_u64, 0xa5_u8), [0x1234_u16, 0xabcd_u16])
    } else {
        ((value, 0x5a_u8), [0x2468_u16, 0x1357_u16])
    }
}

#[inline(never)]
fn direct_constant(flag: u64, small: u8, wide: u64) -> (u64, u8) {
    let selected = if flag == 0 {
        Direct::Empty
    } else if flag == 1 {
        Direct::Narrow(small)
    } else {
        Direct::Wide(wide)
    };
    match selected {
        Direct::Empty => (0_u64, 249_u8),
        Direct::Narrow(value) => (value as u64, 12_u8),
        Direct::Wide(value) => (value, 91_u8),
    }
}

#[inline(never)]
fn niche_constant(flag: u64, truth: bool, value: u64) -> (u64, u8) {
    let selected = if flag == 0 {
        Niche::Empty
    } else if flag == 1 {
        Niche::Filled(truth, value)
    } else {
        Niche::Other
    };
    match selected {
        Niche::Empty => (0_u64, 2_u8),
        Niche::Filled(truth, value) => (value, truth as u8),
        Niche::Other => (0_u64, 4_u8),
    }
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn typed_indirect_constants(flag: u64, value: u64,
    mut nested: DisjointSlice<u64>, mut nested_small: DisjointSlice<u64>,
    mut direct: DisjointSlice<u64>, mut direct_tag_output: DisjointSlice<u64>,
    mut niche: DisjointSlice<u64>, mut niche_tag_output: DisjointSlice<u64>) {
    let ((wide, tag), [left, right]) = nested_constant(flag, value);
    let small_bits = ((tag as u64) << 32) | ((left as u64) << 16) | right as u64;
    let (direct_value, direct_tag) = direct_constant(flag, value as u8, value);
    let (niche_value, niche_tag) = niche_constant(flag, value & 1 != 0, value);
    if let core::option::Option::Some(slot) = nested.get_mut(thread::index_1d()) {
        *slot = wide;
    }
    if let core::option::Option::Some(slot) = nested_small.get_mut(thread::index_1d()) {
        *slot = small_bits;
    }
    if let core::option::Option::Some(slot) = direct.get_mut(thread::index_1d()) {
        *slot = direct_value;
    }
    if let core::option::Option::Some(slot) = direct_tag_output.get_mut(thread::index_1d()) {
        *slot = direct_tag as u64;
    }
    if let core::option::Option::Some(slot) = niche.get_mut(thread::index_1d()) {
        *slot = niche_value;
    }
    if let core::option::Option::Some(slot) = niche_tag_output.get_mut(thread::index_1d()) {
        *slot = niche_tag as u64;
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

    fn export(fixture: &Path, target: &ScratchTarget, cpu: &str, opt: u8, case: &str) -> PathBuf {
        let path = target.path().join(format!("{case}.fe2sim"));
        assert!(!path.exists(), "each export must produce its own bundle");
        let mut command = clean_command(fixture);
        command
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
            .arg(format!("typed_indirect_constant_case=\"{case}\""));
        // Optional dump flags are used only in a separate failure observation;
        // the returned status and successful bundle belong to the original run.
        let output = super::source_failure_mir_v1::run(
            &mut command,
            &["nested_constant", "direct_constant", "niche_constant"],
        );
        assert_eq!(
            std::fs::read_to_string(fixture.join("src/lib.rs")).unwrap(),
            SOURCE,
        );
        assert!(
            output.status.success(),
            "{case}: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        let bundle =
            VerifiedSimulationBundleV1::from_canonical_bytes(std::fs::read(&path).unwrap())
                .expect("admit this run's compiler-produced source bundle");
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
        path
    }

    fn simulate(target: &ScratchTarget, bundle: &Path, case: &str, flag: u64, value: u64) {
        let request = target.path().join("typed-constant-request.json");
        let buffer = || {
            json!({
                "kind": "buffer", "element": "u64", "access": "read_write",
                "alignment": 8, "bytes": format!("0x{}", "a5".repeat(64 * 8))
            })
        };
        std::fs::write(
            &request,
            serde_json::to_vec(&json!({
                "schema": "fe2o3-simulation-request-v1",
                "kernel": "typed_indirect_constants",
                "grid": [64, 1, 1],
                "workgroup": [64, 1, 1],
                "arguments": [
                    {"kind": "scalar", "type": "u64", "bits": format!("0x{flag:016x}")},
                    {"kind": "scalar", "type": "u64", "bits": format!("0x{value:016x}")},
                    buffer(), buffer(), buffer(), buffer(), buffer(), buffer()
                ]
            }))
            .unwrap(),
        )
        .unwrap();
        let admitted = fe2o3_kir_sim_cli::load_debug_simulation_bundle_v1(bundle, &request)
            .expect("admit the actual source bundle and scalar-bit request");
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

        // Independent scalar oracle: no Rust aggregate layouts or enum decoding.
        let (nested, nested_small) = if flag == 0 {
            (0x0123_4567_89ab_cdef, 0xa5_1234_abcd)
        } else {
            (value, 0x5a_2468_1357)
        };
        let (direct, direct_tag) = match flag {
            0 => (0, 249),
            1 => (value & 255, 12),
            _ => (value, 91),
        };
        let (niche, niche_tag) = if flag == 0 {
            (0, 2)
        } else if flag == 1 {
            (value, value & 1)
        } else {
            (0, 4)
        };
        for (index, expected) in [nested, nested_small, direct, direct_tag, niche, niche_tag]
            .into_iter()
            .enumerate()
        {
            let output = execution.buffer(index + 2).unwrap().bytes();
            assert_eq!(output.len(), 64 * 8);
            let expected = expected.to_le_bytes();
            assert!(
                output.chunks_exact(8).all(|actual| actual == expected),
                "{case}/{flag}/{value}: typed constant buffer {index} changed",
            );
        }
    }

    #[test]
    #[ignore = "requires pinned nightly rust-src and AMD target; CPU execution only"]
    fn genuine_typed_indirect_constants_preserve_source_semantics() {
        let target = ScratchTarget::new();
        let fixture = materialize_source_safety_fixture(&target, SOURCE);
        let metadata = clean_command(&fixture)
            .args(["metadata", "--offline", "--format-version", "1"])
            .output()
            .expect("resolve only this fixture's lockfile");
        assert!(
            metadata.status.success(),
            "fixture resolution: {}",
            String::from_utf8_lossy(&metadata.stderr),
        );
        for cpu in ["gfx942", "gfx950"] {
            for opt in [0, 3] {
                let case = format!("typed-constant-{cpu}-opt{opt}");
                let bundle = export(&fixture, &target, cpu, opt, &case);
                for flag in [0, 1, 2, u64::MAX] {
                    for value in [0, 1, 0x0123_4567_89ab_cdef, u64::MAX] {
                        simulate(&target, &bundle, &case, flag, value);
                    }
                }
            }
        }
    }
}
