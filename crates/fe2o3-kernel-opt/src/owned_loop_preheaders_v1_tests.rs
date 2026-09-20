use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate, ComparePredicate, Constant, Function,
    IntegerSwitchCase, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Operation,
    OperationKind as Kind, ScalarType, Signature, SwitchCase,
};
#[path = "loop_preheaders_fixture_v1_tests.rs"]
mod fixture;
use fixture::*;
#[path = "loop_preheaders_resources_v1_tests.rs"]
mod resource_tests;
#[path = "loop_preheaders_sim_v1_tests.rs"]
mod simulation;

#[test]
fn actual_duplicate_conditional_switch_integer_and_distinct_edges_preserve_typed_arguments() {
    for mode in [
        Incoming::Conditional,
        Incoming::Switch,
        Incoming::IntegerSwitch,
        Incoming::Distinct,
    ] {
        with_input(fixture(mode), |input, budget| {
            let floor = budget.storage();
            let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(owner.retained_storage()).unwrap();
            let old = &input.module().functions[0].body.as_ref().unwrap().blocks;
            let new = &owner.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks;
            assert_eq!(
                owner.preheaders(),
                [Row {
                    header: coordinate(0, 1),
                    preheader: coordinate(0, old.len() as u32)
                }]
            );
            assert_eq!(new.len(), old.len() + 1);
            assert_eq!(
                &new[1..4],
                &old[1..4],
                "header, latch and effects are untouched"
            );
            let preheader = new.last().unwrap();
            assert!(preheader.operations.is_empty());
            assert_eq!(
                preheader
                    .parameters
                    .iter()
                    .map(|p| p.id)
                    .collect::<Vec<_>>(),
                [ValueId(205), ValueId(206)]
            );
            assert_eq!(preheader.terminator, Some(branch(50, &[205, 206])));
            let mut seen = 0;
            for (before, after) in old.iter().zip(new) {
                let mut previous = Vec::new();
                before
                    .terminator
                    .as_ref()
                    .unwrap()
                    .try_visit_edges_v1(|target, args| {
                        previous.push((target, args.to_vec()));
                        Ok::<_, ()>(())
                    })
                    .unwrap();
                let mut cursor = 0;
                after
                    .terminator
                    .as_ref()
                    .unwrap()
                    .try_visit_edges_v1(|target, args| {
                        let (original_target, original_args) = &previous[cursor];
                        assert_eq!(args, original_args);
                        let external = before.id != BlockId(80) && *original_target == BlockId(50);
                        assert_eq!(
                            target,
                            if external {
                                preheader.id
                            } else {
                                *original_target
                            }
                        );
                        seen += usize::from(external);
                        cursor += 1;
                        Ok::<_, ()>(())
                    })
                    .unwrap();
            }
            assert_eq!(
                seen,
                if matches!(mode, Incoming::Switch | Incoming::IntegerSwitch) {
                    3
                } else {
                    2
                }
            );
            replay(&owner, input, budget);
            assert!(!owner.grants_authority());
            let again = prepare_owned_loop_preheaders_v1(owner.output(), budget).unwrap();
            budget.reserve_storage(again.retained_storage()).unwrap();
            assert!(again.preheaders().is_empty());
            assert_eq!(
                again.output().canonical().canonical_bytes(),
                owner.output().canonical().canonical_bytes()
            );
            replay(&again, owner.output(), budget);
            release(again, budget);
            release(owner, budget);
        });
    }
}

