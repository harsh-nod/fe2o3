use super::*;
use fe2o3_kernel_ir::{
    VerifiedCanonicalKernelIrModuleV12 as Owner,
    encode_canonical_kir_occurrence_row_bytes_v1 as encode,
    materialize_canonical_kir_occurrence_rows_v1 as materialize,
    read_canonical_kir_occurrence_row_bytes_v1 as read,
};

// Invoked by genuine Direct and N/E source fixtures on both targets. The row
// owner is actually encoded, parsed and materialized independently of PLIRON.
pub(in crate::production_semantic_kir_v1) fn exercise_decoded_source_pair(
    anchor: CanonicalOutputFormalSourceAnchorV1<'_>,
    bound: &Owner,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    budget: &mut AssertOriginBudgetV1<'_>,
) {
    let source = match anchor {
        CanonicalOutputFormalSourceAnchorV1::Direct(v) => GeneralSourceContextV1::Direct(v),
        CanonicalOutputFormalSourceAnchorV1::Erased(v) => GeneralSourceContextV1::Erased(v),
    };
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    budget
        .reserve_storage(
            checked.owner().module().kernels.len() * std::mem::size_of::<FormalMemoryObligations>(),
        )
        .unwrap();
    let expected = check_general_context_v1(source, bound, checked, budget).unwrap();
    let (bytes, receipt) = encode(checked.occurrences().candidate(), budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let view = read(bytes.canonical_row_bytes(), bytes.counts(), budget).unwrap();
    budget
        .reserve_storage(view.storage().retained_storage())
        .unwrap();
    let (decoded, receipt) = materialize(&view, budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let (output, receipt) = Owner::from_canonical_bytes_with_verification_budget_v12(
        checked.owner().canonical().canonical_bytes(),
        budget,
    )
    .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert!(!std::ptr::eq(&output, checked.owner()));
    let candidate = decoded.candidate();
    assert!(!std::ptr::eq(
        candidate.operations,
        checked.occurrences().candidate().operations
    ));
    budget
        .reserve_storage(std::mem::size_of_val(expected.as_ref()))
        .unwrap();
    let required = budget.storage();
    let actual = check_decoded_source_output_pair_v1(
        source,
        bound,
        &output,
        candidate,
        checked.native_input_audit_bytes(),
        required,
        budget,
    )
    .unwrap();
    assert_eq!(actual, expected);
    drop(actual);
    assert_eq!(budget.storage(), required);
    assert!(matches!(
        check_decoded_source_output_pair_v1(
            source,
            bound,
            &output,
            candidate,
            &[],
            required,
            budget,
        ),
        Err(E::SourceOutput(ProductionSourceOutputErrorV1::InputCustody))
    ));
    assert_eq!(budget.storage(), required);
    assert!(!candidate.operations.is_empty());
    let incomplete = fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1 {
        operations: &candidate.operations[..candidate.operations.len() - 1],
        ..candidate
    };
    assert!(matches!(
        check_decoded_source_output_pair_v1(
            source,
            bound,
            &output,
            incomplete,
            checked.native_input_audit_bytes(),
            required,
            budget,
        ),
        Err(E::SourceOutput(ProductionSourceOutputErrorV1::Transition(
            _
        )))
    ));
    assert_eq!(budget.storage(), required);
    assert!(matches!(
        check_decoded_source_output_pair_v1(
            source,
            bound,
            &output,
            candidate,
            checked.native_input_audit_bytes(),
            required + 1,
            budget,
        ),
        Err(E::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert_eq!(budget.storage(), required);
    if let GeneralSourceContextV1::Erased(source) = source {
        exercise_erased_pair_custody(source, bound, checked, &output, candidate, budget);
        assert_eq!(budget.storage(), required);
    }
    drop(output);
    drop(decoded);
    drop(view);
    drop(bytes);
    drop(expected);
    budget.release_storage(budget.storage() - floor).unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

fn exercise_erased_pair_custody(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    bound: &Owner,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    output: &Owner,
    candidate: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) {
    erased_general_scratch_v1(budget, |budget| {
        let (coordinates, r) =
            fe2o3_kernel_analysis::check_canonical_kir_coordinate_preservation_v1(
                source.erased(),
                bound,
                budget,
            )
            .unwrap();
        budget.reserve_storage(r.retained_storage()).unwrap();
        let (input, r) = CanonicalKirInventoryV1::derive(bound, budget).unwrap();
        budget.reserve_storage(r.retained_storage()).unwrap();
        let (after, r) = CanonicalKirInventoryV1::derive(output, budget).unwrap();
        budget.reserve_storage(r.retained_storage()).unwrap();
        let (pair, r) = fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
            &input, &after, candidate, budget,
        )
        .unwrap();
        budget.reserve_storage(r.retained_storage()).unwrap();
        let (control, r) =
            fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV1::derive(&pair, budget)
                .unwrap();
        budget.reserve_storage(r.retained_storage()).unwrap();
        let live = budget.storage();
        // Equal canonical contents do not excuse a different owning endpoint or
        // any of the nine row slice identities in the historical owning API.
        assert!(matches!(
            with_erased_source_output_occurrences_v1(
                source,
                &coordinates,
                checked,
                &pair,
                &control,
                budget,
                |_, _| -> Result<(), ProductionSourceOutputErrorV1> {
                    panic!("foreign owning view reached callback")
                },
            ),
            Err(ProductionSourceOutputErrorV1::InputCustody)
        ));
        let mut reached = false;
        with_erased_source_output_pair_v1(
            source,
            &coordinates,
            output,
            candidate,
            checked.native_input_audit_bytes(),
            live,
            &pair,
            &control,
            budget,
            |view, budget| {
                reached = true;
                assert!(matches!(
                    view.checked_output(budget),
                    Err(ProductionSourceOutputErrorV1::InputCustody)
                ));
                assert!(std::ptr::eq(view.bound(budget).unwrap(), bound));
                Ok(())
            },
        )
        .unwrap();
        assert!(reached);
        assert_eq!(budget.storage(), live);
        // Submitted rows from the producer have equal values but different
        // backing from this independently checked decoded pair.
        assert!(matches!(
            with_erased_source_output_pair_v1(
                source,
                &coordinates,
                output,
                checked.occurrences().candidate(),
                checked.native_input_audit_bytes(),
                live,
                &pair,
                &control,
                budget,
                |_, _| Ok(())
            ),
            Err(ProductionSourceOutputErrorV1::InputCustody)
        ));
        assert_eq!(budget.storage(), live);
        let result = with_erased_source_output_pair_v1(
            source,
            &coordinates,
            output,
            candidate,
            checked.native_input_audit_bytes(),
            live,
            &pair,
            &control,
            budget,
            |_, _| -> Result<(), ProductionSourceOutputErrorV1> {
                panic!("decoded callback cleanup")
            },
        );
        assert!(matches!(
            result,
            Err(ProductionSourceOutputErrorV1::Panicked)
        ));
        assert_eq!(budget.storage(), live);
        Ok(())
    })
    .unwrap();
}

pub(in crate::production_semantic_kir_v1) fn exercise_transport_rows(
    input: &Owner,
    output: &Owner,
    rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    budget: &mut AssertOriginBudgetV1<'_>,
) {
    let floor = budget.storage();
    erased_general_scratch_v1(budget, |budget| {
        let (inventory, r) = CanonicalKirInventoryV1::derive(input, budget).unwrap();
        budget.reserve_storage(r.retained_storage()).unwrap();
        // Inert artificial sites test mapping mechanics only, never native or
        // source admission. Each old operation has a distinct donor identity.
        let mut previous =
            scratch::<ProductionExpandedSourceOriginV1>(inventory.operations().len(), budget)
                .unwrap();
        for (ordinal, operation) in inventory.operations().iter().enumerate() {
            previous.push(ProductionExpandedSourceOriginV1 {
                output: operation.coordinate,
                original_u: Some(operation.coordinate),
                source_statement: Some((
                    SemanticFunctionIdV1::from_index(0),
                    SemanticBlockIdV1::from_index(0),
                    ordinal.try_into().unwrap(),
                )),
            });
        }
        let mut next =
            scratch::<ProductionExpandedSourceOriginV1>(rows.operations.len(), budget).unwrap();
        expanded_transport_pair_v1(input, output, rows, &previous, &mut next, budget).unwrap();
        assert_eq!(next.len(), rows.operations.len());
        for (at, row) in rows.operations.iter().enumerate() {
            assert_eq!(next[at].output, row.output);
            match row.origin {
                CanonicalKirOperationOriginV1::Retained(old) => {
                    let ordinal = operation_ordinal(&inventory, old).unwrap();
                    assert_eq!(next[at].original_u, Some(old));
                    assert_eq!(
                        next[at].source_statement,
                        previous[ordinal].source_statement
                    );
                }
                CanonicalKirOperationOriginV1::ConstantFrom(_) => {
                    assert_eq!(
                        (next[at].original_u, next[at].source_statement),
                        (None, None)
                    );
                }
            }
        }
        next.clear();
        if let Some(old) = rows.operations.iter().find_map(|row| match row.origin {
            CanonicalKirOperationOriginV1::Retained(old) => Some(old),
            CanonicalKirOperationOriginV1::ConstantFrom(_) => None,
        }) {
            // Target an actually retained occurrence, never one deleted before
            // this pair, so the hostile input must be visited by reconstruction.
            let ordinal = operation_ordinal(&inventory, old).unwrap();
            previous[ordinal].output.operation = u32::MAX;
            assert!(
                expanded_transport_pair_v1(input, output, rows, &previous, &mut next, budget)
                    .is_err()
            );
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
}
