//! Actual gfx942 source collection and canonical import. Mounted under the
//! importer, independently of the small arithmetic-authenticator test runner.

use crate::collector::collect_authenticated_kernel_closure_v1;
use crate::production_semantic_terminal_v1::{
    ProductionSemanticTerminalRuleV1, ProductionTerminalExpansionV1,
};
use crate::production_target_v1::RetainedProductionTargetV1;
use crate::rustc_semantic_adapter_v1::{
    canonical_function_identities_v1, rustc_block_identity_v1, rustc_mir_body_sha256_v1,
};
use crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2;
use crate::trusted_device_items::{
    authenticate_reviewed_safe_core_arithmetic_helper_v1,
    authenticate_reviewed_safe_core_checked_div_v1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticBasicBlockV1, SemanticBinaryOpV1, SemanticCallableDeclV1,
    SemanticCompilerIntrinsicOperationV1, SemanticFunctionDeclV1, SemanticMirLimitsV1,
    SemanticRvalueKindV1, SemanticStatementKindV1, SemanticTerminatorKindV1,
};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::{
    mir::{BinOp, Operand, Rvalue, StatementKind, TerminatorKind},
    ty::{Instance, TyCtxt, TyKind, TypingEnv},
};
use rustc_session::config::Input;
use rustc_span::FileName;

#[path = "import_tests/harness.rs"]
mod harness;

#[derive(Clone, Copy, Debug)]
enum Case {
    Import,
    UnsafeSource,
    PanicSource,
}

impl Case {
    fn source(self) -> String {
        let extra = match self {
            Self::Import => "",
            Self::UnsafeSource => "let _untrusted = unsafe { rhs };",
            Self::PanicSource => "if rhs == 7 { panic!(\"reachable after checked_div\"); }",
        };
        format!(
            r#"#![no_std]
#![allow(unused_unsafe)]
use fe2o3_device::{{KernelContext, kernel}};
pub fn checked_div(lhs: usize, rhs: usize) -> Option<usize> {{
    let quotient = lhs.checked_div(rhs);
    {extra}
    quotient
}}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn checked_div_import_source(context: KernelContext<'_>, lhs: usize, rhs: usize) {{
    let _ = context;
    let _quotient = checked_div(lhs, rhs);
}}
"#
        )
    }

    fn test_name(self) -> &'static str {
        match self {
            Self::Import => "checked_div_actual_amdgpu_full_import_retains_complete_core_closure",
            Self::UnsafeSource => "checked_div_actual_amdgpu_collection_rejects_unsafe_source",
            Self::PanicSource => "checked_div_actual_amdgpu_collection_rejects_panicking_source",
        }
    }
}

fn callee<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> Instance<'tcx> {
    let Operand::Constant(constant) = operand else {
        panic!("constant callee")
    };
    let TyKind::FnDef(definition, args) = constant.const_.ty().kind() else {
        panic!("exact direct call");
    };
    Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *definition, args)
        .unwrap()
        .expect("resolved direct call")
}

fn retained_block(
    function: &SemanticFunctionDeclV1,
    body_hash: [u8; 32],
    raw_block: usize,
) -> &SemanticBasicBlockV1 {
    // Preflight sorts blocks by identity; a raw MIR index is not a semantic ID.
    let identity = rustc_block_identity_v1(
        function.identity(),
        body_hash,
        u32::try_from(raw_block).unwrap(),
    );
    function
        .blocks()
        .iter()
        .find(|block| block.identity() == identity)
        .expect("retained block bound to exact source function, body and raw index")
}

struct Probe {
    case: Case,
    completed: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("checked_div_import_source.rs".into()),
            input: self.case.source(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert_eq!(tcx.sess.target.llvm_target.as_ref(), "amdgcn-amd-amdhsa");
        assert_eq!(tcx.data_layout.pointer_size().bits(), 64);
        let local = Instance::mono(
            tcx,
            tcx.iter_local_def_id()
                .find(|id| {
                    tcx.def_kind(*id) == DefKind::Fn
                        && tcx.item_name(id.to_def_id()).as_str() == "checked_div"
                })
                .expect("local source helper")
                .to_def_id(),
        );
        assert!(!authenticate_reviewed_safe_core_checked_div_v1(tcx, local));
        let core = tcx
            .instance_mir(local.def)
            .basic_blocks
            .iter()
            .find_map(|block| {
                let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                    return None;
                };
                let instance = callee(tcx, func);
                authenticate_reviewed_safe_core_checked_div_v1(tcx, instance).then_some(instance)
            })
            .expect("all cases actually call authenticated usize::checked_div");
        let body = tcx.instance_mir(core.def);
        assert_eq!(body.arg_count, 2);
        assert!(
            body.args_iter()
                .all(|arg| body.local_decls[arg].ty == tcx.types.usize)
        );
        let selected = crate::production_rustc_intrinsic_v1::production_mir_v1(tcx, core);
        assert!(!selected.is_source_expansion());
        assert!(std::ptr::eq(selected.body(), body));

