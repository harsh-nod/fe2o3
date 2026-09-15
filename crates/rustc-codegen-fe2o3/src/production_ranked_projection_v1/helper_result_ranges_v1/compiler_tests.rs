//! Cached AMD core through collection, canonical import and replayed direct-call
//! expansion. Uses the producer fixture root, not physical launch authority.

use super::*;
use crate::collector::{DeviceCollector, KernelRoot};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssertMessageV1, SemanticBinaryOpV1, SemanticLayoutIdentityV1, SemanticRvalueKindV1,
    SemanticStatementKindV1, SemanticTerminatorKindV1,
};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;

#[path = "array_compiler_tests.rs"]
mod array_compiler_tests;

// Same bounds and operations as advanced_attention::batch_count_for_launch_v1.
// The source guard, core checked_mul and every return remain in the closure.
fn source(guard: bool, arrays: bool) -> String {
    if arrays {
        return array_compiler_tests::source(guard);
    }
    let upper = if guard { "|| grid_x > 4" } else { "" };
    format!(
        r#"#![no_std]
pub const fn batch_count_for_launch_v1(grid_x: u32, invocations_per_item: u32) -> Option<usize> {{
    if grid_x == 0 {upper} || invocations_per_item == 0 || invocations_per_item > 256
        || 256 % invocations_per_item != 0 {{ return None; }}
    (grid_x as usize).checked_mul((256 / invocations_per_item) as usize)
}}
pub fn helper_range_root(grid_x: u32, out: &mut usize) {{
    let Some(batches) = batch_count_for_launch_v1(grid_x, 256) else {{ return; }};
    *out = batches * 256;
}}
"#
    )
}

fn produce(tcx: TyCtxt<'_>, guard: bool, arrays: bool) {
    let definition = tcx
        .iter_local_def_id()
        .find(|id| {
            tcx.def_kind(*id) == DefKind::Fn
                && tcx.item_name(id.to_def_id()).as_str() == "helper_range_root"
        })
        .unwrap();
    let instance = Instance::mono(tcx, definition.to_def_id());
    let source_hash = crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, instance);
    let mut collector = DeviceCollector::new(tcx, false, Vec::new(), "gfx950".into());
    collector
        .add_root(KernelRoot {
            target: instance,
            logical_name: "helper_range_root".into(),
            export_name: "helper_range_root".into(),
            generated_host_contract_identity: None,
            kernel_binding: Some(reserved_fe2o3_symbols::KernelBindingIdV1::from_bytes(
                [63; 32],
            )),
            frontend_contract: None,
            kernel_context_contract: None,
            reference_effect_binding: None,
        })
        .unwrap();
    let collection = collector
        .collect()
        .expect("source helper and complete safe core arithmetic closure");
    assert!(collection.functions.len() >= 4);
    assert!(collection.functions.iter().any(|function| {
        let definition = function.instance.def_id();
        !definition.is_local() && tcx.item_name(definition).as_str() == "checked_mul"
    }));
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
            .position(|function| function.role == CollectedFunctionRole::KernelEntry)
            .unwrap() as u32,
    );
    let target =
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([64; 32]));
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
    .expect("complete retained checked multiplication plan");
    let types = construct_production_semantic_types_v1(tcx, plan.type_producers()).unwrap();
    let abis = construct_production_semantic_fn_abis_v1(
        tcx,
        plan.function_abi_producers(),
        plan.type_producers(),
    )
    .unwrap();
    let terminal_abis = plan
        .terminal_producers()
        .iter()
        .map(|terminal| terminal.abi.clone())
        .collect::<Vec<_>>();
    let terminals =
        construct_production_semantic_fn_abis_v1(tcx, &terminal_abis, plan.type_producers())
            .unwrap();
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
    .expect("actual canonical helper/core import, not a fabricated result summary");
    let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        mir.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(decoded.functions(), mir.functions());
    let expansion = fe2o3_mir_model::SemanticCallExpansionV1::try_new(
        &decoded,
        fe2o3_mir_model::SemanticCallExpansionLimitsV1::default(),
    )
    .unwrap();
    expansion.verify_replay(&decoded).unwrap();
    let view = expansion.root(root).unwrap();
    assert!(view.has_expanded_calls());
    assert!(
        view.block_origins()
            .iter()
            .any(|block| block.statements().iter().any(|origin| matches!(
                origin,
                fe2o3_mir_model::SemanticExpandedStatementOriginV1::ReturnTransfer { .. }
            )))
    );
    let function = view.body();
    let (proofs, ranges) =
        crate::production_ranked_projection_v1::helper_result_assertions_for_test_v1(
            decoded.types(),
            function,
        );
    if arrays {
        array_compiler_tests::verify(function, guard, &proofs, &ranges);
        assert_eq!(
            crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, instance),
            source_hash,
        );
        return;
    }
    let mut tested = 0;
    for (index, block) in function.blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Assert {
            message:
                SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Multiply,
                    right,
                    ..
                },
            ..
        } = block.terminator().kind()
        else {
            continue;
        };
        let fe2o3_mir_model::semantic_mir_v1::SemanticOperandV1::Constant(value) = right else {
            continue;
        };
        if !matches!(value.value(), fe2o3_mir_model::semantic_mir_v1::SemanticConstantValueV1::Scalar(value) if value.bits() == 256)
        {
            continue;
        }
        // Even without the upper guard, u32::MAX * 256 fits a 64-bit usize.
        // The guard negative therefore checks the stronger [1,4] payload fact,
        // not an invalid expectation that this particular product overflows.
        assert!(
            proofs[index],
            "retained checked product at expanded bb{index}: {block:?}"
        );
        let (_, _, minimum, maximum) = ranges
            .iter()
            .find(|(b, _, _, _)| *b == index)
            .expect("exact retained product operand range");
        assert_eq!(*minimum >= 1 && *maximum <= 4, guard);
        assert!(block.statements().iter().any(|statement| matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment) if matches!(assignment.value().kind(), SemanticRvalueKindV1::CheckedBinary(_)))), "original checked product remains present");
        tested += 1;
    }
    assert_eq!(
        tested, 1,
        "exact root batches * 256 assertion must be exercised"
    );
    assert_eq!(
        crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, instance),
        source_hash
    );
}

