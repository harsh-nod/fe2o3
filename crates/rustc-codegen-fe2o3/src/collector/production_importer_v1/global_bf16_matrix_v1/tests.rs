use super::*;
use crate::production_semantic_terminal_v1::{
    ProductionSemanticTerminalRuleV1, ProductionTerminalExpansionV1,
};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{Operand, TerminatorKind};
use rustc_session::config::Input;
use rustc_span::FileName;

mod import_tests;
mod legacy_multiply_tests;
mod typed_terminal_tests;

const CRATE_NAME: &str = "global_bf16_source_types";
const PACKAGE_NAME: &str = "global-bf16-source-types";
const METADATA: &str = "fe2o3-global-bf16-source-v22";
const CHILD_ENV: &str = "FE2O3_GLOBAL_BF16_SOURCE_CHILD";
const OUTPUT_ENV: &str = "FE2O3_GLOBAL_BF16_SOURCE_OUTPUT";

#[test]
fn global_bf16_terminals_have_distinct_production_expansions() {
    for (item, expansion) in [
        (
            TrustedDeviceItem::Bf16MfmaGlobalMatrixALoadZeroFilled,
            ProductionTerminalExpansionV1::GlobalBf16MatrixALoadZeroFilled,
        ),
        (
            TrustedDeviceItem::Bf16MfmaGlobalMatrixBLoadZeroFilled,
            ProductionTerminalExpansionV1::GlobalBf16MatrixBLoadZeroFilled,
        ),
    ] {
        assert_eq!(
            ProductionSemanticTerminalRuleV1::from_trusted_device_item(item),
            ProductionSemanticTerminalRuleV1::Expand(expansion)
        );
    }
}

const SOURCE: &str = r#"
#![no_std]
#![allow(deprecated)]
use fe2o3_device::{KernelCapabilityBrand, CurrentTarget, RegisteredLaunch, Wave64, Wave32, WaveLane,
    GlobalBf16MfmaAMatrix, GlobalBf16MfmaBMatrix, Bf16MfmaAFragment, Bf16MfmaBFragment,
    SubgroupBrand, InitialEpoch, NextEpoch, Global, ReadOnly, StrictIeee,
    PolicyMatrixCapability, Bf16MatrixViewError};
pub enum Root {}
pub enum Foreign {}
type Brand = KernelCapabilityBrand<'static, Root, CurrentTarget, RegisteredLaunch>;
type Other = KernelCapabilityBrand<'static, Foreign, CurrentTarget, RegisteredLaunch>;
type Group = SubgroupBrand<'static, Wave64, Brand, InitialEpoch>;
type NextGroup = SubgroupBrand<'static, Wave64, Brand, NextEpoch<InitialEpoch>>;
type A = GlobalBf16MfmaAMatrix<'static, 'static, Brand, Brand>;
type B = GlobalBf16MfmaBMatrix<'static, 'static, Brand, Brand>;
pub fn read_a<'w>(m: &A, lane: &'w WaveLane<Wave64, Brand>, x: usize, y: usize) -> Bf16MfmaAFragment<'w, Brand> {
    m.load_m16k16(lane, x, y)
}
pub fn read_b<'w>(m: &B, lane: &'w WaveLane<Wave64, Brand>, x: usize, y: usize) -> Bf16MfmaBFragment<'w, Brand> {
    m.load_k16n16(lane, x, y)
}
pub fn read_group<'w>(m: &GlobalBf16MfmaAMatrix<'static, 'static, Group, Brand>, lane: &'w WaveLane<Wave64, Group>, x: usize, y: usize) -> Bf16MfmaAFragment<'w, Group> {
    m.load_m16k16(lane, x, y)
}
pub fn checked_a<'w>(matrix: &PolicyMatrixCapability<'_, Brand, Brand, StrictIeee>, bits: &Global<'static, u16, ReadOnly, Brand>, lane: &'w WaveLane<Wave64, Brand>, n: usize) -> Result<Bf16MfmaAFragment<'w, Brand>, Bf16MatrixViewError> {
    let view = matrix.bf16_a_global_row_major(bits, 0, n, n, n)?;
    Ok(view.load_m16k16(lane, 0, 0))
}
pub fn checked_b<'w>(matrix: &PolicyMatrixCapability<'_, Brand, Brand, StrictIeee>, bits: &Global<'static, u16, ReadOnly, Brand>, lane: &'w WaveLane<Wave64, Brand>, n: usize) -> Result<Bf16MfmaBFragment<'w, Brand>, Bf16MatrixViewError> {
    let view = matrix.bf16_b_global_row_major(bits, 0, n, n, n)?;
    Ok(view.load_k16n16(lane, 0, 0))
}
pub fn wrong_epoch<'w>(_: &GlobalBf16MfmaAMatrix<'static, 'static, Group, Brand>, _: &'w WaveLane<Wave64, NextGroup>, _: usize, _: usize) -> Bf16MfmaAFragment<'w, Group> { loop {} }
pub fn wrong_root<'w>(_: &GlobalBf16MfmaAMatrix<'static, 'static, Brand, Other>, _: &'w WaveLane<Wave64, Brand>, _: usize, _: usize) -> Bf16MfmaAFragment<'w, Brand> { loop {} }
pub fn wrong_width<'w>(_: &A, _: &'w WaveLane<Wave32, Brand>, _: usize, _: usize) -> Bf16MfmaAFragment<'w, Brand> { loop {} }
pub fn wrong_result<'w>(_: &A, _: &'w WaveLane<Wave64, Brand>, _: usize, _: usize) -> Bf16MfmaBFragment<'w, Brand> { loop {} }
pub struct FakeGlobalMatrix;
pub fn fake_matrix<'w>(_: &FakeGlobalMatrix, _: &'w WaveLane<Wave64, Brand>, _: usize, _: usize) -> Bf16MfmaAFragment<'w, Brand> { loop {} }
"#;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProbeStage {
    Nominal,
    Import,
    Ssa,
    TypedTerminals,
    LegacyMultiply,
    LegacyMultiplyImport,
}

