use super::*;
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
