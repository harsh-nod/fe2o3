use super::tests::{BOOL, Meter, assertion_function, constant, limits, ordinary_message, types};
use super::*;

fn bitand_scheduled_prefix(
    analysis: &mut SemanticAssertionAnalysisV1<'_>,
    meter: &mut Bounded,
    frames: &mut Vec<AssertionRangeFrameV1>,
) -> MR<(), Bounded> {
    use super::tests::U8;
    let expression = SemanticRvalueV1::new(
        U8,
        SemanticRvalueKindV1::Binary {
            operation: SemanticBinaryOpV1::BitAnd,
            left: constant(U8, 255),
            right: constant(U8, 7),
        },
    );
    let mut query = analysis.recipe_queries_v1(meter);
    let task = query.assertion_range_expression_task_v1(&expression)?;
    let mut values = Vec::new();
    query.schedule_assertion_range_expression_v1(
        frames,
        &mut values,
        task,
        ScalarAssignmentSiteV1 {
            block: 0,
            statement: 0,
        },
    )?;
    assert!(values.is_empty());
    Ok(())
}

#[test]
fn unsigned_bitand_scheduler_has_literal_work_and_storage_recurrence() {
    let types = types();
    let function = assertion_function(constant(BOOL, 1), ordinary_message());
    let frame = size_of::<AssertionRangeFrameV1>();
    let mut meter = Bounded::new(82, constructor_storage() + 3 * frame);
    let mut analysis = SemanticAssertionAnalysisV1::new_metered(
        &types,
        &function,
        SemanticAssertionLimitsV1::new(82, constructor_storage() + 3 * frame),
        &mut meter,
    )
    .unwrap();
    let mut frames = Vec::new();
    bitand_scheduled_prefix(&mut analysis, &mut meter, &mut frames).unwrap();
    // Existing literal constructor 77; two source clones 1+1; growth copies 0+1+2.
    assert_eq!(meter.work, 77 + 2 + 3);
    assert_eq!(meter.storage, constructor_storage() + 3 * frame);
    assert_eq!(frames.len(), 3);
    assert!(matches!(
        frames[0],
        AssertionRangeFrameV1::FinishBinary {
            operation: SemanticBinaryOpV1::BitAnd,
            destination_maximum: Some(255),
            ..
        }
    ));
    assert!(matches!(
        frames[1],
        AssertionRangeFrameV1::Operand {
            task: AssertionRangeOperandTaskV1::Constant { bits: Some(7), .. },
            ..
        }
    ));
    assert!(matches!(
        frames[2],
        AssertionRangeFrameV1::Operand {
            task: AssertionRangeOperandTaskV1::Constant {
                bits: Some(255),
                ..
            },
            ..
        }
    ));
    drop(frames);
    drop(analysis);
    // This meter records cumulative reservations; it is not a live Budget refund oracle.
    assert_eq!(meter.storage, constructor_storage() + 3 * frame);
}

#[test]
fn unsigned_bitand_scheduler_local_and_external_work_cuts_preserve_exact_prefix() {
    let types = types();
    let function = assertion_function(constant(BOOL, 1), ordinary_message());
    for local in [false, true] {
        for (limit, accepted, requested, frames_count) in [
            (77, 77, 78, 0),
            (78, 78, 79, 0),
            (79, 79, 80, 1),
            (80, 80, 82, 2),
            (81, 80, 82, 2),
        ] {
            let mut meter = Bounded::new(if local { usize::MAX } else { limit }, usize::MAX);
            let mut analysis = SemanticAssertionAnalysisV1::new_metered(
                &types,
                &function,
                SemanticAssertionLimitsV1::new(if local { limit } else { usize::MAX }, usize::MAX),
                &mut meter,
            )
            .unwrap();
            let mut frames = Vec::new();
            let error =
                bitand_scheduled_prefix(&mut analysis, &mut meter, &mut frames).unwrap_err();
            if local {
                assert_eq!(
                    error,
                    SemanticAssertionMeteredErrorV1::Analysis(
                        SemanticAssertionErrorV1::WorkLimit {
                            actual: requested,
                            limit
                        },
                    )
                );
            } else {
                assert_eq!(error, SemanticAssertionMeteredErrorV1::Meter(Refusal::Work));
            }
            assert_eq!(meter.work, accepted);
            assert_eq!(frames.len(), frames_count);
            assert_eq!(
                meter.storage,
                constructor_storage() + frames_count * size_of::<AssertionRangeFrameV1>()
            );
            assert!(matches!(
                analysis.resources.metered(&mut meter).charge(0),
                Err(SemanticAssertionMeteredErrorV1::Analysis(
                    SemanticAssertionErrorV1::Accounting
                ))
            ));
        }
    }
}

