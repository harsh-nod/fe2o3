//! Cached core, collector, proof-bound MIR, complete canonical import and replay.
//! Scalar producer roots do not issue physical launch authority.

use super::*;
use crate::collector::{DeviceCollector, KernelRoot};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;

const SOURCE: &str = r#"
#![no_std]
#![allow(unused_unsafe)]
pub fn promoted_comparison_root(grid: u64, out: &mut bool) {
    let volume = if grid == 0 { None } else { Some(grid) };
    *out = volume != Some(1);
}
pub fn unsafe_comparison(value: Option<u64>, out: &mut bool) {
    unsafe {}
    *out = value != Some(1);
}
pub fn panicking_comparison(value: Option<u64>, out: &mut bool) {
    *out = value != Some(1);
    if *out { panic!("reachable after comparison"); }
}
"#;

fn collect<'tcx>(
    tcx: TyCtxt<'tcx>,
    name: &str,
) -> Result<CollectionResult<'tcx>, crate::collector::CollectError> {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .unwrap();
    let mut collector = DeviceCollector::new(tcx, false, Vec::new(), "gfx942".into());
    collector
        .add_root(KernelRoot {
            target: Instance::mono(tcx, definition.to_def_id()),
            logical_name: name.into(),
            export_name: name.into(),
            generated_host_contract_identity: None,
            kernel_binding: Some(reserved_fe2o3_symbols::KernelBindingIdV1::from_bytes(
                [73; 32],
            )),
            frontend_contract: None,
            kernel_context_contract: None,
            reference_effect_binding: None,
        })
        .unwrap();
    collector.collect()
}

fn produce(tcx: TyCtxt<'_>) {
    let collection = collect(tcx, "promoted_comparison_root").expect("exact safe source closure");
    assert_eq!(
        collection.functions.len(),
        4,
        "root, Option ne, Option eq, primitive eq"
    );
    let mut functions = collection
        .functions
        .iter()
        .map(|function| RetainedSemanticFunctionProducerV1 {
            identities: canonical_function_identities_v1(tcx, function.instance),
            instance: function.instance,
            role: function.role,
            export_name: function
                .is_kernel_entry()
                .then(|| function.export_name.clone()),
            kernel_binding: function.kernel_binding,
            frontend_contract: function.frontend_contract.clone(),
            closure_admission: None,
        })
        .collect::<Vec<_>>();
    functions.sort_unstable_by_key(|function| function.identities.function());
    let root = SemanticFunctionIdV1::from_index(
        functions
            .iter()
            .position(|f| f.role == CollectedFunctionRole::KernelEntry)
            .unwrap() as u32,
    );
    let original = functions[root.index() as usize].instance;
    let original_hash = crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, original);
    let target = SemanticTargetDataLayoutV1::gfx942(
        fe2o3_mir_model::semantic_mir_v1::SemanticLayoutIdentityV1::from_sha256([74; 32]),
    );
    let (inventory, _) = identity_inventory_identity_and_transcript_v1(target, &functions, &[root]);
    let plan = build_production_semantic_preflight_plan_v1(
        tcx,
        target,
        functions.into_boxed_slice(),
        vec![root].into_boxed_slice(),
        inventory,
        &BTreeSet::new(),
        DebugSourceCaptureRequestV2::Disabled,
    )
    .expect("retained promotion proof in both preflight passes");
    assert_eq!(plan.direct_call_producers().len(), 3);
    assert!(plan.terminal_producers().is_empty());
    assert!(plan.normalized_intrinsic_producers().is_empty());
    for (index, function) in plan.function_producers().iter().enumerate() {
        let selected = plan
            .function_mir(SemanticFunctionIdV1::from_index(index as u32))
            .unwrap();
        assert_eq!(
            std::ptr::eq(selected, tcx.instance_mir(function.instance.def)),
            index != root.index() as usize,
            "only local promotion is expanded; every core comparison body is original"
        );
    }
    let selected = plan.function_mir(root).unwrap();
    assert_eq!(
        selected.basic_blocks.len(),
        tcx.instance_mir(original.def).basic_blocks.len()
    );
    for (id, block) in tcx
        .instance_mir(original.def)
        .basic_blocks
        .iter_enumerated()
    {
        assert_eq!(
            format!("{:?}", block.terminator()),
            format!("{:?}", selected.basic_blocks[id].terminator()),
            "same calls, branches, return and unwind edges"
        );
    }
    let types = construct_production_semantic_types_v1(tcx, plan.type_producers()).unwrap();
    let abis = construct_production_semantic_fn_abis_v1(
        tcx,
        plan.function_abi_producers(),
        plan.type_producers(),
    )
    .unwrap();
    let terminals =
        construct_production_semantic_fn_abis_v1(tcx, &[], plan.type_producers()).unwrap();
    let contexts = AuthenticatedProductionKernelContextsV1 {
        frontend_unit_identity: [0; 32],
        target_brand_identity: [0; 32],
        expected_roots: Box::new([]),
        roots: Box::new([]),
        custody_identity: [0; 32],
        transpose_source: None,
        reusable_phase_source: None,
        global_bf16_constructors: None,
    };
    let mir = construct_complete_request_v1(
        tcx,
        target,
        &plan,
        &contexts,
        types.into_records(),
        abis,
        terminals,
    )
    .expect("actual canonical import, no pointer-to-scalar fallback");
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(mir.functions(), decoded.functions());
    assert_eq!(mir.callables(), decoded.callables());
    assert!(mir.allocations().is_empty());
    assert!(mir.statics().is_empty());
    let expansion = fe2o3_mir_model::SemanticCallExpansionV1::try_new(
        &decoded,
        fe2o3_mir_model::SemanticCallExpansionLimitsV1::default(),
    )
    .unwrap();
    expansion.verify_replay(&decoded).unwrap();
    assert!(expansion.root(root).unwrap().has_expanded_calls());
    assert_eq!(
        crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, original),
        original_hash
    );
}

