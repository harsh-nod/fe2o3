use super::*;

fn site(block: u32, operation: u32) -> ConditionalPrefixSiteV1 {
    ConditionalPrefixSiteV1 { block, operation }
}

fn repeated_check(extent: usize) -> PrefixModel {
    let mut m = model();
    let PrefixTerminator::LessThan { yes, .. } = &mut m.blocks[2].terminator else {
        unreachable!()
    };
    *yes = 5;
    m.blocks.extend([
        PrefixBlock {
            accesses: vec![],
            terminator: PrefixTerminator::LessThan {
                site: site(5, 0),
                extent,
                yes: 3,
                no: 6,
            },
        },
        PrefixBlock {
            accesses: vec![],
            terminator: PrefixTerminator::Trap(site(6, 0)),
        },
    ]);
    m
}

#[test]
fn contradictory_trap_paths_preserve_every_source_guard_and_small_cfg_semantics() {
    for extent in 0..3 {
        let m = repeated_check(extent);
        let proof = derive_model(&m).unwrap();
        assert_eq!(proof.conditions.len(), 3);
        let [term] = proof.source_guard_dnf.as_slice() else {
            panic!("one exact conjunction")
        };
        assert_eq!(term.len(), 4);
        assert!(term.iter().all(|atom| atom.less_than));
        assert_eq!(
            term.iter()
                .map(|atom| atom.branch)
                .collect::<BTreeSet<_>>()
                .len(),
            4
        );
        assert_eq!(
            term.iter()
                .map(|atom| atom.extent)
                .collect::<BTreeSet<_>>()
                .len(),
            3
        );
        check_small_theorem(&m);

        // A second incoming path without the matching positive guard must
        // reject, even though the original trap path remains contradictory.
        let mut reachable = m.clone();
        let PrefixTerminator::LessThan { no, .. } = &mut reachable.blocks[0].terminator else {
            unreachable!()
        };
        *no = 6;
        assert_eq!(
            derive_model(&reachable).err(),
            Some(ConditionalPrefixDerivationErrorV1::UnsupportedOperation {
                site: site(6, 0),
                operation: "kernel.trap",
            }),
        );
        assert_eq!(
            execute_small_cfg(&reachable, &[0, 1, 1], 1),
            Err("reachable trap")
        );
    }
}

#[test]
fn input_check_traps_are_excluded_only_under_the_recorded_conditions() {
    let expected = derive_model(&model()).unwrap().conditions;
    for input in [1, 2] {
        let mut m = model();
        let PrefixTerminator::LessThan { no, .. } = &mut m.blocks[input].terminator else {
            unreachable!()
        };
        *no = 5;
        m.blocks.push(PrefixBlock {
            accesses: vec![],
            terminator: PrefixTerminator::Trap(site(5, 0)),
        });
        let proof = derive_model(&m).unwrap();
        assert_eq!(proof.conditions, expected);
        assert_eq!(proof.source_guard_dnf[0].len(), 3);
        check_small_theorem(&m);
        let mut lengths = [2, 2, 2];
        lengths[input] = 1;
        assert_eq!(execute_small_cfg(&m, &lengths, 2), Err("reachable trap"));

        let mut used = 0;
        derive_model_with_work(&m, &mut used).unwrap();
        let mut exact = MAX_CONDITIONAL_PREFIX_WORK_V1 - used;
        derive_model_with_work(&m, &mut exact).unwrap();
        assert_eq!(exact, MAX_CONDITIONAL_PREFIX_WORK_V1);
        let mut short = MAX_CONDITIONAL_PREFIX_WORK_V1 - used + 1;
        assert_eq!(
            derive_model_with_work(&m, &mut short).err(),
            Some(ConditionalPrefixDerivationErrorV1::Limit("work"))
        );
    }
}

#[test]
fn reverse_and_input_to_input_inequalities_cannot_exclude_a_trap() {
    for (order, lengths, workitems) in [([1, 0, 2], [1, 2, 2], 2), ([1, 2, 0], [1, 3, 1], 3)] {
        let mut m = permuted_guards(order);
        let PrefixTerminator::LessThan { no, .. } = &mut m.blocks[1].terminator else {
            unreachable!()
        };
        *no = 5;
        m.blocks.push(PrefixBlock {
            accesses: vec![],
            terminator: PrefixTerminator::Trap(site(5, 0)),
        });
        assert!(lengths[0] <= lengths[1] && lengths[0] <= lengths[2] && lengths[0] <= workitems);
        assert_eq!(
            execute_small_cfg(&m, &lengths, workitems),
            Err("reachable trap")
        );
        assert_eq!(
            derive_model(&m).err(),
            Some(ConditionalPrefixDerivationErrorV1::UnsupportedOperation {
                site: site(5, 0),
                operation: "kernel.trap",
            })
        );
    }
}