#[test]
fn nested_and_multiple_functions_keep_original_coordinates_and_refresh_outer_members() {
    let mut module = nested();
    let mut other = module.functions[0].clone();
    other.id = "other".into();
    module.functions.push(other);
    with_input(module, |input, budget| {
        let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(
            owner.preheaders(),
            [
                Row {
                    header: coordinate(0, 1),
                    preheader: coordinate(0, 6)
                },
                Row {
                    header: coordinate(0, 2),
                    preheader: coordinate(0, 7)
                },
                Row {
                    header: coordinate(1, 1),
                    preheader: coordinate(1, 6)
                },
                Row {
                    header: coordinate(1, 2),
                    preheader: coordinate(1, 7)
                },
            ]
        );
        let (inventory, is) = Inventory::derive(owner.output(), budget).unwrap();
        budget.reserve_storage(is.retained_storage()).unwrap();
        let (loops, ls) = Loops::derive(&inventory, Limits::default(), budget).unwrap();
        budget.reserve_storage(ls.retained_storage()).unwrap();
        loops.replay(&inventory, Limits::default(), budget).unwrap();
        assert_eq!(loops.loop_count(), 4);
        assert!(
            loops
                .members(0, budget)
                .unwrap()
                .contains(&coordinate(0, 7))
        );
        assert!(
            loops
                .members(2, budget)
                .unwrap()
                .contains(&coordinate(1, 7))
        );
        for index in 0..4 {
            assert_eq!(
                loops
                    .natural_loop(index, budget)
                    .unwrap()
                    .unconditional_preheader()
                    .unwrap()
                    .source,
                owner.preheaders()[index].preheader
            );
        }
        drop(loops);
        budget.release_storage(ls.retained_storage()).unwrap();
        drop(inventory);
        budget.release_storage(is.retained_storage()).unwrap();
        replay(&owner, input, budget);
        release(owner, budget);
    });
}

#[test]
fn existing_entry_unreachable_and_external_side_entry_shapes_are_exact_noops() {
    let existing = fixture(Incoming::Branch);
    let mut entry_header = empty_loop();
    blocks(&mut entry_header).remove(0);
    let mut unreachable = empty_loop();
    blocks(&mut unreachable)[0].terminator = Some(Terminator::Return { values: vec![] });
    let mut side_entry = fixture(Incoming::Conditional);
    blocks(&mut side_entry).push(block(120, branch(80, &[])));
    for module in [existing, entry_header, unreachable, side_entry] {
        with_input(module, |input, budget| {
            let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
            budget.reserve_storage(owner.retained_storage()).unwrap();
            assert!(owner.preheaders().is_empty());
            assert_eq!(
                owner.output().canonical().canonical_bytes(),
                input.canonical().canonical_bytes()
            );
            assert!(!std::ptr::eq(owner.output(), input));
            replay(&owner, input, budget);
            release(owner, budget);
        });
    }
}

#[test]
fn unreachable_incoming_occurrence_is_redirected_without_deduplication() {
    let mut module = fixture(Incoming::Conditional);
    blocks(&mut module).push(block(120, branch(50, &[2, 1])));
    with_input(module, |input, budget| {
        let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(owner.preheaders().len(), 1);
        let new = &owner.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks;
        assert_eq!(new[4].terminator, Some(branch(121, &[2, 1])));
        replay(&owner, input, budget);
        release(owner, budget);
    });
}

#[test]
fn nested_boxed_parameter_types_and_equal_typed_positions_are_retained_exactly() {
    let mut ty = Type::Scalar(ScalarType::U32);
    for _ in 0..8 {
        ty = Type::pointer(ty, AddressSpace::Private, AccessMode::ReadWrite);
    }
    let mut module = empty_loop();
    module.functions[0]
        .signature
        .parameters
        .extend([ty.clone(), ty.clone()]);
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .extend([ValueId(1), ValueId(2)]);
    // Replace the maximal bool parameter so this test needs fresh value IDs.
    module.functions[0].body.as_mut().unwrap().parameters[0] = ValueId(0);
    let body = blocks(&mut module);
    body[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(9),
        then_arguments: vec![ValueId(1), ValueId(2), ValueId(0)],
        else_target: BlockId(9),
        else_arguments: vec![ValueId(2), ValueId(1), ValueId(0)],
    });
    body[1].parameters = vec![
        ValueDef::new(ValueId(10), ty.clone()),
        ValueDef::new(ValueId(11), ty.clone()),
        ValueDef::new(ValueId(12), Type::BOOL),
    ];
    body[1].terminator = Some(conditional(0, 12, 30));
    body[2].terminator = Some(branch(9, &[10, 11, 12]));
    with_input(module, |input, budget| {
        let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        let new = owner.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks
            .last()
            .unwrap();
        assert_eq!(
            new.parameters,
            [
                ValueDef::new(ValueId(13), ty.clone()),
                ValueDef::new(ValueId(14), ty.clone()),
                ValueDef::new(ValueId(15), Type::BOOL)
            ]
        );
        assert_eq!(new.terminator, Some(branch(9, &[13, 14, 15])));
        replay(&owner, input, budget);
        release(owner, budget);
    });
}

