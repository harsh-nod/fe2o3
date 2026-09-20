use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    Constant, Function, MemoryAccess, ScalarType, Signature, Terminator, Type, ValueDef,
};
#[path = "private_cell_promotion_fixture_v1_tests.rs"]
mod fixture;
use fixture::*;
#[path = "private_cell_promotion_effects_v1_tests.rs"]
mod effects;
#[path = "private_cell_promotion_failure_v1_tests.rs"]
mod failures;
#[path = "private_cell_promotion_process_v1_tests.rs"]
mod process;

#[test]
fn actual_all_width_scalar_and_array_factories_preserve_dynamic_values_and_inputs() {
    for scalar in TYPES {
        for count in [None, Some(1), Some(32)] {
            with_input(fixture(scalar, count), |input, budget| {
                let before = input.canonical().canonical_bytes().to_vec();
                let floor = budget.storage();
                let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(owned.retained_storage()).unwrap();
                assert_ne!(owned.output().canonical().canonical_bytes(), before);
                assert_eq!(owned.selected_allocations(), [coord(1)]);
                assert_eq!(
                    owned.origins(),
                    [
                        Origin {
                            input: coord(0),
                            output: coord(0),
                            kind: OriginKind::Retained
                        },
                        Origin {
                            input: coord(2),
                            output: coord(1),
                            kind: OriginKind::Retained
                        },
                        Origin {
                            input: coord(6),
                            output: coord(2),
                            kind: OriginKind::LoadCopy {
                                allocation: coord(1),
                                previous_store: coord(5),
                                stored_value: ValueId(1)
                            }
                        }
                    ]
                );
                let original = &input.module().functions[0].body.as_ref().unwrap().blocks[0]
                    .operations[6]
                    .results;
                assert_eq!(
                    &owned.output().module().functions[0]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks[0]
                        .operations[2]
                        .results,
                    original
                );
                let bits = scalar.bit_width().unwrap();
                let mask = (1u128 << bits) - 1;
                for x in [0, 1, mask, 1u128 << (bits - 1)] {
                    for y in [0, mask, x ^ mask] {
                        assert_eq!(evaluate(input.module(), x, y), y);
                        assert_eq!(evaluate(owned.output().module(), x, y), y);
                    }
                }
                replay(&owned, input, budget);
                assert!(!owned.grants_authority());
                assert_eq!(input.canonical().canonical_bytes(), before);
                release(owned, budget);
            });
        }
    }
}

#[test]
fn sparse_count_has_identical_cost_and_exact_distinct_cell_store() {
    let mut expected = None;
    for count in [2, 1 << 16, 1 << 32, 2] {
        let mut module = fixture(ScalarType::U32, Some(count));
        ops(&mut module)[2] = constant(12, count - 1);
        with_input(module, |input, budget| {
            let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
            let observed = (
                budget.work(),
                budget.peak_storage(),
                owned.retained_storage(),
                owned.origins().to_vec(),
            );
            assert!(
                matches!(owned.origins()[2].kind,OriginKind::LoadCopy {previous_store,stored_value:ValueId(0),..}if previous_store==coord(4))
            );
            if let Some(prior) = &expected {
                assert_eq!(&observed, prior);
            } else {
                expected = Some(observed);
            }
            budget.reserve_storage(owned.retained_storage()).unwrap();
            replay(&owned, input, budget);
            release(owned, budget);
        });
    }
}

#[test]
fn copy_chain_keeps_actual_results_and_does_not_add_dce() {
    let mut module = fixture(ScalarType::U32, None);
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    ops(&mut module).push(Operation::new(
        vec![],
        Kind::Store {
            pointer: ValueId(11),
            value: ValueId(14),
            access,
        },
    ));
    ops(&mut module).push(Operation::effect_free(
        ValueDef::new(ValueId(15), Type::Scalar(ScalarType::U32)),
        Kind::Load {
            pointer: ValueId(11),
            access,
        },
    ));
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(15)],
    });
    with_input(module, |input, budget| {
        let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
        budget.reserve_storage(owned.retained_storage()).unwrap();
        assert_eq!(owned.origins().len(), 4);
        assert!(
            matches!(owned.origins()[3].kind,OriginKind::LoadCopy {stored_value:ValueId(14),previous_store,..}if previous_store==coord(7))
        );
        assert_eq!(evaluate(owned.output().module(), 23, 91), 91);
        assert!(matches!(
            owned.output().module().functions[0]
                .body
                .as_ref()
                .unwrap()
                .blocks[0]
                .operations[0]
                .kind,
            Kind::Constant(_)
        ));
        replay(&owned, input, budget);
        release(owned, budget);
    });
}

