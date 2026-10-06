use super::*;
use crate::semantic_mir_v1::*;
use crate::semantic_option_dominance::enum_payload_admission_tests::payload_fixture;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Denied {
    Work(usize),
    Storage(usize),
    Injected(usize),
}

struct Meter {
    work: usize,
    storage: usize,
    legacy: usize,
    work_limit: usize,
    storage_limit: usize,
    calls: usize,
    deny_call: Option<usize>,
    panic_call: Option<usize>,
    first: Option<Denied>,
}

impl Meter {
    fn new(work_limit: usize, storage_limit: usize) -> Self {
        Self {
            work: 0,
            storage: 0,
            legacy: 0,
            work_limit,
            storage_limit,
            calls: 0,
            deny_call: None,
            panic_call: None,
            first: None,
        }
    }

    fn enter(&mut self) -> Result<(), Denied> {
        assert!(self.first.is_none(), "meter called after first refusal");
        self.calls += 1;
        if self.panic_call == Some(self.calls) {
            std::panic::panic_any(self.calls);
        }
        if self.deny_call == Some(self.calls) {
            let error = Denied::Injected(self.calls);
            self.first = Some(error);
            return Err(error);
        }
        Ok(())
    }
}

impl crate::SemanticAssertionMeterV1 for Meter {
    type Error = Denied;
    fn charge_work(&mut self, amount: usize) -> Result<(), Denied> {
        self.enter()?;
        let actual = self.work.checked_add(amount).unwrap();
        if actual > self.work_limit {
            let error = Denied::Work(actual);
            self.first = Some(error);
            return Err(error);
        }
        self.work = actual;
        Ok(())
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Denied> {
        self.enter()?;
        let actual = self.storage.checked_add(amount).unwrap();
        if actual > self.storage_limit {
            let error = Denied::Storage(actual);
            self.first = Some(error);
            return Err(error);
        }
        self.storage = actual;
        Ok(())
    }
    fn charge_legacy_work(&mut self, amount: usize) -> Result<(), Denied> {
        self.charge_work(amount)?;
        self.legacy += amount;
        Ok(())
    }
}

// Deliberately inert synthetic fixture. The collector observes a real Call
// terminator classified as get_mut, and analysis observes its exact Some edge.
// This does not assert that the fabricated intrinsic ABI/types validate.
fn fixture() -> (SemanticFunctionDeclV1, Vec<SemanticCallableDeclV1>) {
    let (_, base) = payload_fixture(&[(0, 2), (1, 3)], 2, false);
    let source = SemanticSourceProvenanceV1::unavailable();
    let option = SemanticTypeIdV1::from_index(2);
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(0),
        Vec::new(),
        Some(SemanticCallDestinationV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), Vec::new(), option).unwrap(),
            edge(SemanticEdgeRoleV1::CallReturn, 1),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([40; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([41; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([42; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([43; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([44; 32]),
        source,
        base.abi().clone(),
        base.locals().to_vec(),
        SemanticBlockIdV1::from_index(0),
        vec![
            block(45, Vec::new(), SemanticTerminatorKindV1::Call(call)),
            block(
                46,
                vec![base.blocks()[0].statements()[1].clone()],
                base.blocks()[0].terminator().kind().clone(),
            ),
            block(47, Vec::new(), SemanticTerminatorKindV1::Return),
            block(48, Vec::new(), SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap();
    let callables = vec![SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([50; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([51; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([52; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([53; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([54; 32]),
            source,
            base.abi().clone(),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
            disjoint_slice: option,
            index_witness: option,
            element: option,
            raw_index: option,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([55; 32]),
    }];
    (function, callables)
}

#[test]
fn original_option_v18_nonempty_inventory_and_reports_preserve_v1_facts() {
    let (function, callables) = fixture();
    let mut meter = Meter::new(usize::MAX, usize::MAX);
    let (producers, producer_bytes) =
        semantic_option_producers_with_meter_v18(&function, &callables, &mut meter).unwrap();
    assert_eq!(
        producers,
        super::super::semantic_option_producers_v1(&function, &callables).unwrap()
    );
    assert_eq!(producers.len(), 1);
    assert_eq!(
        producer_bytes,
        std::mem::size_of_val(&producers)
            + producers.capacity() * std::mem::size_of::<SemanticOptionProducerV1>()
    );
    assert_eq!(meter.legacy, 0);
    let (facts, retained) =
        SemanticOptionDominanceV1::analyze_with_meter_v18(&function, &producers, &mut meter)
            .unwrap();
    let legacy = SemanticOptionDominanceV1::analyze(&function, &producers).unwrap();
    assert_eq!(facts, legacy);
    assert_eq!(meter.legacy, legacy.work_units());
    let expected = std::mem::size_of::<SemanticOptionDominanceV1>()
        + function.locals().len() * std::mem::size_of::<Option<SemanticOptionAvailabilityV1>>()
        + std::mem::size_of::<SemanticBlockIdV1>()
        + 2 * function.blocks().len() * std::mem::size_of::<usize>();
    assert_eq!(retained, expected);
    assert!(meter.storage >= producer_bytes + retained);
    let availability = facts
        .availability(SemanticLocalIdV1::from_index(1))
        .unwrap();
    assert!(facts.allows(availability, SemanticBlockIdV1::from_index(3)));
    assert!(!facts.allows(availability, SemanticBlockIdV1::from_index(2)));
    assert!(!facts.grants_authority());
    let paid = meter.storage;
    drop((facts, producers));
    assert_eq!(
        meter.storage, paid,
        "only the caller may retire retained credit"
    );
}

#[test]
fn original_option_v18_collection_has_an_independent_exact_header_and_work_equation() {
    use std::mem::size_of;
    struct AdapterMirror<'a> {
        meter: &'a mut Meter,
        error: Option<Denied>,
    }
    struct BudgetMirror<'a> {
        used: usize,
        meter: Option<&'a mut dyn DominanceMeterV18>,
    }
    type Rows = (Vec<SemanticOptionProducerV1>, usize);
    let (function, callables) = fixture();
    let header = size_of::<AdapterMirror<'_>>()
        + size_of::<BudgetMirror<'_>>()
        + size_of::<(&SemanticFunctionDeclV1, &[SemanticCallableDeclV1])>()
        + size_of::<Result<Rows, SemanticOptionDominanceErrorV1>>()
        + size_of::<Result<Rows, SemanticOptionDominanceMeteredErrorV18<Denied>>>();
    let bytes = header
        + size_of::<Vec<SemanticOptionProducerV1>>()
        + function.blocks().len() * size_of::<SemanticOptionProducerV1>();
    let work = function.blocks().len() + 1;
    let mut exact = Meter::new(work, bytes);
    let (rows, retained) =
        semantic_option_producers_with_meter_v18(&function, &callables, &mut exact).unwrap();
    assert_eq!(rows.capacity(), function.blocks().len());
    assert_eq!(retained, bytes - header);
    assert_eq!((exact.work, exact.storage), (work, bytes));
    for (limit_work, limit_storage) in [(work - 1, bytes), (work, bytes - 1)] {
        let mut short = Meter::new(limit_work, limit_storage);
        let error = semantic_option_producers_with_meter_v18(&function, &callables, &mut short)
            .unwrap_err();
        let expected = if limit_work < work {
            Denied::Work(work)
        } else {
            Denied::Storage(bytes)
        };
        assert_eq!(
            error,
            SemanticOptionDominanceMeteredErrorV18::Meter(expected)
        );
        assert_eq!(short.first, Some(expected));
        assert!(short.storage <= limit_storage && short.work <= limit_work);
    }
}

#[test]
fn original_option_v18_analysis_exact_and_one_short_preserve_first_refusal() {
    let (function, callables) = fixture();
    let producers = super::super::semantic_option_producers_v1(&function, &callables).unwrap();
    let mut measured = Meter::new(usize::MAX, usize::MAX);
    let expected =
        SemanticOptionDominanceV1::analyze_with_meter_v18(&function, &producers, &mut measured)
            .unwrap();
    let mut exact = Meter::new(measured.work, measured.storage);
    assert_eq!(
        SemanticOptionDominanceV1::analyze_with_meter_v18(&function, &producers, &mut exact)
            .unwrap(),
        expected
    );
    assert_eq!(
        (exact.work, exact.storage),
        (measured.work, measured.storage)
    );
    for (work, storage) in [
        (measured.work - 1, measured.storage),
        (measured.work, measured.storage - 1),
        (0, 0),
    ] {
        let mut short = Meter::new(work, storage);
        let error =
            SemanticOptionDominanceV1::analyze_with_meter_v18(&function, &producers, &mut short)
                .unwrap_err();
        assert_eq!(
            error,
            SemanticOptionDominanceMeteredErrorV18::Meter(short.first.unwrap())
        );
        assert!(short.work <= work && short.storage <= storage);
    }
    let invalid = [SemanticOptionProducerV1::new(
        SemanticLocalIdV1::from_index(0),
        SemanticBlockIdV1::from_index(1),
    )];
    let mut ample = Meter::new(usize::MAX, usize::MAX);
    assert_eq!(
        SemanticOptionDominanceV1::analyze_with_meter_v18(&function, &invalid, &mut ample)
            .unwrap_err(),
        SemanticOptionDominanceMeteredErrorV18::Analysis(
            SemanticOptionDominanceV1::analyze(&function, &invalid).unwrap_err()
        )
    );
    assert_eq!(ample.first, None);
}

#[test]
fn original_option_v18_every_caller_refusal_stops_before_a_later_meter_call() {
    let (function, callables) = fixture();
    let producers = super::super::semantic_option_producers_v1(&function, &callables).unwrap();
    for analyze in [false, true] {
        let run = |meter: &mut Meter| {
            if analyze {
                SemanticOptionDominanceV1::analyze_with_meter_v18(&function, &producers, meter)
                    .map(drop)
            } else {
                semantic_option_producers_with_meter_v18(&function, &callables, meter).map(drop)
            }
        };
        let mut measured = Meter::new(usize::MAX, usize::MAX);
        run(&mut measured).unwrap();
        for call in 1..=measured.calls {
            let mut denied = Meter::new(usize::MAX, usize::MAX);
            denied.deny_call = Some(call);
            assert_eq!(
                run(&mut denied),
                Err(SemanticOptionDominanceMeteredErrorV18::Meter(
                    Denied::Injected(call)
                ))
            );
            assert_eq!(denied.calls, call);
            assert!(denied.work <= measured.work && denied.storage <= measured.storage);
        }
    }
}

#[test]
fn original_option_v18_raw_meter_panics_keep_accepted_credit_and_payload() {
    let (function, callables) = fixture();
    let producers = super::super::semantic_option_producers_v1(&function, &callables).unwrap();
    for analyze in [false, true] {
        let run = |meter: &mut Meter| {
            if analyze {
                SemanticOptionDominanceV1::analyze_with_meter_v18(&function, &producers, meter)
                    .map(drop)
            } else {
                semantic_option_producers_with_meter_v18(&function, &callables, meter).map(drop)
            }
        };
        let mut measured = Meter::new(usize::MAX, usize::MAX);
        run(&mut measured).unwrap();
        for call in [1, 2, measured.calls] {
            let mut meter = Meter::new(usize::MAX, usize::MAX);
            meter.panic_call = Some(call);
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&mut meter)))
                .unwrap_err();
            assert_eq!(panic.downcast_ref::<usize>(), Some(&call));
            assert_eq!(meter.calls, call);
            assert_eq!(meter.first, None);
            if call > 1 {
                assert!(meter.storage > 0);
            }
        }
    }
}

#[test]
fn original_option_v18_primitive_payload_and_logical_work_are_independent() {
    let bytes = std::mem::size_of::<Vec<u64>>() + 3 * std::mem::size_of::<u64>();
    let mut meter = Meter::new(3, bytes);
    let mut adapter = DominanceMeterAdapterV18 {
        meter: &mut meter,
        error: None,
    };
    let mut budget = WorkBudgetV1 {
        used: 0,
        meter: Some(&mut adapter),
    };
    let rows = budget.filled(3, 42_u64).unwrap();
    assert_eq!(rows, [42; 3]);
    assert_eq!(budget.used, 0);
    drop((rows, budget));
    assert_eq!((meter.work, meter.storage, meter.legacy), (3, bytes, 0));
    for (work, storage) in [(2, bytes), (3, bytes - 1)] {
        let mut meter = Meter::new(work, storage);
        let mut adapter = DominanceMeterAdapterV18 {
            meter: &mut meter,
            error: None,
        };
        assert!(
            WorkBudgetV1 {
                used: 0,
                meter: Some(&mut adapter)
            }
            .filled(3, 42_u64)
            .is_err()
        );
        assert_eq!(
            adapter.error,
            Some(if work == 2 {
                Denied::Work(3)
            } else {
                Denied::Storage(bytes)
            })
        );
    }
}
