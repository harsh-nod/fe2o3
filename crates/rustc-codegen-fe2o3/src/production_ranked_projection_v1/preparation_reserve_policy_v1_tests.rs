use super::*;
type Result<T, E = Error> = std::result::Result<T, E>;
use crate::production_ranked_projection_v1::CanonicalAssertionErrorV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
const LIMIT: usize = 1024 * 1024;
const FLOOR: usize = 29;

struct OriginalPolicy<'a, 'b, 'w> {
    resources: &'a mut PreparationResourcesV1<'b, 'w>,
}
impl OriginalPolicy<'_, '_, '_> {
    fn is_metered(&self) -> bool {
        self.resources.is_metered()
    }
    fn work(&mut self, n: usize) -> std::result::Result<(), Error> {
        self.resources.work(n)
    }
    fn reserve_storage(&mut self, n: usize) -> std::result::Result<(), Error> {
        self.resources.reserve_storage(n)
    }
    pub(super) fn reserve<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), Error> {
        let requested = values
            .len()
            .checked_add(additional)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        if requested <= values.capacity() {
            return Ok(());
        }
        if self.is_metered() {
            // Full next capacity is admitted while all prior growth remains
            // owned. Relocation and initialization never use refunded scratch.
            self.work(values.len())?;
            self.reserve_storage(
                requested
                    .checked_mul(size_of::<T>())
                    .ok_or_else(|| resource(Resource::Arithmetic))?,
            )?;
            values
                .try_reserve_exact(additional)
                .map_err(|_| resource(Resource::Allocation))?;
            if size_of::<T>() != 0 && values.capacity() != requested {
                return Err(resource(Resource::Allocation));
            }
        } else {
            // Legacy loops retain amortized growth, not cumulative exact growth.
            values
                .try_reserve(additional)
                .map_err(|_| resource(Resource::Allocation))?;
        }
        Ok(())
    }
    pub(super) fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), Error> {
        self.work(1)?;
        self.reserve(values, 1)?;
        values.push(value);
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum Arm {
    Oracle,
    Preparation,
    Assertion,
}
enum Endpoint<'a, 'b, 'w> {
    Oracle(OriginalPolicy<'a, 'b, 'w>),
    Preparation(&'a mut PreparationResourcesV1<'b, 'w>),
    Assertion(AssertionResourcesV1<'a>),
}
impl<'a, 'b, 'w> Endpoint<'a, 'b, 'w> {
    fn new(arm: Arm, paid: bool, resources: &'a mut PreparationResourcesV1<'b, 'w>) -> Self {
        match arm {
            Arm::Oracle => Self::Oracle(OriginalPolicy { resources }),
            Arm::Preparation => Self::Preparation(resources),
            Arm::Assertion => Self::Assertion(if paid {
                AssertionResourcesV1::strict(resources).unwrap()
            } else {
                AssertionResourcesV1::legacy()
            }),
        }
    }
    fn reserve(&mut self, v: &mut Vec<u64>, n: usize) -> std::result::Result<(), Error> {
        match self {
            Self::Oracle(x) => x.reserve(v, n),
            Self::Preparation(x) => x.reserve(v, n),
            Self::Assertion(x) => x.preparation_reserve_v1(v, n),
        }
    }
    fn push(&mut self, v: &mut Vec<u64>, n: u64) -> std::result::Result<(), Error> {
        match self {
            Self::Oracle(x) => x.push(v, n),
            Self::Preparation(x) => x.push(v, n),
            Self::Assertion(x) => x.preparation_push_v1(v, n),
        }
    }
}
#[derive(Clone, Copy)]
enum Action {
    Reserve(usize),
    Push(u64),
    Pop,
}
#[derive(Debug, PartialEq, Eq)]
enum Failure {
    Work(usize, usize),
    Storage(usize, usize),
    Arithmetic,
    Allocation,
    Accounting,
    Other(String),
}
fn relative(n: usize, base: usize) -> usize {
    if n == usize::MAX {
        n
    } else {
        n.checked_sub(base).unwrap()
    }
}
fn failure(error: Error, work_base: usize, storage_base: usize) -> Failure {
    match error {
        Error::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(e)) => match e {
            Resource::Work(e) => Failure::Work(
                relative(e.actual(), work_base),
                relative(e.limit(), work_base),
            ),
            Resource::Storage(e) => Failure::Storage(
                relative(e.actual(), storage_base),
                relative(e.limit(), storage_base),
            ),
            Resource::Arithmetic => Failure::Arithmetic,
            Resource::Allocation => Failure::Allocation,
            Resource::Accounting => Failure::Accounting,
        },
        e => Failure::Other(format!("{e:?}")),
    }
}
#[derive(Debug, PartialEq, Eq)]
struct Step {
    values: Vec<u64>,
    capacity: usize,
    error: Option<Failure>,
}
#[derive(Debug, PartialEq, Eq)]
struct Observation {
    steps: Vec<Step>,
    work: usize,
    owned: usize,
    peak: usize,
    failed_work: Option<usize>,
    failed_storage: Option<usize>,
}
fn setup() -> (usize, usize) {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    {
        let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
        let strict = AssertionResourcesV1::strict(&mut resources).unwrap();
        drop(strict);
    }
    let result = (budget.work(), owned);
    assert_eq!(budget.storage(), owned);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
    result
}
fn observed(
    arm: Arm,
    actions: &[Action],
    initial: &[u64],
    work_cap: usize,
    storage_cap: usize,
) -> Observation {
    let (work_base, storage_extra) = if matches!(arm, Arm::Assertion) {
        setup()
    } else {
        (0, 0)
    };
    let storage_base = FLOOR + storage_extra;
    let mut work = Work::new(work_cap + work_base);
    let mut budget = Budget::new(&mut work, storage_cap + storage_base);
    budget.reserve_storage(FLOOR).unwrap();
    let identity = budget.work_ledger_identity_v1();
    let slot = &budget as *const Budget<'_> as usize;
    let mut owned = 0;
    let mut values = initial.to_vec();
    let mut steps = Vec::new();
    {
        let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
        let mut endpoint = Endpoint::new(arm, true, &mut resources);
        for action in actions {
            let outcome = match *action {
                Action::Reserve(n) => endpoint.reserve(&mut values, n),
                Action::Push(v) => endpoint.push(&mut values, v),
                Action::Pop => {
                    values.pop();
                    Ok(())
                }
            };
            let stop = outcome.is_err();
            steps.push(Step {
                values: values.clone(),
                capacity: values.capacity(),
                error: outcome.err().map(|e| failure(e, work_base, storage_base)),
            });
            if stop {
                break;
            }
        }
        drop(endpoint);
    }
    assert_eq!(slot, &budget as *const Budget<'_> as usize);
    assert!(identity == budget.work_ledger_identity_v1());
    assert_eq!(budget.storage(), FLOOR + owned);
    let result = Observation {
        steps,
        work: budget.work() - work_base,
        owned: owned - storage_extra,
        peak: budget.peak_storage() - storage_base,
        failed_work: budget.failed_work().map(|n| relative(n, work_base)),
        failed_storage: budget.failed_storage().map(|n| relative(n, storage_base)),
    };
    drop(values);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    result
}
fn assert_both(actions: &[Action], initial: &[u64], w: usize, p: usize) {
    let original = observed(Arm::Oracle, actions, initial, w, p);
    assert_eq!(observed(Arm::Preparation, actions, initial, w, p), original);
    assert_eq!(observed(Arm::Assertion, actions, initial, w, p), original);
}
fn legacy(arm: Arm, actions: &[Action], initial: &[u64]) -> Vec<Step> {
    let mut resources = PreparationResourcesV1::unmetered();
    let mut endpoint = Endpoint::new(arm, false, &mut resources);
    let mut values = initial.to_vec();
    let mut steps = Vec::new();
    for action in actions {
        let outcome = match *action {
            Action::Reserve(n) => endpoint.reserve(&mut values, n),
            Action::Push(v) => endpoint.push(&mut values, v),
            Action::Pop => {
                values.pop();
                Ok(())
            }
        };
        let stop = outcome.is_err();
        steps.push(Step {
            values: values.clone(),
            capacity: values.capacity(),
            error: outcome.err().map(|e| failure(e, 0, 0)),
        });
        if stop {
            break;
        }
    }
    steps
}
#[test]
fn policy_legacy_keeps_original_amortized_growth_and_first_errors() {
    let actions = [
        Action::Reserve(0),
        Action::Push(1),
        Action::Push(2),
        Action::Reserve(5),
        Action::Pop,
        Action::Push(3),
        Action::Reserve(0),
    ];
    for initial in [&[][..], &[9][..]] {
        let old = legacy(Arm::Oracle, &actions, initial);
        assert_eq!(legacy(Arm::Preparation, &actions, initial), old);
        assert_eq!(legacy(Arm::Assertion, &actions, initial), old);
        let overflow = [Action::Reserve(usize::MAX)];
        let old = legacy(Arm::Oracle, &overflow, &[1]);
        assert_eq!(legacy(Arm::Preparation, &overflow, &[1]), old);
        assert_eq!(legacy(Arm::Assertion, &overflow, &[1]), old);
    }
}
#[test]
fn policy_paid_growth_capacity_reuse_and_pop_keep_original_full_trace() {
    let actions = [
        Action::Reserve(0),
        Action::Push(1),
        Action::Push(2),
        Action::Push(3),
        Action::Reserve(5),
        Action::Pop,
        Action::Push(4),
        Action::Reserve(0),
    ];
    assert_both(&actions, &[], LIMIT, LIMIT);
    assert_both(&actions, &[9], LIMIT, LIMIT);
    let three = observed(
        Arm::Preparation,
        &[Action::Push(1), Action::Push(2), Action::Push(3)],
        &[],
        LIMIT,
        LIMIT,
    );
    assert_eq!(three.work, 6);
    assert_eq!(three.owned, (1 + 2 + 3) * size_of::<u64>());
    assert_eq!(three.peak, three.owned);
}
#[test]
fn policy_paid_first_failure_grid_matches_original_without_assertion_vec_debits() {
    let actions = [
        Action::Push(1),
        Action::Push(2),
        Action::Push(3),
        Action::Reserve(5),
    ];
    for w in 0..=12 {
        for p in 0..=65 {
            assert_both(&actions, &[], w, p);
        }
    }
}
#[test]
fn policy_requested_length_and_payload_product_overflow_precede_allocation() {
    assert_both(&[Action::Reserve(usize::MAX)], &[1], LIMIT, LIMIT);
    assert_both(
        &[Action::Reserve(usize::MAX / size_of::<u64>() + 1)],
        &[],
        LIMIT,
        LIMIT,
    );
    let len = observed(Arm::Preparation, &[Action::Reserve(usize::MAX)], &[1], 0, 0);
    assert_eq!(len.work, 0);
    assert_eq!(len.owned, 0);
    assert_eq!(len.steps[0].error, Some(Failure::Arithmetic));
}
#[test]
fn policy_work_refusal_precedes_storage_and_reserve_refusal_keeps_push_prefix() {
    let actions = [Action::Push(9)];
    assert_both(&actions, &[], 0, 0);
    assert_both(&actions, &[], 1, 0);
    let first = observed(Arm::Preparation, &actions, &[], 0, 0);
    assert_eq!(first.steps[0].error, Some(Failure::Work(1, 0)));
    assert_eq!(first.owned, 0);
    let storage = observed(Arm::Preparation, &actions, &[], 1, 0);
    assert_eq!(storage.work, 1);
    assert_eq!(storage.steps[0].error, Some(Failure::Storage(8, 0)));
    assert!(storage.steps[0].values.is_empty());
    let actions = [Action::Push(1), Action::Push(2)];
    assert_both(&actions, &[], LIMIT, 8);
    let partial = observed(Arm::Preparation, &actions, &[], LIMIT, 8);
    assert_eq!(partial.owned, 8);
    assert_eq!(partial.steps[1].values, vec![1]);
    assert_eq!(partial.steps[1].error, Some(Failure::Storage(24, 8)));
}
#[test]
fn policy_original_fitting_capacity_return_and_smaller_work_after_denial_remain() {
    for original in [true, false] {
        let mut work = Work::new(1);
        let mut budget = Budget::new(&mut work, 0);
        let mut owned = 0;
        assert!(budget.charge_work(2).is_err());
        let mut values = vec![7];
        let capacity = values.capacity();
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            if original {
                let mut old = OriginalPolicy {
                    resources: &mut resources,
                };
                old.reserve(&mut values, 0).unwrap();
                values.pop();
                old.push(&mut values, 9).unwrap();
            } else {
                resources.reserve(&mut values, 0).unwrap();
                values.pop();
                resources.push(&mut values, 9).unwrap();
            }
        }
        assert_eq!(budget.work(), 1);
        assert_eq!(budget.failed_work(), Some(2));
        assert_eq!(owned, 0);
        assert_eq!(values, vec![9]);
        assert_eq!(values.capacity(), capacity);
    }
}
#[test]
fn policy_assertion_admission_refuses_denied_handle_even_when_capacity_fits() {
    let (base, bytes) = setup();
    let mut work = Work::new(base + 1);
    let mut budget = Budget::new(&mut work, bytes);
    let mut owned = 0;
    let mut values = vec![7];
    let cap = values.capacity();
    {
        let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
        let mut strict = AssertionResourcesV1::strict(&mut resources).unwrap();
        assert!(strict.extra_work(2).is_err());
        assert!(matches!(
            strict.preparation_reserve_v1(&mut values, 0),
            Err(Error::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Accounting)
            ))
        ));
        assert!(strict.preparation_push_v1(&mut values, 8).is_err());
        assert!(strict.is_denied());
    }
    assert_eq!(values, vec![7]);
    assert_eq!(values.capacity(), cap);
    assert_eq!(budget.work(), base);
    drop(values);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
#[test]
fn policy_assertion_owner_join_is_checked_before_capacity_or_overflow() {
    for (overflow, foreign_work) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut other_work = Work::new(LIMIT);
        let other = Budget::new(&mut other_work, LIMIT);
        let foreign_identity = other.work_ledger_identity_v1();
        let mut values = vec![7];
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            let mut strict = AssertionResourcesV1::strict(&mut resources).unwrap();
            // Private hostile control, not an externally constructible owner.
            if foreign_work {
                strict.owner.as_mut().unwrap().ledger = foreign_identity;
            } else {
                strict.owner.as_mut().unwrap().budget_slot ^= 1;
            }
            let error = strict
                .preparation_reserve_v1(&mut values, if overflow { usize::MAX } else { 0 })
                .unwrap_err();
            assert_eq!(failure(error, 0, 0), Failure::Accounting);
            assert!(strict.is_denied());
        }
        assert_eq!(values, vec![7]);
        drop(values);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn policy_assertion_arithmetic_refusal_poison_is_separate_from_original_policy() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut values = vec![7];
    {
        let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
        let mut strict = AssertionResourcesV1::strict(&mut resources).unwrap();
        assert_eq!(
            failure(
                strict
                    .preparation_reserve_v1(&mut values, usize::MAX)
                    .unwrap_err(),
                0,
                0
            ),
            Failure::Arithmetic
        );
        assert!(strict.is_denied());
        assert_eq!(
            failure(
                strict.preparation_reserve_v1(&mut values, 0).unwrap_err(),
                0,
                0
            ),
            Failure::Accounting
        );
    }
    drop(values);
    budget.release_storage(owned).unwrap();
}
#[test]
fn policy_shared_entry_cannot_bypass_assertion_owner_admission() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut values = vec![7];
    {
        let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
        let mut strict = AssertionResourcesV1::strict(&mut resources).unwrap();
        strict.owner.as_mut().unwrap().budget_slot ^= 1;
        let error =
            super::super::bf16_nominal_preparation_resources_v1::preparation_reserve_with_meter_v1(
                &mut strict,
                &mut values,
                0,
            )
            .unwrap_err();
        assert_eq!(failure(error, 0, 0), Failure::Accounting);
    }
    drop(values);
    budget.release_storage(owned).unwrap();
}
#[test]
fn policy_existing_cache_storage_stays_live_on_the_same_original_handle() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let identity = budget.work_ledger_identity_v1();
    let mut values = Vec::new();
    {
        let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
        let mut strict = AssertionResourcesV1::strict(&mut resources).unwrap();
        let mut set = AssertionSetV1::<usize>::new(&mut strict).unwrap();
        assert!(set.insert(3, &mut strict).unwrap());
        strict.preparation_push_v1(&mut values, 7_u64).unwrap();
        strict.preparation_reserve_v1(&mut values, 3).unwrap();
        assert!(set.contains(&3, &mut strict).unwrap());
        assert!(!set.insert(3, &mut strict).unwrap());
        assert!(!strict.is_denied());
        drop(set);
        drop(strict);
    }
    assert!(identity == budget.work_ledger_identity_v1());
    assert_eq!(budget.storage(), owned);
    assert_eq!(values, vec![7]);
    drop(values);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