struct Probe {
    negative: bool,
    ran: bool,
}
impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("promoted_comparison_production.rs".into()),
            input: SOURCE.into(),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if self.negative {
            let unsafe_error = collect(tcx, "unsafe_comparison")
                .expect_err("no source-safety authority from local materialization")
                .to_string();
            assert!(
                unsafe_error.contains("[FE2O3-CAP-SOURCE006]"),
                "{unsafe_error}"
            );
            let panic_error = collect(tcx, "panicking_comparison")
                .expect_err("all calls remain reachable")
                .to_string();
            assert!(panic_error.contains("panic path"), "{panic_error}");
        } else {
            produce(tcx);
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(negative: bool, mir_opt: Option<u8>) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=promoted_comparison_production".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "--target=amdgcn-amd-amdhsa".into(),
        "-Ctarget-cpu=gfx942".into(),
        "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
        "-Zunstable-options".into(),
        "-Zno-codegen".into(),
        "-Zalways-encode-mir".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        "-Cdebuginfo=2".into(),
        "-Cpanic=abort".into(),
    ];
    for (variable, name) in [
        ("FE2O3_WRAPPING_AMDGPU_CORE", "core"),
        ("FE2O3_WRAPPING_AMDGPU_BUILTINS", "compiler_builtins"),
    ] {
        let path = std::path::PathBuf::from(
            std::env::var_os(variable).expect("complete opt-in cached metadata; no Cargo/fallback"),
        );
        assert!(path.is_file());
        args.extend([
            "--extern".into(),
            format!("noprelude,nounused:{name}={}", path.display()),
            "-L".into(),
            format!("dependency={}", path.parent().unwrap().display()),
        ]);
    }
    if let Some(level) = mir_opt {
        args.push(format!("-Zmir-opt-level={level}"));
    }
    args.push("-".into());
    let mut probe = Probe {
        negative,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE/BUILTINS; no Cargo"]
fn promoted_option_compare_actual_amdgpu_canonical_import() {
    run(false, None);
    run(false, Some(0));
}
#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE/BUILTINS; no Cargo"]
fn promoted_option_compare_actual_amdgpu_rejects_unsafe_and_panic() {
    run(true, None);
}
