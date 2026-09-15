use super::*;

// Private numeric-buffer tests, not source admission or a whole-memory bound.
fn meter(limit: usize) -> PrivateArrayRecorderBudgetV1 {
    PrivateArrayRecorderBudgetV1::new(1, limit).unwrap()
}

fn state<T>(buffer: &PrivateArrayBufferV1<T>) -> (usize, usize, usize, usize) {
    (
        buffer.rows.len(),
        buffer.logical_capacity,
        buffer.admitted_end,
        buffer.capacity_limit,
    )
}

fn work_error(error: ProductionSemanticKirErrorV1, expected: usize, ceiling: usize) {
    assert!(matches!(
        error,
        ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual,
            limit,
        } if actual == expected && limit == ceiling
    ));
}

fn mismatch(error: ProductionSemanticKirErrorV1) {
    assert!(matches!(
        error,
        ProductionSemanticKirErrorV1::CorrespondenceMismatch
    ));
}

fn reserved_one() -> PrivateArrayBufferV1<u8> {
    let mut buffer = PrivateArrayBufferV1::new(1);
    let mut work = meter(17);
    buffer.reserve(1, 1, &mut work).unwrap();
    assert_eq!(work.work.work(), 17);
    buffer
}

#[test]
fn existing_capacity_reservation_has_exact_ten_unit_boundary() {
    for limit in [10, 9] {
        let mut buffer = reserved_one();
        buffer.truncate(0);
        let capacity = buffer.rows.capacity();
        let mut work = meter(limit);
        let result = buffer.reserve(1, 1, &mut work);
        if limit == 10 {
            result.unwrap();
            assert_eq!(work.work.work(), 10);
            assert_eq!(state(&buffer), (0, 1, 1, 1));
        } else {
            work_error(result.unwrap_err(), 10, 9);
            assert_eq!(work.work.work(), 8);
            assert_eq!(work.work.failed_work(), Some(10));
            assert_eq!(state(&buffer), (0, 1, 0, 1));
        }
        assert_eq!(buffer.rows.capacity(), capacity);
    }
}

#[test]
fn fresh_growth_has_exact_seventeen_unit_boundary_before_allocation() {
    for limit in [17, 16] {
        let mut buffer = PrivateArrayBufferV1::<u8>::new(1);
        let mut work = meter(limit);
        let result = buffer.reserve(1, 1, &mut work);
        if limit == 17 {
            result.unwrap();
            assert_eq!(work.work.work(), 17);
            assert_eq!(state(&buffer), (0, 1, 1, 1));
            assert!(buffer.rows.capacity() >= 1);
        } else {
            work_error(result.unwrap_err(), 17, 16);
            assert_eq!(work.work.work(), 15);
            assert_eq!(state(&buffer), (0, 0, 0, 1));
            assert_eq!(buffer.rows.capacity(), 0);
        }
    }
}

#[test]
fn authorized_push_has_exact_three_unit_boundary() {
    for limit in [3, 2] {
        let mut buffer = reserved_one();
        let capacity = buffer.rows.capacity();
        let mut work = meter(limit);
        let result = buffer.push(23, &mut work);
        if limit == 3 {
            result.unwrap();
            assert_eq!(buffer.rows, [23]);
            assert_eq!(work.work.work(), 3);
        } else {
            work_error(result.unwrap_err(), 3, 2);
            assert!(buffer.rows.is_empty());
            assert_eq!(work.work.work(), 2);
            assert_eq!(buffer.admitted_end, 1);
        }
        assert_eq!(buffer.rows.capacity(), capacity);
    }
}

#[test]
fn combined_growth_and_fill_admits_twenty_and_refuses_nineteen() {
    for limit in [20, 19] {
        let mut buffer = PrivateArrayBufferV1::<u8>::new(1);
        let mut work = meter(limit);
        buffer.reserve(1, 1, &mut work).unwrap();
        let result = buffer.push(31, &mut work);
        if limit == 20 {
            result.unwrap();
            assert_eq!(state(&buffer), (1, 1, 1, 1));
            assert_eq!(buffer.rows, [31]);
            assert_eq!(work.work.work(), 20);
        } else {
            work_error(result.unwrap_err(), 20, 19);
            assert_eq!(state(&buffer), (0, 1, 1, 1));
            assert_eq!(work.work.work(), 19);
        }
    }
}

