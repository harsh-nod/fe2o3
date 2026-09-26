//! Synthetic resource controls only; these constructors grant no authority.
use super::*;
use crate::semantic_mir_v1::*;
use crate::semantic_option_dominance::enum_payload_admission_tests::payload_fixture;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Denied {
    Work,
    Storage,
    Injected,
}
#[derive(Default)]
struct Meter {
    work: usize,
    storage: usize,
    work_limit: Option<usize>,
    storage_limit: Option<usize>,
    work_calls: usize,
    storage_calls: usize,
    deny_work_call: Option<usize>,
    deny_storage_call: Option<usize>,
    panic_storage_call: Option<usize>,
    failed: Option<Denied>,
}
impl SemanticEnumPayloadMeterV1 for Meter {
    type Error = Denied;
    fn charge_work(&mut self, amount: usize) -> Result<(), Denied> {
        assert!(self.failed.is_none(), "work after denial");
        self.work_calls += 1;
        let next = self.work.checked_add(amount).unwrap();
        let denied = if self.deny_work_call == Some(self.work_calls) {
            Some(Denied::Injected)
        } else if self.work_limit.is_some_and(|limit| next > limit) {
            Some(Denied::Work)
        } else {
            None
        };
        if let Some(error) = denied {
            self.failed = Some(error);
            return Err(error);
        }
        self.work = next;
        Ok(())
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Denied> {
        assert!(self.failed.is_none(), "storage after denial");
        self.storage_calls += 1;
        if self.panic_storage_call == Some(self.storage_calls) {
            panic!("meter panic");
        }
        let next = self.storage.checked_add(amount).unwrap();
        let denied = if self.deny_storage_call == Some(self.storage_calls) {
            Some(Denied::Injected)
        } else if self.storage_limit.is_some_and(|limit| next > limit) {
            Some(Denied::Storage)
        } else {
            None
        };
        if let Some(error) = denied {
            self.failed = Some(error);
            return Err(error);
        }
        self.storage = next;
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
fn run<M: SemanticEnumPayloadMeterV1>(
    dominance: bool,
    meter: &mut M,
) -> Result<(), SemanticEnumPayloadMeteredErrorV1<M::Error>> {
    // Test fixture construction occurs before the API's accounting boundary.
    let (function, callables) = fixture();
    if dominance {
        let producers = semantic_option_producers_v1(&function, &callables).unwrap();
        SemanticOptionDominanceV1::analyze_with_meter_v1(&function, &producers, meter).map(drop)
    } else {
        semantic_option_producers_with_meter_v1(&function, &callables, meter).map(drop)
    }
}

#[test]
fn nonempty_collection_and_some_facts_match_legacy_exactly() {
    let (function, callables) = fixture();
    let old = semantic_option_producers_v1(&function, &callables).unwrap();
    assert_eq!(
        old,
        vec![SemanticOptionProducerV1::new(
            SemanticLocalIdV1::from_index(1),
            SemanticBlockIdV1::from_index(1)
        )]
    );
    let mut meter = Meter::default();
    let producers =
        semantic_option_producers_with_meter_v1(&function, &callables, &mut meter).unwrap();
    assert_eq!(producers, old);
    let expected = SemanticOptionDominanceV1::analyze(&function, &old).unwrap();
    let actual =
        SemanticOptionDominanceV1::analyze_with_meter_v1(&function, &producers, &mut meter)
            .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.work_units(), expected.work_units());
    assert!(!actual.grants_authority());
    let region = actual
        .availability(SemanticLocalIdV1::from_index(1))
        .unwrap();
    assert!(actual.allows(region, SemanticBlockIdV1::from_index(3)));
    assert!(!actual.allows(region, SemanticBlockIdV1::from_index(2)));
    assert!(!actual.allows(region, SemanticBlockIdV1::from_index(0)));
    assert!(meter.work > actual.work_units());
}
#[test]
fn absent_callable_is_not_invented_as_an_option_producer() {
    let (function, _) = fixture();
    let mut meter = Meter::default();
    assert!(
        semantic_option_producers_with_meter_v1(&function, &[], &mut meter)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        semantic_option_producers_v1(&function, &[]).unwrap(),
        Vec::new()
    );
    assert!(meter.work > 0 && meter.storage > 0);
}
#[test]
fn both_routes_exact_nonzero_budget_and_one_short() {
    for dominance in [false, true] {
        let mut measured = Meter::default();
        run(dominance, &mut measured).unwrap();
        for short in [None, Some(Denied::Work), Some(Denied::Storage)] {
            let mut meter = Meter {
                work: 7,
                storage: 11,
                work_limit: Some(7 + measured.work - usize::from(short == Some(Denied::Work))),
                storage_limit: Some(
                    11 + measured.storage - usize::from(short == Some(Denied::Storage)),
                ),
                ..Default::default()
            };
            let result = run(dominance, &mut meter);
            match short {
                None => {
                    result.unwrap();
                    assert_eq!(
                        (meter.work, meter.storage),
                        (7 + measured.work, 11 + measured.storage)
                    );
                }
                Some(error) => {
                    assert_eq!(result, Err(SemanticEnumPayloadMeteredErrorV1::Meter(error)));
                    assert_eq!(meter.failed, Some(error));
                }
            }
        }
    }
}
#[test]
fn every_work_and_storage_denial_is_terminal_with_accepted_prefix_retained() {
    for dominance in [false, true] {
        let mut measured = Meter::default();
        run(dominance, &mut measured).unwrap();
        for storage in [false, true] {
            let calls = if storage {
                measured.storage_calls
            } else {
                measured.work_calls
            };
            assert!(calls > 0);
            for call in 1..=calls {
                let mut meter = Meter {
                    work: 7,
                    storage: 11,
                    deny_work_call: (!storage).then_some(call),
                    deny_storage_call: storage.then_some(call),
                    ..Default::default()
                };
                assert_eq!(
                    run(dominance, &mut meter),
                    Err(SemanticEnumPayloadMeteredErrorV1::Meter(Denied::Injected))
                );
                assert_eq!(
                    if storage {
                        meter.storage_calls
                    } else {
                        meter.work_calls
                    },
                    call
                );
                assert!(meter.work >= 7 && meter.storage >= 11);
                if storage && call == 1 {
                    assert_eq!((meter.work, meter.storage), (7, 11));
                }
            }
        }
    }
}
#[test]
fn meter_unwind_does_not_refund_accepted_storage() {
    for dominance in [false, true] {
        let mut meter = Meter {
            work: 7,
            storage: 11,
            panic_storage_call: Some(2),
            ..Default::default()
        };
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(dominance, &mut meter)));
        assert!(result.is_err());
        assert_eq!(meter.storage_calls, 2);
        assert!(meter.storage > 11);
    }
}
#[test]
fn inexact_and_duplicate_producers_retain_legacy_refusal() {
    let (function, callables) = fixture();
    let producer = semantic_option_producers_v1(&function, &callables).unwrap()[0];
    for producers in [
        vec![producer, producer],
        vec![SemanticOptionProducerV1::new(
            SemanticLocalIdV1::from_index(1),
            SemanticBlockIdV1::from_index(99),
        )],
    ] {
        let expected = SemanticOptionDominanceV1::analyze(&function, &producers).unwrap_err();
        let mut meter = Meter::default();
        assert_eq!(
            SemanticOptionDominanceV1::analyze_with_meter_v1(&function, &producers, &mut meter),
            Err(SemanticEnumPayloadMeteredErrorV1::Analysis(expected))
        );
        assert!(meter.storage > 0);
    }
}
#[test]
fn legacy_local_limit_is_not_replaced_by_external_meter() {
    let (function, callables) = fixture();
    let producers = semantic_option_producers_v1(&function, &callables).unwrap();
    let mut legacy = WorkBudgetV1 {
        used: MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1,
        ..Default::default()
    };
    let expected =
        SemanticOptionDominanceV1::analyze_with_budget(&function, &producers, &mut legacy)
            .unwrap_err();
    let mut meter = Meter::default();
    let mut adapter = Adapter {
        original: &mut meter,
        failure: None,
    };
    let mut budget = WorkBudgetV1 {
        used: MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1,
        meter: Some(&mut adapter),
    };
    assert_eq!(
        SemanticOptionDominanceV1::analyze_with_budget(&function, &producers, &mut budget),
        Err(expected)
    );
    assert!(matches!(
        expected,
        SemanticOptionDominanceErrorV1::WorkLimit { .. }
    ));
    assert_eq!(meter.work, 0);
}

