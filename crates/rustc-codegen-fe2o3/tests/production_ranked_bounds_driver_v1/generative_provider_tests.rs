#[test]
#[ignore = "requires the pinned nightly rust-src component and AMD target"]
fn staged_generative_providers_reject_without_export_authority() {
    const REASON: &str = EXECUTION_ROLE_MATERIALIZATION_PENDING;
    const ENTRY_REASON: &str = EXECUTION_ROLE_MATERIALIZATION_PENDING;
    let target = ScratchTarget::new();
    let build_dir = target.path().join("provider-target");
    let mut failures = Vec::new();
    for (feature, expected) in [
        ("provider_context", REASON),
        ("provider_context_entry", ENTRY_REASON),
        ("provider_context_entry_result", ENTRY_REASON),
        (
            "provider_context_helper_issue",
            "issuance outside a declared physical root",
        ),
        (
            "provider_context_unregistered_issue",
            "issuance outside a declared physical root",
        ),
        ("provider_context_alias", REASON),
        ("provider_context_nested", REASON),
        ("provider_context_reference", REASON),
        ("provider_context_empty_array", REASON),
        ("provider_context_helper_result", REASON),
        ("provider_workgroup", REASON),
        ("provider_tile", REASON),
        ("provider_fragment", REASON),
        (
            "provider_tile_chain",
            "bounded closure admission failed: bounded closure profile rejected MIR: raw-pointer captures have no allocation authority",
        ),
    ] {
        let bundle = target.path().join(format!("{feature}.fe2sim"));
        let result = output(
            simulation_export_command_for_feature("gfx942", &bundle, &build_dir, Some(5), feature),
            "reject staged nominal authority",
        );
        if result.status.success() || !result.stderr.contains(expected) {
            failures.push(format!(
                "{feature} did not reach {expected:?}:\n{}",
                result.stderr,
            ));
        }
        assert!(!bundle.exists(), "{feature} emitted a simulation bundle");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));

    let bundle = target.path().join("provider-gfx950.fe2sim");
    let result = output(
        simulation_export_command_for_feature(
            "gfx950",
            &bundle,
            &build_dir,
            Some(5),
            "provider_context",
        ),
        "reject staged authority independently of the AMD profile",
    );
    assert!(
        !result.status.success() && result.stderr.contains(REASON),
        "{}",
        result.stderr
    );
    assert!(!bundle.exists());

    let llvm = target.path().join("provider.ll");
    let mut command = base_command("check", &build_dir);
    command
        .env_remove("FE2O3_EXTRACT_RANKED_MEMORY_V1")
        .env(
            "RUSTC_WORKSPACE_WRAPPER",
            env!("CARGO_BIN_EXE_fe2o3-rustc-extract"),
        )
        .env(
            "FE2O3_EXTRACT_CRATE_V1",
            "fe2o3_production_ranked_bounds_fixture",
        )
        .env("FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1", &llvm)
        .args(["--features", "provider_context"]);
    let result = output(command, "reject staged authority before LLVM output");
    assert!(
        !result.status.success() && result.stderr.contains(REASON),
        "{}",
        result.stderr
    );
    assert!(!llvm.exists(), "rejected provider emitted LLVM");

    for feature in [
        "aggregate_zst",
        "provider_phantom",
        "provider_phantom_reference",
    ] {
        let control = target.path().join(format!("{feature}.fe2sim"));
        let result = output(
            simulation_export_command_for_feature("gfx942", &control, &build_dir, Some(5), feature),
            "preserve ordinary zero-sized source arguments",
        );
        assert!(
            result.status.success(),
            "{feature} rejected:\n{}",
            result.stderr
        );
        assert!(control.is_file());
    }
    // The protocol matrix builds its own fixture; release this completed cache first.
    drop(target);
    check_kernel_context_source_protocol();
    check_context_root_roster();
    check_execution_role_source_shapes();
}