struct Probe {
    ran: bool,
    cpu: String,
    stage: ProbeStage,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("global_bf16_types.rs".into()),
            input: match self.stage {
                ProbeStage::Nominal => SOURCE.into(),
                ProbeStage::Import | ProbeStage::Ssa => import_tests::SOURCE.into(),
                ProbeStage::TypedTerminals => typed_terminal_tests::SOURCE.into(),
                ProbeStage::LegacyMultiply | ProbeStage::LegacyMultiplyImport => {
                    legacy_multiply_tests::SOURCE.into()
                }
            },
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if matches!(
            self.stage,
            ProbeStage::LegacyMultiply | ProbeStage::LegacyMultiplyImport
        ) {
            legacy_multiply_tests::check(
                tcx,
                &self.cpu,
                self.stage == ProbeStage::LegacyMultiplyImport,
            );
            self.ran = true;
            return Compilation::Stop;
        }
        if self.stage == ProbeStage::TypedTerminals {
            typed_terminal_tests::check(tcx, &self.cpu);
            self.ran = true;
            return Compilation::Stop;
        }
        if self.stage != ProbeStage::Nominal {
            import_tests::check(tcx, &self.cpu, self.stage == ProbeStage::Ssa);
            self.ran = true;
            return Compilation::Stop;
        }
        for item in [
            TrustedDeviceItem::Bf16MfmaGlobalMatrixView,
            TrustedDeviceItem::Bf16MfmaGlobalMatrixALoadZeroFilled,
            TrustedDeviceItem::Bf16MfmaGlobalMatrixBLoadZeroFilled,
            TrustedDeviceItem::WaveLane,
            TrustedDeviceItem::Bf16MfmaFragment,
            TrustedDeviceItem::CapabilityMemoryView,
        ] {
            trusted_device_items::authenticated_compiler_definition_observation_v1(tcx, item)
                .unwrap_or_else(|error| panic!("{item:?}: {error}"));
        }
        let mut checked = 0;
        for (name, role, positive) in [
            ("read_a", SemanticMfmaOperandRoleV1::A, true),
            ("read_b", SemanticMfmaOperandRoleV1::B, true),
            ("read_group", SemanticMfmaOperandRoleV1::A, true),
            ("wrong_epoch", SemanticMfmaOperandRoleV1::A, false),
            ("wrong_root", SemanticMfmaOperandRoleV1::A, false),
            ("wrong_width", SemanticMfmaOperandRoleV1::A, false),
            ("wrong_result", SemanticMfmaOperandRoleV1::A, false),
            ("fake_matrix", SemanticMfmaOperandRoleV1::A, false),
        ] {
            let definition = tcx
                .iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == name
                })
                .unwrap();
            let instance = Instance::mono(tcx, definition.to_def_id());
            let signature = tcx.instantiate_bound_regions_with_erased(
                tcx.fn_sig(instance.def_id())
                    .instantiate(tcx, instance.args),
            );
            let result = validate_source(tcx, role, signature.inputs(), signature.output());
            assert_eq!(result.is_ok(), positive, "{name}: {:?}", result.err());
            if positive {
                let actual = tcx
                    .instance_mir(instance.def)
                    .basic_blocks
                    .iter()
                    .find_map(|block| {
                        let TerminatorKind::Call {
                            func: Operand::Constant(callee),
                            ..
                        } = &block.terminator().kind
                        else {
                            return None;
                        };
                        let TyKind::FnDef(definition, _) = callee.const_.ty().kind() else {
                            return None;
                        };
                        trusted_device_items::classify(tcx, *definition)
                    })
                    .expect("the original source calls an authenticated device terminal");
                let expected = match role {
                    SemanticMfmaOperandRoleV1::A => {
                        TrustedDeviceItem::Bf16MfmaGlobalMatrixALoadZeroFilled
                    }
                    SemanticMfmaOperandRoleV1::B => {
                        TrustedDeviceItem::Bf16MfmaGlobalMatrixBLoadZeroFilled
                    }
                };
                assert_eq!(actual, expected);
                assert!(matches!(
                    ProductionSemanticTerminalRuleV1::from_trusted_device_item(actual),
                    ProductionSemanticTerminalRuleV1::Expand(_)
                ));
                let opposite = match role {
                    SemanticMfmaOperandRoleV1::A => SemanticMfmaOperandRoleV1::B,
                    SemanticMfmaOperandRoleV1::B => SemanticMfmaOperandRoleV1::A,
                };
                assert!(
                    validate_source(tcx, opposite, signature.inputs(), signature.output()).is_err()
                );
            }
            checked += 1;
        }
        assert_eq!(checked, 8);
        for (name, role, constructor) in [
            (
                "checked_a",
                SemanticMfmaOperandRoleV1::A,
                "bf16_a_global_row_major",
            ),
            (
                "checked_b",
                SemanticMfmaOperandRoleV1::B,
                "bf16_b_global_row_major",
            ),
        ] {
            let definition = tcx
                .iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == name
                })
                .unwrap();
            let body = tcx.instance_mir(Instance::mono(tcx, definition.to_def_id()).def);
            let mut retained_constructor = false;
            let mut retained_read = false;
            for block in body.basic_blocks.iter() {
                let TerminatorKind::Call {
                    func: Operand::Constant(callee),
                    ..
                } = &block.terminator().kind
                else {
                    continue;
                };
                let TyKind::FnDef(definition, arguments) = callee.const_.ty().kind() else {
                    continue;
                };
                retained_constructor |= tcx.item_name(*definition).as_str() == constructor;
                let expected = match role {
                    SemanticMfmaOperandRoleV1::A => {
                        TrustedDeviceItem::Bf16MfmaGlobalMatrixALoadZeroFilled
                    }
                    SemanticMfmaOperandRoleV1::B => {
                        TrustedDeviceItem::Bf16MfmaGlobalMatrixBLoadZeroFilled
                    }
                };
                if trusted_device_items::classify(tcx, *definition) == Some(expected) {
                    let signature = tcx.instantiate_bound_regions_with_erased(
                        tcx.fn_sig(*definition).instantiate(tcx, arguments),
                    );
                    assert!(
                        validate_source(tcx, role, signature.inputs(), signature.output()).is_ok()
                    );
                    retained_read = true;
                }
            }
            assert!(
                retained_constructor && retained_read,
                "{name}: checked Result construction and exact terminal must remain real calls"
            );
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn cached(name: &str) -> std::path::PathBuf {
    std::fs::canonicalize(
        std::env::var_os(name)
            .unwrap_or_else(|| panic!("{name} must name existing actual AMD metadata")),
    )
    .unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn source_types(cpu: &str) {
    run_probe(
        cpu,
        &format!("global_bf16_nominal_source_types_{cpu}"),
        ProbeStage::Nominal,
    );
}

fn run_probe(cpu: &str, name: &str, stage: ProbeStage) {
    use fe2o3_rustc_invocation::{
        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, derive_cargo_metadata_build_observation_v2,
    };

    let device = cached("FE2O3_CORE_TRY_DEVICE_RMETA");
    let host_deps = cached("FE2O3_CORE_TRY_HOST_DEPS");
    let core = cached("FE2O3_CORE_TRY_AMDGPU_CORE");
    let builtins = cached("FE2O3_CORE_TRY_AMDGPU_BUILTINS");
    let observation = derive_cargo_metadata_build_observation_v2(&[METADATA]).to_hex();
    let child_key = format!("{name}:{cpu}:{observation}");
    let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    if std::env::var(CHILD_ENV).ok().as_deref() != Some(child_key.as_str()) {
        let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-global-bf16-source");
        let device_source = repo.join("crates/fe2o3-device").canonicalize().unwrap();
        assert!(device_source.join("Cargo.toml").is_file());
        // proc_macro_crate reads this manifest to resolve the actual device
        // dependency. It is not source evidence and never triggers Cargo.
        std::fs::write(
            scratch.path().join("Cargo.toml"),
            format!(
                "[package]\nname = \"{PACKAGE_NAME}\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nfe2o3-device = {{ path = {device_source:?} }}\n"
            ),
        )
        .unwrap();
        let test =
            format!("collector::production_importer_v1::global_bf16_matrix_v1::tests::{name}");
        // Keep the invocation observation and provider source cache isolated
        // from other tests while authenticating the actual cached metadata.
        let output = crate::process_execution::capture_output(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &test,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .current_dir(&repo)
                .env(CHILD_ENV, &child_key)
                .env(OUTPUT_ENV, scratch.path())
                .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, &observation)
                .env("FE2O3_CORE_TRY_DEVICE_RMETA", &device)
                .env("FE2O3_CORE_TRY_HOST_DEPS", &host_deps)
                .env("FE2O3_CORE_TRY_AMDGPU_CORE", &core)
                .env("FE2O3_CORE_TRY_AMDGPU_BUILTINS", &builtins)
                .env("CARGO_MANIFEST_DIR", scratch.path())
                .env("CARGO_PKG_NAME", PACKAGE_NAME)
                .env(
                    reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1,
                    reserved_fe2o3_symbols::derive_crate_binding_id_v1(CRATE_NAME, [METADATA])
                        .to_hex(),
                )
                .env("FE2O3_SIMULATION_MODE_V1", "1")
                .env(
                    "FE2O3_SIMULATION_ATTEMPT_V1",
                    "80808080808080808080808080808080",
                ),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "global BF16 child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        return;
    }
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        observation
    );
    assert_eq!(std::env::current_dir().unwrap(), repo);
    assert_eq!(
        std::env::var(reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1).unwrap(),
        reserved_fe2o3_symbols::derive_crate_binding_id_v1(CRATE_NAME, [METADATA]).to_hex()
    );
    let scratch = cached(OUTPUT_ENV);
    assert_eq!(cached("CARGO_MANIFEST_DIR"), scratch);
    assert!(scratch.join("Cargo.toml").is_file());
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        format!("--crate-name={CRATE_NAME}"),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        format!("--out-dir={}", scratch.display()),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "--target=amdgcn-amd-amdhsa".into(),
        format!("-Ctarget-cpu={cpu}"),
        "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
        "-Zno-codegen".into(),
        "-Zalways-encode-mir".into(),
        "-Zunstable-options".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-Cdebuginfo=2".into(),
        format!("-Cmetadata={METADATA}"),
        "--extern".into(),
        format!("fe2o3_device={}", device.display()),
        "-L".into(),
        format!("dependency={}", device.parent().unwrap().display()),
        "-L".into(),
        format!("dependency={}", host_deps.display()),
        "--extern".into(),
        format!("noprelude,nounused:core={}", core.display()),
        "--extern".into(),
        format!(
            "noprelude,nounused:compiler_builtins={}",
            builtins.display()
        ),
        "-".into(),
    ];
    let mut probe = Probe {
        ran: false,
        cpu: cpu.into(),
        stage,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
#[ignore = "requires existing actual AMD device/core/builtins metadata and host dependencies; never builds dependencies"]
fn global_bf16_nominal_source_types_gfx942() {
    source_types("gfx942");
}

#[test]
#[ignore = "requires existing actual AMD device/core/builtins metadata and host dependencies; never builds dependencies"]
fn global_bf16_nominal_source_types_gfx950() {
    source_types("gfx950");
}
