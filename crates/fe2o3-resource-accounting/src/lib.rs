//! Bounded transactional credit accounting, independent of runtime or native identities.
//!
//! An account admits only the supplied vectors. Costs and disposal are reviewed
//! adapter contracts, not facts established by this ledger. Creating an account,
//! reading usage or releasing one's own token grants no allocation, execution or
//! native-disposal authority. Adapters must retain tokens privately with the
//! actual resources whose charges they represent.
//!
//! This is the extracted R67 account engine, not a hierarchical budget or native
//! cost adapter. Each account has a bounded owner arena; its actual host bytes,
//! parent/device ceilings, bootstrap storage and aggregate quarantine remain
//! separate implementation work. No batch split or partial refund is provided.
//!
//! Retained-token Drop quarantines without refund. One Arc anchor preserves an
//! ambiguous account after outside handles disappear, until process exit. This
//! conservative fallback is not a process-wide quarantine manager.

use fe2o3_runtime_model::{
    R67CreditActionV1 as Action, R67CreditPhaseV1 as Phase, r67_credit_transition_v1,
    r67_resource_release_v1, r67_resource_reserve_v1,
};
use std::sync::{Arc, Mutex, MutexGuard};

pub use fe2o3_runtime_model::{
    R67ResourceKindV1 as ResourceKindV1, R67ResourceVectorV1 as ResourceVectorV1,
};

pub const MAX_RESOURCE_CREDIT_RECORDS_V1: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceCreditErrorV1 {
    InvalidRecordCapacity,
    AllocationFailed,
    Capacity,
    RecordCapacity,
    GenerationExhausted,
    Invariant,
}

impl core::fmt::Display for ResourceCreditErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "runtime resource credit error: {self:?}")
    }
}

impl std::error::Error for ResourceCreditErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceCreditUsageV1 {
    pub capacity: ResourceVectorV1,
    pub used: ResourceVectorV1,
    pub reserved_records: usize,
    pub retained_records: usize,
    pub quarantined_records: usize,
    pub record_capacity: usize,
    pub poisoned: bool,
}

#[derive(Clone, Copy)]
struct Record {
    owner: u64,
    charge: ResourceVectorV1,
    phase: Phase,
}

struct AccountState {
    used: ResourceVectorV1,
    records: Box<[Option<Record>]>,
    free: Vec<usize>,
    next_owner: u64,
    reserved: usize,
    retained: usize,
    quarantined: usize,
    poisoned: bool,
    quarantine_anchor: Option<Arc<Account>>,
}

struct Account {
    capacity: ResourceVectorV1,
    state: Mutex<AccountState>,
}

impl Account {
    fn lock(&self) -> MutexGuard<'_, AccountState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.poisoned = true;
                state
            }
        }
    }

    fn transition(
        self: &Arc<Self>,
        slot: usize,
        owner: u64,
        action: Action,
    ) -> Result<(), ResourceCreditErrorV1> {
        let mut state = self.lock();
        if state.poisoned {
            state
                .quarantine_anchor
                .get_or_insert_with(|| Arc::clone(self));
            return Err(ResourceCreditErrorV1::Invariant);
        }
        let result = (|| {
            let record = state
                .records
                .get(slot)
                .copied()
                .flatten()
                .ok_or(ResourceCreditErrorV1::Invariant)?;
            let next_phase = r67_credit_transition_v1(record.owner, owner, record.phase, action)
                .ok_or(ResourceCreditErrorV1::Invariant)?;
            let count = match record.phase {
                Phase::Reserved => state.reserved,
                Phase::Retained => state.retained,
                _ => return Err(ResourceCreditErrorV1::Invariant),
            };
            if count == 0 {
                return Err(ResourceCreditErrorV1::Invariant);
            }
            let next_used = if next_phase == Phase::Vacant {
                if state.free.len() == state.records.len() {
                    return Err(ResourceCreditErrorV1::Invariant);
                }
                r67_resource_release_v1(state.used, record.charge)
                    .ok_or(ResourceCreditErrorV1::Invariant)?
            } else {
                state.used
            };
            // No fallible operation, allocation, callback or destructor follows
            // this complete transition preflight while the mutex is held.
            match record.phase {
                Phase::Reserved => state.reserved -= 1,
                Phase::Retained => state.retained -= 1,
                _ => unreachable!(),
            }
            state.used = next_used;
            match next_phase {
                Phase::Vacant => {
                    state.records[slot] = None;
                    state.free.push(slot);
                }
                Phase::Retained => {
                    state.retained += 1;
                    state.records[slot] = Some(Record {
                        phase: next_phase,
                        ..record
                    });
                }
                Phase::Quarantined => {
                    state.quarantined += 1;
                    state.records[slot] = Some(Record {
                        phase: next_phase,
                        ..record
                    });
                    state
                        .quarantine_anchor
                        .get_or_insert_with(|| Arc::clone(self));
                }
                Phase::Reserved => unreachable!(),
            }
            Ok(())
        })();
        if result.is_err() {
            state.poisoned = true;
            state
                .quarantine_anchor
                .get_or_insert_with(|| Arc::clone(self));
        }
        result
    }
}

