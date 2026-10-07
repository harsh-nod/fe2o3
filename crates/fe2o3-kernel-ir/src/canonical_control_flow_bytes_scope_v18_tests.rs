//! Byte-accounted V18 scope tests; LegacyRows comparisons concern answers, not units.
use super::*;
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Constant,
    Function as KirFunction, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutLimitsV1, StorageLayoutV1,
    StorageOperationV1, Terminator, Type, ValueDef, ValueId,
};
use std::{cell::Cell, mem::size_of};

const LIMIT: usize = 100_000_000;
const FLOOR: usize = 43;

fn block(block: u32) -> Block {
    Block {
        function: Function(0),
        block,
    }
}

fn owner() -> Owner18 {
    let mut module = Module::new("byte-scoped-storage-cfg");
    module.storage_layouts.push(StorageLayoutV1 {
        size: 8,
        alignment: 8,
        kind: StorageLayoutKindV1::Scalar(ScalarType::U64),
    });
    let make = |id, terminator| {
        let mut row = BasicBlock::new(BlockId(id));
        row.terminator = Some(terminator);
        row
    };
    let mut entry = make(
        91,
        Terminator::ConditionalBranch {
            condition: ValueId(103),
            then_target: BlockId(7),
            then_arguments: vec![],
            else_target: BlockId(303),
            else_arguments: vec![],
        },
    );
    entry.operations = vec![
        Operation::new(
            vec![ValueDef::new(
                ValueId(11),
                Type::pointer(
                    Type::StorageObject(StorageLayoutIdV1(0)),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            )],
            OperationKind::Alloca {
                element: Type::StorageObject(StorageLayoutIdV1(0)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 8,
            },
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(29), Type::Scalar(ScalarType::U64))],
            OperationKind::Constant(Constant::U64(17)),
        ),
        Operation::new(
            vec![],
            OperationKind::Storage(StorageOperationV1::WriteValue {
                address: ValueId(11),
                value: ValueId(29),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            }),
        ),
        Operation::new(
            vec![ValueDef::new(ValueId(47), Type::Scalar(ScalarType::U64))],
            OperationKind::Storage(StorageOperationV1::ReadValue {
                address: ValueId(11),
                access: MemoryAccess::new(AddressSpace::Private, 8),
            }),
        ),
    ];
    module.functions.push(KirFunction::internal_helper(
        "entry",
        Signature::new(vec![Type::BOOL], vec![Type::Scalar(ScalarType::U64)]),
        vec![ValueId(103)],
        vec![
            entry,
            make(
                7,
                Terminator::Branch {
                    target: BlockId(28),
                    arguments: vec![],
                },
            ),
            make(
                303,
                Terminator::Branch {
                    target: BlockId(28),
                    arguments: vec![],
                },
            ),
            make(
                28,
                Terminator::Return {
                    values: vec![ValueId(47)],
                },
            ),
            make(
                1001,
                Terminator::Branch {
                    target: BlockId(1001),
                    arguments: vec![],
                },
            ),
        ],
    ));
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, _) = Owner18::from_module_ref_with_verification_budget_v18(
        &module,
        StorageLayoutLimitsV1 {
            rows: 64,
            edges: 256,
            containment_depth: 32,
            object_bytes: 4096,
        },
        &mut budget,
    )
    .unwrap();
    assert_eq!(owner.module(), &module);
    owner
}

fn prepared(work: &mut Work, storage: usize) -> Budget<'_> {
    let mut budget = Budget::new(work, storage);
    budget.reserve_storage(FLOOR).unwrap();
    budget
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Facts {
    reachable: [bool; 5],
    dominates: [[bool; 5]; 5],
    reducible: bool,
}

fn facts(
    view: &mut CanonicalKirControlFlowViewV18<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<Facts, Error> {
    let mut result = Facts::default();
    for index in 0..5 {
        result.reachable[index] = view.is_reachable(block(index as u32), budget)?;
        for other in 0..5 {
            result.dominates[index][other] =
                view.dominates(block(index as u32), block(other as u32), budget)?;
        }
    }
    result.reducible = view.is_reducible(budget)?;
    Ok(result)
}

#[derive(Clone, Copy)]
enum Units {
    TypedBytes,
    LegacyRows,
}

fn run(
    owner: &Owner18,
    units: Units,
    queries: bool,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<Facts, Error>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let mut work = Work::new(work_limit);
    let mut budget = prepared(&mut work, storage_limit);
    let callback = |view: &mut CanonicalKirControlFlowViewV18<'_, '_>, budget: &mut Budget<'_>| {
        assert!(std::ptr::eq(view.owner(), owner));
        assert_eq!(view.function(), Function(0));
        if queries {
            facts(view, budget)
        } else {
            Ok(Facts::default())
        }
    };
    let result = match units {
        Units::TypedBytes => crate::with_canonical_kir_control_flow_bytes_v18(
            owner,
            Function(0),
            Default::default(),
            &mut budget,
            callback,
        ),
        Units::LegacyRows => crate::with_canonical_kir_control_flow_v18(
            owner,
            Function(0),
            Default::default(),
            &mut budget,
            callback,
        ),
    };
    assert_eq!(budget.storage(), FLOOR);
    (
        result,
        budget.work(),
        budget.peak_storage(),
        budget.failed_work(),
        budget.failed_storage(),
    )
}

