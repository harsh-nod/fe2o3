//! Isolated synthetic state/ledger/CFG controls, not genuine same-owner source
//! qualification. No sealed candidate, nominal facts or caller origin is forged.
use super::super::tensor_capability_read_v1::CapabilityStateReadV1;
use super::super::{
    ProjectedCapabilityOriginV1, ProjectedCapabilityStateV1, SemanticLocalIdV1, SemanticOperandV1,
    SemanticPlaceV1, SemanticTypeIdV1, consume_capability_operand_v1,
};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

const LIMIT: usize = 8 * 1024 * 1024;
struct SyntheticConsumer<'b, 'w> {
    budget: &'b mut Budget<'w>,
}
impl NominalCapabilityConsumerV1 for SyntheticConsumer<'_, '_> {
    fn charge_work_v1(&mut self, amount: usize) -> Result<()> {
        self.budget.charge_work(amount).map_err(Error::Resource)
    }
    fn reserve_storage_v1(&mut self, amount: usize) -> Result<()> {
        self.budget.reserve_storage(amount).map_err(Error::Resource)
    }
    fn release_storage_v1(&mut self, amount: usize) -> Result<()> {
        self.budget.release_storage(amount).map_err(Error::Resource)
    }
    fn ledger_v1(&self) -> NominalCapabilityLedgerV1 {
        NominalCapabilityLedgerV1 {
            slot: self.budget as *const Budget<'_> as usize,
            identity: self.budget.work_ledger_identity_v1(),
            work: self.budget.work(),
            storage: self.budget.storage(),
            peak: self.budget.peak_storage(),
            denied_work: self.budget.failed_work().is_some(),
            denied_storage: self.budget.failed_storage().is_some(),
        }
    }
    fn with_nominal_call_v1(
        &mut self,
        _: usize,
        _: &SemanticDirectCallV1,
        _: SemanticSourceProvenanceV1,
        _: &mut NominalCallVisitorV1<'_>,
    ) -> Result<()> {
        Err(Error::Unavailable(
            "synthetic consumer has no actual nominal candidate",
        ))
    }
}
fn known(root: u64) -> ProjectedCapabilityValueV1 {
    ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::MatrixContext { root })
}
fn operand(local: u32, moved: bool) -> SemanticOperandV1 {
    let place = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![],
        SemanticTypeIdV1::from_index(0),
    )
    .unwrap();
    if moved {
        SemanticOperandV1::Move(place)
    } else {
        SemanticOperandV1::Copy(place)
    }
}
#[test]
fn synthetic_dense_get_insert_remove_matches_legacy_without_growth() {
    let mut slots = [None; 4];
    let mut dense = DenseCapabilityStateV1::new(&mut slots);
    let mut legacy = ProjectedCapabilityStateV1::new();
    for (local, value) in [(3, known(13)), (0, known(10)), (3, known(23))] {
        assert_eq!(dense.insert(local, value), legacy.insert(local, value));
        assert_eq!(dense.get(&local).copied(), legacy.get(&local).copied());
    }
    assert_eq!(dense.remove(&3), legacy.remove(&3));
    for local in 0..4 {
        assert_eq!(dense.contains_key(&local), legacy.contains_key(&local));
        assert_eq!(
            dense.capability_value_v1(local),
            legacy.get(&local).copied()
        );
    }
    assert_eq!(
        dense.precharged_values_v1().copied().collect::<Vec<_>>(),
        vec![known(10)]
    );
    dense.check_bounds_v1().unwrap();
}

#[test]
fn synthetic_dense_out_of_range_is_sticky_structural_not_missing_lattice() {
    for operation in 0..4 {
        let mut slots = [None; 2];
        let mut dense = DenseCapabilityStateV1::new(&mut slots);
        assert_eq!(dense.get(&1), None);
        dense.check_bounds_v1().unwrap();
        match operation {
            0 => {
                assert_eq!(dense.get(&2), None);
            }
            1 => {
                assert_eq!(dense.insert(2, known(1)), None);
            }
            2 => {
                assert_eq!(dense.remove(&2), None);
            }
            _ => {
                assert!(!dense.contains_key(&2));
            }
        }
        dense.insert(0, known(10));
        assert_eq!(
            dense.check_bounds_v1(),
            Err(Error::Unavailable(
                "nominal capability local is outside its dense source table"
            ))
        );
    }
}

#[test]
fn synthetic_shared_move_and_copy_consumption_remains_byte_equivalent_state() {
    for moved in [false, true] {
        let mut slots = [
            Some(known(10)),
            None,
            Some(ProjectedCapabilityValueV1::Invalid),
        ];
        let mut dense = DenseCapabilityStateV1::new(&mut slots);
        let mut legacy = ProjectedCapabilityStateV1::from([
            (0, known(10)),
            (2, ProjectedCapabilityValueV1::Invalid),
        ]);
        let operand = operand(0, moved);
        consume_capability_operand_v1(&mut dense, &operand);
        consume_capability_operand_v1(&mut legacy, &operand);
        for local in 0..3 {
            assert_eq!(dense.get(&local), legacy.get(&local));
        }
        dense.check_bounds_v1().unwrap();
    }
}

