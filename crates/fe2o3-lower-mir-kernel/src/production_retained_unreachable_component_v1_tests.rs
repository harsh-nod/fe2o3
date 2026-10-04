use super::*;
// Independently checked identity-transition component, not fixed-optimizer output.
// The paired original test exercises actual optimizer omission of this source block.
use fe2o3_kernel_ir::{
    CanonicalKirBlockSegmentV1, CanonicalKirBlockTransitionV1,
    CanonicalKirDefinitionDescendantKindV1, CanonicalKirDefinitionDescendantV1,
    CanonicalKirDefinitionTransitionV1, CanonicalKirEdgeArgumentTransitionV1,
    CanonicalKirEdgeTransitionV1, CanonicalKirFunctionTransitionV1, CanonicalKirOperationOriginV1,
    CanonicalKirOperationTransitionV1, CanonicalKirTransitionCandidateV1,
    CanonicalKirTransitionRangeV1, CanonicalKirUseTransitionV1,
};

#[test]
fn independently_checked_identity_retains_unreachable_assertion_physical_placement() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = materialize_transformed(
        Fixture::Literal(true),
        |_, _| {
            vec![
                block(
                    31,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: constant(BOOL, 1, 1),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                1,
                                edge(SemanticEdgeRoleV1::SwitchValue, 2),
                            )],
                            edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                        )
                        .unwrap(),
                    },
                ),
                block(32, vec![], literal_terminator(true, 2)),
                block(33, vec![], SemanticTerminatorKindV1::Return),
            ]
        },
        &mut budget,
    );
    let owner_bytes = retained(&owner);
    budget.reserve_storage(owner_bytes).unwrap();
    let floor = budget.storage();
    let (input, storage) = Inventory::derive(owner.executable(), &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let range = |start: usize| CanonicalKirTransitionRangeV1 {
        start: u32::try_from(start).unwrap(),
        len: 1,
    };
    let functions: Vec<_> = input
        .functions()
        .iter()
        .map(|row| CanonicalKirFunctionTransitionV1 {
            input: row.coordinate,
            output: row.coordinate,
        })
        .collect();
    let blocks: Vec<_> = input
        .blocks()
        .iter()
        .enumerate()
        .map(|(i, row)| CanonicalKirBlockTransitionV1 {
            output: row.coordinate,
            segments: range(i),
        })
        .collect();
    let segments: Vec<_> = input
        .blocks()
        .iter()
        .map(|row| CanonicalKirBlockSegmentV1 {
            input: row.coordinate,
            connector: None,
        })
        .collect();
    let operations: Vec<_> = input
        .operations()
        .iter()
        .map(|row| CanonicalKirOperationTransitionV1 {
            output: row.coordinate,
            origin: CanonicalKirOperationOriginV1::Retained(row.coordinate),
        })
        .collect();
    let definitions: Vec<_> = input
        .definitions()
        .iter()
        .enumerate()
        .map(|(i, row)| CanonicalKirDefinitionTransitionV1 {
            input: row.coordinate,
            outputs: range(i),
        })
        .collect();
    let descendants: Vec<_> = input
        .definitions()
        .iter()
        .map(|row| CanonicalKirDefinitionDescendantV1 {
            output: row.coordinate,
            kind: CanonicalKirDefinitionDescendantKindV1::Retained,
        })
        .collect();
    let uses: Vec<_> = input
        .uses()
        .iter()
        .map(|row| CanonicalKirUseTransitionV1 {
            input: row.coordinate,
            output: row.coordinate,
        })
        .collect();
    let edges: Vec<_> = input
        .edges()
        .iter()
        .map(|row| CanonicalKirEdgeTransitionV1 {
            input: row.coordinate,
            output: row.coordinate,
        })
        .collect();
    let arguments: Vec<_> = input
        .edge_arguments()
        .iter()
        .map(|row| CanonicalKirEdgeArgumentTransitionV1 {
            input: row.coordinate,
            output: row.coordinate,
        })
        .collect();
    let rows = CanonicalKirTransitionCandidateV1 {
        functions: &functions,
        blocks: &blocks,
        segments: &segments,
        operations: &operations,
        definitions: &definitions,
        definition_outputs: &descendants,
        uses: &uses,
        edges: &edges,
        edge_arguments: &arguments,
    };
    // Candidate buffers are test-owned inputs, not output of the observed producer.
    let (checked, checked_storage) =
        check_canonical_kir_transition_v1(&input, &input, rows, &mut budget).unwrap();
    budget
        .reserve_storage(checked_storage.retained_storage())
        .unwrap();
    let (control, control_storage) = Control::derive(&checked, &mut budget).unwrap();
    budget
        .reserve_storage(control_storage.retained_storage())
        .unwrap();
    let (transported, transported_storage) =
        transport_semantic_kir_assert_origins_v1(owner.assert_origins(), &control, &mut budget)
            .unwrap();
    budget
        .reserve_storage(transported_storage.retained_storage())
        .unwrap();
    assert!(matches!(
        assert_binding(&transported, 1, &mut budget).outcome(),
        SemanticKirOptimizedAssertOutcomeV1::RemovedUnreachable {
            block: Some(_),
            condition: Some(_),
            ..
        }
    ));
    assert!(!transported.grants_authority());
    drop(transported);
    budget
        .release_storage(transported_storage.retained_storage())
        .unwrap();
    drop(control);
    budget
        .release_storage(control_storage.retained_storage())
        .unwrap();
    #[allow(
        clippy::drop_non_drop,
        reason = "End the borrowed witness before releasing its ledger reservation"
    )]
    drop(checked);
    budget
        .release_storage(checked_storage.retained_storage())
        .unwrap();
    drop(input);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    drop(owner);
    budget.release_storage(owner_bytes).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
