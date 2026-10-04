//! Synthetic shared-analysis/meter controls, NOT an authoritative source owner.
use super::super::enum_payload_admission_tests::payload_fixture;
use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

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
    failed: Option<Denied>,
    storage_calls: usize,
    deny_storage_call: Option<usize>,
    panic_storage_call: Option<usize>,
}
impl SemanticEnumPayloadMeterV1 for Meter {
    type Error = Denied;
    fn charge_work(&mut self, amount: usize) -> Result<(), Denied> {
        assert!(
            self.failed.is_none(),
            "analysis continued after original denial"
        );
        let next = self.work.checked_add(amount).ok_or(Denied::Work)?;
        if self.work_limit.is_some_and(|limit| next > limit) {
            self.failed = Some(Denied::Work);
            return Err(Denied::Work);
        }
        self.work = next;
        Ok(())
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Denied> {
        assert!(
            self.failed.is_none(),
            "analysis continued after original denial"
        );
        self.storage_calls += 1;
        if self.panic_storage_call == Some(self.storage_calls) {
            panic!("injected meter unwind");
        }
        if self.deny_storage_call == Some(self.storage_calls) {
            self.failed = Some(Denied::Injected);
            return Err(Denied::Injected);
        }
        let next = self.storage.checked_add(amount).ok_or(Denied::Storage)?;
        if self.storage_limit.is_some_and(|limit| next > limit) {
            self.failed = Some(Denied::Storage);
            return Err(Denied::Storage);
        }
        self.storage = next;
        Ok(())
    }
}
fn analyze(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    meter: &mut Meter,
) -> Result<SemanticEnumPayloadDominanceV1, SemanticEnumPayloadMeteredErrorV1<Denied>> {
    SemanticEnumPayloadDominanceV1::analyze_with_meter_v1(function, types, meter)
}
#[test]
fn metered_shared_core_preserves_exact_facts_and_legacy_work_units() {
    for (cases, otherwise, second) in [
        (vec![(0, 1), (1, 2)], 3, false),
        (vec![(0, 1)], 1, false),
        (vec![(0, 1), (1, 2)], 3, true),
    ] {
        let (types, function) = payload_fixture(&cases, otherwise, second);
        let old = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
        let mut meter = Meter::default();
        let new = analyze(&function, &types, &mut meter).unwrap();
        assert_eq!(old, new);
        assert!(!new.grants_authority());
        assert!(meter.work > new.work_units());
        assert!(meter.storage > 4096);
        assert!(meter.storage_calls > 10);
        // The model returns no refundable caller receipt and releases nothing.
        let retained = meter.storage;
        drop(new);
        assert_eq!(meter.storage, retained);
    }
}
#[test]
fn exact_and_one_short_work_storage_preserve_accepted_prefixes() {
    let (types, function) = payload_fixture(&[(0, 1), (1, 2)], 3, false);
    let mut baseline = Meter::default();
    drop(analyze(&function, &types, &mut baseline).unwrap());
    let (w, p) = (baseline.work, baseline.storage);
    let mut exact = Meter {
        work: 7,
        storage: 11,
        work_limit: Some(w + 7),
        storage_limit: Some(p + 11),
        ..Default::default()
    };
    drop(analyze(&function, &types, &mut exact).unwrap());
    assert_eq!((exact.work, exact.storage), (w + 7, p + 11));
    let mut short_work = Meter {
        work: 7,
        storage: 11,
        work_limit: Some(w + 6),
        ..Default::default()
    };
    assert_eq!(
        analyze(&function, &types, &mut short_work),
        Err(SemanticEnumPayloadMeteredErrorV1::Meter(Denied::Work))
    );
    assert!(short_work.work >= 7 && short_work.work < w + 7);
    assert!(short_work.storage >= 11);
    let mut short_storage = Meter {
        work: 7,
        storage: 11,
        storage_limit: Some(p + 10),
        ..Default::default()
    };
    assert_eq!(
        analyze(&function, &types, &mut short_storage),
        Err(SemanticEnumPayloadMeteredErrorV1::Meter(Denied::Storage))
    );
    assert!(short_storage.storage >= 11 && short_storage.storage < p + 11);
    assert!(short_storage.work >= 7);
}
#[test]
fn each_allocation_admission_denial_is_terminal_and_exact() {
    let (types, function) = payload_fixture(&[(0, 1), (1, 2)], 3, false);
    let mut baseline = Meter::default();
    drop(analyze(&function, &types, &mut baseline).unwrap());
    for call in 1..=baseline.storage_calls {
        let mut meter = Meter {
            deny_storage_call: Some(call),
            ..Default::default()
        };
        assert_eq!(
            analyze(&function, &types, &mut meter),
            Err(SemanticEnumPayloadMeteredErrorV1::Meter(Denied::Injected))
        );
        assert_eq!(meter.storage_calls, call);
        assert_eq!(meter.failed, Some(Denied::Injected));
    }
}
#[test]
fn panic_and_analysis_refusal_leave_caller_reservations_owned() {
    let (types, function) = payload_fixture(&[(0, 1), (1, 2)], 3, false);
    let mut meter = Meter {
        storage: 31,
        panic_storage_call: Some(3),
        ..Default::default()
    };
    assert!(catch_unwind(AssertUnwindSafe(|| analyze(&function, &types, &mut meter))).is_err());
    assert!(meter.storage > 31);
    assert_eq!(meter.storage_calls, 3);
    let mut bad = Meter::default();
    assert!(matches!(
        analyze(&function, &[], &mut bad),
        Err(SemanticEnumPayloadMeteredErrorV1::Analysis(
            SemanticOptionDominanceErrorV1::InvalidControlFlow(_)
        ))
    ));
    assert!(bad.storage > 4096);
}
#[test]
fn resource_arithmetic_is_classified_before_allocator() {
    let mut meter = Meter::default();
    let mut adapter = Adapter {
        original: &mut meter,
        failure: None,
    };
    let result = {
        let mut budget = WorkBudgetV1 {
            used: 0,
            meter: Some(&mut adapter),
        };
        let mut vector: Vec<u64> = Vec::new();
        budget.reserve(&mut vector, usize::MAX)
    };
    assert_eq!(result, Err(SemanticOptionDominanceErrorV1::Storage));
    assert_eq!(
        adapter.failure,
        Some(SemanticEnumPayloadMeteredErrorV1::Arithmetic)
    );
    assert_eq!(meter.storage, 0);
}
#[test]
fn legacy_local_guard_is_not_replaced_by_external_ledger() {
    let mut meter = Meter::default();
    let mut adapter = Adapter {
        original: &mut meter,
        failure: None,
    };
    let mut budget = WorkBudgetV1 {
        used: MAX_SEMANTIC_OPTION_DOMINANCE_WORK_V1,
        meter: Some(&mut adapter),
    };
    assert!(matches!(
        budget.charge(1),
        Err(SemanticOptionDominanceErrorV1::WorkLimit { .. })
    ));
    assert_eq!(meter.work, 0);
}

// Large generic errors must be paid as typed frames, not hidden in the bounded
// 4096 lexical allowance. This remains a synthetic meter, not a source owner.
#[derive(Default)]
struct LargeErrorMeter {
    inner: Meter,
    first_storage: Option<usize>,
}
// Deliberately exercise the public trait's unbounded associated error width.
#[allow(clippy::result_large_err)]
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
fn large_generic_error_frame_is_exactly_prepaid_before_analysis_allocations() {
    type Facts = SemanticEnumPayloadDominanceV1;
    type WideError = SemanticEnumPayloadMeteredErrorV1<[u8; 8192]>;
    let fixed = size_of::<Facts>()
        + size_of::<DominatorIntervalsV1>()
        + size_of::<WorkBudgetV1<'_>>()
        + 4096;
    let generic = size_of::<Adapter<'_, LargeErrorMeter>>()
        + 2 * size_of::<Result<Facts, SemanticOptionDominanceErrorV1>>()
        + 2 * size_of::<Result<Facts, WideError>>()
        + size_of::<Result<(), [u8; 8192]>>()
        + size_of::<[u8; 8192]>()
        + size_of::<WideError>();
    let frame = metered_frame_storage_v1::<LargeErrorMeter>().unwrap();
    assert_eq!(frame, fixed + generic);
    assert!(frame >= fixed + 6 * 8192);
    let (types, function) = payload_fixture(&[(0, 1), (1, 2)], 3, false);
    for short in [false, true] {
        let mut meter = LargeErrorMeter {
            inner: Meter {
                work: 7,
                storage: 11,
                storage_limit: Some(11 + frame - usize::from(short)),
                ..Default::default()
            },
            ..Default::default()
        };
        let result = Facts::analyze_with_meter_v1(&function, &types, &mut meter);
        assert!(matches!(
            result,
            Err(SemanticEnumPayloadMeteredErrorV1::Meter(error)) if error == [2; 8192]
        ));
        assert_eq!(meter.first_storage, Some(frame));
        assert_eq!(meter.inner.failed, Some(Denied::Storage));
        if short {
            // First admission refuses: no analysis traversal/allocation began.
            assert_eq!(meter.inner.storage_calls, 1);
            assert_eq!((meter.inner.work, meter.inner.storage), (7, 11));
        } else {
            // Exact frame succeeds; the next, separately paid definition table
            // cannot fit. Do not confuse exact frame with complete route fit.
            assert_eq!(meter.inner.storage_calls, 2);
            assert_eq!(meter.inner.storage, 11 + frame);
            assert!(meter.inner.work > 7);
        }
    }
}

#[test]
fn large_generic_error_complete_route_exact_storage_and_one_short() {
    let (types, function) = payload_fixture(&[(0, 1), (1, 2)], 3, false);
    let expected = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    let mut baseline = LargeErrorMeter::default();
    let result =
        SemanticEnumPayloadDominanceV1::analyze_with_meter_v1(&function, &types, &mut baseline)
            .unwrap();
    assert_eq!(result, expected);
    drop(result);
    let (work, storage) = (baseline.inner.work, baseline.inner.storage);
    assert_eq!(
        baseline.first_storage,
        metered_frame_storage_v1::<LargeErrorMeter>()
    );
    assert!(storage > baseline.first_storage.unwrap());
    for short in [false, true] {
        let mut meter = LargeErrorMeter {
            inner: Meter {
                work: 7,
                storage: 11,
                work_limit: Some(7 + work),
                storage_limit: Some(11 + storage - usize::from(short)),
                ..Default::default()
            },
            ..Default::default()
        };
        let result =
            SemanticEnumPayloadDominanceV1::analyze_with_meter_v1(&function, &types, &mut meter);
        assert_eq!(meter.first_storage, baseline.first_storage);
        if short {
            assert!(matches!(
                result,
                Err(SemanticEnumPayloadMeteredErrorV1::Meter(error)) if error == [2; 8192]
            ));
            assert_eq!(meter.inner.failed, Some(Denied::Storage));
            assert!(meter.inner.storage >= 11 + baseline.first_storage.unwrap());
            assert!(meter.inner.storage < 11 + storage);
        } else {
            assert_eq!(result.unwrap(), expected);
            assert_eq!(
                (meter.inner.work, meter.inner.storage),
                (7 + work, 11 + storage)
            );
            assert_eq!(meter.inner.failed, None);
        }
    }
}
