//! Synthetic C3 mapping/hash/custody controls only. No authenticated caller is
//! forged, no genuine source pass is claimed, and no detached row is authority.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
};
use std::cell::Cell;
const LIMIT: usize = 1_000_000;
const FLOOR: usize = 37;

fn operation(function: u32, block: u32, ordinal: u32) -> OperationCoordinate {
    OperationCoordinate {
        block: BlockCoordinate {
            function: FunctionCoordinate(function),
            block,
        },
        operation: ordinal,
    }
}
fn ids(start: u32) -> [ValueId; 4] {
    std::array::from_fn(|j| ValueId(start + j as u32))
}
fn synthetic_input() -> BindingInput {
    BindingInput {
        owner_digest: [11; 32],
        owner_length: 1000,
        source: [2, 2, 7, 3, 9, 20, 30, 4],
        canonical: [0, 3, 0, 40, 1, 9, 0, 50, 1, 10, 60],
        input_roots: std::array::from_fn(|j| j as u64 + 100),
        arguments: [ids(0), ids(4), ids(8)],
        formals: [ids(100), ids(104), ids(108)],
        conversion_values: ids(200),
        call_values: ids(300),
        returned: ids(400),
        produced: ids(500),
        permutation: [0, 1, 2, 3],
    }
}

#[test]
fn identity_and_swap01_keep_call_return_and_matrix_namespaces() {
    let call = operation(2, 3, 0);
    let matrix = operation(7, 9, 0);
    let ret = BlockCoordinate {
        function: FunctionCoordinate(7),
        block: 10,
    };
    for permutation in [[0, 1, 2, 3], [1, 0, 2, 3]] {
        let rows = return_rows(call, matrix, ret, ids(0), ids(40), ids(80), permutation).unwrap();
        for j in 0..4 {
            let row = rows[j];
            assert_eq!(
                row.caller_definition(),
                DefinitionCoordinate::Result {
                    operation: call,
                    result: j as u32
                }
            );
            assert_eq!(
                row.helper_return(),
                UseCoordinate::TerminatorOperand {
                    block: ret,
                    operand: j as u32
                }
            );
            assert_eq!(
                row.matrix_definition(),
                DefinitionCoordinate::Result {
                    operation: matrix,
                    result: u32::from(permutation[j])
                }
            );
            assert_eq!(row.caller_value(), ValueId(j as u32));
            assert_eq!(row.return_value(), ValueId(40 + j as u32));
            assert_eq!(row.matrix_value(), ValueId(80 + u32::from(permutation[j])));
            // Actual transport may cross block arguments: equality is NOT
            // required between Return operands and Matrix definitions.
            assert_ne!(row.return_value(), row.matrix_value());
        }
    }
}

#[test]
fn same_numeric_ids_across_distinct_functions_are_not_conflated() {
    let call = operation(2, 3, 0);
    let matrix = operation(7, 3, 0);
    let ret = BlockCoordinate {
        function: FunctionCoordinate(7),
        block: 4,
    };
    let rows = return_rows(call, matrix, ret, ids(0), ids(0), ids(0), [0, 1, 2, 3]).unwrap();
    assert_eq!(rows[0].caller_value(), rows[0].matrix_value());
    assert_ne!(rows[0].caller_definition(), rows[0].matrix_definition());
    assert!(
        return_rows(
            call,
            operation(2, 3, 0),
            ret,
            ids(0),
            ids(4),
            ids(8),
            [0, 1, 2, 3]
        )
        .is_err()
    );
    assert!(
        return_rows(
            call,
            matrix,
            BlockCoordinate {
                function: FunctionCoordinate(2),
                block: 4
            },
            ids(0),
            ids(4),
            ids(8),
            [0, 1, 2, 3]
        )
        .is_err()
    );
}

#[test]
fn unsupported_duplicate_or_out_of_range_permutations_refuse() {
    let call = operation(0, 0, 0);
    let matrix = operation(1, 0, 0);
    let ret = BlockCoordinate {
        function: FunctionCoordinate(1),
        block: 1,
    };
    for permutation in [[0, 0, 2, 3], [0, 1, 2, 4], [2, 1, 0, 3], [3, 2, 1, 0]] {
        assert_eq!(
            return_rows(call, matrix, ret, ids(0), ids(4), ids(8), permutation),
            Err(Error::Unavailable(
                "nominal return permutation outside Identity/Swap01"
            ))
        );
    }
}