struct Probe {
    guard: bool,
    arrays: bool,
    ran: bool,
}
impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("helper_result_range_production.rs".into()),
            input: source(self.guard, self.arrays),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(tcx.sess.target.llvm_target.as_ref(), "amdgcn-amd-amdhsa");
        produce(tcx, self.guard, self.arrays);
        self.ran = true;
        Compilation::Stop
    }
}

fn run(guard: bool) {
    run_profile(guard, false);
}

fn run_profile(guard: bool, arrays: bool) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=helper_result_range_production".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "--target=amdgcn-amd-amdhsa".into(),
        "-Ctarget-cpu=gfx950".into(),
        "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
        "-Zno-codegen".into(),
        "-Zalways-encode-mir".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Zunstable-options".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-Cdebuginfo=2".into(),
    ];
    for (variable, name) in [
        ("FE2O3_WRAPPING_AMDGPU_CORE", "core"),
        ("FE2O3_WRAPPING_AMDGPU_BUILTINS", "compiler_builtins"),
    ] {
        let path = std::path::PathBuf::from(std::env::var_os(variable).unwrap_or_else(|| {
            panic!("set {variable} to complete cached AMD metadata; no Cargo or fallback")
        }));
        assert!(path.is_file());
        args.extend([
            "--extern".into(),
            format!("noprelude,nounused:{name}={}", path.display()),
            "-L".into(),
            format!("dependency={}", path.parent().unwrap().display()),
        ]);
    }
    args.push("-".into());
    let mut probe = Probe {
        guard,
        arrays,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; no Cargo"]
fn helper_result_ranges_actual_amdgpu_import_and_ranked_assertion() {
    run(true);
}

#[test]
#[ignore = "requires FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; no Cargo"]
fn helper_result_ranges_actual_amdgpu_import_requires_exact_upper_guard() {
    run(false);
}