#[test]
fn synthetic_dense_merge_preserves_invalid_absence_and_exact_shared_lattice() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut consumer = SyntheticConsumer {
        budget: &mut budget,
    };
    let mut count = 0;
    let mut existing = [Some(known(10)), Some(known(20)), None, None];
    let incoming = [Some(known(10)), None, Some(known(30)), None];
    assert_eq!(
        merge_row(&mut existing, &incoming, &mut consumer, &mut count).unwrap(),
        1
    );
    assert_eq!(
        existing,
        [
            Some(known(10)),
            Some(ProjectedCapabilityValueV1::Invalid),
            Some(ProjectedCapabilityValueV1::Invalid),
            None
        ]
    );
    assert_eq!(count, 12);
    let mut reverse = incoming;
    merge_row(
        &mut reverse,
        &[Some(known(10)), Some(known(20)), None, None],
        &mut consumer,
        &mut count,
    )
    .unwrap();
    assert_eq!(reverse, existing);
    let mut incompatible = [Some(known(99))];
    merge_row(
        &mut incompatible,
        &[Some(known(100))],
        &mut consumer,
        &mut count,
    )
    .unwrap();
    assert_eq!(incompatible, [Some(ProjectedCapabilityValueV1::Invalid)]);
}

#[test]
fn synthetic_merge_denial_happens_before_any_successor_mutation() {
    let mut work = Work::new(5);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut consumer = SyntheticConsumer {
        budget: &mut budget,
    };
    let mut count = 0;
    let before = [Some(known(1)), None];
    let mut row = before;
    assert!(matches!(
        merge_row(&mut row, &[None, Some(known(2))], &mut consumer, &mut count),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(row, before);
    assert!(budget.failed_work().is_some());
}

fn synthetic_order(edges: &[u32]) -> Result<Vec<usize>> {
    let mut indegree = vec![0u8; edges.len()];
    for &bits in edges {
        for (target, degree) in indegree.iter_mut().enumerate() {
            *degree += u8::from(bits & (1u32 << target) != 0);
        }
    }
    let mut order = Vec::new();
    order.try_reserve_exact(edges.len()).unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut consumer = SyntheticConsumer {
        budget: &mut budget,
    };
    order_cfg(edges, &mut indegree, &mut order, &mut consumer, &mut 0)?;
    Ok(order)
}
#[test]
fn synthetic_topology_uses_real_edges_not_numeric_block_order() {
    // 3 -> 1 -> 0 -> 2. Numeric source indices are deliberately not topological.
    assert_eq!(
        synthetic_order(&[1 << 2, 1 << 0, 0, 1 << 1]).unwrap(),
        vec![3, 1, 0, 2]
    );
    assert_eq!(synthetic_order(&[0, 0, 0]).unwrap(), vec![0, 1, 2]);
}
#[test]
fn synthetic_topology_rejects_cycles_and_mismatched_tables() {
    assert_eq!(
        synthetic_order(&[1 << 1, 1 << 0]),
        Err(Error::Unavailable(
            "nominal dense source CFG contains a cycle"
        ))
    );
    assert_eq!(
        synthetic_order(&[1]),
        Err(Error::Unavailable(
            "nominal dense source CFG contains a cycle"
        ))
    );
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut consumer = SyntheticConsumer {
        budget: &mut budget,
    };
    let mut order = Vec::with_capacity(2);
    assert!(order_cfg(&[0], &mut [0], &mut order, &mut consumer, &mut 0).is_err());
}

#[test]
fn synthetic_shape_bounds_and_checked_arithmetic_never_allocate_max_locals() {
    for (blocks, locals) in [(0, 1), (33, 1), (1, 0), (1, 4097)] {
        assert!(shape::<()>(blocks, locals).is_err());
    }
    let small = shape::<()>(2, 3).unwrap();
    assert_eq!(small.slots, 6);
    assert!(small.total < shape::<()>(2, 4).unwrap().total);
    assert_eq!(
        mul(usize::MAX, 2),
        Err(Error::Resource(Resource::Arithmetic))
    );
    assert_eq!(
        add(usize::MAX, 1),
        Err(Error::Resource(Resource::Arithmetic))
    );
    assert_eq!(
        require_capacity(5, 4),
        Err(Error::Resource(Resource::Allocation))
    );
}

#[test]
fn synthetic_full_frame_exact_storage_and_one_byte_short() {
    let s = shape::<()>(2, 3).unwrap();
    for short in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, 23 + s.total - usize::from(short));
        budget.reserve_storage(23).unwrap();
        budget.charge_work(7).unwrap();
        let mut consumer = SyntheticConsumer {
            budget: &mut budget,
        };
        let called = Cell::new(false);
        let result = with_owned_storage(&mut consumer, s.total, |consumer, _| {
            called.set(true);
            let mut count = 0;
            let frames = Frames::new(s, consumer, &mut count)?;
            assert_eq!(frames.initial.as_ref().unwrap().capacity(), 6);
            assert_eq!(frames.repeated.capacity(), 6);
            assert_eq!(frames.scratch.capacity(), 3);
            assert_eq!(frames.order.capacity(), 2);
            assert!(frames.pipeline_payloads.is_empty());
            drop(frames);
            Ok(())
        });
        if short {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
            assert!(!called.get());
            assert_eq!(budget.work(), 7);
        } else {
            assert_eq!(result, Ok(()));
            assert!(called.get());
            assert_eq!(budget.peak_storage(), 23 + s.total);
        }
        assert_eq!(budget.storage(), 23);
    }
}