#[test]
fn spare_capacity_does_not_authorize_an_unreserved_push() {
    let mut buffer = PrivateArrayBufferV1::new(8);
    let mut work = meter(62);
    for value in 0..3_u8 {
        buffer
            .reserve(1, usize::from(value) + 1, &mut work)
            .unwrap();
        buffer.push(value, &mut work).unwrap();
    }
    assert_eq!(work.work.work(), 60);
    assert_eq!(state(&buffer), (3, 4, 3, 8));
    mismatch(buffer.push(3, &mut work).unwrap_err());
    assert_eq!(buffer.rows, [0, 1, 2]);
    assert_eq!(state(&buffer), (3, 4, 3, 8));
    assert_eq!(work.work.work(), 62);
    assert!(work.first_denial.is_none());
}

#[test]
fn truncate_keeps_capacity_and_work_but_requires_fresh_authorization() {
    let mut buffer = PrivateArrayBufferV1::new(8);
    let mut work = meter(76);
    for value in 0..3_u8 {
        buffer
            .reserve(1, usize::from(value) + 1, &mut work)
            .unwrap();
        buffer.push(value, &mut work).unwrap();
    }
    let pointer = buffer.rows.as_ptr();
    let capacity = buffer.rows.capacity();
    buffer.truncate(1);
    assert_eq!(state(&buffer), (1, 4, 1, 8));
    assert_eq!(work.work.work(), 60);
    buffer.reserve(2, 3, &mut work).unwrap();
    assert_eq!(work.work.work(), 70);
    buffer.push(8, &mut work).unwrap();
    buffer.push(9, &mut work).unwrap();
    assert_eq!(work.work.work(), 76);
    let rows = buffer.into_rows();
    assert_eq!(rows, [0, 8, 9]);
    assert_eq!(rows.as_ptr(), pointer);
    assert_eq!(rows.capacity(), capacity);
}

#[test]
fn no_op_truncate_discards_unused_admitted_frontier() {
    let mut buffer = PrivateArrayBufferV1::new(8);
    let mut work = meter(22);
    buffer.reserve(3, 3, &mut work).unwrap();
    buffer.push(11_u8, &mut work).unwrap();
    assert_eq!(state(&buffer), (1, 3, 3, 8));
    buffer.truncate(usize::MAX);
    assert_eq!(state(&buffer), (1, 3, 1, 8));
    assert_eq!(work.work.work(), 20);
    mismatch(buffer.push(12, &mut work).unwrap_err());
    assert_eq!(buffer.rows, [11]);
    assert_eq!(work.work.work(), 22);
}

#[test]
fn immutable_capacity_ceiling_truncates_geometric_growth() {
    let mut buffer = PrivateArrayBufferV1::new(3);
    let mut work = meter(67);
    for (value, capacity) in [(0_u8, 1), (1, 2), (2, 3)] {
        buffer
            .reserve(1, usize::from(value) + 1, &mut work)
            .unwrap();
        buffer.push(value, &mut work).unwrap();
        assert_eq!(buffer.logical_capacity, capacity);
        assert_eq!(buffer.capacity_limit, 3);
    }
    assert_eq!(work.work.work(), 60);
    mismatch(buffer.reserve(1, 4, &mut work).unwrap_err());
    assert_eq!(work.work.work(), 67);
    assert_eq!(state(&buffer), (3, 3, 3, 3));
    assert_eq!(buffer.rows, [0, 1, 2]);
}

#[test]
fn zero_capacity_can_reserve_empty_but_cannot_append() {
    let mut buffer = PrivateArrayBufferV1::<u8>::new(0);
    let mut work = meter(12);
    buffer.reserve(0, 0, &mut work).unwrap();
    assert_eq!(work.work.work(), 10);
    assert_eq!(buffer.rows.capacity(), 0);
    mismatch(buffer.push(1, &mut work).unwrap_err());
    assert_eq!(work.work.work(), 12);
    assert_eq!(state(&buffer), (0, 0, 0, 0));
}

