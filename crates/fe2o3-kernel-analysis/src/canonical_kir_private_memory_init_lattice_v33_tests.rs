use super::*;
use crate::canonical_kir_private_memory_v1::physical_cfg::{InitializationState, Initialized};

#[test]
fn definite_initialization_meet_preserves_init_without_inventing_a_writer() {
    use Initialized::{Multiple, Uninitialized, Unique};
    let values = [
        Uninitialized,
        Unique(0),
        Unique(1),
        Unique(usize::MAX),
        Multiple,
    ];
    fn paths(value: Initialized) -> Vec<Option<usize>> {
        match value {
            Initialized::Uninitialized => vec![None],
            Initialized::Unique(writer) => vec![Some(writer)],
            Initialized::Multiple => vec![Some(2), Some(3)],
        }
    }
    for left in values {
        assert_eq!(left.meet(left), left);
        for right in values {
            let result = left.meet(right);
            assert_eq!(result, right.meet(left));
            let concrete: Vec<_> = paths(left).into_iter().chain(paths(right)).collect();
            let initialized = concrete.iter().all(Option::is_some);
            let unique = initialized && concrete.iter().all(|writer| *writer == concrete[0]);
            assert_eq!(result.initialized(), initialized);
            assert_eq!(result.writer(), if unique { concrete[0] } else { None });
            for third in values {
                assert_eq!(left.meet(right).meet(third), left.meet(right.meet(third)));
            }
        }
    }
    assert_eq!(Unique(7).meet(Unique(8)), Multiple);
    assert_eq!(Multiple.meet(Uninitialized), Uninitialized);
    assert_eq!(Initialized::MAX_DESCENTS, 2);
    assert_eq!(size_of::<Initialized>(), size_of::<(usize, usize)>());
}

fn distinct_writers() -> Module {
    let mut other = write(4);
    let OperationKind::Storage(StorageOperationV1::WriteValue { value, .. }) = &mut other.kind
    else {
        unreachable!()
    };
    *value = ValueId(21);
    fixture(
        ScalarType::U32,
        4,
        4,
        vec![
            block(100, vec![allocation(4)], conditional(200, 300)),
            block(200, vec![write(4)], branch(400)),
            block(
                300,
                vec![
                    Operation::effect_free(
                        ValueDef::new(ValueId(21), Type::Scalar(ScalarType::U32)),
                        OperationKind::Constant(Constant::U32(71)),
                    ),
                    other,
                ],
                branch(400),
            ),
            block(400, vec![read(ScalarType::U32, 4)], ret()),
        ],
    )
}

#[test]
fn typed_initialization_diamond_accepts_distinct_values_without_forwarding() {
    with_inventory(&distinct_writers(), |inventory, floor| {
        exercise(inventory, floor, WORK, STORAGE, 1, &[None; 5])
            .0
            .unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let (proof, receipt) = check_canonical_kir_private_memory_v18(
            inventory,
            CanonicalKirPrivateMemoryLimitsV1 { max_cells: 1 },
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(proof.operation(4), "the actual read was checked");
        assert_eq!(proof.latest_stores()[4], None);
        assert!(!proof.grants_authority());
        drop(proof);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn typed_initialization_late_uninitialized_predecessor_revokes_provisional_join() {
    let input = fixture(
        ScalarType::U32,
        4,
        4,
        vec![
            block(100, vec![allocation(4)], conditional(200, 300)),
            block(200, vec![write(4)], branch(600)),
            block(300, vec![], conditional(400, 500)),
            block(400, vec![write(4)], branch(600)),
            block(500, vec![], branch(600)),
            block(600, vec![read(ScalarType::U32, 4)], ret()),
        ],
    );
    with_inventory(&input, |inventory, floor| {
        assert!(matches!(
            exercise(inventory, floor, WORK, STORAGE, 1, &[]).0,
            Err(Error::Unsupported {
                detail: "Load requires initialized storage on every path",
                ..
            })
        ));
    });
}

#[test]
fn typed_initialization_join_uses_exact_and_one_short_work_and_storage() {
    with_inventory(&distinct_writers(), |inventory, floor| {
        let expected = &[None; 5];
        let (result, work, peak) = exercise(inventory, floor, WORK, STORAGE, 1, expected);
        result.unwrap();
        let exact = exercise(inventory, floor, work, peak, 1, expected);
        exact.0.unwrap();
        assert_eq!((exact.1, exact.2), (work, peak));
        assert!(matches!(
            exercise(inventory, floor, work - 1, peak, 1, expected).0,
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert!(matches!(
            exercise(inventory, floor, work, peak - 1, 1, expected).0,
            Err(Error::Resource(Resource::Storage(_)))
        ));
    });
}

#[test]
fn typed_initialization_reset_revokes_both_unique_and_multiple_writers() {
    use crate::canonical_kir_private_memory_v1::physical_cfg::{Event, transfer};
    let events = [
        Event::Reset {
            start: 8,
            length: 2,
        },
        Event::Read(8),
        Event::Read(9),
        Event::Read(10),
    ];
    let mut state = [
        Initialized::Unique(3),
        Initialized::Multiple,
        Initialized::Multiple,
    ];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(3 * events.len() + 2);
    let mut budget = Budget::new(&mut work, 0);
    let mut reads = Vec::new();
    transfer(
        &events,
        0..events.len(),
        8,
        &mut state,
        &mut budget,
        |_, value, _| {
            reads.push(value);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(
        reads,
        vec![
            Initialized::Uninitialized,
            Initialized::Uninitialized,
            Initialized::Multiple
        ]
    );
    assert_eq!(state.as_slice(), reads.as_slice());
    assert_eq!(budget.storage(), 0);
}

#[test]
fn typed_initialization_multiple_functions_keep_independent_domains() {
    let mut input = distinct_writers();
    let mut second = input.functions[0].clone();
    second.id = "second".into();
    input.functions.push(second);
    with_inventory(&input, |inventory, floor| {
        exercise(inventory, floor, WORK, STORAGE, 2, &[None; 10])
            .0
            .unwrap();
    });
}