fn check_context_root_roster() {
    let target = ScratchTarget::new();
    let build_dir = target.path().join("context-roster-target");
    let mut failures = Vec::new();
    for profile in ["gfx942", "gfx950"] {
        for names in [["alpha", "zeta"], ["zeta", "alpha"]] {
            let label = format!("{profile}-{}", names[0]);
            let source_path = target.path().join(format!("{label}.rs"));
            let bundle = target.path().join(format!("{label}.fe2sim"));
            let mut source = "use fe2o3_device::{kernel, KernelContext};\ntype Tag = core::marker::PhantomData<&'static u32>;\n".to_owned();
            for name in names {
                source.push_str(&format!(
                    r#"
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn {name}(_context: KernelContext<'_>, a: u32, b: u32, tag: Tag) {{ let _ = (a, b, tag); }}
"#
                ));
            }
            std::fs::write(&source_path, source).unwrap();
            let mut command = simulation_export_command_for_feature(
                profile,
                &bundle,
                &build_dir,
                Some(5),
                "provider_context_protocol",
            );
            command.env("FE2O3_CONTEXT_PROTOCOL_SOURCE", &source_path);
            let result = output(command, "construct each distinct physical context root");
            if let Some(diagnostic) = result
                .stderr
                .lines()
                .find(|line| line.contains("fe2o3 rustc extraction:"))
            {
                eprintln!("context roster {label}: {diagnostic}");
            }
            if result.status.success()
                || !result
                    .stderr
                    .contains(EXECUTION_ROLE_MATERIALIZATION_PENDING)
            {
                failures.push(format!(
                    "{label} did not reach context materialization refusal:\n{}",
                    result.stderr
                ));
            }
            assert!(
                std::fs::symlink_metadata(&bundle)
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn check_execution_role_source_shapes() {
    const GEOMETRY: &str = "execution tile role requires 1..=256 lanes and 1..=125 elements";
    const ELEMENT: &str = "execution tile role requires the u32 element profile";
    let target = ScratchTarget::new();
    let build_dir = target.path().join("role-shapes-target");
    let mut failures = Vec::new();
    for profile in ["gfx942", "gfx950"] {
        for (name, carrier, element, lanes, elements, expected) in [
            (
                "minimum",
                "MaskedTile1D",
                "u32",
                1,
                1,
                EXECUTION_ROLE_MATERIALIZATION_PENDING,
            ),
            (
                "non_power_of_two",
                "MaskedTile1D",
                "u32",
                3,
                2,
                EXECUTION_ROLE_MATERIALIZATION_PENDING,
            ),
            (
                "above_wave_width",
                "LaneFragment",
                "u32",
                65,
                2,
                EXECUTION_ROLE_MATERIALIZATION_PENDING,
            ),
            (
                "maximum_tile",
                "MaskedTile1D",
                "u32",
                256,
                125,
                EXECUTION_ROLE_MATERIALIZATION_PENDING,
            ),
            (
                "maximum_fragment",
                "LaneFragment",
                "u32",
                256,
                125,
                EXECUTION_ROLE_MATERIALIZATION_PENDING,
            ),
            ("zero_lanes", "MaskedTile1D", "u32", 0, 2, GEOMETRY),
            ("excess_lanes", "MaskedTile1D", "u32", 257, 2, GEOMETRY),
            (
                "truncated_lanes",
                "LaneFragment",
                "u32",
                65_537,
                2,
                GEOMETRY,
            ),
            ("zero_elements", "LaneFragment", "u32", 3, 0, GEOMETRY),
            ("excess_elements", "MaskedTile1D", "u32", 3, 126, GEOMETRY),
            ("float_element", "MaskedTile1D", "f32", 3, 2, ELEMENT),
            ("wide_element", "LaneFragment", "u64", 3, 2, ELEMENT),
        ] {
            let source_path = target.path().join(format!("{profile}-{name}.rs"));
            let bundle = target.path().join(format!("{profile}-{name}.fe2sim"));
            // A referenced carrier keeps all nominal type/layout facts without
            // requiring an issuer or pretending its bytes are caller authority.
            let source = format!(
                r#"
use fe2o3_device::{{DisjointSlice, kernel, thread}};
type Role = fe2o3_device::{carrier}<'static, {element}, {lanes}, {elements}, ()>;
#[inline(never)]
fn carrier_type() -> Option<&'static Role> {{ None }}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn role_probe(mut output: DisjointSlice<u32>) {{
    let _ = carrier_type();
    if let Some(slot) = output.get_mut(thread::index_1d()) {{ *slot = 7; }}
}}
"#
            );
            std::fs::write(&source_path, source).unwrap();
            let mut command = simulation_export_command_for_feature(
                profile,
                &bundle,
                &build_dir,
                Some(5),
                "provider_context_protocol",
            );
            command.env("FE2O3_CONTEXT_PROTOCOL_SOURCE", &source_path);
            let result = output(command, "preserve exact source execution role shape");
            if let Some(diagnostic) = result
                .stderr
                .lines()
                .find(|line| line.contains("fe2o3 rustc extraction:"))
            {
                eprintln!("execution role {profile}/{name}: {diagnostic}");
            }
            if result.status.success() || !result.stderr.contains(expected) {
                failures.push(format!(
                    "{profile}/{name} did not reach {expected:?}:\n{}",
                    result.stderr,
                ));
            }
            assert!(
                std::fs::symlink_metadata(&bundle)
                    .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound),
                "{profile}/{name} emitted a simulation bundle"
            );
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
const EXECUTION_ROLE_MATERIALIZATION_PENDING: &str =
    "execution capabilities require checked canonical KIR materialization";
