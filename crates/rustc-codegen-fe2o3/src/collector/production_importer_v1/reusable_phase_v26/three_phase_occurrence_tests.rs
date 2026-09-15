//! Expected coordinates come only from the same live source/expansion/SSA owner.
use super::super::super::ssa_protocol::CheckedSsa;
use fe2o3_kernel_ir::{
    ExecutionCapabilitySourceOccurrenceV1, ExecutionCapabilitySourceV1, PhaseCallOccurrenceV1,
    PhaseTerminalCallOccurrenceV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticCallableDeclV1, SemanticDefinedCapabilityContractV1 as Defined,
    SemanticDefinedReusablePhaseRecipeV1 as Recipe, SemanticFunctionIdV1, SemanticTerminatorKindV1,
    SemanticTypeIdV1,
};
use fe2o3_mir_model::{SemanticCallInstanceIdV1, SemanticExpandedTerminatorOriginV1 as Origin};

pub(super) struct ExpectedPhase {
    pub root: SemanticFunctionIdV1,
    pub issue: PhaseCallOccurrenceV1,
    pub source_binding: [u8; 32],
    pub barrier: PhaseTerminalCallOccurrenceV1,
    pub initial_marker: [u8; 32],
    pub advanced_marker: [u8; 32],
    pub workgroup_type: [u8; 32],
}

struct CallCoordinates {
    source: ExecutionCapabilitySourceV1,
    original_normal_target: u32,
    expanded_normal_target: u32,
}

pub(super) fn collect(checked: &CheckedSsa<'_, '_>) -> Vec<ExpectedPhase> {
    let semantic = checked.owner.source_semantic();
    let expansion = &checked.expansion;
    assert_eq!(expansion.phases.len(), 3);
    assert_eq!(checked.phases.len(), 3);
    assert_eq!(
        checked
            .owner
            .execution_plan_for_root(expansion.view.root())
            .unwrap()
            .defined_reusable_phase_relays()
            .len(),
        3
    );
    let identity =
        |ty: SemanticTypeIdV1| *semantic.types()[ty.index() as usize].identity().as_bytes();
    for (index, phase) in checked.phases.iter().enumerate() {
        assert_eq!(phase.leases.len(), 1);
        for previous in &checked.phases[..index] {
            assert_eq!(phase.owner_workgroup, previous.owner_workgroup);
            assert_eq!(phase.converted_owner, previous.converted_owner);
            assert_eq!(phase.leases[0].allocation, previous.leases[0].allocation);
            assert_ne!(phase.owner_reference, previous.owner_reference);
            assert_ne!(phase.issued_phase, previous.issued_phase);
            assert_ne!(phase.closure_phase, previous.closure_phase);
            assert_ne!(phase.leases[0].result, previous.leases[0].result);
            assert_ne!(phase.leases[0].borrowed_at, previous.leases[0].borrowed_at);
            assert_ne!(phase.begin, previous.begin);
            assert_ne!(phase.end, previous.end);
            assert_ne!(phase.relay, previous.relay);
        }
    }
    let mut expected = Vec::new();
    for phase in &expansion.phases {
        let issue = &expansion.bindings[phase.issue];
        let finish = &expansion.bindings[phase.finish];
        let Defined::ReusablePhase(issue_record) = issue.contract() else {
            panic!("Issue recipe");
        };
        let Recipe::Issue {
            brands,
            phase_workgroup,
            ..
        } = issue_record.recipe()
        else {
            panic!("original Issue role");
        };
        let Defined::ReusablePhase(finish_record) = finish.contract() else {
            panic!("Finish recipe");
        };
        let Recipe::Finish {
            input_epoch,
            advanced_epoch,
            barrier_block,
            barrier,
            ..
        } = finish_record.recipe()
        else {
            panic!("original Finish role");
        };
        let (issue_call, issue_origin) =
            observed_call(checked, issue.caller_instance(), issue.call_block());
        assert_eq!(
            issue_call.source.operation,
            *issue_record.source_identity().as_bytes()
        );
        assert!(
            matches!(issue_origin, Origin::CallEntry { callee } if callee == issue.callee_instance())
        );
        let (barrier_call, barrier_origin) =
            observed_call(checked, finish.callee_instance(), barrier_block);
        assert_eq!(barrier_origin, Origin::Source);
        assert_eq!(barrier_call.source.operation, *barrier.identity.as_bytes());
        assert_eq!(identity(input_epoch), identity(brands.dynamic_epoch));
        expected.push(ExpectedPhase {
            root: expansion.view.root(),
            issue: PhaseCallOccurrenceV1 {
                source: issue_call.source,
                callee_instance: issue.callee_instance().index(),
                original_normal_target: issue_call.original_normal_target,
                expanded_normal_target: issue_call.expanded_normal_target,
            },
            source_binding: *issue_record.source_binding(),
            barrier: PhaseTerminalCallOccurrenceV1 {
                source: barrier_call.source,
                original_normal_target: barrier_call.original_normal_target,
                expanded_normal_target: barrier_call.expanded_normal_target,
            },
            initial_marker: identity(brands.dynamic_epoch),
            advanced_marker: identity(advanced_epoch),
            workgroup_type: identity(phase_workgroup),
        });
    }
    for (index, phase) in expected.iter().enumerate() {
        assert_ne!(phase.initial_marker, phase.advanced_marker);
        for previous in &expected[..index] {
            assert_eq!(
                phase.initial_marker, previous.initial_marker,
                "same original Rust epoch marker"
            );
            assert_eq!(phase.advanced_marker, previous.advanced_marker);
            assert_eq!(phase.workgroup_type, previous.workgroup_type);
            assert_eq!(
                phase.issue.source.operation,
                previous.issue.source.operation
            );
            assert_eq!(
                phase.barrier.source.operation,
                previous.barrier.source.operation
            );
            assert_ne!(phase.issue, previous.issue);
            assert_ne!(
                phase.barrier.source.occurrence,
                previous.barrier.source.occurrence
            );
        }
    }
    expected
}