#[test]
fn cfg_bytes_v18_preserves_owner_storage_graph_and_legacy_query_answers() {
    let owner = owner();
    let original = owner.canonical_bytes().to_vec();
    let bytes = run(&owner, Units::TypedBytes, true, LIMIT, LIMIT);
    let rows = run(&owner, Units::LegacyRows, true, LIMIT, LIMIT);
    let expected = Facts {
        reachable: [true, true, true, true, false],
        dominates: [
            [true, true, true, true, false],
            [false, true, false, false, false],
            [false, false, true, false, false],
            [false, false, false, true, false],
            [false, false, false, false, false],
        ],
        reducible: true,
    };
    assert_eq!(bytes.0, Ok(expected.clone()));
    assert_eq!(rows.0, Ok(expected));
    assert_eq!(owner.canonical_bytes(), original);
    // These peaks have different units. Neither is an RSS measurement, and
    // equality of graph answers does not authorize reusing one as the other.
    assert!(bytes.2 > FLOOR);
    assert!(rows.2 > FLOOR);
}

#[test]
fn cfg_bytes_v18_exact_and_one_short_limits_keep_failure_history_and_floor() {
    let owner = owner();
    for queries in [false, true] {
        let full = run(&owner, Units::TypedBytes, queries, LIMIT, LIMIT);
        assert!(full.0.is_ok());
        assert_eq!((full.3, full.4), (None, None));
        let exact = run(&owner, Units::TypedBytes, queries, full.1, full.2);
        assert_eq!(exact, full);
        let short_work = run(&owner, Units::TypedBytes, queries, full.1 - 1, full.2);
        let Err(Error::Resource(Resource::Work(error))) = short_work.0 else {
            panic!("exact work boundary");
        };
        assert_eq!((error.actual(), error.limit()), (full.1, full.1 - 1));
        assert_eq!(short_work.3, Some(full.1));
        let short_storage = run(&owner, Units::TypedBytes, queries, full.1, full.2 - 1);
        let Err(Error::Resource(Resource::Storage(error))) = short_storage.0 else {
            panic!("exact byte boundary");
        };
        assert_eq!((error.actual(), error.limit()), (full.2, full.2 - 1));
        assert_eq!(short_storage.4, Some(full.2));
    }
}

fn scope_header_oracle<T, E, R>(run: &R) -> usize {
    // Independent simultaneous scope-owner, return/unwind and callback census.
    type ByteOwner = super::super::ByteAccountedIndexedControlFlowV2<'static, 'static>;
    size_of::<Accounting>()
        + size_of::<Option<MeteredIndexedControlFlowV1>>()
        + size_of::<CanonicalKirControlFlowViewV1<'static, 'static>>()
        + size_of::<std::thread::Result<Result<Result<T, E>, Error>>>()
        + size_of::<Option<Result<T, E>>>()
        + size_of::<Option<Error>>()
        + size_of::<[Option<Box<dyn std::any::Any + Send>>; 3]>()
        + 2 * size_of::<Option<R>>()
        + 2 * size_of::<Result<MeteredIndexedControlFlowV1, MeteredControlFlowErrorV1>>()
        + size_of::<ByteOwner>()
        + size_of::<Result<ByteOwner, MeteredControlFlowErrorV1>>()
        + 12 * size_of::<&()>()
        + 4 * size_of::<usize>()
        + size_of::<ControlFlowLimits>()
        + size_of::<Function>()
        + size_of::<ControlFlowStorageV2>()
        + size_of::<bool>()
        + 2 * std::mem::size_of_val(run)
        + std::mem::align_of_val(run)
}