        let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        let collected =
            collect_authenticated_kernel_closure_v1(tcx, partitions.codegen_units, false, target);
        let closure = match self.case {
            Case::UnsafeSource | Case::PanicSource => {
                let error = match collected {
                    Ok(_) => panic!("source authentication must not bypass {:?}", self.case),
                    Err(error) => error.to_string(),
                };
                match self.case {
                    Case::UnsafeSource => {
                        assert!(error.contains("[FE2O3-CAP-SOURCE006]"), "{error}");
                        assert!(error.contains("user-provided unsafe block"), "{error}");
                    }
                    Case::PanicSource => assert!(error.contains("panic path"), "{error}"),
                    Case::Import => unreachable!(),
                }
                assert!(error.contains("checked_div"), "{error}");
                self.completed = true;
                return Compilation::Stop;
            }
            Case::Import => {
                collected.expect("production source-safety hook must admit checked_div")
            }
        };
        let retained = closure
            .exact_codegen_function_symbols_v1()
            .map(|(instance, _, _)| instance)
            .collect::<Vec<_>>();
        assert!(retained.contains(&local));

        // Walk the actual bounded core closure, not a hand-constructed call
        // inventory. Only the already modeled cold_path intrinsic is terminal.
        let mut graph = vec![core];
        let mut edges = Vec::new();
        let mut hashes = Vec::new();
        let mut next = 0;
        while next < graph.len() {
            assert!(graph.len() <= 8, "closed arithmetic helper budget");
            let caller = graph[next];
            next += 1;
            assert_eq!(retained.iter().filter(|&&item| item == caller).count(), 1);
            let body = tcx.instance_mir(caller.def);
            let selected = crate::production_rustc_intrinsic_v1::production_mir_v1(tcx, caller);
            assert!(!selected.is_source_expansion());
            assert!(std::ptr::eq(selected.body(), body));
            hashes.push((caller, rustc_mir_body_sha256_v1(tcx, caller)));
            for (index, block) in body.basic_blocks.iter_enumerated() {
                let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                    continue;
                };
                let target = callee(tcx, func);
                let cold = matches!(
                    crate::production_semantic_terminal_v1::classify(tcx, target.def_id()),
                    Some(ProductionSemanticTerminalRuleV1::Expand(
                        ProductionTerminalExpansionV1::ColdPath
                    ))
                );
                edges.push((caller, index.as_usize(), target, cold));
                if !cold {
                    assert!(authenticate_reviewed_safe_core_arithmetic_helper_v1(
                        tcx, target
                    ));
                    if !graph.contains(&target) {
                        graph.push(target);
                    }
                }
            }
        }
        assert_eq!(
            graph.len(),
            2,
            "actual AMD core retains checked_div and unlikely"
        );
        assert_eq!(edges.len(), 2, "division -> unlikely -> cold_path");
        assert_eq!(edges.iter().filter(|edge| edge.3).count(), 1);

        let imported = crate::collector::construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .expect("checked_div source must reach actual complete canonical import");
        let mir = &imported.semantic_mir;
        mir.require_complete_external_entries().unwrap();
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .expect("canonical roundtrip retains source proof and complete call closure");
        assert_eq!(decoded.functions(), mir.functions());
        assert_eq!(decoded.callables(), mir.callables());
        for (instance, hash) in hashes {
            assert_eq!(rustc_mir_body_sha256_v1(tcx, instance), hash);
            let identity = canonical_function_identities_v1(tcx, instance).function();
            let index = mir
                .functions()
                .iter()
                .position(|f| f.identity() == identity)
                .unwrap();
            assert_eq!(
                mir.functions()[index].blocks().len(),
                tcx.instance_mir(instance.def).basic_blocks.len()
            );
            assert_eq!(mir.callables().iter().filter(|callable| matches!(callable,
                SemanticCallableDeclV1::Defined { function } if function.index() as usize == index
            )).count(), 1, "core helper stays a defined body, not a summary");
            assert!(!mir.callables().iter().any(|callable| {
                callable
                    .binding()
                    .is_some_and(|binding| binding.identity() == identity)
            }));
        }
        for (caller, block, target, cold) in edges {
            let caller_identity = canonical_function_identities_v1(tcx, caller).function();
            let target_identity = canonical_function_identities_v1(tcx, target).function();
            let function = mir
                .functions()
                .iter()
                .find(|f| f.identity() == caller_identity)
                .unwrap();
            let body_hash = rustc_mir_body_sha256_v1(tcx, caller);
            let source_block = &tcx.instance_mir(caller.def).basic_blocks
                [rustc_middle::mir::BasicBlock::from_usize(block)];
            let semantic_block = retained_block(function, body_hash, block);
            let SemanticTerminatorKindV1::Call(call) = semantic_block.terminator().kind() else {
                panic!("retain the original core call terminator at {caller:?} bb{block}");
            };
            let TerminatorKind::Call {
                args,
                target: return_block,
                ..
            } = &source_block.terminator().kind
            else {
                unreachable!("edge inventory comes from original calls");
            };
            assert_eq!(call.arguments().len(), args.len());
            let return_block =
                retained_block(function, body_hash, return_block.unwrap().as_usize());
            assert_eq!(
                function.blocks()[call.destination().unwrap().edge().target().index() as usize]
                    .identity(),
                return_block.identity(),
                "retain the exact call return edge after canonical block permutation"
            );
            match &mir.callables()[call.callee().index() as usize] {
                SemanticCallableDeclV1::Defined { function } if !cold => {
                    assert_eq!(
                        mir.functions()[function.index() as usize].identity(),
                        target_identity
                    );
                }
                SemanticCallableDeclV1::CompilerIntrinsic {
                    binding, operation, ..
                } if cold => {
                    assert_eq!(binding.identity(), target_identity);
                    assert_eq!(*operation, SemanticCompilerIntrinsicOperationV1::ColdPath);
                }
                other => panic!("original edge changed identity or terminal kind: {other:?}"),
            }
        }
        let function = mir
            .functions()
            .iter()
            .find(|f| f.identity() == canonical_function_identities_v1(tcx, core).function())
            .unwrap();
        let body_hash = rustc_mir_body_sha256_v1(tcx, core);
        let mut rustc_ops = Vec::new();
        let mut semantic_ops = Vec::new();
        for (raw_block, block) in body.basic_blocks.iter_enumerated() {
            let semantic = retained_block(function, body_hash, raw_block.as_usize());
            assert_eq!(semantic.statements().len(), block.statements.len());
            for (source, imported) in block.statements.iter().zip(semantic.statements()) {
                let raw_operation = match &source.kind {
                    StatementKind::Assign(assignment) => match &assignment.1 {
                        Rvalue::BinaryOp(operation, _) => Some(*operation),
                        _ => None,
                    },
                    _ => None,
                };
                let semantic_operation = match imported.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        match assignment.value().kind() {
                            SemanticRvalueKindV1::Binary { operation, .. } => Some(*operation),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                let expected = raw_operation.map(|operation| match operation {
                    BinOp::Eq => SemanticBinaryOpV1::Equal,
                    BinOp::Div => SemanticBinaryOpV1::Divide,
                    other => panic!("unexpected checked_div arithmetic: {other:?}"),
                });
                assert_eq!(
                    semantic_operation, expected,
                    "retain the exact arithmetic source site"
                );
                rustc_ops.extend(raw_operation);
                semantic_ops.extend(semantic_operation);
            }
        }
        assert_eq!(rustc_ops, [BinOp::Eq, BinOp::Div]);
        assert_eq!(
            semantic_ops,
            [SemanticBinaryOpV1::Equal, SemanticBinaryOpV1::Divide]
        );
        self.completed = true;
        Compilation::Stop
    }
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_DEVICE_RMETA, HOST_DEPS, AMDGPU_CORE and AMDGPU_BUILTINS; no Cargo"]
fn checked_div_actual_amdgpu_full_import_retains_complete_core_closure() {
    harness::run(Case::Import);
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_DEVICE_RMETA, HOST_DEPS, AMDGPU_CORE and AMDGPU_BUILTINS; no Cargo"]
fn checked_div_actual_amdgpu_collection_rejects_unsafe_source() {
    harness::run(Case::UnsafeSource);
}

#[test]
#[ignore = "requires cached FE2O3_CORE_TRY_DEVICE_RMETA, HOST_DEPS, AMDGPU_CORE and AMDGPU_BUILTINS; no Cargo"]
fn checked_div_actual_amdgpu_collection_rejects_panicking_source() {
    harness::run(Case::PanicSource);
}