#[test]
fn policy_caller_unwind_retains_credits_until_all_payloads_drop() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
        let mut strict = AssertionResourcesV1::strict(&mut resources).unwrap();
        let mut values = Vec::new();
        strict.preparation_push_v1(&mut values, 7_u64).unwrap();
        let mut set = AssertionSetV1::<usize>::new(&mut strict).unwrap();
        set.insert(3, &mut strict).unwrap();
        panic!("intentional preparation policy caller unwind");
    }));
    assert!(outcome.is_err());
    drop(outcome);
    assert!(owned > 0);
    assert_eq!(budget.storage(), owned);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn policy_storage_denial_preserves_original_capacity_return_but_refuses_assertion_facet() {
    for original in [true, false] {
        let mut work = Work::new(1);
        let mut budget = Budget::new(&mut work, 0);
        let mut owned = 0;
        assert!(budget.reserve_storage(1).is_err());
        let mut values = vec![7];
        {
            let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
            if original {
                OriginalPolicy {
                    resources: &mut resources,
                }
                .reserve(&mut values, 0)
                .unwrap();
            } else {
                resources.reserve(&mut values, 0).unwrap();
            }
        }
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.failed_storage(), Some(1));
        assert_eq!(owned, 0);
    }
    let (work_base, bytes) = setup();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, bytes);
    let mut owned = 0;
    let mut values = vec![7];
    {
        let mut resources = PreparationResourcesV1::new(&mut budget, &mut owned);
        let mut strict = AssertionResourcesV1::strict(&mut resources).unwrap();
        assert!(strict.reserve_storage(1).is_err());
        assert_eq!(
            failure(
                strict.preparation_reserve_v1(&mut values, 0).unwrap_err(),
                0,
                0
            ),
            Failure::Accounting
        );
    }
    assert_eq!(budget.work(), work_base);
    assert_eq!(values, vec![7]);
    drop(values);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