#[test]
fn eight_single_row_rounds_have_exact_geometric_132_131_boundary() {
    // Four growth reserves, four reuse reserves, eight pushes: 4*17+4*10+8*3.
    let capacities = [1, 2, 4, 4, 8, 8, 8, 8];
    let prefixes = [20, 40, 60, 73, 93, 106, 119, 132];
    for limit in [132, 131] {
        let mut buffer = PrivateArrayBufferV1::new(8);
        let mut work = meter(limit);
        for index in 0..8 {
            buffer.reserve(1, index + 1, &mut work).unwrap();
            assert_eq!(buffer.logical_capacity, capacities[index]);
            assert_eq!(buffer.admitted_end, index + 1);
            let result = buffer.push(index as u8, &mut work);
            if limit == 131 && index == 7 {
                work_error(result.unwrap_err(), 132, 131);
                assert_eq!(work.work.work(), 131);
                assert_eq!(state(&buffer), (7, 8, 8, 8));
                assert_eq!(buffer.rows, [0, 1, 2, 3, 4, 5, 6]);
            } else {
                result.unwrap();
                assert_eq!(work.work.work(), prefixes[index]);
            }
        }
        if limit == 132 {
            assert_eq!(buffer.rows, [0, 1, 2, 3, 4, 5, 6, 7]);
            assert!(work.first_denial.is_none());
        }
    }
}

#[test]
fn checked_row_count_overflow_precedes_any_reserve() {
    let mut buffer = reserved_one();
    buffer.push(1, &mut meter(3)).unwrap();
    let mut work = meter(1);
    mismatch(buffer.reserve(usize::MAX, 1, &mut work).unwrap_err());
    assert_eq!(work.work.work(), 1);
    assert_eq!(state(&buffer), (1, 1, 1, 1));
    assert!(work.first_denial.is_none());
    let mut denied = meter(0);
    work_error(
        buffer.reserve(usize::MAX, 1, &mut denied).unwrap_err(),
        1,
        0,
    );
    assert_eq!(denied.work.work(), 0);
    assert_eq!(state(&buffer), (1, 1, 1, 1));
}

#[test]
fn typed_byte_overflow_has_exact_preallocation_fourteen_unit_boundary() {
    let count = usize::MAX / std::mem::size_of::<u64>() + 1;
    for limit in [14, 13] {
        let mut buffer = PrivateArrayBufferV1::<u64>::new(count);
        let mut work = meter(limit);
        let error = buffer.reserve(count, count, &mut work).unwrap_err();
        if limit == 14 {
            mismatch(error);
            assert_eq!(work.work.work(), 14);
            assert!(work.first_denial.is_none());
        } else {
            work_error(error, 14, 13);
            assert_eq!(work.work.work(), 13);
        }
        assert_eq!(state(&buffer), (0, 0, 0, count));
        assert_eq!(buffer.rows.capacity(), 0);
    }
}

#[test]
fn impossible_allocation_layout_is_refused_after_seventeen_units() {
    // For u8 this fits usize bytes but exceeds Layout's isize::MAX limit.
    // The pinned RawVec rejects the layout before calling the allocator.
    let count = isize::MAX as usize + 1;
    for limit in [17, 16] {
        let mut buffer = PrivateArrayBufferV1::<u8>::new(count);
        let mut work = meter(limit);
        let error = buffer.reserve(count, count, &mut work).unwrap_err();
        if limit == 17 {
            assert!(matches!(
                error,
                ProductionSemanticKirErrorV1::AllocationFailure {
                    resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                }
            ));
            assert_eq!(work.work.work(), 17);
            assert!(work.first_denial.is_none());
        } else {
            work_error(error, 17, 16);
            assert_eq!(work.work.work(), 15);
        }
        assert_eq!(state(&buffer), (0, 0, 0, count));
        assert_eq!(buffer.rows.capacity(), 0);
    }
}

