#[test]
fn presburger_relations_prove_disjoint_effects_beyond_the_trace_limit() {
    let context = &mut setup();
    let function = function(context, "presburger_large_disjoint");
    let entry = function.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, 65_537);
    let memory = view(context, vec![131_074], MemorySpaceAttr::Global);
    let two = IndexConstantOp::new(context, 2);
    let one = IndexConstantOp::new(context, 1);
    let even = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Multiply,
        invocation.result(context),
        two.result(context),
    );
    let odd = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        even.result(context),
        one.result(context),
    );
    let write = access(
        context,
        AccessKindAttr::Write,
        memory.result(context),
        even.result(context),
    );
    let read = access(
        context,
        AccessKindAttr::Read,
        memory.result(context),
        odd.result(context),
    );
    let ret = ReturnOp::new(context);
    for operation in [
        invocation.get_operation(),
        memory.get_operation(),
        two.get_operation(),
        one.get_operation(),
        even.get_operation(),
        odd.get_operation(),
        write.get_operation(),
        read.get_operation(),
        ret.get_operation(),
    ] {
        operation.insert_at_back(entry, context);
    }

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert!(report.is_clean(), "{:#?}", report.findings());
}

#[test]
fn affine_residue_family_proves_eight_blocked_stores_beyond_the_trace_limit() {
    let formulas = (0..8).map(|offset| (8, offset)).collect::<Vec<_>>();
    let report = large_affine_store_family(&formulas);
    assert!(report.is_clean(), "{:#?}", report.findings());
}

#[test]
fn affine_residue_family_checks_every_pair_and_rejects_collisions() {
    for formulas in [
        vec![(8, 0), (8, 1), (8, 9)],
        vec![(8, 0), (8, 8)],
        vec![(8, 0), (4, 0)],
    ] {
        let report = large_affine_store_family(&formulas);
        assert!(!report.is_clean(), "hostile formulas passed: {formulas:?}");
    }
}

#[test]
fn oversized_presburger_pair_inventory_fails_closed_before_pair_enumeration() {
    let context = &mut setup();
    let function = function(context, "presburger_pair_budget");
    let entry = function.get_entry_block(context);
    let invocation = InvocationIndexOp::new(context, 0, 65_537);
    let memory = view(context, vec![1], MemorySpaceAttr::Global);
    let zero = IndexConstantOp::new(context, 0);
    append(context, entry, &invocation);
    append(context, entry, &memory);
    append(context, entry, &zero);
    for _ in 0..1_448 {
        let write = RankedAccessOp::new(
            context,
            AccessKindAttr::Write,
            memory.result(context),
            vec![zero.result(context)],
        )
        .unwrap();
        append(context, entry, &write);
    }
    let ret = ReturnOp::new(context);
    append(context, entry, &ret);

    let report = run_pliron_ranked_race_check_v1(context, &function);
    assert!(matches!(
        report.findings(),
        [RankedRaceFindingV1::LaunchDomainTooLarge {
            invocations: 65_537,
            ..
        }]
    ));
}