/// One independent account. Clones share its opaque ledger identity and capacity.
/// This handle is not a runtime-device or native-session capability.
#[derive(Clone)]
pub struct ResourceCreditAccountV1(Arc<Account>);

impl ResourceCreditAccountV1 {
    /// Creates one bounded account without performing any native operation.
    pub fn new(
        capacity: ResourceVectorV1,
        max_reservations: usize,
    ) -> Result<Self, ResourceCreditErrorV1> {
        if max_reservations == 0 || max_reservations > MAX_RESOURCE_CREDIT_RECORDS_V1 {
            return Err(ResourceCreditErrorV1::InvalidRecordCapacity);
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(max_reservations)
            .map_err(|_| ResourceCreditErrorV1::AllocationFailed)?;
        records.resize(max_reservations, None);
        let mut free = Vec::new();
        free.try_reserve_exact(max_reservations)
            .map_err(|_| ResourceCreditErrorV1::AllocationFailed)?;
        free.extend((0..max_reservations).rev());
        Ok(Self(Arc::new(Account {
            capacity,
            state: Mutex::new(AccountState {
                used: ResourceVectorV1::ZERO,
                records: records.into_boxed_slice(),
                free,
                next_owner: 1,
                reserved: 0,
                retained: 0,
                quarantined: 0,
                poisoned: false,
                quarantine_anchor: None,
            }),
        })))
    }

    /// Returns an inert snapshot, not resource ownership or disposal evidence.
    pub fn usage(&self) -> ResourceCreditUsageV1 {
        let state = self.0.lock();
        ResourceCreditUsageV1 {
            capacity: self.0.capacity,
            used: state.used,
            reserved_records: state.reserved,
            retained_records: state.retained,
            quarantined_records: state.quarantined,
            record_capacity: state.records.len(),
            poisoned: state.poisoned,
        }
    }

    /// Reserves a complete vector and one record atomically in this account.
    pub fn reserve(
        &self,
        charge: ResourceVectorV1,
    ) -> Result<ResourceReservationV1, ResourceCreditErrorV1> {
        let mut state = self.0.lock();
        if state.poisoned {
            return Err(ResourceCreditErrorV1::Invariant);
        }
        let next_used = r67_resource_reserve_v1(state.used, charge, self.0.capacity)
            .ok_or(ResourceCreditErrorV1::Capacity)?;
        let slot = *state
            .free
            .last()
            .ok_or(ResourceCreditErrorV1::RecordCapacity)?;
        let owner = state.next_owner;
        let next_owner = owner
            .checked_add(1)
            .ok_or(ResourceCreditErrorV1::GenerationExhausted)?;
        if owner == 0 || state.records[slot].is_some() {
            state.poisoned = true;
            return Err(ResourceCreditErrorV1::Invariant);
        }
        state.free.pop();
        state.records[slot] = Some(Record {
            owner,
            charge,
            phase: Phase::Reserved,
        });
        state.next_owner = next_owner;
        state.used = next_used;
        state.reserved += 1;
        Ok(ResourceReservationV1 {
            token: Some(Token {
                account: Arc::clone(&self.0),
                slot,
                owner,
            }),
        })
    }
}

struct Token {
    account: Arc<Account>,
    slot: usize,
    owner: u64,
}

/// Cancellation of this pre-issue reservation is safe because the adapter must
/// consume it with `retain` before making any potentially effectful backend call.
#[must_use = "retain before backend entry; dropping cancels only unissued credits"]
pub struct ResourceReservationV1 {
    token: Option<Token>,
}

impl ResourceReservationV1 {
    pub fn retain(mut self) -> RetainedResourceCreditsV1 {
        let token = self.token.take().expect("reservation consumed once");
        token
            .account
            .transition(token.slot, token.owner, Action::Retain)
            .expect("private reservation retains its exact live owner");
        RetainedResourceCreditsV1 { token: Some(token) }
    }
}

impl Drop for ResourceReservationV1 {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            let _ = token
                .account
                .transition(token.slot, token.owner, Action::CancelUnissued);
        }
    }
}