#[test]
fn first_work_denial_blocks_later_small_and_zero_charges() {
    let mut work = PrivateArrayRecorderBudgetV1::new(2, 5).unwrap();
    assert_eq!(work.work.limit(), 10);
    work.charge_private_array_work(7).unwrap();
    work_error(work.charge_private_array_work(4).unwrap_err(), 11, 10);
    for charge in [1, 0, usize::MAX] {
        work_error(work.charge_private_array_work(charge).unwrap_err(), 11, 10);
        assert_eq!(work.work.work(), 7);
        assert_eq!(work.work.failed_work(), Some(11));
    }
    let first = work.first_denial.unwrap();
    assert_eq!((first.actual(), first.limit()), (11, 10));
    let mut buffer = PrivateArrayBufferV1::<u8>::new(1);
    work_error(buffer.reserve(1, 1, &mut work).unwrap_err(), 11, 10);
    assert_eq!(state(&buffer), (0, 0, 0, 1));
    assert_eq!(buffer.rows.capacity(), 0);
    assert_eq!(work.work.work(), 7);
}

#[test]
fn recorder_meter_rejects_root_times_operation_overflow() {
    match PrivateArrayRecorderBudgetV1::new(2, usize::MAX) {
        Err(error) => mismatch(error),
        Ok(_) => panic!("overflow must not create a recorder meter"),
    }
}

#[test]
fn lazy_phase_defers_zero_and_overflow_limits_until_actual_activation() {
    let mut zero = PrivateArrayLazyBudgetV1::new(1, 0);
    let mut overflow = PrivateArrayLazyBudgetV1::new(2, usize::MAX);
    for phase in [&mut zero, &mut overflow] {
        assert!(phase.active.is_none());
        mismatch(phase.active_limit().unwrap_err());
        mismatch(phase.charge_private_array_work(0).unwrap_err());
        assert!(phase.active.is_none());
    }
    // This exercises private state only, not whole-owner admission with M=0.
    zero.activate().unwrap();
    assert_eq!(zero.active_limit().unwrap(), 0);
    assert_eq!(zero.active.as_ref().unwrap().work.work(), 0);
    zero.charge_private_array_work(0).unwrap();
    work_error(zero.charge_private_array_work(1).unwrap_err(), 1, 0);
    assert_eq!(zero.active.as_ref().unwrap().work.work(), 0);
    for _ in 0..2 {
        mismatch(overflow.activate().unwrap_err());
        assert!(overflow.active.is_none());
        assert_eq!((overflow.roots, overflow.operations), (2, usize::MAX));
    }
}

#[test]
fn repeated_shared_activation_keeps_work_and_the_first_denial() {
    let mut phase = PrivateArrayLazyBudgetV1::new(2, 5);
    {
        let mut first = PrivateArrayRecorderWorkV1::Shared(&mut phase);
        first.activate().unwrap();
        first.charge_private_array_work(7).unwrap();
    }
    assert_eq!(phase.active_limit().unwrap(), 10);
    assert_eq!(phase.active.as_ref().unwrap().work.work(), 7);
    {
        let mut second = PrivateArrayRecorderWorkV1::Shared(&mut phase);
        second.activate().unwrap();
        work_error(second.charge_private_array_work(4).unwrap_err(), 11, 10);
    }
    phase.activate().unwrap();
    work_error(phase.charge_private_array_work(0).unwrap_err(), 11, 10);
    let active = phase.active.as_ref().unwrap();
    assert_eq!(active.work.work(), 7);
    assert_eq!(active.work.failed_work(), Some(11));
    let first = active.first_denial.unwrap();
    assert_eq!((first.actual(), first.limit()), (11, 10));
}

#[test]
fn test_owned_wrapper_uses_the_same_lazy_meter_and_latch() {
    let mut work = PrivateArrayRecorderWorkV1::Owned(PrivateArrayLazyBudgetV1::new(2, 5));
    mismatch(work.charge_private_array_work(0).unwrap_err());
    work.activate().unwrap();
    work.charge_private_array_work(7).unwrap();
    work.activate().unwrap();
    work_error(work.charge_private_array_work(4).unwrap_err(), 11, 10);
    work.activate().unwrap();
    work_error(work.charge_private_array_work(1).unwrap_err(), 11, 10);
    let PrivateArrayRecorderWorkV1::Owned(phase) = work else {
        panic!("test-owned work must retain its own lazy state");
    };
    let active = phase.active.unwrap();
    assert_eq!(active.work.work(), 7);
    assert_eq!(active.work.failed_work(), Some(11));
    assert_eq!(active.work.limit(), 10);
}

