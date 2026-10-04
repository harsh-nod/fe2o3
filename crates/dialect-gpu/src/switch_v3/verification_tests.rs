fn payload_switch(ctx: &mut Context) -> (SwitchOpV3, TypeHandle) {
    let ty = integer(ctx, 64, false);
    let source = BasicBlock::new(ctx, None, vec![ty; 3]);
    let selector = source.deref(ctx).get_argument(0);
    let a = source.deref(ctx).get_argument(1);
    let b = source.deref(ctx).get_argument(2);
    let target = BasicBlock::new(ctx, None, vec![ty; 2]);
    let edges = vec![
        SwitchEdgeV3::new(target, vec![a, b]),
        SwitchEdgeV3::new(target, vec![b, a]),
        SwitchEdgeV3::new(target, vec![a, a]),
    ];
    (
        SwitchOpV3::try_new(ctx, selector, SwitchKeyKindAttrV3::U64, vec![0, 1], edges).unwrap(),
        ty,
    )
}

fn assert_both_signature_checks_reject(ctx: &Context, switch: SwitchOpV3) {
    assert!(switch.verify(ctx).is_err());
    assert!(<SwitchOpV3 as BranchOpInterface>::verify(&switch, ctx).is_err());
}

#[test]
fn switch_v3_borrowed_verifiers_reject_every_case_and_default_payload() {
    for width in [32, 64] {
        for position in 1..=6 {
            let ctx = &mut context();
            let (switch, _) = payload_switch(ctx);
            let wrong = integer(ctx, width, true);
            let source = BasicBlock::new(ctx, None, vec![wrong]);
            let value = source.deref(ctx).get_argument(0);
            Operation::replace_operand(switch.get_operation(), ctx, position, value);
            assert_both_signature_checks_reject(ctx, switch);
        }
    }
}

#[test]
fn switch_v3_borrowed_verifiers_reject_every_case_and_default_arity() {
    for arity in [1, 3] {
        for ordinal in 0..3 {
            let ctx = &mut context();
            let (switch, ty) = payload_switch(ctx);
            let wrong = BasicBlock::new(ctx, None, vec![ty; arity]);
            Operation::replace_successor(switch.get_operation(), ctx, ordinal, wrong);
            assert_both_signature_checks_reject(ctx, switch);
        }
    }
}

#[test]
fn switch_v3_borrowed_interface_rejects_malformed_empty_and_nonempty_ranges() {
    for offsets in [
        vec![],
        vec![0],
        vec![1, 1],
        vec![0, 1],
        vec![0, 0, 0],
        vec![2, 1],
    ] {
        let ctx = &mut context();
        let ty = integer(ctx, 128, true);
        let switch = build_empty(ctx, ty, SwitchKeyKindAttrV3::EmptyTyped, vec![]).unwrap();
        switch.get_operation().deref_mut(ctx).attributes.set(
            "gpu_switch_offsets".try_into().unwrap(),
            SwitchSuccessorOffsetsAttrV3(offsets),
        );
        assert_both_signature_checks_reject(ctx, switch);
    }
    for offsets in [
        vec![0, 2, 4],
        vec![0, 2, 4, 6, 6],
        vec![1, 2, 4, 6],
        vec![0, 4, 2, 6],
        vec![0, 2, 4, 7],
        vec![0, 1, 4, 6],
    ] {
        let ctx = &mut context();
        let (switch, _) = payload_switch(ctx);
        switch.get_operation().deref_mut(ctx).attributes.set(
            "gpu_switch_offsets".try_into().unwrap(),
            SwitchSuccessorOffsetsAttrV3(offsets),
        );
        assert_both_signature_checks_reject(ctx, switch);
    }
}

#[test]
fn switch_v3_verification_does_not_request_owned_payload_vectors() {
    use super::interfaces::instrumentation::PAYLOAD_COPIES;
    let ctx = &mut context();
    let (switch, ty) = payload_switch(ctx);
    PAYLOAD_COPIES.with(|count| count.set(0));
    assert_eq!(switch.successor_operands(ctx, 2).len(), 2);
    assert_eq!(PAYLOAD_COPIES.with(|count| count.replace(0)), 1);
    switch.verify(ctx).unwrap();
    <SwitchOpV3 as BranchOpInterface>::verify(&switch, ctx).unwrap();
    switch.verify_interfaces(ctx).unwrap();
    assert_eq!(PAYLOAD_COPIES.with(|count| count.get()), 0);
    let wrong = BasicBlock::new(ctx, None, vec![ty]);
    let target = switch.get_operation().deref(ctx).get_successor(2);
    Operation::replace_successor(switch.get_operation(), ctx, 2, wrong);
    assert_both_signature_checks_reject(ctx, switch);
    Operation::replace_successor(switch.get_operation(), ctx, 2, target);
    let wrong_type = integer(ctx, 64, true);
    let wrong_source = BasicBlock::new(ctx, None, vec![wrong_type]);
    let wrong_value = wrong_source.deref(ctx).get_argument(0);
    Operation::replace_operand(switch.get_operation(), ctx, 6, wrong_value);
    assert_both_signature_checks_reject(ctx, switch);
    assert_eq!(PAYLOAD_COPIES.with(|count| count.get()), 0);
}
