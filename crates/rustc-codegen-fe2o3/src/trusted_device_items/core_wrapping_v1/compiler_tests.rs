//! Pinned rustc collection -> preflight -> semantic producer tests.
//! These exercise compiler construction, not target launch qualification.
//! The positive kernel fixture requires existing AMDGPU core/builtins metadata
//! via FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS. It never
//! builds a sysroot or substitutes a host ABI for the kernel entry.

use super::*;
use crate::collector::{DeviceCollector, KernelRoot};
use crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBinaryOpV1, SemanticLayoutIdentityV1, SemanticRvalueKindV1, SemanticStatementKindV1,
    SemanticTerminatorKindV1,
};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::mir::{BinOp, Rvalue, StatementKind, TerminatorKind};
use rustc_session::config::Input;
use rustc_span::FileName;

const SOURCE: &str = r#"
#![no_std]
pub fn route(a: u32, b: u32, out: &mut u32) { *out = a.wrapping_shr(b); }
pub fn unchecked(a: u32, b: u32, out: &mut u32) { *out = unsafe { a.unchecked_shr(b) }; }
pub fn panic_route() { panic!("reachable panic"); }
pub fn wrong_width(a: u64, b: u32, out: &mut u64) { *out = a.wrapping_shr(b); }
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
    let instance = Instance::mono(tcx, definition.to_def_id());
    let mut collector = DeviceCollector::new(tcx, false, Vec::new(), "gfx942".into());
    collector
        .add_root(KernelRoot {
            target: instance,
            logical_name: name.into(),
            export_name: name.into(),
            generated_host_contract_identity: None,
            kernel_binding: Some(reserved_fe2o3_symbols::KernelBindingIdV1::from_bytes(
                [21; 32],
            )),
            frontend_contract: None,
            kernel_context_contract: None,
            reference_effect_binding: None,
        })
        .unwrap();
    collector.collect()
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("wrapping_shift_production.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        if self.negative {
            for name in ["unchecked", "panic_route", "wrong_width"] {
                let error = collect(tcx, name).expect_err("unproved source must still recurse");
                assert!(error.to_string().contains("panic path"), "{name}: {error}");
            }
        } else {
            assert_eq!(tcx.sess.target.llvm_target.as_ref(), "amdgcn-amd-amdhsa");
            let collection =
                collect(tcx, "route").expect("proved shift must precede panic recursion");
            assert_eq!(
                collection.functions.len(),
                2,
                "retain caller and original wrapping callable"
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
            let shift = SemanticFunctionIdV1::from_index(
                functions
                    .iter()
                    .position(|f| f.role == CollectedFunctionRole::InternalHelper)
                    .unwrap() as u32,
            );
            let original = functions[shift.index() as usize].instance;
            let identities = canonical_function_identities_v1(tcx, original);
            assert!(
                tcx.instance_mir(original.def)
                    .basic_blocks
                    .iter()
                    .any(|block| matches!(block.terminator().kind, TerminatorKind::Call { .. }))
            );
            let target =
                SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([23; 32]));
            let (inventory, _) =
                identity_inventory_identity_and_transcript_v1(target, &functions, &[root]);
            let plan = build_production_semantic_preflight_plan_v1(
                tcx,
                target,
                functions.into_boxed_slice(),
                vec![root].into_boxed_slice(),
                inventory,
                &BTreeSet::new(),
                DebugSourceCaptureRequestV2::Disabled,
            )
            .expect("the exact expansion must pass both preflight passes");
            assert_eq!(plan.direct_call_producers().len(), 1);
            assert!(plan.terminal_producers().is_empty());
            assert!(plan.normalized_intrinsic_producers().is_empty());
            assert_eq!(
                plan.function_producers()[shift.index() as usize].identities,
                identities
            );
            let selected = plan.function_mir(shift).unwrap();
            assert_eq!(selected.source.instance, original.def);
            assert!(!std::ptr::eq(selected, tcx.instance_mir(original.def)));
            assert!(std::ptr::eq(selected, plan.function_mir(shift).unwrap()));
            let original_mir =
                crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, original);
            for local in &plan.body_producers()[shift.index() as usize].locals {
                assert_ne!(
                    local.identity,
                    crate::rustc_semantic_adapter_v1::rustc_local_identity_v1(
                        identities.function(),
                        original_mir,
                        local.rustc_local,
                    ),
                    "expansion proof must be bound into the preflight node identities",
                );
            }
            assert_eq!(selected.basic_blocks.len(), 1);
            let ops = selected
                .basic_blocks
                .iter()
                .flat_map(|block| &block.statements)
                .map(|statement| {
                    let StatementKind::Assign(assignment) = &statement.kind else {
                        panic!("assignment")
                    };
                    let Rvalue::BinaryOp(op, _) = &assignment.1 else {
                        panic!("binary")
                    };
                    *op
                })
                .collect::<Vec<_>>();
            assert_eq!(ops, [BinOp::BitAnd, BinOp::Shr]);
            let abi = &plan.function_abi_producers()[shift.index() as usize];
            assert_eq!(&*abi.source_inputs, &[tcx.types.u32, tcx.types.u32]);
            assert_eq!(abi.source_output, tcx.types.u32);
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
            // No capability terminal exists in this fixture; empty custody is
            // inert and grants no context or target authority.
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
            .expect("the production importer must consume the retained expanded MIR");
            let function = &semantic.functions()[shift.index() as usize];
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
            let ops = block
                .statements()
                .iter()
                .map(|statement| {
                    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                        panic!("semantic assignment")
                    };
                    let SemanticRvalueKindV1::Binary { operation, .. } = assignment.value().kind()
                    else {
                        panic!("ordinary binary, not unchecked")
                    };
                    *operation
                })
                .collect::<Vec<_>>();
            assert_eq!(
                ops,
                [SemanticBinaryOpV1::BitAnd, SemanticBinaryOpV1::ShiftRight]
            );
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
        "--crate-name=wrapping_shift_production".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "-Zno-codegen".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
    ];
    if let Some(level) = mir_opt {
        assert_eq!(level, 0);
        args.push(format!("-Zmir-opt-level={level}"));
    }
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
    eprintln!("wrapping compiler fixture arguments: {args:?}");
    let mut probe = Probe {
        negative,
        ran: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.ran);
}

#[test]
#[ignore = "requires cached pinned AMDGPU metadata: FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS"]
fn core_wrapping_shr_collection_to_semantic_producer() {
    run(false, None);
}

#[test]
#[ignore = "requires cached pinned AMDGPU metadata: FE2O3_WRAPPING_AMDGPU_CORE and FE2O3_WRAPPING_AMDGPU_BUILTINS"]
fn core_wrapping_shr_collection_to_semantic_producer_mir_opt_zero() {
    run(false, Some(0));
}

#[test]
fn core_wrapping_shr_collection_rejects_unproved_panic_and_unsafe_helpers() {
    run(true, None);
}