#[test]
fn payload_preview_has_exact_five_and_ten_unit_boundaries_without_mutation() {
    for limit in [5, 4] {
        let mut buffer = reserved_one();
        buffer.truncate(0);
        let mut work = meter(limit);
        let result = private_array_buffer_payload_v1(&buffer, 1, &mut work);
        if limit == 5 {
            let payload = result.unwrap();
            assert_eq!((payload.occupied, payload.capacity), (1, 1));
            assert_eq!(work.work.work(), 5);
        } else {
            let error = match result {
                Err(error) => error,
                Ok(_) => panic!("preview must refuse its final two-unit batch"),
            };
            work_error(error, 5, 4);
            assert_eq!(work.work.work(), 3);
        }
        assert_eq!(state(&buffer), (0, 1, 0, 1));
    }
    for limit in [10, 9] {
        let buffer = PrivateArrayBufferV1::<u8>::new(8);
        let mut work = meter(limit);
        let result = private_array_buffer_payload_v1(&buffer, 3, &mut work);
        if limit == 10 {
            let payload = result.unwrap();
            assert_eq!((payload.occupied, payload.capacity), (3, 3));
            assert_eq!(work.work.work(), 10);
        } else {
            let error = match result {
                Err(error) => error,
                Ok(_) => panic!("growth preview must refuse its final two-unit batch"),
            };
            work_error(error, 10, 9);
            assert_eq!(work.work.work(), 8);
        }
        assert_eq!(state(&buffer), (0, 0, 0, 8));
        assert_eq!(buffer.rows.capacity(), 0);
    }
    let mut pending = PrivateArrayBufferV1::<u8>::new(8);
    pending.reserve(3, 3, &mut meter(17)).unwrap();
    let mut work = meter(5);
    let payload = private_array_buffer_payload_v1(&pending, 0, &mut work).unwrap();
    assert_eq!((payload.occupied, payload.capacity), (3, 3));
    assert_eq!(state(&pending), (0, 3, 3, 8));
    assert_eq!(work.work.work(), 5);
}

#[test]
fn payload_add_prepays_both_checked_sums_in_one_two_unit_batch() {
    let left = PrivateArrayPayloadV1 {
        occupied: 7,
        capacity: 11,
    };
    let right = PrivateArrayPayloadV1 {
        occupied: 13,
        capacity: 17,
    };
    let mut exact = meter(2);
    let sum = left.add(right, &mut exact).unwrap();
    assert_eq!((sum.occupied, sum.capacity), (20, 28));
    assert_eq!(exact.work.work(), 2);
    let mut denied = meter(1);
    match left.add(right, &mut denied) {
        Err(error) => work_error(error, 2, 1),
        Ok(_) => panic!("payload sums require their full batch"),
    }
    assert_eq!(denied.work.work(), 0);
    assert_eq!((left.occupied, left.capacity), (7, 11));
    assert_eq!((right.occupied, right.capacity), (13, 17));
    for (left, right) in [
        (
            PrivateArrayPayloadV1 {
                occupied: usize::MAX,
                capacity: 0,
            },
            PrivateArrayPayloadV1 {
                occupied: 1,
                capacity: 0,
            },
        ),
        (
            PrivateArrayPayloadV1 {
                occupied: 0,
                capacity: usize::MAX,
            },
            PrivateArrayPayloadV1 {
                occupied: 0,
                capacity: 1,
            },
        ),
    ] {
        let mut work = meter(2);
        match left.add(right, &mut work) {
            Err(error) => mismatch(error),
            Ok(_) => panic!("coexisting payload overflow must be refused"),
        }
        assert_eq!(work.work.work(), 2);
        assert!(work.first_denial.is_none());
    }
}