/// A move-only accounting record, not native ownership or disposal authority.
///
/// ```compile_fail
/// use fe2o3_resource_accounting::RetainedResourceCreditsV1;
/// fn duplicate(credits: RetainedResourceCreditsV1) {
///     let duplicate = credits.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_resource_accounting::RetainedResourceCreditsV1;
/// fn refund_twice(credits: RetainedResourceCreditsV1) {
///     credits.release_after_disposal().unwrap();
///     credits.release_after_disposal().unwrap();
/// }
/// ```
#[must_use = "retained credits require definite rejection, successful disposal or quarantine"]
pub struct RetainedResourceCreditsV1 {
    token: Option<Token>,
}

impl RetainedResourceCreditsV1 {
    /// Accounting transition for an adapter-established no-effect rejection.
    /// This method does not observe a backend or certify that rejection occurred.
    /// Quiescence alone is not sufficient disposal evidence.
    pub fn release_after_rejection(self) -> Result<(), ResourceCreditErrorV1> {
        self.release(Action::ReleaseRejected)
    }

    /// Accounting transition after the adapter disposed every represented resource.
    /// Calling this consumes only this account's charge; it performs no native
    /// operation and produces no disposal certificate. Native owners must keep
    /// their tokens private until their actual disposal contract is satisfied.
    pub fn release_after_disposal(self) -> Result<(), ResourceCreditErrorV1> {
        self.release(Action::ReleaseDisposed)
    }

    fn release(mut self, action: Action) -> Result<(), ResourceCreditErrorV1> {
        let token = self.token.as_ref().expect("retained credits consumed once");
        token.account.transition(token.slot, token.owner, action)?;
        self.token = None;
        Ok(())
    }

    pub fn quarantine(self) {
        drop(self);
    }
}

impl Drop for RetainedResourceCreditsV1 {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            let _ = token
                .account
                .transition(token.slot, token.owner, Action::Quarantine);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ResourceKindV1 as K;

    fn charge(bytes: u64) -> ResourceVectorV1 {
        ResourceVectorV1::ZERO
            .with(K::RequestedAllocationBytes, bytes)
            .with(K::AllocationRecords, 1)
    }

    fn account(bytes: u64, count: u64, records: usize) -> ResourceCreditAccountV1 {
        ResourceCreditAccountV1::new(
            ResourceVectorV1::ZERO
                .with(K::RequestedAllocationBytes, bytes)
                .with(K::AllocationRecords, count),
            records,
        )
        .unwrap()
    }

    #[test]
    fn complete_vector_rejection_changes_neither_usage_nor_record_identity() {
        let account = account(16, 1, 3);
        let first = account.reserve(charge(8)).unwrap();
        let before = account.usage();
        let next_owner = account.0.lock().next_owner;
        assert!(matches!(
            account.reserve(charge(1)),
            Err(ResourceCreditErrorV1::Capacity)
        ));
        assert_eq!(account.usage(), before);
        assert_eq!(account.0.lock().next_owner, next_owner);
        drop(first);
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
    }

    #[test]
    fn unissued_drop_and_definite_rejection_refund_exactly_once() {
        let account = account(16, 2, 2);
        drop(account.reserve(charge(7)).unwrap());
        let retained = account.reserve(charge(9)).unwrap().retain();
        assert_eq!(account.usage().retained_records, 1);
        retained.release_after_rejection().unwrap();
        let retained = account.reserve(charge(16)).unwrap().retain();
        retained.release_after_disposal().unwrap();
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
        assert_eq!(account.usage().retained_records, 0);
        assert_eq!(account.0.lock().free.len(), 2);
    }

