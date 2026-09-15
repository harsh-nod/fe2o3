//! Assertions in the original gfx942/gfx950 two-phase callbacks.
use fe2o3_kernel_ir::{
    ExecutionCapabilityOperationV1 as Exec, ExecutionCapabilitySourceOccurrenceV1,
    ExecutionCapabilitySourceV1, ExecutionMemoryOrderingV1, ExecutionMemoryScopeV1,
    ExecutionMemorySpacesV1, Module, OperationKind, ReusablePhaseOperationV1 as Phase,
};
use fe2o3_lower_mir_kernel::SemanticKirCorrespondenceV1;
use fe2o3_mir_model::SemanticExpandedTerminatorOriginV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
    SemanticExecutionCapabilityOperationV1, SemanticFunctionIdV1, SemanticTerminatorKindV1,
};
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;

pub(super) struct ExpectedBarrier {
    root: SemanticFunctionIdV1,
    source: ExecutionCapabilitySourceV1,
    expanded_block: u32,
    normal_target: u32,
}

pub(super) fn collect(owner: &ProductionSemanticSsaOwnerV1) -> Vec<ExpectedBarrier> {
    owner.verify_replay().unwrap();
    let semantic = owner.source_semantic();
    let [root] = semantic.roots() else {
        panic!("one original phase root");
    };
    let view = owner.execution_view_for_root(*root).unwrap();
    assert!(view.has_expanded_calls());
    let mut expected = Vec::new();
    for (block_index, block) in view.body().blocks().iter().enumerate() {
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let callable = &semantic.callables()[call.callee().index() as usize];
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        } = callable
        else {
            continue;
        };
        if !matches!(
            contract.operation(),
            SemanticExecutionCapabilityOperationV1::WorkgroupBarrier { .. }
        ) {
            continue;
        }
        assert!(
            fe2o3_lower_mir_kernel::project_execution_workgroup_barrier_v1(callable, call)
                .unwrap()
                .is_some()
        );
        let origin = &view.block_origins()[block_index];
        assert_eq!(
            origin.terminator(),
            SemanticExpandedTerminatorOriginV1::Source
        );
        let original_function = &semantic.functions()[origin.function().index() as usize];
        let SemanticTerminatorKindV1::Call(original) = original_function.blocks()
            [origin.block().index() as usize]
            .terminator()
            .kind()
        else {
            panic!("original barrier call");
        };
        assert_eq!(original.callee(), call.callee());
        let expanded_block = u32::try_from(block_index).unwrap();
        let occurrence = ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
            *semantic.functions()[root.index() as usize]
                .identity()
                .as_bytes(),
            *owner.execution_expansion().identity(),
            *view.identity(),
            origin.instance().index(),
            expanded_block,
        )
        .unwrap();
        expected.push(ExpectedBarrier {
            root: *root,
            source: ExecutionCapabilitySourceV1 {
                function: *original_function.identity().as_bytes(),
                operation: *binding.identity().as_bytes(),
                block: origin.block().index(),
                occurrence: Some(occurrence),
            },
            expanded_block,
            normal_target: call.destination().unwrap().edge().target().index(),
        });
    }
    assert_eq!(expected.len(), 2, "both original Finish barriers survive");
    assert_ne!(expected[0].source.occurrence, expected[1].source.occurrence);
    expected
}

pub(super) fn check(
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    expected: &[ExpectedBarrier],
) {
    let mut seen = vec![false; expected.len()];
    let mut seals = 0usize;
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        for block in &body.blocks {
            for (operation_index, operation) in block.operations.iter().enumerate() {
                match &operation.kind {
                    OperationKind::ExecutionCapability(capability) => {
                        let Exec::WorkgroupBarrier { semantics, .. } = capability.operation else {
                            continue;
                        };
                        assert_eq!(semantics.scope, ExecutionMemoryScopeV1::Workgroup);
                        assert_eq!(
                            semantics.ordering,
                            ExecutionMemoryOrderingV1::AcquireRelease
                        );
                        assert_eq!(semantics.spaces, ExecutionMemorySpacesV1::Workgroup);
                        let matching = expected
                            .iter()
                            .enumerate()
                            .filter(|(_, original)| original.source == capability.source)
                            .collect::<Vec<_>>();
                        let [(index, original)] = matching.as_slice() else {
                            panic!("barrier has one exact original source occurrence");
                        };
                        assert!(!seen[*index], "no duplicated barrier occurrence");
                        seen[*index] = true;
                        let spans = correspondence
                            .terminator_operation_spans()
                            .iter()
                            .filter(|span| {
                                span.correspondence_owner() == original.root
                                    && span.semantic_block().index() == original.expanded_block
                            })
                            .collect::<Vec<_>>();
                        let [span] = spans.as_slice() else {
                            panic!("one source terminator span");
                        };
                        assert_eq!(span.kernel_ir_block(), block.id);
                        let first = span.first_operation_ordinal() as usize;
                        let end = first.checked_add(span.operation_count() as usize).unwrap();
                        assert!(
                            (first..end).contains(&operation_index),
                            "barrier cannot move outside its original source terminator"
                        );
                    }
                    OperationKind::ReusablePhase(phase) => {
                        let Phase::Seal { barrier_call, .. } = &phase.operation else {
                            continue;
                        };
                        let matching = expected
                            .iter()
                            .filter(|original| original.source == barrier_call.source)
                            .collect::<Vec<_>>();
                        let [original] = matching.as_slice() else {
                            panic!("Seal's original barrier");
                        };
                        assert_eq!(barrier_call.expanded_normal_target, original.normal_target);
                        assert_eq!(block.id.0, original.normal_target);
                        seals += 1;
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(seen.into_iter().all(|value| value));
    assert_eq!(seals, expected.len());
}
