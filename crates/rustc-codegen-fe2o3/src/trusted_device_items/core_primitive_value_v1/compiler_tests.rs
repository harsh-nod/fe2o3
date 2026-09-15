//! Mount under `collector::production_importer_v1`, like the wrapping fixture.
//! Exercises collection, retained preflight, canonical import and decode replay.
//! Opt in to the ignored kernel test with existing pinned AMDGPU metadata in
//! FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS. No sysroot build
//! or host kernel ABI substitution is performed. Collection negatives need no
//! AMDGPU metadata and remain in the default suite.

use super::*;
use crate::collector::{DeviceCollector, KernelRoot};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCastKindV1, SemanticLayoutIdentityV1, SemanticRvalueKindV1, SemanticStatementKindV1,
    SemanticTerminatorKindV1,
};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{CastKind, Operand, Rvalue, StatementKind, TerminatorKind};
use rustc_session::config::Input;
use rustc_span::FileName;

const SOURCE: &str = r#"
#![no_std]
pub fn from_u32(a: u32, out: &mut u64) { *out = u64::from(a); }
pub fn from_u8(a: u8, out: &mut f32) { *out = f32::from(a); }
pub fn wrong_width(a: u16, out: &mut f32) { *out = f32::from(a); }
pub fn panic_route(a: u32, out: &mut u64) { *out = u64::from(a); panic!("reachable panic"); }
unsafe fn unsafe_leaf(a: u32) -> u64 { u64::from(a) }
pub fn unsafe_route(a: u32, out: &mut u64) { *out = unsafe { unsafe_leaf(a) }; }
"#;

struct Probe {
    negative: bool,
    ran: bool,
}

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
                [31; 32],
            )),
            frontend_contract: None,
            kernel_context_contract: None,
            reference_effect_binding: None,
        })
        .unwrap();
    collector.collect()
}