#[test]
fn live_input_trap_retains_exact_conditions_and_no_authority() {
    let mut context = Context::new();
    let f = production_values::production_values(&mut context);
    let function = &f.base.function;
    let before = derive_pliron_conditional_prefix_coverage_v1(&context, function).unwrap();
    let blocks = function
        .get_region(&context)
        .deref(&context)
        .iter(&context)
        .collect::<Vec<_>>();
    let failed = BasicBlock::new(
        &mut context,
        Some("input_check_failed".try_into().unwrap()),
        vec![],
    );
    failed.insert_at_back(function.get_region(&context), &context);
    let trap = TrapOp::new(&mut context);
    append(&context, failed, &trap);
    let guard = blocks[1].deref(&context).iter(&context).last().unwrap();
    Operation::replace_successor(guard, &context, 1, failed);
    let record = derive_pliron_conditional_prefix_coverage_v1(&context, function).unwrap();
    assert_eq!(record.conditions(), before.conditions());
    assert_eq!(
        record.host_binding_obligations(),
        before.host_binding_obligations()
    );
    assert_eq!(record.source_guard_dnf(), before.source_guard_dnf());
    assert_ne!(record.canonical_bytes(), before.canonical_bytes());
    assert!(!record.proves_unconditional_total_view());
    assert!(!record.grants_launch_authority());
    revalidate_pliron_conditional_prefix_coverage_v1(&context, function, &record).unwrap();
    assert!(revalidate_pliron_conditional_prefix_coverage_v1(&context, function, &before).is_err());
}

#[test]
fn repeated_checks_do_not_allow_negative_store_guards() {
    let mut m = repeated_check(0);
    let PrefixTerminator::LessThan { yes, no, .. } = &mut m.blocks[5].terminator else {
        unreachable!()
    };
    std::mem::swap(yes, no);
    m.blocks[6].terminator = PrefixTerminator::Return;
    assert_eq!(
        derive_model(&m).err(),
        Some(ConditionalPrefixDerivationErrorV1::InexactWriteGuard)
    );
}

#[test]
fn trap_contradiction_checks_share_the_exact_work_budget() {
    let m = repeated_check(0);
    let mut used = 0;
    derive_model_with_work(&m, &mut used).unwrap();
    assert!(used > 0 && used < MAX_CONDITIONAL_PREFIX_WORK_V1);
    let mut exact = MAX_CONDITIONAL_PREFIX_WORK_V1 - used;
    derive_model_with_work(&m, &mut exact).unwrap();
    assert_eq!(exact, MAX_CONDITIONAL_PREFIX_WORK_V1);
    let mut short = MAX_CONDITIONAL_PREFIX_WORK_V1 - used + 1;
    assert_eq!(
        derive_model_with_work(&m, &mut short).err(),
        Some(ConditionalPrefixDerivationErrorV1::Limit("work"))
    );
}

#[test]
fn live_checked_access_trap_requires_exact_contradiction_and_revalidation() {
    let mut context = Context::new();
    let f = production_values::production_values(&mut context);
    let function = &f.base.function;
    let blocks = function
        .get_region(&context)
        .deref(&context)
        .iter(&context)
        .collect::<Vec<_>>();
    let guard_ptr = blocks[0].deref(&context).iter(&context).last().unwrap();
    let guard = Operation::get_op_dyn(guard_ptr, &context);
    let guard = guard.downcast_ref::<IndexLessThanBranchOp>().unwrap();
    let (lhs, rhs) = (guard.lhs(&context), guard.rhs(&context));
    let repeated = BasicBlock::new(
        &mut context,
        Some("repeated_check".try_into().unwrap()),
        vec![],
    );
    repeated.insert_at_back(function.get_region(&context), &context);
    let failed = BasicBlock::new(
        &mut context,
        Some("failed_check".try_into().unwrap()),
        vec![],
    );
    failed.insert_at_back(function.get_region(&context), &context);
    let branch = IndexLessThanBranchOp::new(&mut context, lhs, rhs, blocks[3], failed);
    append(&context, repeated, &branch);
    let trap = TrapOp::new(&mut context);
    append(&context, failed, &trap);
    let predecessor = blocks[2].deref(&context).iter(&context).last().unwrap();
    Operation::replace_successor(predecessor, &context, 0, repeated);

    let record = derive_pliron_conditional_prefix_coverage_v1(&context, function).unwrap();
    assert_eq!(record.conditions().len(), 3);
    assert_eq!(record.source_guard_dnf()[0].len(), 4);
    assert!(!record.proves_unconditional_total_view());
    assert!(!record.grants_launch_authority());
    revalidate_pliron_conditional_prefix_coverage_v1(&context, function, &record).unwrap();

    Operation::replace_successor(guard_ptr, &context, 1, failed);
    assert_eq!(
        derive_pliron_conditional_prefix_coverage_v1(&context, function).err(),
        Some(ConditionalPrefixDerivationErrorV1::UnsupportedOperation {
            site: site(6, 0),
            operation: "kernel.trap",
        }),
    );
    assert!(revalidate_pliron_conditional_prefix_coverage_v1(&context, function, &record).is_err());
}
