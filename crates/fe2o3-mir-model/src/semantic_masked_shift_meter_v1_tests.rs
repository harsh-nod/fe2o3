use super::*;
use std::{cell::Cell, rc::Rc};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Work(usize),
    Storage(usize),
    Accounting,
}

struct Meter {
    work: usize,
    storage: usize,
    work_limit: usize,
    storage_limit: usize,
    calls: Vec<(bool, usize)>,
    refuse: bool,
}

impl Meter {
    fn new(work_limit: usize, storage_limit: usize) -> Self {
        Self {
            work: 0,
            storage: 0,
            work_limit,
            storage_limit,
            calls: vec![],
            refuse: false,
        }
    }
}

impl SemanticMaskedShiftMeterV1 for Meter {
    type Error = Refusal;

    fn charge_work(&mut self, amount: usize) -> Result<(), Refusal> {
        self.calls.push((false, amount));
        if self.refuse {
            return Err(Refusal::Accounting);
        }
        let actual = self.work + amount;
        if actual > self.work_limit {
            return Err(Refusal::Work(actual));
        }
        self.work = actual;
        Ok(())
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Refusal> {
        self.calls.push((true, amount));
        if self.refuse {
            return Err(Refusal::Accounting);
        }
        let actual = self.storage + amount;
        if actual > self.storage_limit {
            return Err(Refusal::Storage(actual));
        }
        self.storage = actual;
        Ok(())
    }
}

#[test]
fn live_meter_preserves_local_results_and_one_cumulative_ledger() {
    let owner = fixture(Shape::default()).unwrap();
    let mut local = index(&owner);
    let mut meter = Meter::new(usize::MAX, usize::MAX);
    let mut live = SemanticMaskedShiftIndexV1::analyze_metered(
        &owner,
        FUNCTION,
        SemanticMaskedShiftLimitsV1::default(),
        &mut meter,
    )
    .unwrap();
    assert_eq!(
        (live.work_units(), live.storage_bytes()),
        (local.work_units(), local.storage_bytes())
    );
    let fact = live
        .assertion_metered(block_id(0), &mut meter)
        .unwrap()
        .unwrap();
    assert_eq!(
        fact.mask_statement(),
        local
            .assertion(block_id(0))
            .unwrap()
            .unwrap()
            .mask_statement()
    );
    assert!(std::ptr::eq(fact.owner(), &owner));
    assert!(
        live.shift_metered(block_id(1), 0, &mut meter)
            .unwrap()
            .is_some()
    );
    local.shift(block_id(1), 0).unwrap();
    assert_eq!(
        (meter.work, meter.storage),
        (local.work_units(), local.storage_bytes())
    );
    assert_eq!(
        (live.work_units(), live.storage_bytes()),
        (meter.work, meter.storage)
    );
    assert!(meter.calls.iter().filter(|event| event.0).count() >= 22);
}

#[test]
fn meter_failures_remain_errors_and_preserve_the_local_accepted_prefix() {
    let owner = fixture(Shape::default()).unwrap();
    let measured = index(&owner);
    for storage_short in [false, true] {
        let mut meter = Meter::new(
            measured.work_units() - usize::from(!storage_short),
            measured.storage_bytes() - usize::from(storage_short),
        );
        let error = SemanticMaskedShiftIndexV1::analyze_metered(
            &owner,
            FUNCTION,
            SemanticMaskedShiftLimitsV1::default(),
            &mut meter,
        )
        .err()
        .unwrap();
        match error {
            SemanticMaskedShiftMeteredErrorV1::Meter(Refusal::Storage(actual)) if storage_short => {
                assert!(actual > meter.storage_limit)
            }
            SemanticMaskedShiftMeteredErrorV1::Meter(Refusal::Work(actual)) if !storage_short => {
                assert!(actual > meter.work_limit)
            }
            _ => panic!("wrong meter failure: {error:?}"),
        }
        assert!(meter.work <= meter.work_limit);
        assert!(meter.storage <= meter.storage_limit);
    }
    let mut meter = Meter::new(usize::MAX, usize::MAX);
    let mut live = SemanticMaskedShiftIndexV1::analyze_metered(
        &owner,
        FUNCTION,
        SemanticMaskedShiftLimitsV1::default(),
        &mut meter,
    )
    .unwrap();
    let accepted = live.work_units();
    meter.refuse = true;
    assert!(matches!(
        live.assertion_metered(block_id(0), &mut meter),
        Err(SemanticMaskedShiftMeteredErrorV1::Meter(
            Refusal::Accounting
        ))
    ));
    assert_eq!(live.work_units(), accepted);
    assert_eq!(meter.work, accepted);
    assert!(matches!(
        live.shift_metered(block_id(1), 0, &mut meter),
        Err(SemanticMaskedShiftMeteredErrorV1::Meter(
            Refusal::Accounting
        ))
    ));
    assert_eq!(live.work_units(), accepted);
}

struct Element(Rc<Cell<usize>>);
impl Clone for Element {
    fn clone(&self) -> Self {
        self.0.set(self.0.get() + 1);
        Self(self.0.clone())
    }
}

#[test]
fn work_and_storage_are_prepaid_before_table_initialization() {
    for (work_limit, storage_limit, expected) in [
        (0, usize::MAX, Refusal::Work(3)),
        (3, 0, Refusal::Storage(3 * size_of::<Element>())),
    ] {
        let clones = Rc::new(Cell::new(0));
        let mut local = Budget {
            limits: SemanticMaskedShiftLimitsV1::default(),
            work: 0,
            storage: 0,
        };
        let mut meter = Meter::new(work_limit, storage_limit);
        let result = local.metered(&mut meter).table(3, Element(clones.clone()));
        assert!(
            matches!(result, Err(SemanticMaskedShiftMeteredErrorV1::Meter(error)) if error == expected)
        );
        assert_eq!(clones.get(), 0);
        assert_eq!(local.work, meter.work);
        assert_eq!(local.storage, meter.storage);
        assert_eq!(meter.calls.first(), Some(&(false, 3)));
    }
}

#[test]
fn capacity_excess_is_forwarded_before_local_commit_and_local_caps_precede_meter() {
    let mut local = Budget {
        limits: SemanticMaskedShiftLimitsV1::new(10, 32),
        work: 0,
        storage: 16,
    };
    let mut meter = Meter::new(10, 32);
    meter.storage = 16;
    local
        .metered(&mut meter)
        .account_capacity::<u64>(2, 4)
        .unwrap();
    assert_eq!((local.storage, meter.storage), (32, 32));
    assert_eq!(meter.calls, [(true, 16)]);
    let result = local.metered(&mut meter).account_capacity::<u64>(2, 3);
    assert!(matches!(
        result,
        Err(SemanticMaskedShiftMeteredErrorV1::Analysis(
            SemanticMaskedShiftErrorV1::StorageLimit {
                actual: 40,
                limit: 32
            }
        ))
    ));
    assert_eq!(meter.calls, [(true, 16)]);
    local.limits = SemanticMaskedShiftLimitsV1::new(10, 64);
    let result = local.metered(&mut meter).account_capacity::<u64>(2, 3);
    assert!(matches!(
        result,
        Err(SemanticMaskedShiftMeteredErrorV1::Meter(Refusal::Storage(
            40
        )))
    ));
    assert_eq!((local.storage, meter.storage), (32, 32));
    assert_eq!(meter.calls, [(true, 16), (true, 8)]);
}
