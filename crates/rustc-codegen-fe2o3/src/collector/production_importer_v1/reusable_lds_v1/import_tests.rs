//! Actual allocation/conversion import, source replay and consuming SSA receipt.
use super::*;
#[path = "import_tests/lowering.rs"]
mod lowering;
#[path = "import_tests/mixed43_bind_diagnostic.rs"]
mod mixed43_bind_diagnostic;
use crate::rustc_semantic_plan_v1::{
    DebugSourceCaptureRequestV2, build_production_semantic_preflight_plan_v1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;
use std::collections::BTreeSet;
use std::path::PathBuf;

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{KernelContext, kernel};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn reusable(mut context: KernelContext<'_>, enabled: u32) {
    if enabled == 0 { return; }
    context.with_workgroup(|workgroup| {
        let _reusable = workgroup.allocate_lds::<[f32; 2], 64>().into_reusable();
    });
}
"#;
struct Probe {
    cpu: &'static str,
    check_kir: bool,
    completed: bool,
}
impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("reusable_lds_import_source.rs".into()),
            input: if self.check_kir {
                lowering::SOURCE
            } else {
                SOURCE
            }
            .into(),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::collector::production_importer_v1::{
            build_identity_inventory_v1, canonical_target_layout_v1,
            construct_production_semantic_mir_v1, validate_execution_terminal_carriage_v1,
        };
        use crate::production_target_v1::RetainedProductionTargetV1;
        let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        let closure = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
        )
        .expect("collect registered matrix constructors as original defined helpers");
        let typed_roots = self.check_kir.then(|| {
            eprintln!("[fe2o3-reusable-source-kir-v1] stage=actual-collection passed");
            closure
                .rederive_typed_descriptor_roots(tcx)
                .expect("actual scalar source descriptor roots")
        });
        let observed_target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
            .unwrap()
            .authenticate_import_session(tcx)
            .unwrap();
        let inventory =
            build_identity_inventory_v1(tcx, &observed_target, &closure.collection, &closure.roots)
                .unwrap();
        let closure_types = closure
            .collection
            .functions
            .iter()
            .filter_map(|function| function.closure_plan.as_ref())
            .flat_map(|plan| plan.authenticated_closure_type_identities())
            .map(SemanticTypeIdentityV1::from_sha256)
            .collect::<BTreeSet<_>>();
        let plan = build_production_semantic_preflight_plan_v1(
            tcx,
            canonical_target_layout_v1(observed_target.rustc_layout()),
            inventory.functions,
            inventory.roots,
            inventory.sha256,
            &closure_types,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("retain the actual matrix producer plan, including authenticated closure types");
        crate::rustc_semantic_plan_v1::lds_type_dependencies_v1::assert_live_roster(tcx, &plan);
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("actual nested LDS allocation and ownership conversion must import");
        assert_eq!(imported.rustc_identity_inventory.sha256(), inventory.sha256);
        assert_eq!(
            imported.rustc_preflight_plan.canonical_transcript(),
            plan.canonical_transcript()
        );
        let mir = &imported.semantic_mir;
        assert_eq!(mir.wire_version(), SemanticMirWireVersionV1::V24);
        mir.require_complete_external_entries().unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(decoded.canonical_encoding(), mir.canonical_encoding());
        validate_execution_terminal_carriage_v1(tcx, &plan, &imported.kernel_contexts, &decoded)
            .unwrap();
        validate_carriage(tcx, &plan, &imported.kernel_contexts, &decoded).unwrap();
        source_replay_tests::check(tcx, &plan, &decoded);
        let records = mir
            .functions()
            .iter()
            .filter_map(|f| match f.defined_capability_contract() {
                Some(SemanticDefinedCapabilityContractV1::ReusableLdsConversion(r)) => Some(*r),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [record] = records.as_slice() else {
            panic!("one exact reusable LDS converter")
        };
        assert_eq!(record.elements(), 64);
        assert_eq!(record.element_size(), if self.check_kir { 4 } else { 8 });
        assert_eq!(record.element_align(), 4);
        assert_ne!(record.types().input, record.types().output);
        let mut source = record.source();
        source.source_binding[0] ^= 1;
        let replacement = SemanticReusableLdsConversionV1::for_defined_function(
            record.function(),
            mir.functions(),
            mir.callables(),
            mir.types(),
            record.types(),
            source,
            record.provenance(),
            record.brand(),
            record.epoch(),
            record.elements(),
        )
        .unwrap();
        let mut functions = mir.functions().to_vec();
        let f = &functions[record.function().index() as usize];
        functions[record.function().index() as usize] = SemanticFunctionDeclV1::new(
            f.identity(),
            f.role(),
            f.item_definition_identity(),
            f.monomorphization_identity(),
            f.generic_type_arguments_identity(),
            f.const_generic_arguments_identity(),
            f.source(),
            f.abi().clone(),
            f.locals().to_vec(),
            f.entry(),
            f.blocks().to_vec(),
        )
        .unwrap()
        .with_defined_capability_contract(
            SemanticDefinedCapabilityContractV1::ReusableLdsConversion(replacement),
        )
        .unwrap();
        let substituted = InertSemanticMirRequestV1::new_with_callables(
            mir.target(),
            mir.types().to_vec(),
            mir.allocations().to_vec(),
            mir.statics().to_vec(),
            mir.vtables().to_vec(),
            functions,
            mir.callables().to_vec(),
            mir.roots().to_vec(),
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        assert!(matches!(
            validate_carriage(tcx, &plan, &imported.kernel_contexts, &substituted),
            Err(ProductionSemanticImportErrorV1::KernelContextBinding(
                "reusable LDS source custody was forged, erased or substituted"
            ))
        ));
        let owner = fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
            fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
                decoded,
                fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        )
        .expect("retained LDS allocation and conversion must build consuming SSA");
        owner.verify_replay().unwrap();
        let [root] = owner.source_semantic().roots() else {
            panic!("one root")
        };
        let results = owner
            .execution_plan_for_root(*root)
            .unwrap()
            .defined_reusable_lds_results();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].record(), *record);
        assert_ne!(results[0].allocation(), results[0].parameter());
        assert_ne!(results[0].parameter(), results[0].return_local());
        assert!(
            owner
                .execution_plan_for_root(*root)
                .unwrap()
                .defined_math_results()
                .is_empty()
        );
        if let Some(typed_roots) = typed_roots {
            eprintln!(
                "[fe2o3-reusable-source-kir-v1] stage=import-decode-source-replay-substitution-and-ssa passed"
            );
            lowering::check(imported, typed_roots);
        }
        assert_eq!(tcx.sess.opts.cg.target_cpu.as_deref(), Some(self.cpu));
        self.completed = true;
        Compilation::Stop
    }
}

