pub(crate) use conditional_bounds_resources_v1::{
    ConditionalBoundsResourceCaseV1, test_conditional_bounds_composition_v1,
};

mod conditional_bounds_resources_v1 {
    use super::*;
    use conditional_validation::ErrorV1 as Error;
    use invocation_receipt_v1::InvocationReceiptV1;
    type Bound = ProductionAnalysisResourceUpperBoundV1;
    type Limits = ProductionAnalysisResourceLimitsV1;
    type Phase = ProductionAnalysisResourcePhaseV1;
    type Manager = PlironAnalysisManagerV1;

    #[derive(Clone, Copy, Debug)]
    pub(crate) enum ConditionalBoundsResourceCaseV1 {
        ForeignManager,
        Exact,
        ManagerWorkShort,
        ReceiptWorkShort,
    }

    fn manager(input: &ConditionalPipelineSubjectV1<'_>) -> Manager {
        let mut analyses = Manager::new_with_resource_contract(
            input.function(),
            input.census(),
            Bound::default(),
            0,
            Limits::production_hard_ceiling(),
        )
        .unwrap();
        analyses.prepare_function_inventory(input.context(), input.function());
        analyses
    }

    pub(crate) fn test_conditional_bounds_composition_v1(input: &ConditionalPipelineSubjectV1<'_>) {
        assert!(input.census().ownership_contracts > 0);
        let phase = Phase::HierarchicalOwnership;
        let race = Bound::checked_phase(phase, 23, 11, 19).unwrap();
        let prepare = |family: PipelineFamilyV1<'_>, bounds| {
            let mut analyses = manager(input);
            let prepared = family
                .prepare_ownership_stage_v1(
                    &mut analyses,
                    input.census(),
                    None,
                    (bounds, race),
                    None,
                )
                .unwrap();
            let named = prepared
                .input
                .as_ref()
                .map_or(input.census(), |rows| rows.named_census);
            let local = preflight_hierarchical_ownership_resource_upper_bound_v1(
                named,
                None,
                Limits::production_hard_ceiling(),
            )
            .unwrap();
            let semantic = family
                .prepare_semantic_stage_v1(
                    &mut analyses,
                    input.census(),
                    prepared.stage.producing_phase_upper_bound,
                    None,
                )
                .unwrap();
            (
                local,
                prepared.stage.producing_phase_upper_bound,
                semantic.stage.producing_phase_upper_bound,
            )
        };
        let family = PipelineFamilyV1::Conditional(input);
        let (local, ownership, semantic) = prepare(family, Bound::default());
        let nested = local
            .checked_with_nested_sequence_discard(&[race], phase)
            .unwrap();
        let expected = Bound::checked_phase(
            phase,
            nested.work_upper_bound(),
            nested.peak_storage_upper_bound(),
            0,
        )
        .unwrap();
        assert_eq!(
            ownership, expected,
            "conditional ownership must retain the race reservation"
        );
        for unused in [
            Bound::checked_phase(phase, usize::MAX, 0, 0).unwrap(),
            Bound::checked_phase(phase, 0, usize::MAX, 0).unwrap(),
        ] {
            let (_, actual, nested_semantic) = prepare(family, unused);
            assert_eq!(
                actual, ownership,
                "slot 4 must not reserve the bounds producer again"
            );
            assert_eq!(
                nested_semantic, semantic,
                "slot 8 must not reserve it indirectly"
            );
        }
        let bounds = Bound::checked_phase(phase, 97, 101, 109).unwrap();
        let (local, ordinary, _) = prepare(PipelineFamilyV1::Ordinary, bounds);
        assert_eq!(
            ordinary,
            local
                .checked_with_nested_sequence_discard(&[bounds, race], phase)
                .unwrap()
        );
        let (_, without_bounds, _) = prepare(PipelineFamilyV1::Ordinary, Bound::default());
        assert_eq!(
            ordinary.work_upper_bound() - without_bounds.work_upper_bound(),
            97
        );
        assert!(ordinary.peak_storage_upper_bound() > without_bounds.peak_storage_upper_bound());
    }

    fn leave_work(analyses: &mut Manager, work: usize) {
        let remaining = analyses
            .remaining_resource_limits(Phase::ReportValidation)
            .unwrap();
        let padding = remaining.max_work().checked_sub(work).unwrap();
        analyses
            .admit_retained_resource_upper_bound(
                Phase::ReportValidation,
                Bound::checked_phase(Phase::ReportValidation, padding, 0, 0).unwrap(),
            )
            .unwrap();
        assert_eq!(
            analyses
                .remaining_resource_limits(Phase::ReportValidation)
                .unwrap()
                .max_work(),
            work
        );
    }

    pub(super) fn check_borrow(
        input: &ConditionalPipelineSubjectV1<'_>,
        validation: &conditional_validation::SessionV1<'_>,
        analyses: &mut Manager,
        case: ConditionalBoundsResourceCaseV1,
    ) {
        let phase = Phase::ReportValidation;
        let work = std::mem::size_of::<ProductionAnalysisInputCensusV1>()
            + 128
            + 8 * (input.census().operands
                + input.conditional_reads().map_or(0, <[_]>::len)
                + input.occurrences().len());
        let expected = Bound::checked_phase(phase, work, 0, 0).unwrap();
        if matches!(case, ConditionalBoundsResourceCaseV1::ForeignManager) {
            let original = analyses.resource_upper_bound();
            for exhausted in [false, true] {
                let mut foreign = manager(input);
                if exhausted {
                    leave_work(&mut foreign, 0);
                }
                let before = foreign.resource_upper_bound();
                assert!(matches!(
                    validation.borrow_bounds_with_observation_v1(&mut foreign, None),
                    Err(Error::Ledger)
                ));
                assert_eq!(foreign.resource_upper_bound(), before);
                let mut receipt =
                    InvocationReceiptV1::new(Bound::default(), Limits::new(0, 0)).unwrap();
                let untouched = receipt.snapshot();
                let scoped = receipt.phase(phase, 0).unwrap();
                let observer = scoped.observer(&Ok);
                let result =
                    validation.borrow_bounds_with_observation_v1(&mut foreign, Some(&observer));
                assert!(
                    matches!(result, Err(Error::Ledger)),
                    "foreign manager must be refused before resource admission"
                );
                drop(scoped);
                assert_eq!(foreign.resource_upper_bound(), before);
                assert_eq!(analyses.resource_upper_bound(), original);
                assert_eq!(receipt.snapshot(), untouched);
            }
            return;
        }
        let manager_short = matches!(case, ConditionalBoundsResourceCaseV1::ManagerWorkShort);
        let receipt_short = matches!(case, ConditionalBoundsResourceCaseV1::ReceiptWorkShort);
        leave_work(analyses, work - usize::from(manager_short));
        let before = analyses.resource_upper_bound();
        let mut receipt = InvocationReceiptV1::new(
            Bound::default(),
            Limits::new(work - usize::from(receipt_short), 0),
        )
        .unwrap();
        let result = with_invocation_phase_v1(Some(&mut receipt), phase, 0, |observer| {
            let (dependency, admitted) =
                validation.borrow_bounds_with_observation_v1(analyses, observer)?;
            assert_eq!(admitted, expected);
            assert!(dependency.is_clean());
            assert_eq!(dependency.reads(), input.conditional_reads());
            Ok(((), Some(admitted)))
        });
        let state = receipt.snapshot();
        assert!(!state.caught_panic);
        if manager_short || receipt_short {
            let PipelineErrorV1::ConditionalValidation(Error::Resource(error)) =
                result.unwrap_err()
            else {
                panic!("wrong bounds borrow refusal")
            };
            assert_eq!(error.phase, phase);
            assert_eq!(error.resource, "work upper bound");
            assert_eq!(state.first_denial, Some(error));
            assert_eq!(state.current, Bound::default());
            assert_eq!(state.committed, Bound::default());
            assert_eq!(analyses.resource_upper_bound(), before);
        } else {
            result.unwrap();
            assert_eq!(state.first_denial, None);
            assert_eq!(state.current, expected);
            assert_eq!(state.committed, expected);
            assert_eq!(receipt.complete(), Ok(expected));
            assert_eq!(
                analyses.resource_upper_bound(),
                before.checked_then_retain(expected, phase).unwrap()
            );
            assert_eq!(
                analyses
                    .remaining_resource_limits(phase)
                    .unwrap()
                    .max_work(),
                0
            );
        }
    }
}