#[test]
fn numeric_binary_search_charges_empty_hits_misses_and_each_key_field() {
    let empty: [usize; 0] = [];
    let mut work = meter(1);
    assert_eq!(
        private_array_binary_search_v1(&empty, |row| [*row], [7], &mut work).unwrap(),
        Err(0)
    );
    assert_eq!(work.work.work(), 1);
    let mut denied = meter(0);
    work_error(
        private_array_binary_search_v1(&empty, |row| [*row], [7], &mut denied).unwrap_err(),
        1,
        0,
    );
    for (target, expected, total) in [(7, Ok(0), 5), (6, Err(0), 6), (8, Err(1), 7)] {
        let rows = [7usize];
        let mut work = meter(total);
        assert_eq!(
            private_array_binary_search_v1(&rows, |row| [*row], [target], &mut work).unwrap(),
            expected
        );
        assert_eq!(work.work.work(), total);
        let mut denied = meter(total - 1);
        work_error(
            private_array_binary_search_v1(&rows, |row| [*row], [target], &mut denied).unwrap_err(),
            total,
            total - 1,
        );
        assert_eq!(denied.work.work(), total - 1);
        assert_eq!(rows, [7]);
    }
    let rows = [[7usize, 9]];
    let mut work = meter(6);
    assert_eq!(
        private_array_binary_search_v1(&rows, |row| *row, [7, 9], &mut work).unwrap(),
        Ok(0)
    );
    assert_eq!(work.work.work(), 6);
    let mut denied = meter(5);
    work_error(
        private_array_binary_search_v1(&rows, |row| *row, [7, 9], &mut denied).unwrap_err(),
        6,
        5,
    );
    assert_eq!(denied.work.work(), 5);
}

#[test]
fn in_place_heapsort_has_source_derived_21_16_and_two_field_22_boundaries() {
    // Ascending input needs a heap-building swap and another sift; descending does not.
    for (input, total, denied_prefix) in [([1usize, 2], 21, 17), ([2, 1], 16, 12)] {
        for limit in [total, total - 1] {
            let mut rows = input;
            let mut work = meter(limit);
            let result = private_array_heapsort_v1(
                &mut rows,
                |row| [*row],
                &mut work,
                || ProductionSemanticKirErrorV1::CorrespondenceMismatch,
            );
            if limit == total {
                result.unwrap();
                assert_eq!(work.work.work(), total);
            } else {
                work_error(result.unwrap_err(), total, limit);
                assert_eq!(work.work.work(), denied_prefix);
                work_error(work.charge_private_array_work(0).unwrap_err(), total, limit);
            }
            // The final denied sift is after the extraction swap, not an atomic sort.
            assert_eq!(rows, [1, 2]);
        }
    }
    for limit in [22, 21] {
        let mut rows = [[0usize, 1], [0, 2]];
        let mut work = meter(limit);
        let result = private_array_heapsort_v1(
            &mut rows,
            |row| *row,
            &mut work,
            || ProductionSemanticKirErrorV1::CorrespondenceMismatch,
        );
        if limit == 22 {
            result.unwrap();
            assert_eq!(work.work.work(), 22);
        } else {
            work_error(result.unwrap_err(), 22, 21);
            assert_eq!(work.work.work(), 18);
        }
        assert_eq!(rows, [[0, 1], [0, 2]]);
    }
}

// Borrowed-operation components only; no source admission or output transport is implied.
#[test]
fn counted_allocation_helper_preserves_work_and_first_refusal() {
    let facts = PrivateRetainedSlotFactsV1 {
        element: PrivateRetainedElementFactsV1::Scalar(ScalarType::U32),
        size: 4,
        alignment: 4,
    };
    let allocation = Operation::new(
        vec![ValueDef::new(
            ValueId(11),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        )],
        OperationKind::Alloca {
            element: Type::Scalar(ScalarType::U32),
            count: Some(ValueId(10)),
            address_space: AddressSpace::Private,
            alignment: 4,
        },
    );
    // Shape 2 + identity 4 + scalar 2 + pointer (1 + 2 + scalar 2).
    for (limit, accepted) in [(13, 13), (12, 12)] {
        let mut work = meter(limit);
        let result = private_array_check_allocation_operation_v1(
            &allocation,
            ValueId(11),
            ValueId(10),
            facts,
            &mut work,
        );
        if limit == 13 {
            assert!(result.is_ok());
        } else {
            match result {
                Err(PrivateArrayRelationErrorV1::Work(error)) => work_error(error, 13, 12),
                _ => panic!("allocation must refuse the final scalar comparison"),
            }
        }
        assert_eq!(work.work.work(), accepted);
    }
    let mut malformed = allocation.clone();
    malformed.results.clear();
    malformed.kind = OperationKind::Constant(Constant::U32(0));
    let mut work = meter(2);
    assert!(matches!(
        private_array_check_allocation_operation_v1(
            &malformed,
            ValueId(11),
            ValueId(10),
            facts,
            &mut work
        ),
        Err(PrivateArrayRelationErrorV1::Mismatch(
            "private counted allocation result count changed"
        ))
    ));
    assert_eq!(work.work.work(), 2);

    let mut wrong_pointer = allocation.clone();
    wrong_pointer.results[0].ty = Type::Scalar(ScalarType::U32);
    let mut work = meter(6);
    assert!(matches!(
        private_array_check_allocation_operation_v1(
            &wrong_pointer,
            ValueId(99),
            ValueId(10),
            facts,
            &mut work
        ),
        Err(PrivateArrayRelationErrorV1::Mismatch(
            "private counted allocation identity or access changed"
        ))
    ));
    assert_eq!(work.work.work(), 6);
    let mut work = meter(9);
    assert!(matches!(
        private_array_check_allocation_operation_v1(
            &wrong_pointer,
            ValueId(11),
            ValueId(10),
            facts,
            &mut work
        ),
        Err(PrivateArrayRelationErrorV1::Mismatch(
            "private counted allocation element type changed"
        ))
    ));
    assert_eq!(work.work.work(), 9);
}