#[test]
fn duplicate_component_in_each_namespace_refuses() {
    let call = operation(0, 0, 0);
    let matrix = operation(1, 0, 0);
    let ret = BlockCoordinate {
        function: FunctionCoordinate(1),
        block: 1,
    };
    for bad in 0..3 {
        let mut groups = [ids(0), ids(4), ids(8)];
        groups[bad][3] = groups[bad][0];
        assert_eq!(
            return_rows(
                call,
                matrix,
                ret,
                groups[0],
                groups[1],
                groups[2],
                [0, 1, 2, 3]
            ),
            Err(Error::Unavailable(
                "nominal return repeats a component within its namespace"
            ))
        );
    }
}

#[test]
fn binding_hash_has_distinct_domain_and_changes_for_every_namespace_word() {
    let original = synthetic_input();
    let baseline = binding_digest(&original);
    assert!(!baseline.is_zero());
    assert_eq!(baseline, binding_digest(&original));
    for index in 0..8 {
        let mut changed = original;
        changed.source[index] ^= 1;
        assert_ne!(baseline, binding_digest(&changed));
    }
    for index in 0..11 {
        let mut changed = original;
        changed.canonical[index] ^= 1;
        assert_ne!(baseline, binding_digest(&changed));
    }
    let mut changed = original;
    changed.owner_digest[0] ^= 1;
    assert_ne!(baseline, binding_digest(&changed));
    changed = original;
    changed.owner_length += 1;
    assert_ne!(baseline, binding_digest(&changed));
    assert_ne!(DOMAIN, super::super::TENSOR_CAPABILITY_ROOT_DOMAIN_V1);
    assert_ne!(baseline, super::super::tensor_capability_root_v1(6, &[100]));
}

#[test]
fn binding_hash_retains_inputs_components_and_permutation() {
    let original = synthetic_input();
    let baseline = binding_digest(&original);
    for index in 0..14 {
        let mut changed = original;
        changed.input_roots[index] ^= 1;
        assert_ne!(baseline, binding_digest(&changed));
    }
    for group in 0..3 {
        for index in 0..4 {
            let mut changed = original;
            changed.arguments[group][index].0 ^= 1;
            assert_ne!(baseline, binding_digest(&changed));
            changed = original;
            changed.formals[group][index].0 ^= 1;
            assert_ne!(baseline, binding_digest(&changed));
        }
    }
    for group in 0..4 {
        for index in 0..4 {
            let mut changed = original;
            match group {
                0 => changed.conversion_values[index].0 ^= 1,
                1 => changed.call_values[index].0 ^= 1,
                2 => changed.returned[index].0 ^= 1,
                _ => changed.produced[index].0 ^= 1,
            }
            assert_ne!(baseline, binding_digest(&changed));
        }
    }
    let mut changed = original;
    changed.permutation = [1, 0, 2, 3];
    assert_ne!(baseline, binding_digest(&changed));
}

#[test]
fn scope_exact_and_one_short_boundaries_are_typed() {
    let bytes = scope_storage::<()>().unwrap();
    for (work_limit, storage_limit, expected) in [
        (16, FLOOR + bytes, 0),
        (15, FLOOR + bytes, 1),
        (16, FLOOR + bytes - 1, 2),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(FLOOR).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let entered = Cell::new(false);
        let result = with_owned_scope(&mut budget, |_| {
            entered.set(true);
            Ok(())
        });
        assert!(budget.work_ledger_identity_v1() == identity);
        assert_eq!(budget.storage(), FLOOR);
        match expected {
            0 => {
                assert_eq!(result, Ok(()));
                assert!(entered.get());
                assert_eq!(budget.work(), 16);
                assert_eq!(budget.peak_storage(), FLOOR + bytes);
            }
            1 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert!(!entered.get());
                assert!(budget.failed_work().is_some());
                assert_eq!(budget.failed_storage(), None);
            }
            _ => {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                assert!(!entered.get());
                assert_eq!(budget.failed_work(), None);
                assert!(budget.failed_storage().is_some());
            }
        }
    }
}