#[test]
fn cfg_bytes_v18_scope_adds_named_headers_to_actual_typed_cfg_charge() {
    let owner = owner();
    let function = &owner.module().functions[0];
    let mut raw_work = Work::new(LIMIT);
    let mut raw_budget = prepared(&mut raw_work, LIMIT);
    let graph =
        analyze_control_flow_with_byte_budget_v2(function, Default::default(), &mut raw_budget)
            .unwrap();
    assert!(graph.indexed_v2(function, &raw_budget).is_ok());
    let raw_retained = raw_budget.storage() - FLOOR;
    let raw_peak = raw_budget.peak_storage();
    let raw_work = raw_budget.work();
    graph.release(&mut raw_budget).unwrap();
    assert_eq!(raw_budget.storage(), FLOOR);

    let observed = Cell::new(0);
    let callback = |_: &mut CanonicalKirControlFlowViewV18<'_, '_>, budget: &mut Budget<'_>| {
        observed.set(budget.storage());
        Ok::<_, Error>(())
    };
    let header = scope_header_oracle::<(), Error, _>(&callback);
    assert_eq!(
        byte_scope_headers::<(), Error, _>(&callback).unwrap(),
        header
    );
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    assert_eq!(
        crate::with_canonical_kir_control_flow_bytes_v18(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            callback
        ),
        Ok(())
    );
    assert_eq!(observed.get(), FLOOR + header + raw_retained);
    assert_eq!(budget.peak_storage(), raw_peak + header);
    assert_eq!(budget.work(), raw_work + 3);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn cfg_bytes_v18_foreign_slot_and_ledger_poison_without_charging_or_refunding_them() {
    let owner = owner();
    for wrong_slot in [false, true] {
        let mut original_work = Work::new(LIMIT);
        let mut foreign_work = Work::new(LIMIT);
        let mut budget = prepared(&mut original_work, LIMIT);
        let mut spare = prepared(&mut foreign_work, LIMIT);
        let mut retained = 0;
        let result = crate::with_canonical_kir_control_flow_bytes_v18(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            |view, budget| {
                retained = budget.storage() - FLOOR;
                std::mem::swap(budget, &mut spare);
                let target = if wrong_slot { &mut spare } else { &mut *budget };
                let before = (target.work(), target.storage());
                assert_eq!(
                    view.is_reachable(block(0), target),
                    Err(Resource::Accounting.into())
                );
                assert_eq!((target.work(), target.storage()), before);
                std::mem::swap(budget, &mut spare);
                Ok::<_, Error>(())
            },
        );
        assert_eq!(result, Err(Resource::Accounting.into()));
        assert!(retained > 0);
        assert_eq!(budget.storage(), FLOOR + retained);
        assert_eq!((spare.work(), spare.storage()), (0, FLOOR));
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn cfg_bytes_v18_callback_errors_panics_and_excess_preserve_exact_outer_floor() {
    let owner = owner();
    for mode in 0..4 {
        let mut work = Work::new(LIMIT);
        let mut budget = prepared(&mut work, LIMIT);
        let result = crate::with_canonical_kir_control_flow_bytes_v18(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            |view, budget| -> Result<(), Error> {
                assert!(view.is_reachable(block(0), budget)?);
                if mode >= 2 {
                    budget.reserve_storage(17)?;
                }
                match mode {
                    0 => Err(Error::InvalidFunction(Function(9))),
                    1 | 3 => panic!("byte CFG callback"),
                    _ => Ok(()),
                }
            },
        );
        assert_eq!(
            result,
            Err(match mode {
                0 => Error::InvalidFunction(Function(9)),
                1 => Error::Panicked,
                _ => Resource::Accounting.into(),
            })
        );
        assert_eq!(budget.storage(), FLOOR + if mode >= 2 { 17 } else { 0 });
        if mode >= 2 {
            budget.release_storage(17).unwrap();
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn cfg_bytes_v18_payload_unwind_occurs_after_owned_storage_refund() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("byte CFG payload destructor");
        }
    }
    let owner = owner();
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    let dropped = Arc::new(AtomicUsize::new(0));
    let result = catch_unwind(AssertUnwindSafe(|| {
        crate::with_canonical_kir_control_flow_bytes_v18(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            |_, _| -> Result<(), Error> { std::panic::panic_any(Payload(dropped.clone())) },
        )
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
}

#[test]
fn cfg_bytes_v18_abandoned_callback_drop_preserves_first_resource_error() {
    use std::rc::Rc;
    struct Capture(Rc<Cell<usize>>);
    impl Drop for Capture {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
            panic!("abandoned callback capture");
        }
    }
    let owner = owner();
    for work_denial in [false, true] {
        let dropped = Rc::new(Cell::new(0));
        let reached = Rc::new(Cell::new(false));
        let mark = reached.clone();
        let capture = Capture(dropped.clone());
        let callback = move |_: &mut CanonicalKirControlFlowViewV18<'_, '_>, _: &mut Budget<'_>| {
            mark.set(true);
            drop(capture);
            Ok::<_, Error>(())
        };
        let header = scope_header_oracle::<(), Error, _>(&callback);
        let work_limit = if work_denial { 0 } else { LIMIT };
        let storage_limit = if work_denial {
            LIMIT
        } else {
            FLOOR + header - 1
        };
        let mut work = Work::new(work_limit);
        let mut budget = prepared(&mut work, storage_limit);
        let result = crate::with_canonical_kir_control_flow_bytes_v18(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            callback,
        );
        assert!(!reached.get());
        assert_eq!(dropped.get(), 1);
        assert_eq!(budget.storage(), FLOOR);
        if work_denial {
            let Err(Error::Resource(Resource::Work(error))) = result else {
                panic!("capture panic replaced first work denial");
            };
            assert_eq!((error.actual(), error.limit()), (3, 0));
            assert_eq!(budget.failed_work(), Some(3));
        } else {
            let Err(Error::Resource(Resource::Storage(error))) = result else {
                panic!("capture panic replaced first header denial");
            };
            assert_eq!(
                (error.actual(), error.limit()),
                (FLOOR + header, storage_limit)
            );
            assert_eq!(budget.failed_storage(), Some(FLOOR + header));
        }
    }
}

#[test]
fn cfg_bytes_v18_abandoned_capture_payload_drops_only_after_floor_refund() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Payload(Arc<AtomicUsize>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("abandoned capture payload destructor");
        }
    }
    struct Capture(Arc<AtomicUsize>);
    impl Drop for Capture {
        fn drop(&mut self) {
            std::panic::panic_any(Payload(self.0.clone()));
        }
    }
    let owner = owner();
    let dropped = Arc::new(AtomicUsize::new(0));
    let reached = Cell::new(false);
    let capture = Capture(dropped.clone());
    let callback = |_: &mut CanonicalKirControlFlowViewV18<'_, '_>, _: &mut Budget<'_>| {
        reached.set(true);
        drop(capture);
        Ok::<_, Error>(())
    };
    let header = scope_header_oracle::<(), Error, _>(&callback);
    // Admit the scope header, then deny the byte builder's first work unit.
    let mut work = Work::new(3);
    let mut budget = prepared(&mut work, LIMIT);
    let result = catch_unwind(AssertUnwindSafe(|| {
        crate::with_canonical_kir_control_flow_bytes_v18(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            callback,
        )
    }));
    assert!(result.is_err());
    assert!(!reached.get());
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(budget.peak_storage(), FLOOR + header);
    assert_eq!(budget.failed_work(), Some(4));
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
}