#[test]
fn sparse_maximum_ids_only_fail_when_the_corresponding_new_ids_are_needed() {
    with_input(empty_loop(), |input, budget| {
        let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert_eq!(
            owner.preheaders().len(),
            1,
            "no new parameter means no max ValueId increment"
        );
        replay(&owner, input, budget);
        release(owner, budget);
    });
    let mut module = empty_loop();
    blocks(&mut module)[3].id = BlockId(u32::MAX);
    blocks(&mut module)[1].terminator = Some(conditional(u32::MAX, 12, u32::MAX));
    with_input(module.clone(), |input, budget| {
        assert!(matches!(
            prepare_owned_loop_preheaders_v1(input, budget),
            Err(Error::Resource(Resource::Arithmetic))
        ));
    });
    blocks(&mut module)[0].terminator = Some(branch(9, &[]));
    with_input(module, |input, budget| {
        let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        assert!(owner.preheaders().is_empty());
        replay(&owner, input, budget);
        release(owner, budget);
    });
    let mut values_full = fixture(Incoming::Conditional);
    blocks(&mut values_full)[3].operations[0].results[0].id = ValueId(u32::MAX);
    if let Kind::Store { value, .. } = &mut blocks(&mut values_full)[3].operations[1].kind {
        *value = ValueId(u32::MAX);
    }
    with_input(values_full, |input, budget| {
        assert!(matches!(
            prepare_owned_loop_preheaders_v1(input, budget),
            Err(Error::Resource(Resource::Arithmetic))
        ));
    });
}

fn reject_changed(input: &Owner, output: &Module, rows: &[Row], budget: &mut Budget<'_>) {
    let (changed, receipt) = admit(output);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    assert!(
        check_canonical_kir_loop_preheaders_v1(input, &changed, rows, Limits::default(), budget)
            .is_err()
    );
    assert_eq!(budget.storage(), floor);
    drop(changed);
    budget.release_storage(receipt.retained_storage()).unwrap();
}

#[test]
fn checker_rejects_mutated_external_internal_arguments_controls_metadata_and_nonempty_preheaders() {
    with_input(fixture(Incoming::Conditional), |input, budget| {
        let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        for mode in 0..8 {
            let mut changed = owner.output().module().clone();
            match mode {
                0 => {
                    if let Some(Terminator::ConditionalBranch { then_target, .. }) =
                        &mut blocks(&mut changed)[0].terminator
                    {
                        *then_target = BlockId(50);
                    }
                }
                1 => {
                    if let Some(Terminator::ConditionalBranch { else_arguments, .. }) =
                        &mut blocks(&mut changed)[0].terminator
                    {
                        else_arguments.swap(0, 1);
                    }
                }
                2 => {
                    if let Some(Terminator::Branch { target, .. }) =
                        &mut blocks(&mut changed)[2].terminator
                    {
                        *target = BlockId(91);
                    }
                }
                3 => {
                    if let Some(Terminator::Branch { arguments, .. }) =
                        &mut blocks(&mut changed)[4].terminator
                    {
                        arguments.swap(0, 1);
                    }
                }
                4 => blocks(&mut changed)[4]
                    .operations
                    .push(Operation::effect_free(
                        ValueDef::new(ValueId(300), Type::Scalar(ScalarType::U32)),
                        Kind::Constant(Constant::U32(7)),
                    )),
                5 => {
                    if let Kind::Compare { predicate, .. } =
                        &mut blocks(&mut changed)[1].operations[0].kind
                    {
                        *predicate = ComparePredicate::GreaterThan;
                    }
                }
                6 => changed.id = "foreign-module".into(),
                _ => {
                    blocks(&mut changed)[4].parameters.swap(0, 1);
                    if let Some(Terminator::Branch { arguments, .. }) =
                        &mut blocks(&mut changed)[4].terminator
                    {
                        arguments.swap(0, 1);
                    }
                }
            }
            reject_changed(input, &changed, owner.preheaders(), budget);
        }
        assert!(
            check_canonical_kir_loop_preheaders_v1(
                input,
                owner.output(),
                &[],
                Limits::default(),
                budget
            )
            .is_err()
        );
        let mut rows = owner.preheaders().to_vec();
        rows[0].header.block = 2;
        assert!(
            check_canonical_kir_loop_preheaders_v1(
                input,
                owner.output(),
                &rows,
                Limits::default(),
                budget
            )
            .is_err()
        );
        release(owner, budget);
    });
}

