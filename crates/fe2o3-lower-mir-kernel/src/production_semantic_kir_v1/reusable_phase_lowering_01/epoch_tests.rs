use super::*;
use fe2o3_kernel_ir::{ExecutionCapabilitySourceOccurrenceV1, PhaseCallOccurrenceV1};
use fe2o3_mir_model::semantic_mir_v1::*;

// Inert classifier fixtures only. Original-source custody is exercised by the
// existing phase67 callbacks, including the stale original SSA operand mutant.
fn fixture(
    phase: u32,
) -> (
    FinishEpoch,
    ExecutionCapabilityOperationV1,
    ExecutionCapabilityProvenanceV1,
    Type,
) {
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticKernelBindingIdentityV1::from_sha256([1; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([2; 32]),
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([4; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([5; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([6; 32]),
    )
    .unwrap();
    let ty = SemanticTypeIdV1::from_index;
    let contract = SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::WorkgroupBarrier {
            input_workgroup: ty(1),
            output_workgroup: ty(2),
            semantics: SemanticExecutionMemorySemanticsV1::new(
                SemanticExecutionMemoryScopeV1::Workgroup,
                SemanticExecutionMemoryOrderingV1::AcquireRelease,
                SemanticExecutionMemorySpacesV1::Workgroup,
            ),
        },
        SemanticExecutionCapabilitySignatureV1::new(&[ty(1)], ty(2)).unwrap(),
        provenance,
        SemanticTypeIdentityV1::from_sha256([7; 32]),
        SemanticTypeIdentityV1::from_sha256([8; 32]),
        Some(SemanticTypeIdentityV1::from_sha256([9; 32])),
        SemanticFunctionIdentityV1::from_sha256([10; 32]),
    )
    .unwrap();
    let source = |caller, block, operation| ExecutionCapabilitySourceV1 {
        function: [11; 32],
        operation,
        block: 2,
        occurrence: ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
            [12; 32], [13; 32], [14; 32], caller, block,
        ),
    };
    let key = PhaseKeyV1::for_begin(PhaseCallOccurrenceV1 {
        source: source(phase * 10, phase * 10 + 1, [15; 32]),
        callee_instance: phase * 10 + 1,
        original_normal_target: 3,
        expanded_normal_target: phase * 10 + 2,
    })
    .unwrap();
    let terminal = PhaseTerminalCallOccurrenceV1 {
        source: source(phase * 10 + 2, phase * 10 + 3, [10; 32]),
        original_normal_target: 3,
        expanded_normal_target: phase * 10 + 4,
    };
    let epoch = FinishEpoch::new(key, terminal, contract).unwrap();
    let provenance = ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("epoch94"),
        kernel_binding: [1; 32],
        frontend_unit: [2; 32],
        kernel_marker: [3; 32],
        target_brand: [4; 32],
        launch_brand: [5; 32],
        issuance: [6; 32],
    };
    let operation = ExecutionCapabilityOperationV1::WorkgroupBarrier {
        input_workgroup: ExecutionTypeIdentityV1::new([31; 32]),
        output_workgroup: ExecutionTypeIdentityV1::new([32; 32]),
        semantics: ExecutionMemorySemanticsV1 {
            scope: ExecutionMemoryScopeV1::Workgroup,
            ordering: ExecutionMemoryOrderingV1::AcquireRelease,
            spaces: ExecutionMemorySpacesV1::Workgroup,
        },
    };
    let input = Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type: ExecutionTypeIdentityV1::new([31; 32]),
        role: ExecutionCapabilityRoleV1::Workgroup,
        provenance: provenance.clone(),
        workgroup_brand: Some([7; 32]),
        epoch: Some(epoch.before),
    });
    (epoch, operation, provenance, input)
}

#[test]
fn phase_epochs_distinguish_occurrences_without_changing_static_source_contracts() {
    let (first, _, _, _) = fixture(1);
    let (second, operation, provenance, input) = fixture(2);
    assert_eq!(first.contract, second.contract);
    assert_ne!(first.before, second.before);
    assert_ne!(first.after, second.after);
    assert_ne!(first.before, first.after);
    assert_ne!(second.before, [8; 32]);
    assert_eq!(
        second
            .lower(
                second.terminal.source,
                second.contract,
                second.terminal.expanded_normal_target,
                &operation,
                &provenance,
                &[input]
            )
            .unwrap(),
        (Some(second.before), Some(second.after))
    );
    assert_eq!(fixture(2).0.before, second.before);
}