const CRATE_NAME: &str = "reusable_lds_import_source";
const METADATA: &str = "fe2o3-reusable-lds-import-v1";
const CHILD: &str = "FE2O3_REUSABLE_LDS_IMPORT_CHILD";

fn configured(name: &str) -> PathBuf {
    let path = PathBuf::from(
        std::env::var_os(name)
            .unwrap_or_else(|| panic!("set {name}; this test never builds dependencies")),
    );
    path.canonicalize()
        .unwrap_or_else(|error| panic!("{name}: {error}"))
}

fn run(cpu: &'static str, name: &str) {
    run_with_lowering(cpu, name, false);
}

fn run_with_lowering(cpu: &'static str, name: &str, check_kir: bool) {
    use fe2o3_rustc_invocation::{
        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, derive_cargo_metadata_build_observation_v2,
    };
    let device = configured("FE2O3_CORE_TRY_DEVICE_RMETA");
    let host = configured("FE2O3_CORE_TRY_HOST_DEPS");
    let core = configured("FE2O3_CORE_TRY_AMDGPU_CORE");
    let builtins = configured("FE2O3_CORE_TRY_AMDGPU_BUILTINS");
    let observation = derive_cargo_metadata_build_observation_v2(&[METADATA]).to_hex();
    let binding =
        reserved_fe2o3_symbols::derive_crate_binding_id_v1(CRATE_NAME, [METADATA]).to_hex();
    let key = format!("{cpu}:{observation}:kir={check_kir}");
    if std::env::var(CHILD).ok().as_deref() != Some(key.as_str()) {
        let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-reusable-lds-source");
        let device_source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../fe2o3-device");
        std::fs::write(scratch.path().join("Cargo.toml"), format!(
            "[package]\nname = \"reusable-lds-import-source\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[workspace]\n[dependencies]\nfe2o3-device = {{ path = {device_source:?} }}\n"
        )).unwrap();
        let test =
            format!("collector::production_importer_v1::reusable_lds_v1::import_tests::{name}");
        let output = crate::process_execution::capture_output(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &test,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .env(CHILD, &key)
                .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, &observation)
                .env("FE2O3_CORE_TRY_DEVICE_RMETA", &device)
                .env("FE2O3_CORE_TRY_HOST_DEPS", &host)
                .env("FE2O3_CORE_TRY_AMDGPU_CORE", &core)
                .env("FE2O3_CORE_TRY_AMDGPU_BUILTINS", &builtins)
                .env("CARGO_MANIFEST_DIR", scratch.path())
                .env("CARGO_PKG_NAME", "reusable-lds-import-source")
                .env(reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1, &binding)
                .env("FE2O3_SIMULATION_MODE_V1", "1")
                .env(
                    "FE2O3_SIMULATION_ATTEMPT_V1",
                    "92929292929292929292929292929292",
                ),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "matrix import child:\n{}\n{}",
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
    assert_eq!(
        std::env::var(reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1).unwrap(),
        binding
    );
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        format!("--crate-name={CRATE_NAME}"),
        format!("--out-dir={}", std::env::var("CARGO_MANIFEST_DIR").unwrap()),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "--target=amdgcn-amd-amdhsa".into(),
        format!("-Ctarget-cpu={cpu}"),
        "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
        "-Zno-codegen".into(),
        "-Zalways-encode-mir".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Zunstable-options".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-Cdebuginfo=2".into(),
        format!("-Cmetadata={METADATA}"),
        "--extern".into(),
        format!("fe2o3_device={}", device.display()),
        "-L".into(),
        format!("dependency={}", device.parent().unwrap().display()),
        "-L".into(),
        format!("dependency={}", host.display()),
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
        cpu,
        check_kir,
        completed: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(
        probe.completed,
        "canonical reusable LDS import and substitutions must complete"
    );
}

#[test]
#[ignore = "requires cached authenticated AMD metadata; no Cargo"]
fn reusable_lds_conversion_full_import_gfx942() {
    run("gfx942", "reusable_lds_conversion_full_import_gfx942");
}
#[test]
#[ignore = "requires cached authenticated AMD metadata; no Cargo"]
fn reusable_lds_conversion_full_import_gfx950() {
    run("gfx950", "reusable_lds_conversion_full_import_gfx950");
}