#[test]
fn synthetic_first_pass_payload_is_dropped_before_its_exact_refund() {
    let s = shape::<u8>(2, 3).unwrap();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(23).unwrap();
    let mut consumer = SyntheticConsumer {
        budget: &mut budget,
    };
    let result = with_owned_storage(&mut consumer, s.total, |consumer, owned| {
        let mut frames = Frames::new(s, consumer, &mut 0)?;
        let before = consumer.ledger_v1().storage;
        drop(frames.initial.take());
        drop(frames.initial_reached.take());
        consumer.release_storage_v1(s.first_payload)?;
        *owned -= s.first_payload;
        assert_eq!(consumer.ledger_v1().storage, before - s.first_payload);
        assert!(frames.initial.is_none() && frames.initial_reached.is_none());
        assert_eq!(frames.repeated.len(), s.slots);
        consumer.reserve_storage_v1(5)?; // callback-owned, not refunded by driver
        drop(frames);
        Ok(9)
    });
    assert_eq!(result, Ok(9));
    assert_eq!(budget.storage(), 28);
    assert!(budget.peak_storage() >= 23 + s.total);
}

#[test]
fn synthetic_scope_error_and_panic_drop_before_owned_only_refund() {
    let dropped = Cell::new(false);
    struct DropFlag<'a>(&'a Cell<bool>);
    impl Drop for DropFlag<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    for panic in [false, true] {
        dropped.set(false);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(23).unwrap();
        let mut consumer = SyntheticConsumer {
            budget: &mut budget,
        };
        let result = with_owned_storage::<()>(&mut consumer, 47, |consumer, _| {
            let _owned = DropFlag(&dropped);
            consumer.reserve_storage_v1(5)?;
            consumer.charge_work_v1(3)?;
            if panic {
                panic!("synthetic dense callback panic");
            }
            Err(Error::Unavailable("synthetic callback error"))
        });
        assert!(dropped.get());
        assert_eq!(
            result,
            Err(if panic {
                Error::CallbackPanicked
            } else {
                Error::Unavailable("synthetic callback error")
            })
        );
        assert_eq!(budget.storage(), 28);
        assert_eq!(budget.work(), 3);
        assert_eq!(budget.peak_storage(), 75);
    }
}

#[test]
fn synthetic_scope_ignored_work_or_storage_denial_cannot_complete() {
    for storage in [false, true] {
        let mut work = Work::new(3);
        let mut budget = Budget::new(&mut work, 23 + 47);
        budget.reserve_storage(23).unwrap();
        let mut consumer = SyntheticConsumer {
            budget: &mut budget,
        };
        let result = with_owned_storage(&mut consumer, 47, |consumer, _| {
            if storage {
                let _ = consumer.reserve_storage_v1(1);
            } else {
                let _ = consumer.charge_work_v1(4);
            }
            Ok(17u8)
        });
        assert_eq!(result, Err(Error::Resource(Resource::Accounting)));
        assert_eq!(budget.storage(), 23);
        assert!(budget.failed_work().is_some() || budget.failed_storage().is_some());
    }
}

#[test]
fn synthetic_scope_floor_debit_is_not_repaired_with_a_blanket_refund() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(23).unwrap();
    let mut consumer = SyntheticConsumer {
        budget: &mut budget,
    };
    assert_eq!(
        with_owned_storage(&mut consumer, 47, |consumer, _| {
            consumer.release_storage_v1(48)?;
            Ok(())
        }),
        Err(Error::Resource(Resource::Accounting))
    );
    assert_eq!(budget.storage(), 22);
    assert_eq!(budget.peak_storage(), 70);
}

#[test]
fn synthetic_two_propagations_remain_distinct_from_final_replay_observations() {
    assert_eq!(NominalCapabilityPassV1::Initial.index(), 0);
    assert_eq!(NominalCapabilityPassV1::Repeated.index(), 1);
    assert_eq!(NominalCapabilityPassV1::Final.index(), 2);
    let run = NominalCapabilityRunV1 {
        query_visits: [1, 1, 0],
        authenticated_visits: [1, 1, 0],
        reached_blocks: [4, 4, 0],
        array_destination_has_origin: false,
    };
    assert_eq!(run.authenticated_visits[2], 0);
    // Inert counts cannot construct AuthenticatedNominalCaller or a C3 occurrence.
}