    #[test]
    fn abandoned_retained_token_preserves_charge_after_all_external_handles_drop() {
        let account = account(16, 2, 2);
        let weak = Arc::downgrade(&account.0);
        let retained = account.reserve(charge(9)).unwrap().retain();
        retained.quarantine();
        assert_eq!(account.usage().quarantined_records, 1);
        assert_eq!(account.usage().used, charge(9));
        drop(account);
        let anchored = weak
            .upgrade()
            .expect("ambiguous account outlives external handles");
        assert_eq!(anchored.lock().used, charge(9));
        // Only the test dismantles its fake-resource anchor after inspection.
        anchored.lock().quarantine_anchor = None;
    }

    #[test]
    fn slot_reuse_changes_owner_and_stale_release_never_refunds() {
        let account = account(16, 1, 1);
        let first = account.reserve(charge(8)).unwrap().retain();
        let old_owner = first.token.as_ref().unwrap().owner;
        first.release_after_disposal().unwrap();
        let second = account.reserve(charge(8)).unwrap().retain();
        let token = second.token.as_ref().unwrap();
        assert_ne!(token.owner, old_owner);
        assert_eq!(
            account
                .0
                .transition(token.slot, old_owner, Action::ReleaseDisposed),
            Err(ResourceCreditErrorV1::Invariant)
        );
        assert_eq!(account.usage().used, charge(8));
        assert!(account.usage().poisoned);
        drop(second);
        account.0.lock().quarantine_anchor = None;
    }

    #[test]
    fn account_identity_is_bound_in_the_token_not_chosen_during_release() {
        let first = account(16, 1, 1);
        let second = account(16, 1, 1);
        let a = first.reserve(charge(8)).unwrap().retain();
        let b = second.reserve(charge(8)).unwrap().retain();
        assert!(!Arc::ptr_eq(
            &a.token.as_ref().unwrap().account,
            &b.token.as_ref().unwrap().account
        ));
        a.release_after_disposal().unwrap();
        assert_eq!(first.usage().used, ResourceVectorV1::ZERO);
        assert_eq!(second.usage().used, charge(8));
        b.release_after_disposal().unwrap();
    }

    #[test]
    fn bounded_record_and_generation_exhaustion_are_failure_atomic() {
        let account = account(16, 3, 1);
        let first = account.reserve(charge(1)).unwrap();
        let before = account.usage();
        assert!(matches!(
            account.reserve(charge(1)),
            Err(ResourceCreditErrorV1::RecordCapacity)
        ));
        assert_eq!(account.usage(), before);
        drop(first);
        account.0.lock().next_owner = u64::MAX;
        let before = account.usage();
        assert!(matches!(
            account.reserve(charge(1)),
            Err(ResourceCreditErrorV1::GenerationExhausted)
        ));
        assert_eq!(account.usage(), before);
    }

    #[test]
    fn account_configuration_has_a_fixed_record_bound() {
        for capacity in [0, MAX_RESOURCE_CREDIT_RECORDS_V1 + 1] {
            assert!(matches!(
                ResourceCreditAccountV1::new(charge(1), capacity),
                Err(ResourceCreditErrorV1::InvalidRecordCapacity)
            ));
        }
        let account = account(8, 1, 1);
        assert_eq!(account.usage().capacity, charge(8));
    }

    #[test]
    fn poisoned_account_cannot_refund_or_admit_more_custody() {
        let account = account(16, 2, 2);
        let retained = account.reserve(charge(8)).unwrap().retain();
        let panic = std::panic::catch_unwind(|| {
            let _locked = account.0.lock();
            panic!("injected private account failure");
        });
        assert!(panic.is_err());
        drop(retained);
        assert!(account.usage().poisoned);
        assert_eq!(account.usage().used, charge(8));
        assert!(matches!(
            account.reserve(charge(1)),
            Err(ResourceCreditErrorV1::Invariant)
        ));
        account.0.lock().quarantine_anchor = None;
    }

    #[test]
    fn concurrent_complete_vector_admission_preserves_conservation() {
        let account = account(4, 4, 4);
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let account = account.clone();
                scope.spawn(move || {
                    for _ in 0..100 {
                        let retained = account.reserve(charge(1)).unwrap().retain();
                        let used = account.usage().used;
                        assert!(used.get(K::RequestedAllocationBytes) <= 4);
                        assert_eq!(
                            used.get(K::RequestedAllocationBytes),
                            used.get(K::AllocationRecords)
                        );
                        retained.release_after_disposal().unwrap();
                    }
                });
            }
        });
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
        assert_eq!(account.usage().quarantined_records, 0);
    }
}
