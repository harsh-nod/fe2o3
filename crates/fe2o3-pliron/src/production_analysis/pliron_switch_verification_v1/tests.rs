use super::*;
use crate::production_analysis::pliron_pipeline::invocation_receipt_v1::{
    InvocationReceiptFailureV1 as ReceiptFailure, InvocationReceiptV1 as Receipt,
};
use crate::production_analysis::pliron_resource_envelope::{
    ProductionAnalysisResourceLimitsV1 as Limits, ProductionAnalysisResourceUpperBoundV1 as Bound,
};
use dialect_gpu::switch_v3::{SwitchEdgeV3, SwitchKeyKindAttrV3};
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        attributes::UnitAttr,
        types::{IntegerType, Signedness},
    },
    common_traits::Verify,
};

fn fixture(
    context: &mut Context,
    width: u32,
    signed: bool,
    kind: SwitchKeyKindAttrV3,
    keys: Vec<u64>,
    source_arity: usize,
    edge_arities: &[usize],
) -> SwitchOpV3 {
    dialect_gpu::register_dialect(context).unwrap();
    let ty = IntegerType::get(
        context,
        width,
        if signed {
            Signedness::Signed
        } else {
            Signedness::Unsigned
        },
    )
    .into();
    let source = BasicBlock::new(context, None, vec![ty; source_arity]);
    let selector = source.deref(context).get_argument(source_arity - 3);
    let a = source.deref(context).get_argument(source_arity - 2);
    let b = source.deref(context).get_argument(source_arity - 1);
    let repeated = BasicBlock::new(context, None, vec![ty; edge_arities[0]]);
    let edges = edge_arities
        .iter()
        .enumerate()
        .map(|(ordinal, &arity)| {
            let target = if arity == edge_arities[0] {
                repeated
            } else {
                BasicBlock::new(context, None, vec![ty; arity])
            };
            let arguments = (0..arity)
                .map(|i| if (i + ordinal) % 2 == 0 { a } else { b })
                .collect();
            SwitchEdgeV3::new(target, arguments)
        })
        .collect();
    SwitchOpV3::try_new(context, selector, kind, keys, edges).unwrap()
}

fn collect(context: &Context, switch: SwitchOpV3) -> SwitchVerificationCensusV1 {
    census_switch_verification_v1(
        context,
        switch,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )
    .unwrap()
}

const OBSERVED_PHASE: ProductionAnalysisResourcePhaseV1 =
    ProductionAnalysisResourcePhaseV1::StructuralIdentity;

#[test]
fn observed_switch_census_preserves_each_admitted_prefix() {
    let mut context = Context::new();
    let switch = fixture(
        &mut context,
        128,
        false,
        SwitchKeyKindAttrV3::LegacyU64,
        (0..17).collect(),
        3,
        &[2; 18],
    );
    let census = collect(&context, switch);
    let work = census.traversal_work + census.callback_work;
    let peak = census.callback_scratch;
    for (limits, prefix_work, prefix_peak, denial) in [
        (Limits::new(127, peak), 0, 0, Some("work upper bound")),
        (Limits::new(128, peak), 128, 64, Some("work upper bound")),
        (
            Limits::new(census.traversal_work, peak),
            census.traversal_work,
            64,
            Some("work upper bound"),
        ),
        (
            Limits::new(work - 1, peak),
            census.traversal_work,
            64,
            Some("work upper bound"),
        ),
        (
            Limits::new(work, peak - 1),
            census.traversal_work,
            64,
            Some("peak storage upper bound"),
        ),
        (Limits::new(work, peak), work, peak, None),
    ] {
        let mut receipt =
            Receipt::new(Bound::default(), Limits::production_hard_ceiling()).unwrap();
        let phase = receipt.phase(OBSERVED_PHASE, 0).unwrap();
        let result = census_switch_verification_with_observation_v1(
            &context,
            switch,
            limits,
            Some(&phase.observer(&Ok)),
        );
        if let Some(resource) = denial {
            let error = result.unwrap_err();
            assert_eq!(error.phase, OBSERVED_PHASE);
            assert_eq!(error.resource, resource);
            drop(phase);
            assert_eq!(receipt.snapshot().first_denial, Some(error));
            assert_eq!(receipt.complete(), Err(ReceiptFailure::Denied(error)));
        } else {
            assert_eq!(result.unwrap(), census);
            let bound = Bound::checked_phase(OBSERVED_PHASE, work, 0, peak).unwrap();
            phase.commit(bound).unwrap();
            assert_eq!(receipt.complete(), Ok(bound));
        }
        let state = receipt.snapshot();
        assert_eq!(state.current, state.committed);
        assert!(!state.caught_panic);
        assert_eq!(
            (
                state.committed.work_upper_bound(),
                state.committed.peak_storage_upper_bound()
            ),
            (prefix_work, prefix_peak)
        );
    }
}

