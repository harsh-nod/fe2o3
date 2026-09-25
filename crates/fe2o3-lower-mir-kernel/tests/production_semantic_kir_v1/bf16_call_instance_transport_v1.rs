//! Full inert owners exercise the public borrowed checker, not source authenticity.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_lower_mir_kernel::{
    Bf16CallInstanceErrorV1 as Error, Bf16CallInstanceRoleV1 as Role, CheckedBf16CallInstanceV1,
    with_checked_bf16_call_instance_v1,
};
use fe2o3_pliron::{ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1};

#[path = "bf16_call_instance_fixture_v1.rs"]
mod fixture;
use fixture::Case;

const WORK: usize = 4_000_000;
const STORAGE: usize = 4 * 1024 * 1024;
const PREFIX: usize = 17;
const PREFIX_WORK: usize = 7;

#[derive(Debug)]
struct Observation {
    result: Result<u8, Error>,
    callbacks: usize,
    work: usize,
    peak: usize,
    capture_work: usize,
    capture_peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}

fn assert_view(
    owner: &ProductionSemanticSsaOwnerV1,
    view: &CheckedBf16CallInstanceV1<'_>,
    case: Case,
) {
    assert!(std::ptr::eq(owner, view.owner()));
    assert_eq!(view.root(), SemanticFunctionIdV1::from_index(0));
    assert_eq!(view.helper(), SemanticFunctionIdV1::from_index(1));
    assert_eq!(view.call_block(), SemanticBlockIdV1::from_index(5));
    assert_eq!(view.return_permutation(), case.permutation());
    assert!(std::ptr::eq(
        view.helper_declaration(),
        &owner.source_semantic().functions()[1],
    ));
    let SemanticTerminatorKindV1::Call(call) = owner.source_semantic().functions()[0].blocks()[5]
        .terminator()
        .kind()
    else {
        panic!("fixture's actual call disappeared");
    };
    assert!(std::ptr::eq(call, view.source_call()));
    assert_eq!(view.source_call().arguments().len(), 4);
    for index in 0..4 {
        assert!(view.call_argument_ssa(index).is_some());
        assert!(view.formal_ssa(index).is_some());
    }
    assert!(view.call_argument_ssa(4).is_none());
    assert!(view.formal_ssa(4).is_none());
    for (role, function, block) in [
        (Role::Context, 0, 0),
        (Role::Lane, 0, 1),
        (Role::Lhs, 0, 2),
        (Role::Rhs, 0, 3),
        (Role::Zero, 0, 4),
        (Role::Result, 1, 0),
        (Role::Values, 1, 1),
    ] {
        let producer = view.producer(role);
        assert_eq!(
            producer.function(),
            SemanticFunctionIdV1::from_index(function)
        );
        assert_eq!(producer.block(), SemanticBlockIdV1::from_index(block));
        // Join the returned value to actual captured definitions, not a guessed
        // ordinal. The checker owns the unique source-role proof.
        let rows = owner
            .occurrences_v1()
            .unwrap()
            .function(producer.function())
            .unwrap();
        let count = rows
            .edge_definitions()
            .iter()
            .filter(|row| {
                row.edge().source().get() == block
                    && row.is_reachable()
                    && row.is_promoted()
                    && row.value() == Some(producer.value())
            })
            .count();
        assert_eq!(count, 1, "actual producer edge definition");
    }
    for (index, role) in [(1, Role::Lhs), (2, Role::Rhs), (3, Role::Zero)] {
        assert_eq!(
            view.call_argument_ssa(index),
            Some(view.producer(role).value())
        );
    }
    let entries = owner
        .occurrences_v1()
        .unwrap()
        .function(view.helper())
        .unwrap();
    for index in 0..4 {
        let actual = entries
            .entry_definitions()
            .iter()
            .find(|entry| {
                entry.origin() == fe2o3_pliron::ProductionSemanticSsaEntryOriginV1::Argument(index)
            })
            .expect("actual helper argument entry");
        assert_eq!(view.formal_ssa(index as usize), actual.value());
    }
}