fn observed_call(
    checked: &CheckedSsa<'_, '_>,
    instance: SemanticCallInstanceIdV1,
    original_block: SemanticBlockIdV1,
) -> (CallCoordinates, Origin) {
    let semantic = checked.owner.source_semantic();
    let view = checked.expansion.view;
    let frame = &view.instances()[instance.index() as usize];
    let function = &semantic.functions()[frame.function().index() as usize];
    assert_eq!(frame.function_identity(), function.identity());
    let SemanticTerminatorKindV1::Call(original) = function.blocks()
        [original_block.index() as usize]
        .terminator()
        .kind()
    else {
        panic!("original call terminator");
    };
    let operation = match &semantic.callables()[original.callee().index() as usize] {
        SemanticCallableDeclV1::Defined { function } => {
            semantic.functions()[function.index() as usize].identity()
        }
        SemanticCallableDeclV1::CompilerIntrinsic { binding, .. } => binding.identity(),
        _ => panic!("closed source call"),
    };
    let mapped = |block| {
        let matches = view
            .block_origins()
            .iter()
            .enumerate()
            .filter(|(_, origin)| {
                origin.instance() == instance
                    && origin.block() == block
                    && origin.function() == frame.function()
            })
            .map(|(index, _)| u32::try_from(index).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(matches.len(), 1, "unique original source occurrence");
        matches[0]
    };
    let expanded_block = mapped(original_block);
    let normal = original.destination().unwrap().edge().target();
    let origin = view.block_origins()[expanded_block as usize].terminator();
    if origin == Origin::Source {
        let SemanticTerminatorKindV1::Call(actual) = view.body().blocks()[expanded_block as usize]
            .terminator()
            .kind()
        else {
            panic!("original terminal survives expansion");
        };
        assert_eq!(actual.callee(), original.callee());
        assert_eq!(
            actual.destination().unwrap().edge().target().index(),
            mapped(normal)
        );
        assert!(
            fe2o3_lower_mir_kernel::project_execution_workgroup_barrier_v1(
                &semantic.callables()[actual.callee().index() as usize],
                actual,
            )
            .unwrap()
            .is_some()
        );
    }
    let source = ExecutionCapabilitySourceV1 {
        function: *function.identity().as_bytes(),
        operation: *operation.as_bytes(),
        block: original_block.index(),
        occurrence: Some(
            ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                *semantic.functions()[view.root().index() as usize]
                    .identity()
                    .as_bytes(),
                *checked.owner.execution_expansion().identity(),
                *view.identity(),
                instance.index(),
                expanded_block,
            )
            .unwrap(),
        ),
    };
    (
        CallCoordinates {
            source,
            original_normal_target: normal.index(),
            expanded_normal_target: mapped(normal),
        },
        origin,
    )
}