#[test]
fn checker_rejects_switch_case_mutations_and_reordered_complete_rows_or_appended_blocks() {
    for incoming in [Incoming::Switch, Incoming::IntegerSwitch] {
        with_input(fixture(incoming), |input, budget| {
            let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
            budget.reserve_storage(owner.retained_storage()).unwrap();
            let mut changed = owner.output().module().clone();
            match blocks(&mut changed)[0].terminator.as_mut().unwrap() {
                Terminator::Switch { cases, .. } => cases[1].value = 2,
                Terminator::IntegerSwitch { cases, .. } => cases[1].value = Constant::U32(2),
                _ => unreachable!(),
            }
            reject_changed(input, &changed, owner.preheaders(), budget);
            release(owner, budget);
        });
    }
    with_input(nested(), |input, budget| {
        let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
        budget.reserve_storage(owner.retained_storage()).unwrap();
        let mut rows = owner.preheaders().to_vec();
        rows.swap(0, 1);
        assert!(
            check_canonical_kir_loop_preheaders_v1(
                input,
                owner.output(),
                &rows,
                Limits::default(),
                budget
            )
            .is_err()
        );
        let mut changed = owner.output().module().clone();
        blocks(&mut changed).swap(6, 7);
        reject_changed(input, &changed, owner.preheaders(), budget);
        release(owner, budget);
    });
}

#[test]
fn malformed_raw_graphs_cannot_be_admitted_as_a_pair_endpoint() {
    let mut module = fixture(Incoming::Conditional);
    blocks(&mut module)[1].terminator = None;
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(SIBLING).unwrap();
    assert!(Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).is_err());
    assert_eq!(budget.storage(), SIBLING);
}

#[test]
fn full_identity_and_actual_pair_replay_remain_required_after_input_drop() {
    let mut saved = None;
    with_input(fixture(Incoming::Conditional), |input, budget| {
        let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
        assert_eq!(
            owner.retained_storage(),
            header().unwrap()
                + owner.output_storage.retained_storage()
                + owner.rows.capacity() * size_of::<Row>()
        );
        saved = Some(owner);
    });
    let mut owner = saved.unwrap();
    with_input(fixture(Incoming::Conditional), |input, budget| {
        budget.reserve_storage(owner.retained_storage()).unwrap();
        replay(&owner, input, budget);
        let mut foreign = input.module().clone();
        foreign.id = "foreign".into();
        let (foreign, receipt) = admit(&foreign);
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(matches!(
            owner.replay_against(&foreign, budget),
            Err(Error::ForeignInput)
        ));
        owner.input_identity = *foreign.canonical().identity();
        assert!(matches!(
            owner.replay_against(&foreign, budget),
            Err(Error::Pair(_))
        ));
        drop(foreign);
        budget.release_storage(receipt.retained_storage()).unwrap();
        release(owner, budget);
    });
}