#[test]
fn unsigned_bitand_scheduler_local_and_external_storage_cuts_preserve_exact_prefix() {
    let types = types();
    let function = assertion_function(constant(BOOL, 1), ordinary_message());
    let frame = size_of::<AssertionRangeFrameV1>();
    for local in [false, true] {
        for (admission, work) in [(1, 79), (2, 80), (3, 82)] {
            let requested = constructor_storage() + admission * frame;
            let limit = requested - 1;
            let mut meter = Bounded::new(usize::MAX, if local { usize::MAX } else { limit });
            let mut analysis = SemanticAssertionAnalysisV1::new_metered(
                &types,
                &function,
                SemanticAssertionLimitsV1::new(usize::MAX, if local { limit } else { usize::MAX }),
                &mut meter,
            )
            .unwrap();
            let mut frames = Vec::new();
            let error =
                bitand_scheduled_prefix(&mut analysis, &mut meter, &mut frames).unwrap_err();
            if local {
                assert_eq!(
                    error,
                    SemanticAssertionMeteredErrorV1::Analysis(
                        SemanticAssertionErrorV1::StorageLimit {
                            actual: requested,
                            limit
                        },
                    )
                );
            } else {
                assert_eq!(
                    error,
                    SemanticAssertionMeteredErrorV1::Meter(Refusal::Storage)
                );
            }
            assert_eq!(meter.work, work);
            assert_eq!(frames.len(), admission - 1);
            assert_eq!(
                meter.storage,
                constructor_storage() + (admission - 1) * frame
            );
            assert!(matches!(
                analysis.resources.metered(&mut meter).charge(0),
                Err(SemanticAssertionMeteredErrorV1::Analysis(
                    SemanticAssertionErrorV1::Accounting
                ))
            ));
        }
    }
}
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    Work,
    Storage,
}
struct Bounded {
    work: usize,
    storage: usize,
    work_limit: usize,
    storage_limit: usize,
    events: Vec<(char, usize)>,
}
impl Bounded {
    fn new(work_limit: usize, storage_limit: usize) -> Self {
        Self {
            work: 0,
            storage: 0,
            work_limit,
            storage_limit,
            events: vec![],
        }
    }
}
impl SemanticAssertionMeterV1 for Bounded {
    type Error = Refusal;
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        let next = self.work.checked_add(amount).ok_or(Refusal::Work)?;
        if next > self.work_limit {
            return Err(Refusal::Work);
        }
        self.events.push(('w', amount));
        self.work = next;
        Ok(())
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Self::Error> {
        let next = self.storage.checked_add(amount).ok_or(Refusal::Storage)?;
        if next > self.storage_limit {
            return Err(Refusal::Storage);
        }
        self.events.push(('s', amount));
        self.storage = next;
        Ok(())
    }
}
#[test]
fn default_legacy_and_scan_hooks_debit_the_same_total_once() {
    let mut local = Budget::new(SemanticAssertionLimitsV1::new(10, 0)).unwrap();
    let mut external = Bounded::new(10, 0);
    let mut paid = local.metered(&mut external);
    paid.legacy(3).unwrap();
    paid.legacy_scan(5).unwrap();
    paid.charge(2).unwrap();
    assert_eq!(local.observation().work(), 10);
    assert_eq!(local.observation().legacy_visits(), 8);
    assert_eq!(external.work, 10);
    assert_eq!(external.events, vec![('w', 3), ('w', 5), ('w', 2)]);
}
#[test]
fn local_total_denial_precedes_external_debit_and_is_sticky() {
    let mut local = Budget::new(SemanticAssertionLimitsV1::new(7, 0)).unwrap();
    let mut external = Bounded::new(100, 0);
    assert_eq!(
        local.metered(&mut external).legacy_scan(8),
        Err(SemanticAssertionMeteredErrorV1::Analysis(
            SemanticAssertionErrorV1::WorkLimit {
                actual: 8,
                limit: 7
            }
        ))
    );
    assert_eq!(external.work, 0);
    assert!(external.events.is_empty());
    assert_eq!(
        local.metered(&mut external).charge(1),
        Err(SemanticAssertionMeteredErrorV1::Analysis(
            SemanticAssertionErrorV1::Accounting
        ))
    );
}