#[test]
fn unused_eligible_addresses_are_removed_and_noop_remains_a_real_owned_output() {
    let mut module = fixture(ScalarType::U32, Some(32));
    ops(&mut module).truncate(4);
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    with_input(module, |input, budget| {
        let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
        budget.reserve_storage(owned.retained_storage()).unwrap();
        assert_eq!(owned.origins().len(), 2);
        assert!(
            owned
                .origins()
                .iter()
                .all(|r| r.kind == OriginKind::Retained)
        );
        replay(&owned, input, budget);
        let again = prepare_owned_private_cell_promotion_v1(owned.output(), budget).unwrap();
        budget.reserve_storage(again.retained_storage()).unwrap();
        assert!(again.selected_allocations().is_empty());
        assert_eq!(
            again.output().canonical().canonical_bytes(),
            owned.output().canonical().canonical_bytes()
        );
        assert!(!std::ptr::eq(again.output(), owned.output()));
        replay(&again, owned.output(), budget);
        release(again, budget);
        release(owned, budget);
    });
}

#[test]
fn reversed_physical_layout_preserves_original_and_final_coordinates() {
    let mut module = fixture(ScalarType::U32, None);
    let old = std::mem::take(ops(&mut module));
    let body = module.functions[0].body.as_mut().unwrap();
    let mut uses = BasicBlock::new(BlockId(17));
    uses.operations = old[4..].to_vec();
    uses.terminator = Some(Terminator::Return {
        values: vec![ValueId(14)],
    });
    let mut allocation = BasicBlock::new(BlockId(23));
    allocation.operations = old[..4].to_vec();
    allocation.terminator = Some(Terminator::Branch {
        target: BlockId(17),
        arguments: vec![],
    });
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(23),
        arguments: vec![],
    });
    body.blocks.extend([uses, allocation]);
    with_input(module, |input, budget| {
        let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
        budget.reserve_storage(owned.retained_storage()).unwrap();
        assert_eq!(owned.selected_allocations(), [location(0, 2, 1)]);
        assert_eq!(owned.origins()[0].input, location(0, 1, 2));
        assert_eq!(owned.origins()[0].output, location(0, 1, 0));
        replay(&owned, input, budget);
        release(owned, budget);
    });
}

#[test]
fn every_eligible_function_is_selected_once_even_with_repeated_local_ids() {
    let mut module = fixture(ScalarType::U32, None);
    let mut second = module.functions[0].clone();
    second.id = "other".into();
    module.functions.push(second);
    with_input(module, |input, budget| {
        let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
        budget.reserve_storage(owned.retained_storage()).unwrap();
        assert_eq!(owned.selected_allocations(), [coord(1), location(1, 0, 1)]);
        assert_eq!(owned.origins().len(), 6);
        assert_eq!(owned.origins()[5].input, location(1, 0, 6));
        replay(&owned, input, budget);
        release(owned, budget);
    });
}

#[test]
fn actual_output_receipt_capacities_and_post_input_drop_replay_stay_distinct() {
    let mut saved = None;
    with_input(fixture(ScalarType::U32, None), |input, budget| {
        let owned = prepare_owned_private_cell_promotion_v1(input, budget).unwrap();
        assert_eq!(
            owned.retained_storage(),
            owned.output_storage.retained_storage()
                + header().unwrap()
                + capacity(&owned.selected).unwrap()
                + capacity(&owned.origins).unwrap()
        );
        assert_eq!(owned.input_identity(), input.canonical().identity());
        saved = Some(owned);
    });
    let owned = saved.unwrap();
    let bytes = owned.output().canonical().canonical_bytes().to_vec();
    with_input(fixture(ScalarType::U32, None), |new_input, budget| {
        budget.reserve_storage(owned.retained_storage()).unwrap();
        replay(&owned, new_input, budget);
        assert_eq!(owned.output().canonical().canonical_bytes(), bytes);
        release(owned, budget);
    });
}