#[test]
fn callback_success_error_and_unwind_preserve_extra_storage_and_work() {
    for outcome in 0..3 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let result = with_owned_scope(&mut budget, |budget| {
            budget.reserve_storage(23)?;
            budget.charge_work(17)?;
            match outcome {
                0 => Ok(()),
                1 => Err(Error::Unavailable("synthetic scope refusal")),
                _ => panic!("synthetic C3 scope unwind"),
            }
        });
        assert_eq!(
            result,
            match outcome {
                0 => Ok(()),
                1 => Err(Error::Unavailable("synthetic scope refusal")),
                _ => Err(Error::CallbackPanicked),
            }
        );
        assert!(budget.work_ledger_identity_v1() == identity);
        assert_eq!(budget.storage(), FLOOR + 23);
        assert_eq!(budget.work(), 33);
        assert_eq!(
            (budget.failed_work(), budget.failed_storage()),
            (None, None)
        );
    }
}

#[test]
fn ignored_denial_is_accounting_without_erasing_failure_history() {
    for storage in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let result = with_owned_scope(&mut budget, |budget| {
            if storage {
                let _ = budget.reserve_storage(LIMIT + 1);
            } else {
                let _ = budget.charge_work(LIMIT + 1);
            }
            Ok(())
        });
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage().is_some(), storage);
        assert_eq!(budget.failed_work().is_some(), !storage);
    }
}

#[test]
fn stolen_floor_is_not_repaired_and_foreign_budget_is_not_charged() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let bytes = scope_storage::<()>().unwrap();
    assert_eq!(
        with_owned_scope(&mut budget, |budget| {
            budget.release_storage(1)?;
            Ok(())
        }),
        Err(Resource::Accounting.into())
    );
    assert_eq!(budget.storage(), FLOOR + bytes - 1);

    let mut original_work = Work::new(LIMIT);
    let mut foreign_work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut original_work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let replacement = Budget::new(&mut foreign_work, LIMIT);
    assert_eq!(
        with_owned_scope(&mut budget, |budget| {
            *budget = replacement;
            Ok(())
        }),
        Err(Resource::Accounting.into())
    );
    assert!(budget.work_ledger_identity_v1() != identity);
    assert_eq!(
        (budget.storage(), budget.work(), budget.peak_storage()),
        (0, 0, 0)
    );
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
}

#[test]
fn preexisting_denial_refuses_before_body_and_no_receipt_is_manufactured() {
    let mut work = Work::new(1);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert!(budget.charge_work(2).is_err());
    let result = with_owned_scope(&mut budget, |_| -> Result<()> {
        panic!("sticky denial must not enter C3")
    });
    assert_eq!(result, Err(Resource::Accounting.into()));
    assert_eq!((budget.storage(), budget.work()), (0, 0));
    assert_eq!(budget.failed_work(), Some(2));
}

#[test]
fn callback_explicit_resource_and_payload_drop_are_preserved() {
    for error in [
        Resource::Allocation,
        Resource::Arithmetic,
        Resource::Accounting,
    ] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let result = with_owned_scope(&mut budget, |_| -> Result<()> { Err(error.into()) });
        assert_eq!(result, Err(Error::Resource(error)));
        assert_eq!(budget.storage(), 0);
    }
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct DropFlag(Arc<AtomicBool>);
    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let dropped = Arc::new(AtomicBool::new(false));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let result = with_owned_scope(&mut budget, |_| -> Result<()> {
        std::panic::panic_any(DropFlag(dropped.clone()));
    });
    assert_eq!(result, Err(Error::CallbackPanicked));
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn fixed_digest_stack_and_scratch_are_finite() {
    assert!(size_of::<BindingInput>() < 1024);
    assert!(size_of::<NominalTensorOccurrenceV1<'static>>() < 1024);
    assert!(size_of::<Sha256>() < 512);
    assert!(scope_storage::<()>().unwrap() < 8192);
    // Pure hash input is fixed arity; no function names/graphs/vector backing
    // are cloned or serialized by C3.
    let serialized_bytes =
        8 + DOMAIN.len() + 32 + 8 + 4 + 8 * 4 + 11 * 4 + 14 * 8 + 6 * 4 * 4 + 4 * 4 * 4 + 4;
    assert!(serialized_bytes < DIGEST_WORK);
    assert_eq!(MAX_HELPER_BLOCKS, 32);
}

#[test]
fn helper_block_scan_bound_rejects_excess_and_inconsistent_ranges() {
    assert_eq!(require_helper_block_bound(32, 9, 41), Ok(()));
    for (count, start, end) in [(33, 9, 42), (32, 9, 40), (1, 2, 1)] {
        assert_eq!(
            require_helper_block_bound(count, start, end),
            Err(Error::Unavailable(
                "nominal layout helper Return scan exceeds closed block bound"
            ))
        );
    }
}