#[test]
fn indexed_scan_failure_keeps_the_full_neutral_debit() {
    let types = types();
    let function = assertion_function(constant(BOOL, 1), ordinary_message());
    let mut meter = Bounded::new(usize::MAX, usize::MAX);
    let mut analysis =
        SemanticAssertionAnalysisV1::new_metered(&types, &function, limits(), &mut meter).unwrap();
    analysis
        .resources
        .metered(&mut meter)
        .legacy(MAX_SEMANTIC_ASSERTION_WORK_V1 - 1)
        .unwrap();
    let before = meter.work;
    assert!(
        matches!(analysis.recipe_queries_v1(&mut meter).scan_equivalent(3),
        Err(SemanticAssertionMeteredErrorV1::Analysis(SemanticAssertionErrorV1::LogicalVisitLimit{actual,limit}))
        if actual==MAX_SEMANTIC_ASSERTION_WORK_V1+2 && limit==MAX_SEMANTIC_ASSERTION_WORK_V1)
    );
    assert_eq!(meter.work - before, 3);
    assert_eq!(analysis.resources().work(), meter.work);
}
#[test]
fn external_denial_and_storage_reconciliation_failure_cannot_recover_in_place() {
    let mut local = Budget::new(limits()).unwrap();
    let mut external = Bounded::new(0, 0);
    assert_eq!(
        local.metered(&mut external).charge(1),
        Err(SemanticAssertionMeteredErrorV1::Meter(Refusal::Work))
    );
    external.work_limit = 100;
    assert_eq!(
        local.metered(&mut external).charge(1),
        Err(SemanticAssertionMeteredErrorV1::Analysis(
            SemanticAssertionErrorV1::Accounting
        ))
    );
    let mut storage = Budget::new(limits()).unwrap();
    assert!(matches!(
        storage.metered(&mut external).backing::<u64>(1),
        Err(SemanticAssertionMeteredErrorV1::Meter(Refusal::Storage))
    ));
    external.storage_limit = 100;
    assert!(matches!(
        storage.metered(&mut external).backing::<u64>(1),
        Err(SemanticAssertionMeteredErrorV1::Analysis(
            SemanticAssertionErrorV1::Accounting
        ))
    ));
    let mut fresh = Budget::new(limits()).unwrap();
    assert_eq!(
        fresh
            .metered(&mut external)
            .backing::<u64>(1)
            .unwrap()
            .len(),
        0
    );
}
#[test]
fn checked_total_and_capacity_arithmetic_never_wrap() {
    let mut work = Budget::new(limits()).unwrap();
    let mut meter = Meter::default();
    work.metered(&mut meter).charge(usize::MAX).unwrap();
    assert!(matches!(
        work.metered(&mut meter).charge(1),
        Err(SemanticAssertionMeteredErrorV1::Analysis(
            SemanticAssertionErrorV1::Arithmetic
        ))
    ));
    let mut storage = Budget::new(limits()).unwrap();
    assert!(matches!(
        storage
            .metered(&mut Meter::default())
            .backing::<u64>(usize::MAX),
        Err(SemanticAssertionMeteredErrorV1::Analysis(
            SemanticAssertionErrorV1::Arithmetic
        ))
    ));
}
#[test]
fn requested_and_actual_capacity_payload_are_separate_accounting_steps() {
    let mut local = Budget::new(limits()).unwrap();
    let mut external = Bounded::new(usize::MAX, usize::MAX);
    let values = local.metered(&mut external).backing::<u64>(3).unwrap();
    assert_eq!(external.events[0], ('s', 3 * size_of::<u64>()));
    assert_eq!(
        external.events[1],
        ('s', (values.capacity() - 3) * size_of::<u64>())
    );
    assert_eq!(external.storage, values.capacity() * size_of::<u64>());
    assert_eq!(
        local.observation().reserved_payload_bytes(),
        external.storage
    );
}
fn constructor_storage() -> usize {
    // B=2,E=1,L=5,O=1. Formula is from allocations in CFG/census/checked index.
    size_of::<SemanticAssertionAnalysisV1<'_>>()
        + size_of::<SemanticAssertionCfgV1>()
        + 3 * size_of::<Vec<usize>>()
        + 4 * size_of::<Vec<usize>>()
        + 6 * size_of::<usize>()
        + 2 * size_of::<bool>()
        + 5 * size_of::<u8>()
        + 5 * size_of::<Option<ScalarAssignmentSiteV1>>()
        + 5 * size_of::<bool>()
        + 2 * size_of::<Vec<usize>>()
        + 4 * size_of::<usize>()
        + 5 * size_of::<Vec<usize>>()
}
#[test]
fn independent_literal_constructor_and_constant_query_resource_formula() {
    let types = types();
    let function = assertion_function(constant(BOOL, 1), ordinary_message());
    let storage = constructor_storage()
        + size_of::<AssertionRangeFrameV1>()
        + size_of::<Option<UnsignedRangeProofV1>>();
    let mut meter = Bounded::new(79, storage);
    let mut analysis = SemanticAssertionAnalysisV1::new_metered(
        &types,
        &function,
        SemanticAssertionLimitsV1::new(79, storage),
        &mut meter,
    )
    .unwrap();
    // Constructor W=1+34+33+9. Query adds one classification + one logical visit.
    assert_eq!(meter.work, 77);
    assert_eq!(meter.storage, constructor_storage());
    assert_eq!(
        &meter.events[..4],
        &[
            ('s', size_of::<SemanticAssertionAnalysisV1<'_>>()),
            ('w', 1),
            ('w', 4),
            (
                's',
                size_of::<SemanticAssertionCfgV1>() + 3 * size_of::<Vec<usize>>()
            )
        ]
    );
    assert!(matches!(
        analysis
            .assertion_at_v1(SemanticBlockIdV1::from_index(0), &mut meter)
            .unwrap(),
        SemanticAssertionOutcomeV1::Proved(_)
    ));
    assert_eq!(meter.work, 79);
    assert_eq!(meter.storage, storage);
    assert_eq!(analysis.resources().legacy_visits(), 1);
    assert_eq!(analysis.resources().work(), meter.work);
}
#[test]
fn independent_work_and_storage_boundaries_refuse_without_a_fact() {
    let types = types();
    let function = assertion_function(constant(BOOL, 1), ordinary_message());
    let mut empty = Bounded::new(usize::MAX, 0);
    assert!(matches!(
        SemanticAssertionAnalysisV1::new_metered(&types, &function, limits(), &mut empty)
            .err()
            .unwrap(),
        SemanticAssertionMeteredErrorV1::Meter(Refusal::Storage)
    ));
    assert_eq!(empty.work, 0);
    assert_eq!(empty.storage, 0);
    let mut meter = Bounded::new(usize::MAX, usize::MAX);
    let mut analysis = SemanticAssertionAnalysisV1::new_metered(
        &types,
        &function,
        SemanticAssertionLimitsV1::new(78, usize::MAX),
        &mut meter,
    )
    .unwrap();
    assert!(matches!(
        analysis
            .assertion_at_v1(SemanticBlockIdV1::from_index(0), &mut meter)
            .err()
            .unwrap(),
        SemanticAssertionMeteredErrorV1::Analysis(SemanticAssertionErrorV1::WorkLimit {
            actual: 79,
            limit: 78
        })
    ));
    assert!(matches!(
        analysis
            .assertion_at_v1(SemanticBlockIdV1::from_index(0), &mut meter)
            .err()
            .unwrap(),
        SemanticAssertionMeteredErrorV1::Analysis(SemanticAssertionErrorV1::Accounting)
    ));
}
#[test]
fn assertion_proof_cache_is_bounded_and_overwrites_without_growth() {
    let mut cache = HashMap::new();
    let mut budget = Budget::new(limits()).unwrap();
    let mut meter = Meter::default();
    insert_assertion_proof_cache_with_limit(
        budget.metered(&mut meter),
        &mut cache,
        (0, 0),
        true,
        2,
    )
    .unwrap();
    insert_assertion_proof_cache_with_limit(
        budget.metered(&mut meter),
        &mut cache,
        (1, 0),
        false,
        2,
    )
    .unwrap();
    let storage = meter.storage;
    insert_assertion_proof_cache_with_limit(
        budget.metered(&mut meter),
        &mut cache,
        (0, 0),
        false,
        2,
    )
    .unwrap();
    assert_eq!(cache.len(), 2);
    assert!(!cache[&(0, 0)]);
    assert_eq!(meter.storage, storage);
    assert!(matches!(
        insert_assertion_proof_cache_with_limit(
            budget.metered(&mut meter),
            &mut cache,
            (2, 0),
            true,
            2
        ),
        Err(SemanticAssertionMeteredErrorV1::Analysis(
            SemanticAssertionErrorV1::Unsupported(
                "assertion proof cache exceeds the bounded entry limit"
            )
        ))
    ));
}
struct Probe {
    credit: Arc<AtomicUsize>,
    dropped: Arc<AtomicUsize>,
}
impl Drop for Probe {
    fn drop(&mut self) {
        assert!(self.credit.load(Ordering::SeqCst) > 0);
        self.dropped.fetch_add(1, Ordering::SeqCst);
    }
}
struct PanicMeter {
    credit: Arc<AtomicUsize>,
    panic: bool,
    payload: Option<Probe>,
}
impl SemanticAssertionMeterV1 for PanicMeter {
    type Error = Refusal;
    fn charge_work(&mut self, _: usize) -> Result<(), Self::Error> {
        Ok(())
    }
    fn reserve_storage(&mut self, n: usize) -> Result<(), Self::Error> {
        if self.panic {
            std::panic::panic_any(self.payload.take().unwrap());
        }
        self.credit.fetch_add(n, Ordering::SeqCst);
        Ok(())
    }
}
#[test]
fn partial_backing_and_panic_payload_drop_before_caller_refund() {
    let credit = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));
    let mut meter = PanicMeter {
        credit: credit.clone(),
        panic: false,
        payload: Some(Probe {
            credit: credit.clone(),
            dropped: dropped.clone(),
        }),
    };
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut budget = Budget::new(limits()).unwrap();
        let mut rows = budget.metered(&mut meter).backing::<Probe>(1).unwrap();
        rows.push(Probe {
            credit: credit.clone(),
            dropped: dropped.clone(),
        });
        meter.panic = true;
        let _ = budget.metered(&mut meter).grow(&mut rows, 2);
    }))
    .err()
    .expect("fault meter panic");
    assert_eq!(dropped.load(Ordering::SeqCst), 1);
    assert!(credit.load(Ordering::SeqCst) > 0);
    drop(panic);
    assert_eq!(dropped.load(Ordering::SeqCst), 2);
    credit.store(0, Ordering::SeqCst);
}

// Required pinned-host/compiler premises, never successful-cost calibration.
#[test]
fn private_call_whole_entry_pinned_callable_layout_equivalence_premises() {
    use std::mem::{align_of, size_of};
    type Direct = (bool, bool, Vec<usize>);
    type Decision = DefinedCallableEmptyEffectDecisionV1;
    assert_eq!(size_of::<usize>(), 8);
    assert_eq!(
        size_of::<DefinedCallableDirectSummaryV1>(),
        size_of::<Direct>()
    );
    assert_eq!(
        align_of::<DefinedCallableDirectSummaryV1>(),
        align_of::<Direct>()
    );
    assert_eq!(size_of::<Decision>(), size_of::<u8>());
    assert_eq!(align_of::<Decision>(), align_of::<u8>());
}