fn run(case: Case, work_limit: usize, storage_limit: usize) -> Observation {
    // MIR/SSA constructors retain their existing independent limits. The
    // occurrence capture and borrowed relation below share one original meter.
    let mut owner = fixture::owner(case);
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.charge_work(PREFIX_WORK).unwrap();
    budget.reserve_storage(PREFIX).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let receipt = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .expect("actual occurrence capture; earlier refusal is not a checker result");
    assert_eq!(
        budget.storage(),
        PREFIX,
        "capture returns an unreserved retained receipt"
    );
    let retained = receipt.retained_storage();
    assert!(retained > 0);
    budget
        .reserve_storage(retained)
        .expect("retain actual capture once");
    let floor = budget.storage();
    let capture_work = budget.work();
    let capture_peak = budget.peak_storage();
    let mut callbacks = 0;
    let result = with_checked_bf16_call_instance_v1(&owner, &mut budget, |view, original| {
        callbacks += 1;
        assert!(original.work_ledger_identity_v1() == identity);
        assert!(
            original.storage() > floor,
            "paid private checker scratch is live"
        );
        assert_view(&owner, view, case);
        Ok(9)
    });
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!(budget.storage(), floor, "only checker scratch is released");
    assert!(budget.work() >= capture_work);
    let observation = Observation {
        result,
        callbacks,
        work: budget.work(),
        peak: budget.peak_storage(),
        capture_work,
        capture_peak,
        failed_work: budget.failed_work(),
        failed_storage: budget.failed_storage(),
    };
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(
        budget.storage(),
        PREFIX,
        "caller drops its owner before releasing its receipt"
    );
    assert_eq!(budget.work(), observation.work, "work is never refunded");
    assert_eq!(budget.failed_work(), observation.failed_work);
    assert_eq!(budget.failed_storage(), observation.failed_storage);
    observation
}

fn baseline(case: Case) -> Observation {
    let result = run(case, WORK, STORAGE);
    assert!(matches!(result.result, Ok(9)), "{result:?}");
    assert_eq!(result.callbacks, 1);
    assert_eq!(result.failed_work, None);
    assert_eq!(result.failed_storage, None);
    result
}

fn refusal(case: Case, expected: &'static str) {
    // A valid full owner must reach the callback before any negative control
    // can be counted. No inert-record predicate substitutes for this baseline.
    baseline(Case::Identity);
    let result = run(case, WORK, STORAGE);
    assert!(
        matches!(result.result, Err(Error::Unavailable(actual)) if actual == expected),
        "expected exact checker boundary {expected:?}; got {result:?}"
    );
    assert_eq!(result.callbacks, 0);
    assert_eq!(result.failed_work, None);
    assert_eq!(result.failed_storage, None);
}

#[test]
fn bf16_full_owner_identity_and_swap01_borrow_exact_captured_owners() {
    baseline(Case::Identity);
    baseline(Case::Swap01);
}

#[test]
fn bf16_full_owner_copy_instead_of_nominal_move_refuses_before_callback() {
    refusal(
        Case::CopyRootFragment,
        "nominal arguments require actual whole-value moves",
    );
}

#[test]
fn bf16_full_owner_duplicate_returned_component_refuses_before_callback() {
    refusal(
        Case::DuplicateReturnedComponent,
        "return is neither Identity nor Swap01",
    );
}

#[test]
fn bf16_full_owner_conditional_helper_refuses_before_callback() {
    refusal(
        Case::ConditionalHelper,
        "helper conditional/exceptional/extra terminal",
    );
}

#[test]
fn bf16_full_owner_unreachable_helper_block_refuses_before_callback() {
    refusal(
        Case::UnreachableHelperBlock,
        "unreachable extra helper blocks",
    );
}

#[test]
fn bf16_full_owner_helper_cycle_refuses_before_callback() {
    refusal(
        Case::CyclicHelper,
        "looping source unavailable for first inspection profile",
    );
}