#[test]
fn observed_switch_early_error_and_externally_caught_panic_survive() {
    for panic_case in [false, true] {
        let mut context = Context::new();
        let switch = fixture(
            &mut context,
            128,
            false,
            SwitchKeyKindAttrV3::LegacyU64,
            (0..17).collect(),
            3,
            &[2; 18],
        );
        let pointer = switch.get_operation();
        if !panic_case {
            pointer
                .deref_mut(&context)
                .attributes
                .set("extra".try_into().unwrap(), UnitAttr::new());
        }
        let mut receipt =
            Receipt::new(Bound::default(), Limits::production_hard_ceiling()).unwrap();
        let phase = receipt.phase(OBSERVED_PHASE, 0).unwrap();
        let held = panic_case.then(|| pointer.deref_mut(&context));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            census_switch_verification_with_observation_v1(
                &context,
                switch,
                Limits::production_hard_ceiling(),
                Some(&phase.observer(&Ok)),
            )
        }));
        drop(held);
        if panic_case {
            assert!(result.is_err());
        } else {
            assert_eq!(
                result.unwrap().unwrap_err().resource,
                "native-switch verification frame"
            );
        }
        // The unwind is already caught; phase Drop alone cannot detect it.
        drop(phase);
        let state = receipt.snapshot();
        assert_eq!(state.caught_panic, panic_case);
        assert_eq!(
            (
                state.committed.work_upper_bound(),
                state.committed.peak_storage_upper_bound()
            ),
            (128, 64)
        );
        assert_eq!(
            state.first_denial.map(|e| e.resource),
            if panic_case {
                None
            } else {
                Some("native-switch verification frame")
            }
        );
        assert_eq!(
            receipt.complete(),
            Err(match state.first_denial {
                Some(error) => ReceiptFailure::Denied(error),
                None => ReceiptFailure::CaughtPanic,
            })
        );
    }
}

fn assert_boundary(context: &Context, switch: SwitchOpV3, expected: SwitchVerificationCensusV1) {
    assert_eq!(collect(context, switch), expected);
    let work = expected.traversal_work + expected.callback_work;
    let peak = expected.callback_scratch;
    for (work, peak, accepted) in [
        (work, peak, true),
        (work - 1, peak, false),
        (work, peak - 1, false),
    ] {
        assert_eq!(
            census_switch_verification_v1(
                context,
                switch,
                ProductionAnalysisResourceLimitsV1::new(work, peak)
            )
            .is_ok(),
            accepted
        );
    }
    // The census describes costs, not semantic verification or graph ownership.
    switch.verify(context).unwrap();
    switch.verify_interfaces(context).unwrap();
}