#[derive(Default)]
struct LargeErrorMeter {
    inner: Meter,
    first_storage: Option<usize>,
}
impl SemanticEnumPayloadMeterV1 for LargeErrorMeter {
    type Error = [u8; 8192];
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.inner.charge_work(amount).map_err(|_| [1; 8192])
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.first_storage.get_or_insert(amount);
        self.inner.reserve_storage(amount).map_err(|_| [2; 8192])
    }
}
#[test]
fn both_generic_error_frames_are_paid_before_first_analysis_allocation() {
    for dominance in [false, true] {
        let frame = if dominance {
            metered_fact_frame_storage_v1::<LargeErrorMeter, SemanticOptionDominanceV1>().unwrap()
        } else {
            metered_fact_frame_storage_v1::<LargeErrorMeter, Vec<SemanticOptionProducerV1>>()
                .unwrap()
        };
        assert!(frame > 4 * 8192);
        for short in [false, true] {
            let mut meter = LargeErrorMeter {
                inner: Meter {
                    work: 7,
                    storage: 11,
                    storage_limit: Some(11 + frame - usize::from(short)),
                    ..Default::default()
                },
                first_storage: None,
            };
            assert!(
                matches!(run(dominance,&mut meter),Err(SemanticEnumPayloadMeteredErrorV1::Meter(error)) if error==[2;8192])
            );
            assert_eq!(meter.first_storage, Some(frame));
            if short {
                assert_eq!(
                    (
                        meter.inner.work,
                        meter.inner.storage,
                        meter.inner.storage_calls
                    ),
                    (7, 11, 1)
                );
            } else {
                assert_eq!(meter.inner.storage, 11 + frame);
                assert_eq!(meter.inner.storage_calls, 2);
            }
        }
    }
}
#[test]
fn both_large_error_complete_routes_fit_exactly_and_refuse_one_short() {
    for dominance in [false, true] {
        let mut measured = LargeErrorMeter::default();
        run(dominance, &mut measured).unwrap();
        for short in [false, true] {
            let mut meter = LargeErrorMeter {
                inner: Meter {
                    work: 7,
                    storage: 11,
                    work_limit: Some(7 + measured.inner.work),
                    storage_limit: Some(11 + measured.inner.storage - usize::from(short)),
                    ..Default::default()
                },
                first_storage: None,
            };
            let result = run(dominance, &mut meter);
            if short {
                assert!(
                    matches!(result,Err(SemanticEnumPayloadMeteredErrorV1::Meter(error)) if error==[2;8192])
                );
            } else {
                result.unwrap();
                assert_eq!(
                    (meter.inner.work, meter.inner.storage),
                    (7 + measured.inner.work, 11 + measured.inner.storage)
                );
            }
        }
    }
}
#[test]
fn enum_specialization_keeps_the_identical_generic_frame() {
    assert_eq!(
        metered_frame_storage_v1::<Meter>(),
        metered_fact_frame_storage_v1::<Meter, SemanticEnumPayloadDominanceV1>()
    );
    assert_eq!(
        metered_frame_storage_v1::<LargeErrorMeter>(),
        metered_fact_frame_storage_v1::<LargeErrorMeter, SemanticEnumPayloadDominanceV1>()
    );
}