#[test]
fn bf16_full_owner_missing_capture_is_not_reconstructed_from_source() {
    baseline(Case::Identity);
    let owner = fixture::owner(Case::Identity);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(PREFIX).unwrap();
    let mut callbacks = 0;
    let result = with_checked_bf16_call_instance_v1(&owner, &mut budget, |_, _| {
        callbacks += 1;
        Ok(0_u8)
    });
    assert!(matches!(
        result,
        Err(Error::Unavailable("captured source occurrences required"))
    ));
    assert_eq!(callbacks, 0);
    assert_eq!(budget.storage(), PREFIX);
    assert!(budget.work() > 0);
}

#[test]
fn bf16_full_owner_unreserved_real_capture_is_not_accepted_as_paid() {
    baseline(Case::Identity);
    let mut owner = fixture::owner(Case::Identity);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let receipt = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    assert!(receipt.retained_storage() > 0);
    assert_eq!(budget.storage(), 0);
    let mut callbacks = 0;
    let result = with_checked_bf16_call_instance_v1(&owner, &mut budget, |_, _| {
        callbacks += 1;
        Ok(0_u8)
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    assert_eq!(callbacks, 0);
    assert_eq!(budget.storage(), 0);
    drop(owner);
    // Deliberately never reserved: no corresponding release is invented.
}

#[test]
fn bf16_full_owner_exact_and_one_short_original_work_and_storage() {
    let measured = baseline(Case::Identity);
    assert!(measured.work > measured.capture_work + 1);
    assert!(
        measured.peak > measured.capture_peak + 1,
        "the tested peak must belong to the checker, not earlier capture"
    );
    let exact = run(Case::Identity, measured.work, measured.peak);
    assert!(matches!(exact.result, Ok(9)), "{exact:?}");
    assert_eq!(exact.callbacks, 1);
    assert_eq!((exact.work, exact.peak), (measured.work, measured.peak));
    for (work_limit, storage_limit, work_denied) in [
        (measured.work - 1, measured.peak, true),
        (measured.work, measured.peak - 1, false),
    ] {
        let short = run(Case::Identity, work_limit, storage_limit);
        assert!(matches!(short.result, Err(Error::Resource(_))), "{short:?}");
        assert_eq!(short.callbacks, 0);
        assert_eq!(short.failed_work.is_some(), work_denied);
        assert_eq!(short.failed_storage.is_some(), !work_denied);
        assert!(short.work >= short.capture_work);
    }
}

#[test]
fn bf16_full_owner_callback_error_and_panic_keep_original_floor_and_extra_charge() {
    baseline(Case::Identity);
    for panic_callback in [false, true] {
        let mut owner = fixture::owner(Case::Identity);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(PREFIX).unwrap();
        let receipt = owner
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        let retained = receipt.retained_storage();
        budget.reserve_storage(retained).unwrap();
        let floor = budget.storage();
        let prior_work = budget.work();
        let identity = budget.work_ledger_identity_v1();
        let mut callbacks = 0;
        let result = with_checked_bf16_call_instance_v1(&owner, &mut budget, |view, original| {
            callbacks += 1;
            assert_view(&owner, view, Case::Identity);
            original.charge_work(3)?;
            original.reserve_storage(19)?;
            if panic_callback {
                panic!("test-owned callback panic after retained charge");
            }
            Err::<u8, _>(Error::Unavailable("test-owned callback refusal"))
        });
        if panic_callback {
            assert!(matches!(result, Err(Error::CallbackPanicked)));
        } else {
            assert!(matches!(
                result,
                Err(Error::Unavailable("test-owned callback refusal"))
            ));
        }
        assert_eq!(callbacks, 1);
        assert!(budget.work_ledger_identity_v1() == identity);
        assert_eq!(budget.storage(), floor + 19);
        assert!(budget.work() >= prior_work + 3);
        assert_eq!(budget.failed_work(), None);
        assert_eq!(budget.failed_storage(), None);
        drop(owner);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), PREFIX + 19);
        budget.release_storage(19).unwrap();
        assert_eq!(budget.storage(), PREFIX);
    }
}