#[test]
fn raw_128_switch_census_has_literal_exact_and_one_under_thresholds() {
    assert_eq!(std::mem::size_of::<usize>(), 8);
    for (count, traversal_work, callback_work, callback_scratch) in
        [(0, 160, 253, 86), (16, 672, 1845, 86), (17, 704, 6372, 376)]
    {
        let context = &mut Context::new();
        let switch = fixture(
            context,
            128,
            false,
            SwitchKeyKindAttrV3::LegacyU64,
            (0..count as u64).rev().collect(),
            3,
            &vec![2; count + 1],
        );
        assert_boundary(
            context,
            switch,
            SwitchVerificationCensusV1 {
                traversal_work,
                callback_work,
                callback_scratch,
            },
        );
    }
}

#[test]
fn typed_signed_and_empty_wide_census_do_not_pay_for_radix_scratch() {
    let context = &mut Context::new();
    let switch = fixture(
        context,
        64,
        true,
        SwitchKeyKindAttrV3::I64,
        vec![1 << 63, u64::MAX, 0, i64::MAX as u64],
        3,
        &[2; 5],
    );
    assert_boundary(
        context,
        switch,
        SwitchVerificationCensusV1 {
            traversal_work: 288,
            callback_work: 621,
            callback_scratch: 86,
        },
    );
    let context = &mut Context::new();
    let switch = fixture(
        context,
        128,
        true,
        SwitchKeyKindAttrV3::EmptyTyped,
        vec![],
        3,
        &[2],
    );
    assert_boundary(
        context,
        switch,
        SwitchVerificationCensusV1 {
            traversal_work: 160,
            callback_work: 253,
            callback_scratch: 86,
        },
    );
}

#[test]
fn equal_edge_shapes_charge_actual_defining_roster_widths() {
    let mut observations = Vec::new();
    for arity in [3, 17] {
        let context = &mut Context::new();
        let switch = fixture(
            context,
            64,
            false,
            SwitchKeyKindAttrV3::EmptyTyped,
            vec![],
            arity,
            &[2],
        );
        observations.push(collect(context, switch));
    }
    assert_eq!(observations[0].callback_work, 253);
    assert_eq!(observations[1].callback_work, 323);
    assert_eq!(
        observations[0].traversal_work,
        observations[1].traversal_work
    );
    assert_eq!(
        observations[0].callback_scratch,
        observations[1].callback_scratch
    );
}

#[test]
fn unequal_edge_widths_change_type_scans_but_add_no_payload_vector_scratch() {
    let mut observations = Vec::new();
    for arities in [[2, 2, 2], [1, 1, 4]] {
        let context = &mut Context::new();
        let switch = fixture(
            context,
            64,
            false,
            SwitchKeyKindAttrV3::U64,
            vec![0, 1],
            3,
            &arities,
        );
        observations.push(collect(context, switch));
    }
    assert_eq!(observations[0].callback_work, 437);
    assert_eq!(observations[1].callback_work, 443);
    assert_eq!(observations[0].traversal_work, 224);
    assert_eq!(observations[1].traversal_work, 224);
    assert_eq!(observations[0].callback_scratch, 86);
    assert_eq!(observations[1].callback_scratch, 86);
}

#[test]
fn malformed_headers_reject_before_key_or_payload_traversal() {
    let context = &mut Context::new();
    let switch = fixture(
        context,
        128,
        false,
        SwitchKeyKindAttrV3::LegacyU64,
        (0..17).collect(),
        3,
        &[2; 18],
    );
    switch
        .get_operation()
        .deref_mut(context)
        .attributes
        .set("extra".try_into().unwrap(), UnitAttr::new());
    let error = census_switch_verification_v1(
        context,
        switch,
        ProductionAnalysisResourceLimitsV1::new(128, 64),
    )
    .unwrap_err();
    assert_eq!(error.resource, "native-switch verification frame");
    let error = census_switch_verification_v1(
        context,
        switch,
        ProductionAnalysisResourceLimitsV1::new(127, 64),
    )
    .unwrap_err();
    assert_eq!(error.resource, "work upper bound");
}
