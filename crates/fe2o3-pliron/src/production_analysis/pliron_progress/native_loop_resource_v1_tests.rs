use super::native_loop_v1_tests::{parse, source};
use super::*;
use crate::production_analysis::{
    pliron_ir_identity::LivePlironStructuralIdentityProviderV1,
    pliron_pass_contract::{
        PRODUCTION_PLIRON_PASS_CONTRACTS_V1, PlironPassPreservationErrorV1,
        begin_production_pliron_pass_contract_session_v1,
    },
    pliron_pipeline::invocation_receipt_v1::{InvocationReceiptFailureV1, InvocationReceiptV1},
};
use std::mem::size_of;

fn census() -> ProductionAnalysisInputCensusV1 {
    ProductionAnalysisInputCensusV1 {
        blocks: 4,
        operations: 8,
        operands: 10,
        results: 4,
        // Seven semantic attributes plus four block and four result-name maps.
        attributes: 7 + 4 + 4,
        block_arguments: 4,
        successors: 4,
        ..Default::default()
    }
}

fn scratch() -> usize {
    // Header fields, eight widening slots, two integer coordinates, twelve
    // live Values, sixteen typed handles, APInt header, rounded digit payload.
    32 + 64 + 12 + 48 + 32 + 4 + 128_usize.div_ceil(usize::BITS as usize)
}