#[test]
fn phase_finish_rejects_exact_stale_source_occurrence_and_changed_normal_edge() {
    let (first, _, _, _) = fixture(1);
    let (second, operation, provenance, input) = fixture(2);
    // Same original callable, contract and block; only the real occurrence differs.
    assert_eq!(
        first.terminal.source.operation,
        second.terminal.source.operation
    );
    assert_eq!(first.terminal.source.block, second.terminal.source.block);
    assert!(
        second
            .lower(
                first.terminal.source,
                second.contract,
                second.terminal.expanded_normal_target,
                &operation,
                &provenance,
                &[input.clone()]
            )
            .is_err()
    );
    assert!(
        second
            .lower(
                second.terminal.source,
                second.contract,
                first.terminal.expanded_normal_target,
                &operation,
                &provenance,
                &[input.clone()]
            )
            .is_err()
    );
    let original = second.contract;
    let changed = SemanticExecutionCapabilityContractV1::new(
        original.operation(),
        original.signature(),
        original.provenance(),
        original.workgroup_brand().unwrap(),
        SemanticTypeIdentityV1::from_sha256([77; 32]),
        original.epoch_after(),
        original.source_identity(),
    )
    .unwrap();
    assert!(
        second
            .lower(
                second.terminal.source,
                changed,
                second.terminal.expanded_normal_target,
                &operation,
                &provenance,
                &[input]
            )
            .is_err()
    );
}

#[test]
fn phase_finish_never_relabels_an_old_or_foreign_live_operand() {
    let (first, _, _, _) = fixture(1);
    let (second, operation, provenance, input) = fixture(2);
    for variant in 0..7 {
        let Type::ExecutionCapability(mut cap) = input.clone() else {
            unreachable!()
        };
        match variant {
            0 => cap.epoch = Some([8; 32]),
            1 => cap.epoch = Some(first.before),
            2 => cap.epoch = Some(second.after),
            3 => cap.epoch = None,
            4 => cap.workgroup_brand = Some([70; 32]),
            5 => cap.source_type = ExecutionTypeIdentityV1::new([71; 32]),
            6 => cap.provenance.issuance = [72; 32],
            _ => unreachable!(),
        }
        assert!(
            second
                .lower(
                    second.terminal.source,
                    second.contract,
                    second.terminal.expanded_normal_target,
                    &operation,
                    &provenance,
                    &[Type::ExecutionCapability(cap)]
                )
                .is_err(),
            "variant={variant}"
        );
    }
}

#[test]
fn phase_epoch_transport_rejects_empty_keys_missing_occurrences_and_wrong_operations() {
    let (epoch, _, provenance, input) = fixture(1);
    assert!(scoped_epoch(PhaseKeyV1::from_untrusted_bytes([0; 32]), [8; 32]).is_err());
    let key = PhaseKeyV1::from_untrusted_bytes([99; 32]);
    assert!(scoped_epoch(key, [0; 32]).is_err());
    assert_ne!(
        scoped_epoch(key, [8; 32]).unwrap(),
        scoped_epoch(key, [9; 32]).unwrap()
    );
    let mut missing = epoch.terminal;
    missing.source.occurrence = None;
    assert!(FinishEpoch::new(key, missing, epoch.contract).is_err());
    let operation = ExecutionCapabilityOperationV1::WorkgroupFence {
        workgroup: ExecutionTypeIdentityV1::new([31; 32]),
        result: ExecutionTypeIdentityV1::new([32; 32]),
        semantics: ExecutionMemorySemanticsV1 {
            scope: ExecutionMemoryScopeV1::Workgroup,
            ordering: ExecutionMemoryOrderingV1::AcquireRelease,
            spaces: ExecutionMemorySpacesV1::Workgroup,
        },
    };
    assert!(
        epoch
            .lower(
                epoch.terminal.source,
                epoch.contract,
                epoch.terminal.expanded_normal_target,
                &operation,
                &provenance,
                &[input]
            )
            .is_err()
    );
}