#[test]
fn cfg_bytes_v18_bad_coordinates_and_undercut_cannot_escape_through_ignored_errors() {
    let owner = owner();
    let mut work = Work::new(LIMIT);
    let mut budget = prepared(&mut work, LIMIT);
    for bad in [
        block(99),
        Block {
            function: Function(1),
            block: 0,
        },
    ] {
        let result = crate::with_canonical_kir_control_flow_bytes_v18(
            &owner,
            Function(0),
            Default::default(),
            &mut budget,
            |view, budget| {
                assert_eq!(
                    view.is_reachable(bad, budget),
                    Err(Error::InvalidBlock(bad))
                );
                let charged = budget.work();
                assert_eq!(
                    view.is_reachable(block(0), budget),
                    Err(Error::InvalidBlock(bad))
                );
                assert_eq!(budget.work(), charged);
                Ok::<_, Error>(())
            },
        );
        assert_eq!(result, Err(Error::InvalidBlock(bad)));
        assert_eq!(budget.storage(), FLOOR);
    }
    let called = Cell::new(false);
    assert_eq!(
        crate::with_canonical_kir_control_flow_bytes_v18(
            &owner,
            Function(1),
            Default::default(),
            &mut budget,
            |_, _| {
                called.set(true);
                Ok::<_, Error>(())
            }
        ),
        Err(Error::InvalidFunction(Function(1)))
    );
    assert!(!called.get());
    assert_eq!(budget.storage(), FLOOR);
    let mut retained = 0;
    let result = crate::with_canonical_kir_control_flow_bytes_v18(
        &owner,
        Function(0),
        Default::default(),
        &mut budget,
        |view, budget| {
            retained = budget.storage() - FLOOR;
            budget.release_storage(1)?;
            assert_eq!(
                view.is_reachable(block(0), budget),
                Err(Resource::Accounting.into())
            );
            budget.reserve_storage(1)?;
            Ok::<_, Error>(())
        },
    );
    assert_eq!(result, Err(Resource::Accounting.into()));
    assert_eq!(budget.storage(), FLOOR + retained);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}