fn entry_fields() -> usize {
    (size_of::<Option<pliron::value::Value>>()
        + size_of::<Option<usize>>()
        + size_of::<pliron::value::Value>()
        + 2 * size_of::<pliron::r#type::TypeHandle>())
    .div_ceil(size_of::<usize>())
}

fn expected() -> ProductionAnalysisResourceUpperBoundV1 {
    // B=4,E=4,O=8,A=10,D=8: Q=72, scalar sites=932,
    // scalar cost=808. Eight debug-name dictionaries add 5*8 work and
    // 3*8 temporary cells to the original semantic-only census.
    ProductionAnalysisResourceUpperBoundV1::checked_phase(
        ProductionAnalysisResourcePhaseV1::Progress,
        27_674 + 5 * 8 + 932 * 808,
        8_836,
        2 * 58 + 14 * 4 + 8 * 4 + (58 + 16 + 10) + 3 + (3 + 4 * 10) + entry_fields() + scratch(),
    )
    .unwrap()
}

#[test]
fn native_progress_resource_fields_bound_actual_private_layouts() {
    let word = size_of::<usize>();
    assert!(size_of::<ProgressHeaderViewV1>() <= 32 * word);
    assert!(size_of::<Option<NativeProgressCastV1>>() <= 8 * word);
    assert!(size_of::<NativeProgressLiteralV1>() <= 6 * word);
    assert!(size_of::<pliron::value::Value>() <= 4 * word);
    assert!(size_of::<pliron::r#type::TypeHandle>() <= 2 * word);
    assert!(size_of::<pliron::utils::apint::APInt>() <= 4 * word);
    assert_eq!(size_of::<IncomingEdgeV1>(), 2 * word);
    assert_eq!(native_progress_scalar_storage_v1(), scratch());
}

#[test]
fn native_progress_resource_exact_census_and_independent_boundaries() {
    let (context, function) = parse(source("dynamic"));
    let actual = bounded_structural_inventory(&context, &function).unwrap();
    assert_eq!(
        (
            actual.blocks,
            actual.operations,
            actual.operands,
            actual.results,
            actual.attributes,
            actual.block_arguments,
            actual.edges
        ),
        (4, 8, 10, 4, 7 + 4 + 4, 4, 4)
    );
    let expected = expected();
    let exact_limits = ProductionAnalysisResourceLimitsV1::new(
        expected.work_upper_bound(),
        expected.peak_storage_upper_bound(),
    );
    assert_eq!(
        preflight_scoped_progress_resource_upper_bound_v1(census(), exact_limits),
        Ok(expected)
    );
    for (work, peak, resource) in [
        (
            expected.work_upper_bound() - 1,
            expected.peak_storage_upper_bound(),
            "work upper bound",
        ),
        (
            expected.work_upper_bound(),
            expected.peak_storage_upper_bound() - 1,
            "peak storage upper bound",
        ),
    ] {
        assert_eq!(
            preflight_scoped_progress_resource_upper_bound_v1(
                census(),
                ProductionAnalysisResourceLimitsV1::new(work, peak)
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::Progress,
                resource
            })
        );
    }
    let standalone = preflight_progress_resource_upper_bound_v1(
        census(),
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .unwrap();
    assert_eq!(
        standalone.work_upper_bound(),
        expected.work_upper_bound() + 10 * 8
    );
    assert_eq!(
        standalone.retained_storage_upper_bound(),
        expected.retained_storage_upper_bound()
    );
    assert_eq!(
        standalone.peak_storage_upper_bound() + 3,
        expected.peak_storage_upper_bound()
    );
}

#[test]
fn native_progress_scalar_probe_census_is_bounded_for_success_and_refusal() {
    for name in [
        "dynamic",
        "signed",
        "narrow",
        "nested",
        "duplicate",
        "conflict",
        "wrap",
        "zero",
    ] {
        let (context, function) = parse(source(name));
        let inventory = bounded_structural_inventory(&context, &function).unwrap();
        let (b, e, d) = (
            inventory.blocks,
            inventory.edges,
            inventory.results + inventory.block_arguments,
        );
        let queries = e * (2 + b * e);
        let sites = 8 * (b + e) + e + 8 * queries + e * queries;
        native_progress_scalar_observation_v1::reset();
        let _report = run_pliron_progress_check_v1(&context, &function);
        let (domains, literals) = native_progress_scalar_observation_v1::counts();
        assert!(
            native_progress_scalar_observation_v1::probes() <= sites,
            "{name}"
        );
        assert!(domains <= 16 * sites, "{name}");
        assert!(literals <= sites, "{name}");
        assert!(domains > 0, "{name}");
        if name != "conflict" {
            assert!(literals > 0, "{name}");
        }
        if name == "dynamic" {
            assert_eq!((domains, literals), (9, 1));
            assert_eq!(native_progress_scalar_observation_v1::probes(), 5);
        }
        // Type lookup worst-case work is independently 2*D per lookup.
        assert!(2 * d * domains <= sites * (32 * d), "{name}");
    }
}

#[test]
fn native_progress_observed_entry_preserves_prefix_sibling_and_first_denial() {
    let phase_kind = ProductionAnalysisResourcePhaseV1::Progress;
    let hard = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
    let leaf = expected();
    let prefix =
        ProductionAnalysisResourceUpperBoundV1::checked_phase(phase_kind, 11, 7, 5).unwrap();
    let total_work = 11 + leaf.work_upper_bound();
    let total_peak = 7 + leaf.peak_storage_upper_bound();
    for cut in [
        None,
        Some("work upper bound"),
        Some("peak storage upper bound"),
    ] {
        let (context, function) = parse(source("dynamic"));
        let limits = ProductionAnalysisResourceLimitsV1::new(
            total_work - usize::from(cut == Some("work upper bound")),
            total_peak - usize::from(cut == Some("peak storage upper bound")),
        );
        let mut receipt = InvocationReceiptV1::new(Default::default(), limits).unwrap();
        let sibling = receipt.phase(phase_kind, 0).unwrap();
        sibling
            .observer(&Ok)
            .require(hard, phase_kind, Ok(prefix))
            .unwrap();
        sibling.commit(prefix).unwrap();
        let mut session = begin_production_pliron_pass_contract_session_v1(
            LivePlironStructuralIdentityProviderV1::new(&context, &function),
        )
        .unwrap();
        for contract in &PRODUCTION_PLIRON_PASS_CONTRACTS_V1[..8] {
            session
                .run_contiguous_pass(contract.pass(), || Ok::<_, ()>(()))
                .unwrap()
                .unwrap();
        }
        let phase = receipt.phase(phase_kind, 0).unwrap();
        let observer = phase.observer(&Ok);
        native_progress_scalar_observation_v1::reset();
        let admission = observer.require(
            hard,
            phase_kind,
            preflight_scoped_progress_resource_upper_bound_v1(census(), hard),
        );
        if let Some(resource) = cut {
            let denial = ProductionAnalysisResourceLimitV1 {
                phase: phase_kind,
                resource,
            };
            assert_eq!(admission, Err(denial));
            assert_eq!(native_progress_scalar_observation_v1::counts(), (0, 0));
            observer.deny(ProductionAnalysisResourceLimitV1 {
                phase: phase_kind,
                resource: "later refusal",
            });
            drop(phase);
            let snapshot = receipt.snapshot();
            assert_eq!(snapshot.current, prefix);
            assert_eq!(snapshot.committed, prefix);
            assert_eq!(snapshot.first_denial, Some(denial));
            assert_eq!(
                receipt.complete(),
                Err(InvocationReceiptFailureV1::Denied(denial))
            );
        } else {
            assert_eq!(admission, Ok(leaf));
            let report = session
                .run_scoped_semantic_refinement_with_resource_limits_v1(hard, |input| {
                    let result =
                        run_pliron_progress_with_scoped_observation_v1(input, Some(&observer))?;
                    assert!(std::ptr::eq(result.context, &context));
                    assert_eq!(result.function.get_operation(), function.get_operation());
                    Ok(Ok::<_, ()>(result.report))
                })
                .unwrap()
                .unwrap();
            assert_eq!(report.certificates().len(), 1);
            assert!(native_progress_scalar_observation_v1::counts().1 > 0);
            phase.commit(leaf).unwrap();
            let snapshot = receipt.snapshot();
            assert_eq!(snapshot.current.work_upper_bound(), total_work);
            assert_eq!(
                snapshot.current.retained_storage_upper_bound(),
                7 + leaf.retained_storage_upper_bound()
            );
            assert_eq!(snapshot.current.peak_storage_upper_bound(), total_peak);
            assert_eq!(snapshot.current, snapshot.committed);
            assert_eq!(snapshot.first_denial, None);
            assert_eq!(receipt.complete(), Ok(snapshot.committed));
        }
    }
}

#[test]
fn native_progress_paid_scalar_panic_and_mutate_restore_do_not_escape_receipt_or_scope() {
    let hard = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
    let phase_kind = ProductionAnalysisResourcePhaseV1::Progress;
    for mutate in [false, true] {
        let (context, function) = parse(source("dynamic"));
        let before = function.disp(&context).to_string();
        let mut session = begin_production_pliron_pass_contract_session_v1(
            LivePlironStructuralIdentityProviderV1::new(&context, &function),
        )
        .unwrap();
        for contract in &PRODUCTION_PLIRON_PASS_CONTRACTS_V1[..8] {
            session
                .run_contiguous_pass(contract.pass(), || Ok::<_, ()>(()))
                .unwrap()
                .unwrap();
        }
        let mut receipt = InvocationReceiptV1::new(Default::default(), hard).unwrap();
        let phase = receipt.phase(phase_kind, 0).unwrap();
        let observer = phase.observer(&Ok);
        observer
            .require(
                hard,
                phase_kind,
                preflight_scoped_progress_resource_upper_bound_v1(census(), hard),
            )
            .unwrap();
        native_progress_scalar_observation_v1::reset();
        if !mutate {
            native_progress_scalar_observation_v1::panic_after_literal();
        }
        let result =
            session.run_scoped_semantic_refinement_with_resource_limits_v1(hard, |input| {
                if mutate {
                    let pointer = function.get_operation();
                    let attributes = pointer.deref(&context).attributes.clone();
                    pointer.deref_mut(&context).attributes = attributes.clone();
                    pointer.deref_mut(&context).attributes = attributes;
                }
                let result =
                    run_pliron_progress_with_scoped_observation_v1(input, Some(&observer))?;
                Ok(Ok::<_, ()>(result.report))
            });
        if mutate {
            assert!(matches!(
                result,
                Err(PlironPassPreservationErrorV1::MutationAttempted { .. })
            ));
            assert_eq!(native_progress_scalar_observation_v1::counts(), (0, 0));
        } else {
            let report = result.unwrap().unwrap();
            assert!(
                matches!(report.findings(), [PlironProgressFindingV1::StructuralPrerequisiteRejected { reason }]
                if reason.contains("native progress paid literal panic"))
            );
            assert!(native_progress_scalar_observation_v1::counts().1 > 0);
        }
        drop(phase);
        assert_eq!(function.disp(&context).to_string(), before);
        assert_eq!(
            receipt.snapshot().current.work_upper_bound(),
            expected().work_upper_bound()
        );
        assert_eq!(
            receipt.snapshot().current.peak_storage_upper_bound(),
            expected().peak_storage_upper_bound()
        );
        assert_eq!(receipt.snapshot().caught_panic, !mutate);
        assert_eq!(receipt.snapshot().first_denial, None);
        if !mutate {
            assert_eq!(
                receipt.complete(),
                Err(InvocationReceiptFailureV1::CaughtPanic)
            );
        }
        native_progress_scalar_observation_v1::reset();
    }
}