#[test]
fn memory_operation_helper_preserves_read_write_work_and_first_refusal() {
    let facts = PrivateRetainedSlotFactsV1 {
        element: PrivateRetainedElementFactsV1::Scalar(ScalarType::U32),
        size: 4,
        alignment: 4,
    };
    let read = Operation::new(
        vec![ValueDef::new(ValueId(21), Type::Scalar(ScalarType::U32))],
        OperationKind::Load {
            pointer: ValueId(20),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        },
    );
    let write = Operation::new(
        Vec::new(),
        OperationKind::Store {
            pointer: ValueId(20),
            value: ValueId(21),
            access: MemoryAccess::new(AddressSpace::Private, 4),
        },
    );
    // Read: dispatch 1 + shape/pointer 2 + scalar 2 + access 3.
    // Write: dispatch 1 + shape/pointer 2 + access 3.
    for (operation, access, total, prefix) in [
        (&read, PrivateArrayAccessV1::Read, 8, 5),
        (&write, PrivateArrayAccessV1::Write, 6, 3),
    ] {
        for limit in [total, total - 1] {
            let mut work = meter(limit);
            let result = private_array_check_memory_operation_v1(
                operation,
                access,
                ValueId(20),
                facts,
                &mut work,
            );
            if limit == total {
                assert!(result.is_ok());
                assert_eq!(work.work.work(), total);
            } else {
                match result {
                    Err(PrivateArrayRelationErrorV1::Work(error)) => {
                        work_error(error, total, limit)
                    }
                    _ => panic!("memory check must refuse its final access batch"),
                }
                assert_eq!(work.work.work(), prefix);
            }
        }
    }
    let mut work = meter(1);
    assert!(matches!(
        private_array_check_memory_operation_v1(
            &write,
            PrivateArrayAccessV1::Read,
            ValueId(99),
            facts,
            &mut work
        ),
        Err(PrivateArrayRelationErrorV1::Mismatch(
            "private array memory effect kind changed"
        ))
    ));
    assert_eq!(work.work.work(), 1);
    let mut wrong_read = read.clone();
    wrong_read.results[0].ty = Type::Scalar(ScalarType::U64);
    let mut work = meter(3);
    assert!(matches!(
        private_array_check_memory_operation_v1(
            &wrong_read,
            PrivateArrayAccessV1::Read,
            ValueId(99),
            facts,
            &mut work
        ),
        Err(PrivateArrayRelationErrorV1::Mismatch(
            "private array load pointer or element changed"
        ))
    ));
    assert_eq!(work.work.work(), 3);
    let mut volatile_write = write;
    let OperationKind::Store { access, .. } = &mut volatile_write.kind else {
        unreachable!();
    };
    access.volatile = true;
    let mut work = meter(6);
    assert!(matches!(
        private_array_check_memory_operation_v1(
            &volatile_write,
            PrivateArrayAccessV1::Write,
            ValueId(20),
            facts,
            &mut work
        ),
        Err(PrivateArrayRelationErrorV1::Mismatch(
            "private array memory access contract changed"
        ))
    ));
    assert_eq!(work.work.work(), 6);
}