fn import_cast<'tcx>(
    tcx: TyCtxt<'tcx>,
    name: &str,
    input: Ty<'tcx>,
    output: Ty<'tcx>,
    cast: CastKind,
    semantic_cast: SemanticCastKindV1,
) -> Vec<u8> {
    assert_eq!(tcx.sess.target.llvm_target.as_ref(), "amdgcn-amd-amdhsa");
    let collection =
        collect(tcx, name).expect("proved cast must precede impossible panic recursion");
    assert_eq!(
        collection.functions.len(),
        2,
        "retain caller and original From instance"
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
    let role = |role| {
        SemanticFunctionIdV1::from_index(
            functions
                .iter()
                .position(|function| function.role == role)
                .unwrap() as u32,
        )
    };
    let root = role(CollectedFunctionRole::KernelEntry);
    let helper = role(CollectedFunctionRole::InternalHelper);
    let instance = functions[helper.index() as usize].instance;
    let identities = canonical_function_identities_v1(tcx, instance);
    let source = tcx.instance_mir(instance.def);
    let original_mir = crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, instance);
    let target =
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([33; 32]));
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
    .expect("collection and both preflight passes must use the same proved cast");
    assert_eq!(plan.direct_call_producers().len(), 1);
    assert!(
        plan.terminal_producers().is_empty(),
        "From is not a terminal"
    );
    assert!(plan.normalized_intrinsic_producers().is_empty());
    assert_eq!(
        plan.function_producers()[helper.index() as usize].identities,
        identities
    );
    let selected = plan.function_mir(helper).unwrap();
    assert_eq!(selected.source.instance, instance.def);
    assert!(selected.source.promoted.is_none());
    assert!(!std::ptr::eq(selected, source));
    assert!(std::ptr::eq(selected, plan.function_mir(helper).unwrap()));
    assert_eq!(selected.local_decls.len(), 2);
    assert_eq!(selected.basic_blocks.len(), 1);
    let block = &selected.basic_blocks[rustc_middle::mir::START_BLOCK];
    let [statement] = block.statements.as_slice() else {
        panic!("one cast statement");
    };
    assert!(matches!(&statement.kind, StatementKind::Assign(assignment)
        if matches!(&assignment.1, Rvalue::Cast(kind, Operand::Copy(place), ty)
            if *kind == cast && place.local.as_usize() == 1
                && place.projection.is_empty() && *ty == output)));
    assert!(matches!(block.terminator().kind, TerminatorKind::Return));
    for local in &plan.body_producers()[helper.index() as usize].locals {
        assert_ne!(
            local.identity,
            crate::rustc_semantic_adapter_v1::rustc_local_identity_v1(
                identities.function(),
                original_mir,
                local.rustc_local,
            ),
            "expanded local identity must include its source proof"
        );
    }
    let abi = &plan.function_abi_producers()[helper.index() as usize];
    assert_eq!(&*abi.source_inputs, &[input]);
    assert_eq!(abi.source_output, output);
    assert_eq!(abi.extern_abi, rustc_abi::ExternAbi::Rust);
    assert_eq!(
        plan.function_abi_producers()[root.index() as usize].extern_abi,
        rustc_abi::ExternAbi::GpuKernel,
        "the retained root must exercise the actual GPU kernel ABI"
    );
    let types = construct_production_semantic_types_v1(tcx, plan.type_producers()).unwrap();
    let abis = construct_production_semantic_fn_abis_v1(
        tcx,
        plan.function_abi_producers(),
        plan.type_producers(),
    )
    .unwrap();
    let terminals =
        construct_production_semantic_fn_abis_v1(tcx, &[], plan.type_producers()).unwrap();
    // This fixture has no capability terminal or authenticated context request.
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
    let semantic = construct_complete_request_v1(
        tcx,
        target,
        &plan,
        &contexts,
        types.into_records(),
        abis,
        terminals,
    )
    .expect("production canonical import of the retained cast");
    let function = &semantic.functions()[helper.index() as usize];
    assert_eq!(function.identity(), identities.function());
    assert_eq!(
        function.item_definition_identity(),
        identities.item_definition()
    );
    assert_eq!(function.abi().identity(), abi.identity);
    assert_eq!(function.blocks().len(), 1);
    let block = &function.blocks()[0];
    assert!(matches!(
        block.terminator().kind(),
        SemanticTerminatorKindV1::Return
    ));
    let [statement] = block.statements() else {
        panic!("one semantic cast statement");
    };
    assert!(
        matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
        if matches!(assignment.value().kind(), SemanticRvalueKindV1::Cast { kind, .. }
            if *kind == semantic_cast))
    );
    assert_eq!(
        original_mir,
        crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, instance)
    );
    assert_eq!(identities, canonical_function_identities_v1(tcx, instance));
    let replay = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        semantic.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .expect("canonical decode replay");
    assert_eq!(replay.semantic_sha256(), semantic.semantic_sha256());
    assert_eq!(replay.canonical_encoding(), semantic.canonical_encoding());
    semantic.canonical_encoding().to_vec()
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("primitive_cast_production.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if self.negative {
            for (name, diagnostic) in [("panic_route", "panic path"), ("unsafe_route", "unsafe")] {
                let error =
                    collect(tcx, name).expect_err("real panic/unsafe paths remain rejected");
                assert!(error.to_string().contains(diagnostic), "{name}: {error}");
            }
            assert!(
                collect(tcx, "wrong_width").is_err(),
                "unreviewed conversion must recurse"
            );
        } else {
            for (name, input, output, cast, semantic) in [
                (
                    "from_u32",
                    tcx.types.u32,
                    tcx.types.u64,
                    CastKind::IntToInt,
                    SemanticCastKindV1::Integer,
                ),
                (
                    "from_u8",
                    tcx.types.u8,
                    tcx.types.f32,
                    CastKind::IntToFloat,
                    SemanticCastKindV1::Float,
                ),
            ] {
                assert_eq!(
                    import_cast(tcx, name, input, output, cast, semantic),
                    import_cast(tcx, name, input, output, cast, semantic),
                    "recollection and import must preserve exact canonical bytes: {name}",
                );
            }
        }
        self.ran = true;
        Compilation::Stop
    }
}

fn run(negative: bool) {
    let sysroot = crate::process_execution::capture_output(
        std::process::Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let mut args = vec![
        "rustc".into(),
        "--crate-name=primitive_cast_production".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
    ];
    if !negative {
        let metadata = |name| {
            let path = std::path::PathBuf::from(std::env::var_os(name).unwrap_or_else(|| {
                panic!("set {name} to existing pinned AMDGPU metadata; this kernel test cannot use a host TyCtxt")
            }));
            assert!(path.is_file(), "{name}: {}", path.display());
            path
        };
        let core = metadata("FE2O3_WRAPPING_AMDGPU_CORE");
        let builtins = metadata("FE2O3_WRAPPING_AMDGPU_BUILTINS");
        args.extend([
            "--target=amdgcn-amd-amdhsa".into(),
            "-Ctarget-cpu=gfx942".into(),
            "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
            "-Cpanic=abort".into(),
            "-Zalways-encode-mir".into(),
            "-Zunstable-options".into(),
            "--extern".into(),
            format!("noprelude,nounused:core={}", core.display()),
            "--extern".into(),
            format!(
                "noprelude,nounused:compiler_builtins={}",
                builtins.display()
            ),
            "-L".into(),
            format!("dependency={}", core.parent().unwrap().display()),
        ]);
        if builtins.parent() != core.parent() {
            args.extend([
                "-L".into(),
                format!("dependency={}", builtins.parent().unwrap().display()),
            ]);
        }
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
#[ignore = "requires pinned AMDGPU metadata: FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS; run explicitly with --ignored"]
fn core_primitive_cast_collection_to_canonical_import_and_replay() {
    run(false);
}

#[test]
fn core_primitive_cast_collection_rejects_unproved_panic_and_unsafe_helpers() {
    run(true);
}
