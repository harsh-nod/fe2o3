use super::{fixture::*, *};
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};

#[derive(Debug)]
struct Observation {
    result: Result<()>,
    work: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn measure(
    input: &Owner,
    history: Option<&CheckedScalarFixedPointOwnerV1>,
    floor: usize,
    w: usize,
    s: usize,
) -> Observation {
    let mut work = Work::new(w);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, s);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = match history {
            Some(history) => history.replay_against(input, &mut budget),
            None => prepare_checked_scalar_fixed_point_v1(input, &mut budget).map(|owned| {
                budget.reserve_storage(owned.retained_storage()).unwrap();
                release(owned, &mut budget);
            }),
        };
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    Observation {
        result,
        work: accepted,
        peak,
        failed_work: work.failed_work(),
        failed_storage,
    }
}

#[test]
fn exact_factory_and_replay_work_peak_and_final_work_shortfall_preserve_siblings() {
    for module in [
        Module::new("fixed"),
        reverse_chain(1),
        loop_kernel(false, true),
    ] {
        let (input, input_storage) = admit(&module);
        let sibling = vec![0x93; SIBLING];
        let floor = input_storage + size_of::<Vec<u8>>() + sibling.capacity();
        let full = measure(&input, None, floor, W, S);
        assert!(full.result.is_ok(), "{full:?}");
        let exact = measure(&input, None, floor, full.work, full.peak);
        assert!(exact.result.is_ok(), "{exact:?}");
        assert_eq!(
            (
                exact.work,
                exact.peak,
                exact.failed_work,
                exact.failed_storage
            ),
            (full.work, full.peak, None, None)
        );
        let short = measure(&input, None, floor, full.work - 1, full.peak);
        assert!(matches!(
            short.result,
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert_eq!(
            (short.work, short.failed_work, short.failed_storage),
            (full.work - 1, Some(full.work), None)
        );
        assert_eq!(sibling, vec![0x93; SIBLING]);
        let mut work = Work::new(W);
        let mut budget = Budget::new(&mut work, S);
        budget.reserve_storage(floor).unwrap();
        let history = finish(&input, &mut budget);
        let replay_floor = floor + history.retained_storage();
        let replay = measure(&input, Some(&history), replay_floor, W, S);
        assert!(replay.result.is_ok());
        let exact = measure(
            &input,
            Some(&history),
            replay_floor,
            replay.work,
            replay.peak,
        );
        assert!(exact.result.is_ok());
        assert_eq!((exact.work, exact.peak), (replay.work, replay.peak));
        let short = measure(
            &input,
            Some(&history),
            replay_floor,
            replay.work - 1,
            replay.peak,
        );
        assert!(matches!(
            short.result,
            Err(Error::Resource(Resource::Work(_)))
        ));
        assert_eq!(
            (short.work, short.failed_work),
            (replay.work - 1, Some(replay.work))
        );
        let short = measure(
            &input,
            Some(&history),
            replay_floor,
            replay.work,
            replay.peak - 1,
        );
        assert!(short.result.is_err());
        assert_eq!(short.failed_storage, Some(replay.peak));
        release(history, &mut budget);
    }
}

#[test]
fn every_outer_header_capacity_boundary_and_checked_arithmetic_rejects_before_next_phase() {
    let (input, input_storage) = admit(&Module::new("header-floors"));
    let floor = input_storage + SIBLING;
    let meter_header = size_of::<Meter<'_, '_>>();
    let wrapper = size_of::<CheckedScalarFixedPointOwnerV1>();
    let slots = SCALAR_FIXED_POINT_MAX_ROUNDS_V1 * size_of::<CheckedScalarFixedPointRoundV1>();
    for (required, work_before) in [
        (floor + meter_header, 0),
        (floor + meter_header + wrapper, 1),
        (floor + meter_header + wrapper + CHECK_SCRATCH, 1),
        (floor + meter_header + wrapper + CHECK_SCRATCH + slots, 5),
    ] {
        let short = measure(&input, None, floor, W, required - 1);
        assert!(matches!(
            short.result,
            Err(Error::Resource(Resource::Storage(_)))
        ));
        assert_eq!(
            (short.work, short.failed_work, short.failed_storage),
            (work_before, None, Some(required))
        );
    }
    assert!(matches!(
        retained(usize::MAX, &[]),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(floor).unwrap();
    let result: Result<()> = resources::scoped(&mut budget, |m: &mut Meter<'_, '_>| {
        let _ = m.table::<CheckedScalarFixedPointRoundV1>(usize::MAX)?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Arithmetic))));
    assert_eq!(budget.storage(), floor);
}

#[test]
fn compound_integer_and_scalar_transactions_restore_floor_before_receipt_adoption() {
    with_input(reverse_chain(1), |input, budget| {
        resources::scoped(budget, |m: &mut Meter<'_, '_>| -> Result<()> {
            let before = m.budget_for_test().storage();
            let integer = m.derive(|b| checked_integer(input, b))?;
            assert_eq!(m.budget_for_test().storage(), before);
            m.reserve(integer.storage().retained_storage())?;
            let before = m.budget_for_test().storage();
            let scalar = m.derive(|b| {
                optimize_checked_canonical_kernel_ir_policy3_v1(integer.owner(), b)
                    .map_err(Error::Scalar)
            })?;
            assert_eq!(m.budget_for_test().storage(), before);
            m.reserve(scalar.storage().retained_storage())?;
            drop(scalar);
            drop(integer);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn split_observed_adoption_is_an_accounting_negative_not_a_supported_protocol() {
    let (input, retained) = admit(&reverse_chain(1));
    let floor = retained + SIBLING;
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(floor).unwrap();
    let result: Result<()> = resources::scoped(&mut budget, |m: &mut Meter<'_, '_>| {
        let observed = m.derive(|b| {
            optimize_native_neutral_kernel_ir_integer_continuation_v1(&input, b)
                .map_err(Error::IntegerObservation)
        })?;
        m.reserve(observed.storage().retained_storage())?;
        let _invalid = m.derive(|b| {
            observed
                .try_check_and_finish_v1(b)
                .map_err(Error::IntegerCheck)
        })?;
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    // The unsupported sequence removed tracked live storage. Meter correctly
    // refuses to invent a refund; this isolated test owns the known remainder.
    assert_eq!(budget.storage(), floor + size_of::<Meter<'_, '_>>());
    budget.release_storage(size_of::<Meter<'_, '_>>()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn inherited_bridge_tree_cap_is_typed_and_discards_the_entire_attempt() {
    use fe2o3_kernel_ir::{BarrierSemantics, Fence, MemoryOrdering, SynchronizationScope};
    use fe2o3_pliron::{KirBridgeErrorV1, KirBridgeErrorV12, OperationHandleError};
    let mut entry = block(0, Terminator::Return { values: vec![] });
    entry.operations = (0..16_384)
        .map(|_| {
            Operation::new(
                vec![],
                Kind::Fence(Fence {
                    memory_scope: SynchronizationScope::Device,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::Release,
                        [AddressSpace::Global],
                    ),
                }),
            )
        })
        .collect();
    let mut module = Module::new("scalar-inherited-graph-cap");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    // Existing conservative profiles charge logical work far above traversal
    // count for this large structural negative. Keep the storage ceiling finite.
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, S);
    let (input, receipt) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    drop(module);
    let floor = receipt.retained_storage() + SIBLING;
    budget.reserve_storage(floor).unwrap();
    let bytes = input.canonical().canonical_bytes().to_vec();
    // Factory admission, table setup and the first round charge 1 + 4 + 1.
    // Native import prepays these source bytes, then its census refuses the
    // tree before envelope reservation, session allocation or pass execution.
    let expected_work = budget
        .work()
        .checked_add(1 + 4 + 1)
        .and_then(|work| work.checked_add(bytes.len()))
        .unwrap();
    let error = prepare_checked_scalar_fixed_point_v1(&input, &mut budget).unwrap_err();
    assert!(
        matches!(
            error,
            Error::IntegerObservation(KirNeutralOptimizationErrorV1::Bridge(
                KirBridgeErrorV12::Bridge(KirBridgeErrorV1::Session(
                    OperationHandleError::OperationTreeLimitExceeded
                ))
            ))
        ),
        "{error:?}"
    );
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.failed_storage(), None);
    assert_eq!(budget.work(), expected_work);
    assert_eq!(input.canonical().canonical_bytes(), bytes);
    drop(input);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), SIBLING);
    drop(budget);
    assert_eq!(work.failed_work(), None);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Headers,
    Integer,
    IntegerReceipt,
    Scalar,
    ScalarReceipt,
    Joins,
    Finish,
}
#[derive(Debug)]
struct PhaseObservation {
    phase: Phase,
    before: usize,
    after: usize,
    peak: usize,
}
fn phase<T>(
    m: &mut Meter<'_, '_>,
    observations: &mut Vec<PhaseObservation>,
    kind: Phase,
    run: impl FnOnce(&mut Meter<'_, '_>) -> Result<T>,
) -> Result<T> {
    let before = m.budget_for_test().work();
    let value = run(m)?;
    observations.push(PhaseObservation {
        phase: kind,
        before,
        after: m.budget_for_test().work(),
        peak: m.budget_for_test().peak_storage(),
    });
    Ok(value)
}

// Independent successful prefix oracle: uses the actual constituent APIs and
// explicit source-defined orchestration charges, never the factory under test
// or a failed run's reported work as its expected phase/work oracle.
fn oracle(input: &Owner, floor: usize) -> (usize, usize, Vec<PhaseObservation>) {
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(floor).unwrap();
    let mut phases = Vec::new();
    resources::scoped(&mut budget, |m: &mut Meter<'_, '_>| -> Result<()> {
        let (mut rounds, _) = phase(m, &mut phases, Phase::Headers, |m| {
            m.work(1)?;
            m.reserve(size_of::<CheckedScalarFixedPointOwnerV1>())?;
            m.reserve(CHECK_SCRATCH)?;
            m.table::<CheckedScalarFixedPointRoundV1>(SCALAR_FIXED_POINT_MAX_ROUNDS_V1)
        })?;
        for index in 0..SCALAR_FIXED_POINT_MAX_ROUNDS_V1 {
            m.work(1)?;
            let source = rounds
                .last()
                .map_or(input, CheckedScalarFixedPointRoundV1::output);
            let integer = phase(m, &mut phases, Phase::Integer, |m| {
                m.derive(|b| {
                    let observed =
                        optimize_native_neutral_kernel_ir_integer_continuation_v1(source, b)
                            .map_err(Error::IntegerObservation)?;
                    b.reserve_storage(observed.storage().retained_storage())?;
                    observed
                        .try_check_and_finish_v1(b)
                        .map_err(Error::IntegerCheck)
                })
            })?;
            phase(m, &mut phases, Phase::IntegerReceipt, |m| {
                m.reserve(integer.storage().retained_storage())
            })?;
            let scalar = phase(m, &mut phases, Phase::Scalar, |m| {
                m.derive(|b| {
                    optimize_checked_canonical_kernel_ir_policy3_v1(integer.owner(), b)
                        .map_err(Error::Scalar)
                })
            })?;
            phase(m, &mut phases, Phase::ScalarReceipt, |m| {
                m.reserve(scalar.storage().retained_storage())
            })?;
            let terminal = phase(m, &mut phases, Phase::Joins, |m| {
                replay::headers(source, &integer, &scalar, m)?;
                equal_bytes(
                    source.canonical().canonical_bytes(),
                    scalar.owner().canonical().canonical_bytes(),
                    m,
                )
            })?;
            m.push(
                &mut rounds,
                CheckedScalarFixedPointRoundV1 {
                    ordinal: index as u16,
                    integer,
                    scalar,
                },
            )?;
            if terminal {
                phase(m, &mut phases, Phase::Finish, |m| {
                    let _record = record(input, rounds.last().unwrap().output(), rounds.len(), m)?;
                    m.work(rounds.len() * 3)?;
                    let _retained = retained(rounds.capacity(), &rounds)?;
                    m.work(1)
                })?;
                return Ok(());
            }
        }
        panic!("oracle fixture must converge");
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
    (budget.work(), budget.peak_storage(), phases)
}

#[test]
fn interior_peak_shortfall_belongs_to_the_independently_observed_constituent_phase() {
    let (input, retained) = admit(&reverse_chain(7));
    let floor = retained + SIBLING;
    let (work, peak, phases) = oracle(&input, floor);
    let full = measure(&input, None, floor, W, S);
    assert!(full.result.is_ok());
    assert_eq!((full.work, full.peak), (work, peak));
    let expected = phases.iter().find(|row| row.peak == peak).unwrap();
    assert!(
        matches!(expected.phase, Phase::Integer | Phase::Scalar),
        "must observe a real interior constituent peak, got {expected:?}"
    );
    let short = measure(&input, None, floor, work, peak - 1);
    assert!(short.result.is_err());
    assert_eq!(
        (short.failed_storage, short.failed_work),
        (Some(peak), None)
    );
    assert!(
        (expected.before..=expected.after).contains(&short.work),
        "wrong independent phase interval: {expected:?}, {short:?}"
    );
    match expected.phase {
        Phase::Integer => assert!(matches!(
            short.result,
            Err(Error::IntegerObservation(_)) | Err(Error::IntegerCheck(_))
        )),
        Phase::Scalar => assert!(matches!(short.result, Err(Error::Scalar(_)))),
        _ => unreachable!(),
    }
}

#[test]
fn every_successful_work_prefix_refuses_or_finishes_without_storage_drift() {
    // Bound the sweep by actual high-level phase boundaries, including interior
    // integer/checker/Policy3 work; do not perform a quadratic per-unit fuzz loop.
    let (input, retained) = admit(&reverse_chain(1));
    let floor = retained + SIBLING;
    let (work, peak, phases) = oracle(&input, floor);
    let mut limits = vec![0, 1, work - 1, work];
    for phase in phases {
        limits.extend([phase.before, phase.after.saturating_sub(1), phase.after]);
    }
    limits.sort_unstable();
    limits.dedup();
    for limit in limits {
        let outcome = measure(&input, None, floor, limit, peak);
        assert_eq!(outcome.result.is_ok(), limit == work, "{outcome:?}");
        assert!(outcome.work <= limit);
        assert_eq!(outcome.failed_work.is_some(), limit < work);
        assert_eq!(outcome.failed_storage, None);
    }
}

#[test]
fn panic_payload_cleanup_and_foreign_ledger_preserve_original_ownership_boundary() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct Payload(Arc<AtomicBool>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
            panic!("test payload destructor");
        }
    }
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(SIBLING).unwrap();
    let dropped = Arc::new(AtomicBool::new(false));
    let caught = catch_unwind(AssertUnwindSafe(|| {
        let _: Result<()> = resources::scoped(&mut budget, |m: &mut Meter<'_, '_>| {
            m.reserve(19)?;
            m.work(7)?;
            m.derive(|b| {
                b.reserve_storage(11)?;
                panic_any(Payload(dropped.clone()));
            })
        });
    }));
    assert!(caught.is_err());
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!((budget.storage(), budget.work()), (SIBLING, 7));
    let mut foreign_work = Work::new(W);
    let mut foreign = Budget::new(&mut foreign_work, S);
    foreign.reserve_storage(37).unwrap();
    foreign.charge_work(13).unwrap();
    let old = budget.work_ledger_identity_v1();
    let result: Result<()> = resources::scoped(&mut budget, |m: &mut Meter<'_, '_>| {
        m.reserve(23)?;
        m.work(5)?;
        std::mem::swap(m.budget_for_test(), &mut foreign);
        Ok(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert!(budget.work_ledger_identity_v1() != old);
    assert_eq!((budget.storage(), budget.work()), (37, 13));
    assert!(foreign.work_ledger_identity_v1() == old);
    assert_eq!(foreign.work(), 12);
    assert_eq!(foreign.storage(), SIBLING + size_of::<Meter<'_, '_>>() + 23);
    // Only the test that owns this displaced ledger may release its known
    // reservations; production must never refund the replacement ledger.
    foreign
        .release_storage(size_of::<Meter<'_, '_>>() + 23)
        .unwrap();
    assert_eq!(foreign.storage(), SIBLING);
    assert_eq!((budget.storage(), budget.work()), (37, 13));
}

#[test]
fn iteration_refusal_drops_all_owners_without_resetting_work_or_peak() {
    with_input(reverse_chain(1), |input, budget| {
        let floor = budget.storage();
        let before = budget.work();
        let result = prepare(input, 2, budget);
        assert!(matches!(
            result,
            Err(Error::IterationLimit {
                completed: 2,
                limit: 2
            })
        ));
        assert_eq!(budget.storage(), floor);
        assert!(budget.work() > before && budget.peak_storage() > floor);
        assert!(budget.failed_storage().is_none());
    });
}

#[derive(Clone, Copy, Debug)]
struct ExactMapCut {
    required: usize,
    prior_peak: usize,
    accepted: usize,
    before_last_charge: usize,
}

// Map replay's endpoint census reads only these immutable module header lengths.
// On this plain-integer fixture, integer/DCE creates no map nodes: registration
// is exactly the original endpoint roster, even when those nodes are later dead.
fn endpoint_census_reference(module: &Module) -> (usize, usize) {
    let mut work = 1 + module.functions.len();
    let mut endpoints = 0;
    for function in &module.functions {
        let Some(body) = &function.body else { continue };
        work += 1 + body.blocks.len();
        endpoints += body.parameters.len();
        for block in &body.blocks {
            work += 1 + block.operations.len();
            endpoints += block.parameters.len() + usize::from(block.terminator.is_some());
            endpoints += block
                .operations
                .iter()
                .map(|op| 1 + op.results.len())
                .sum::<usize>();
        }
    }
    (endpoints, work)
}

fn exact_replay_map_reference(
    input: &Owner,
    history: &CheckedScalarFixedPointOwnerV1,
    floor: usize,
) -> ExactMapCut {
    use fe2o3_pliron::read_unauthenticated_policy3_execution_claim_v1;
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(floor).unwrap();
    let first = &history.rounds()[0];
    assert!(operations(input).all(|op| matches!(
        op.kind,
        Kind::Constant(Constant::U32(_))
            | Kind::Binary {
                op: BinaryOp::BitOr,
                ..
            }
    )));
    assert!(first.integer().map().synthesized_operations().is_empty());
    let cut = resources::scoped(
        &mut budget,
        |m: &mut Meter<'_, '_>| -> Result<ExactMapCut> {
            // The pinned scalar replay orchestration, not calls to its private
            // headers/record/equality functions. All genuine public witnesses remain
            // prepaid on this ledger; the source/history are not copied or replaced.
            m.work(4)?;
            m.work(history.rounds().len() * 3)?;
            m.reserve(CHECK_SCRATCH)?;
            m.work(SCALAR_FIXED_POINT_EXECUTION_BYTES_V1 * 2)?;
            m.work(SCALAR_FIXED_POINT_EXECUTION_BYTES_V1 * 2 + 1)?;
            m.work(1)?;
            m.work(
                input.canonical().canonical_bytes().len()
                    + first.integer().native_input_audit_bytes().len()
                    + 1,
            )?;
            m.work(
                first.integer().owner().canonical().canonical_bytes().len()
                    + first.scalar().native_input_audit_bytes().len()
                    + 1,
            )?;
            m.work(2 * 2 + 1)?;
            m.work(8 * 2 + 1)?;
            m.work(2 * (2 + 8) + 4 + 160)?;
            m.derive(|b| {
                first.integer().execution().check_against(
                    input,
                    first.integer().owner(),
                    first.integer().report(),
                    first.integer().map(),
                    b,
                )?;
                read_unauthenticated_policy3_execution_claim_v1(
                    first.integer().owner(),
                    first.output(),
                    first.scalar().execution().canonical_bytes(),
                    b,
                )
                .map_err(Error::ScalarExecution)?;
                Ok(())
            })?;
            let (nodes, input_census) = endpoint_census_reference(input.module());
            let (_, output_census) = endpoint_census_reference(first.integer().owner().module());
            let before_last_charge =
                m.budget_for_test().work() + 1 + nodes + input_census + output_census;
            let prior_peak = m.budget_for_test().peak_storage();
            // CaptureLimitsV12::for_policy_bytes(Integer6): N=min(2L+64,131072),
            // E=T=10N, storage=512N+64E+64T+4096. These are closed profile terms,
            // not measured failed-run totals or a factory/replay aggregate oracle.
            let bound = (input.canonical().canonical_bytes().len() * 2 + 64).min(131_072);
            let required = m.budget_for_test().storage() + bound * (512 + 64 * 10 + 64 * 10) + 4096;
            m.derive(|b| {
                first
                    .integer()
                    .map()
                    .check_against(input, first.integer().owner(), b)
                    .map_err(Error::Map)
            })?;
            assert_eq!(m.budget_for_test().peak_storage(), required);
            assert!(prior_peak < required);
            let accepted = m.budget_for_test().work();
            assert!(before_last_charge < accepted);
            // check_against has no charge after its single reservation, making its
            // successful final work the exact accepted prefix at that reservation.
            Ok(ExactMapCut {
                required,
                prior_peak,
                accepted,
                before_last_charge,
            })
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    let failed_storage = budget.failed_storage();
    drop(budget);
    assert_eq!((failed_storage, work.failed_work()), (None, None));
    cut
}

#[test]
fn exact_replay_global_peak_and_adjacent_work_charges_use_successful_public_prefix() {
    let (input, retained) = admit(&reverse_chain(7));
    let input_bytes = input.canonical().canonical_bytes().to_vec();
    let sibling = vec![0x61; SIBLING];
    let floor = retained + size_of::<Vec<u8>>() * 2 + input_bytes.capacity() + sibling.capacity();
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(floor).unwrap();
    let history = finish(&input, &mut budget);
    let replay_floor = floor + history.retained_storage();
    let cut = exact_replay_map_reference(&input, &history, replay_floor);
    let full = measure(&input, Some(&history), replay_floor, W, S);
    assert!(full.result.is_ok(), "{full:?}");
    assert_eq!(
        full.peak, cut.required,
        "selected interior map must be the REAL global replay peak"
    );

    let short = measure(
        &input,
        Some(&history),
        replay_floor,
        full.work,
        cut.required - 1,
    );
    assert!(
        matches!(&short.result,
        Err(Error::Map(KirOptimizationMapErrorV12::Resources(Resource::Storage(error))))
        if error.actual() == cut.required && error.limit() == cut.required - 1),
        "{short:?}"
    );
    assert_eq!(
        (
            short.work,
            short.peak,
            short.failed_work,
            short.failed_storage
        ),
        (cut.accepted, cut.prior_peak, None, Some(cut.required))
    );

    let before = measure(
        &input,
        Some(&history),
        replay_floor,
        cut.accepted - 1,
        cut.required,
    );
    assert!(
        matches!(&before.result,
        Err(Error::Map(KirOptimizationMapErrorV12::Resources(Resource::Work(error))))
        if error.actual() == cut.accepted && error.limit() == cut.accepted - 1),
        "{before:?}"
    );
    assert_eq!(
        (
            before.work,
            before.peak,
            before.failed_work,
            before.failed_storage
        ),
        (
            cut.before_last_charge,
            cut.prior_peak,
            Some(cut.accepted),
            None
        )
    );
    // The following pair's inventory census first charges one, then counts its
    // first function with another charge of one. Neither allocates beforehand.
    for extra in 0..=1 {
        let next = measure(
            &input,
            Some(&history),
            replay_floor,
            cut.accepted + extra,
            cut.required,
        );
        assert!(
            matches!(&next.result,
            Err(Error::Inventory(CanonicalKirInventoryErrorV1::Resource(Resource::Work(error))))
            if error.actual() == cut.accepted + extra + 1 && error.limit() == cut.accepted + extra),
            "{next:?}"
        );
        assert_eq!(
            (next.work, next.peak, next.failed_work, next.failed_storage),
            (
                cut.accepted + extra,
                cut.required,
                Some(cut.accepted + extra + 1),
                None
            )
        );
    }
    assert_eq!(input.canonical().canonical_bytes(), input_bytes);
    assert_eq!(sibling, vec![0x61; SIBLING]);
    release(history, &mut budget);
    assert_eq!(budget.storage(), floor);
}

#[derive(Debug)]
struct SuccessfulCheckpoint {
    phase: &'static str,
    work: usize,
    storage: usize,
    peak: usize,
}
fn checkpoint(phase: &'static str, budget: &Budget<'_>) -> SuccessfulCheckpoint {
    SuccessfulCheckpoint {
        phase,
        work: budget.work(),
        storage: budget.storage(),
        peak: budget.peak_storage(),
    }
}

// A separate successful constituent reference over the same actual borrowed
// input/output/rows. Its floor pays their source/observed/sibling custody. It
// allocates and pays each borrowed inventory and checked view through its entire
// lifetime, then drops each before refund. It never runs a failed cut or optimizer.
fn adoption_reference(
    input: &Owner,
    output: &Owner,
    rows: fe2o3_kernel_ir::CanonicalKirTransitionCandidateV1<'_>,
    floor: usize,
) -> Vec<SuccessfulCheckpoint> {
    use fe2o3_kernel_analysis::{CanonicalKirInventoryV1, check_canonical_kir_transition_v1};
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(1).unwrap();
    let mut log = vec![checkpoint("check-entry", &budget)];
    let (before, bs) = CanonicalKirInventoryV1::derive(input, &mut budget).unwrap();
    log.push(checkpoint("input-inventory-derived", &budget));
    budget.reserve_storage(bs.retained_storage()).unwrap();
    log.push(checkpoint("input-inventory-paid", &budget));
    let (after, after_s) = CanonicalKirInventoryV1::derive(output, &mut budget).unwrap();
    log.push(checkpoint("output-inventory-derived", &budget));
    budget.reserve_storage(after_s.retained_storage()).unwrap();
    log.push(checkpoint("output-inventory-paid", &budget));
    let (checked, receipt) =
        check_canonical_kir_transition_v1(&before, &after, rows, &mut budget).unwrap();
    log.push(checkpoint("transition-derived", &budget));
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    log.push(checkpoint("transition-callback-boundary", &budget));
    drop(checked);
    budget.release_storage(receipt.retained_storage()).unwrap();
    drop(after);
    budget.release_storage(after_s.retained_storage()).unwrap();
    drop(before);
    budget.release_storage(bs.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    log
}

fn factory_successful_checkpoints(
    input: &Owner,
    floor: usize,
) -> (usize, usize, Vec<SuccessfulCheckpoint>) {
    use fe2o3_pliron::optimize_native_neutral_kernel_ir_policy3_v1;
    use std::convert::Infallible;
    let mut work = Work::new(W);
    let mut budget = Budget::new(&mut work, S);
    budget.reserve_storage(floor).unwrap();
    let mut log = Vec::new();
    resources::scoped(&mut budget, |m: &mut Meter<'_, '_>| -> Result<()> {
        m.work(1)?;
        m.reserve(size_of::<CheckedScalarFixedPointOwnerV1>())?;
        m.reserve(CHECK_SCRATCH)?;
        let (mut rounds, _) =
            m.table::<CheckedScalarFixedPointRoundV1>(SCALAR_FIXED_POINT_MAX_ROUNDS_V1)?;
        for ordinal in 0..SCALAR_FIXED_POINT_MAX_ROUNDS_V1 {
            m.work(1)?;
            let source = rounds
                .last()
                .map_or(input, CheckedScalarFixedPointRoundV1::output);
            let integer = m.derive(|b| {
                log.push(checkpoint("integer-observation-before", b));
                let observed = optimize_native_neutral_kernel_ir_integer_continuation_v1(source, b)
                    .map_err(Error::IntegerObservation)?;
                log.push(checkpoint("integer-observation-after", b));
                b.reserve_storage(observed.storage().retained_storage())?;
                log.push(checkpoint("integer-observed-receipt-paid", b));
                let reference = adoption_reference(
                    source,
                    observed.owner(),
                    observed.occurrences().candidate(),
                    b.storage(),
                );
                let start = b.work();
                let (checked, (), _) = observed
                    .try_check_and_finish_with_v1(b, |_, callback| {
                        let expected = reference.last().unwrap();
                        assert_eq!(
                            (callback.work() - start, callback.storage()),
                            (expected.work, expected.storage)
                        );
                        assert!(callback.peak_storage() >= expected.peak);
                        log.push(checkpoint("integer-successful-check-callback", callback));
                        Ok::<_, Infallible>(((), 0))
                    })
                    .map_err(Error::IntegerCheck)?;
                assert_eq!(
                    b.work() - start,
                    reference.last().unwrap().work + source.canonical().canonical_bytes().len() + 2
                );
                log.push(checkpoint("integer-audit-copy-complete", b));
                for mut row in reference {
                    row.work += start;
                    log.push(row);
                }
                Ok(checked)
            })?;
            m.reserve(integer.storage().retained_storage())?;
            log.push(checkpoint(
                "integer-checked-receipt-paid",
                m.budget_for_test(),
            ));
            let scalar = m.derive(|b| {
                log.push(checkpoint("scalar-observation-before", b));
                let observed = optimize_native_neutral_kernel_ir_policy3_v1(integer.owner(), b)
                    .map_err(|e| {
                        Error::Scalar(KernelIrCheckedOptimizationErrorV1::Observation(e))
                    })?;
                log.push(checkpoint("scalar-observation-after", b));
                b.reserve_storage(observed.storage().retained_storage())?;
                log.push(checkpoint("scalar-observed-receipt-paid", b));
                b.charge_work(1)?;
                assert!(std::ptr::eq(integer.owner(), observed.input()));
                let reference = adoption_reference(
                    integer.owner(),
                    observed.owner(),
                    observed.occurrences().candidate(),
                    b.storage(),
                );
                let start = b.work();
                let (checked, (), _) = observed
                    .try_check_and_finish_with_v1(b, |_, callback| {
                        let expected = reference.last().unwrap();
                        assert_eq!(
                            (callback.work() - start, callback.storage()),
                            (expected.work, expected.storage)
                        );
                        assert!(callback.peak_storage() >= expected.peak);
                        log.push(checkpoint("scalar-successful-check-callback", callback));
                        Ok::<_, Infallible>(((), 0))
                    })
                    .map_err(|e| Error::Scalar(KernelIrCheckedOptimizationErrorV1::Check(e)))?;
                assert_eq!(
                    b.work() - start,
                    reference.last().unwrap().work
                        + integer.owner().canonical().canonical_bytes().len()
                        + 2
                );
                log.push(checkpoint("scalar-audit-copy-complete", b));
                for mut row in reference {
                    row.work += start;
                    log.push(row);
                }
                Ok(checked)
            })?;
            m.reserve(scalar.storage().retained_storage())?;
            log.push(checkpoint(
                "scalar-checked-receipt-paid",
                m.budget_for_test(),
            ));
            replay::headers(source, &integer, &scalar, m)?;
            let terminal = equal_bytes(
                source.canonical().canonical_bytes(),
                scalar.owner().canonical().canonical_bytes(),
                m,
            )?;
            m.push(
                &mut rounds,
                CheckedScalarFixedPointRoundV1 {
                    ordinal: ordinal as u16,
                    integer,
                    scalar,
                },
            )?;
            log.push(checkpoint("joined-round-paid", m.budget_for_test()));
            if terminal {
                let _ = record(input, rounds.last().unwrap().output(), rounds.len(), m)?;
                m.work(rounds.len() * 3)?;
                let _ = retained(rounds.capacity(), &rounds)?;
                m.work(1)?;
                log.push(checkpoint(
                    "complete-constituent-schedule",
                    m.budget_for_test(),
                ));
                return Ok(());
            }
        }
        panic!("successful checkpoint fixture must converge");
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
    (budget.work(), budget.peak_storage(), log)
}

#[test]
fn successful_factory_checkpoints_expose_opaque_peak_without_claiming_exact_failed_prefix() {
    let (input, retained) = admit(&reverse_chain(7));
    let sibling = vec![0x6b; SIBLING];
    let source = input.canonical().canonical_bytes().to_vec();
    let floor = retained + size_of::<Vec<u8>>() * 2 + sibling.capacity() + source.capacity();
    let (work, peak, log) = factory_successful_checkpoints(&input, floor);
    let full = measure(&input, None, floor, W, S);
    assert!(full.result.is_ok(), "{full:?}");
    assert_eq!((full.work, full.peak), (work, peak));
    let first_peak = log.iter().find(|row| row.peak == peak).unwrap();
    println!(
        "SCALAR_FACTORY_SUCCESSFUL_GLOBAL_PEAK phase={} checkpoint={first_peak:?} roster={log:?}",
        first_peak.phase
    );
    assert_eq!(sibling, vec![0x6b; SIBLING]);
    assert_eq!(input.canonical().canonical_bytes(), source);
    // This is deliberately a successful checkpoint diagnostic. The existing
    // coarse factory P-1 test remains, but neither asserts a fabricated interior
    // accepted-work total when the observer API does not expose that cut.
}
