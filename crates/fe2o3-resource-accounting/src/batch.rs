//! Whole-roster admission into the existing account and owner arena.

use super::*;
use fe2o3_runtime_model::{
    R70_MAX_RESOURCE_BATCH_MEMBERS_V1, R70ResourceBatchErrorV1, r70_resource_batch_reserve_v1,
};

pub const MAX_RESOURCE_CREDIT_BATCH_MEMBERS_V1: usize = R70_MAX_RESOURCE_BATCH_MEMBERS_V1;

fn corrupt_roster(state: &mut AccountState) -> ResourceCreditErrorV1 {
    state.poisoned = true;
    ResourceCreditErrorV1::Invariant
}

impl ResourceCreditAccountV1 {
    /// Atomically reserves all vectors and their independent owner records.
    ///
    /// The roster must contain between one and 65,536 members. Empty or larger
    /// rosters return `InvalidRecordCapacity`. Output and slot-check metadata are
    /// allocated before ledger mutation; these bounded host allocations are not
    /// themselves included in the supplied resource vectors.
    ///
    /// Ordinary capacity, record and generation rejection changes no account
    /// state. Detected arena/counter corruption poisons the account without
    /// debiting resources or advancing owners. Every reservation follows retain/cancel
    /// rules. Adapters must retain all potentially affected members before their
    /// first native effect, and establish disposal separately for each member.
    /// No parent account, native cost witness or partial debit refund is implied.
    ///
    /// ```compile_fail
    /// use fe2o3_resource_accounting::{ResourceCreditAccountV1, ResourceVectorV1};
    /// fn duplicate_member(account: &ResourceCreditAccountV1) {
    ///     let members = account.reserve_batch(&[ResourceVectorV1::ZERO]).unwrap();
    ///     let duplicate = members[0].clone();
    /// }
    /// ```
    pub fn reserve_batch(
        &self,
        charges: &[ResourceVectorV1],
    ) -> Result<Box<[ResourceReservationV1]>, ResourceCreditErrorV1> {
        if charges.is_empty() || charges.len() > MAX_RESOURCE_CREDIT_BATCH_MEMBERS_V1 {
            return Err(ResourceCreditErrorV1::InvalidRecordCapacity);
        }
        let record_capacity = {
            let state = self.0.lock();
            if state.poisoned {
                return Err(ResourceCreditErrorV1::Invariant);
            }
            state.records.len()
        };
        if charges.len() > record_capacity {
            return Err(ResourceCreditErrorV1::RecordCapacity);
        }

        let mut reservations = Vec::new();
        reservations
            .try_reserve_exact(charges.len())
            .map_err(|_| ResourceCreditErrorV1::AllocationFailed)?;
        reservations.resize_with(charges.len(), || ResourceReservationV1 { token: None });
        let mut reservations = reservations.into_boxed_slice();
        let mut occupied = Vec::new();
        occupied
            .try_reserve_exact(record_capacity.div_ceil(64))
            .map_err(|_| ResourceCreditErrorV1::AllocationFailed)?;
        occupied.resize(record_capacity.div_ceil(64), 0u64);

        let mut state = self.0.lock();
        if state.poisoned {
            return Err(ResourceCreditErrorV1::Invariant);
        }
        if state.next_owner == 0 {
            return Err(corrupt_roster(&mut state));
        }
        let admission = r70_resource_batch_reserve_v1(
            state.used,
            charges,
            self.0.capacity,
            state.free.len(),
            state.next_owner,
        )
        .map_err(|error| match error {
            R70ResourceBatchErrorV1::InvalidMemberCount => {
                ResourceCreditErrorV1::InvalidRecordCapacity
            }
            R70ResourceBatchErrorV1::Capacity => ResourceCreditErrorV1::Capacity,
            R70ResourceBatchErrorV1::RecordCapacity => ResourceCreditErrorV1::RecordCapacity,
            R70ResourceBatchErrorV1::GenerationExhausted => {
                ResourceCreditErrorV1::GenerationExhausted
            }
        })?;
        let reserved = match state.reserved.checked_add(charges.len()) {
            Some(count) if count <= record_capacity => count,
            _ => return Err(corrupt_roster(&mut state)),
        };
        let accounted_records = state
            .reserved
            .checked_add(state.retained)
            .and_then(|count| count.checked_add(state.quarantined))
            .and_then(|count| count.checked_add(state.free.len()));
        if state.records.len() != record_capacity
            || state.free.len() > record_capacity
            || accounted_records != Some(record_capacity)
        {
            return Err(corrupt_roster(&mut state));
        }
        for index in 0..charges.len() {
            let slot = state.free[state.free.len() - 1 - index];
            if state.records.get(slot).is_none_or(Option::is_some) {
                return Err(corrupt_roster(&mut state));
            }
            let bit = 1u64 << (slot % 64);
            if occupied[slot / 64] & bit != 0 {
                return Err(corrupt_roster(&mut state));
            }
            occupied[slot / 64] |= bit;
        }

        // The entire cost, owner interval and distinct vacant roster passed.
        // No fallible operation, allocation or destructor follows under the lock.
        let first_owner = state.next_owner;
        for (index, (reservation, charge)) in reservations.iter_mut().zip(charges).enumerate() {
            let slot = state
                .free
                .pop()
                .expect("complete batch free-slot preflight");
            let owner = first_owner + index as u64;
            state.records[slot] = Some(Record {
                owner,
                charge: *charge,
                phase: Phase::Reserved,
            });
            reservation.token = Some(Token {
                account: Arc::clone(&self.0),
                slot,
                owner,
            });
        }
        state.used = admission.next_used;
        state.next_owner = admission.next_owner;
        state.reserved = reserved;
        drop(state);
        Ok(reservations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ResourceKindV1 as Kind;

    fn bytes(value: u64) -> ResourceVectorV1 {
        ResourceVectorV1::ZERO.with(Kind::ResidentDeviceAllocationBytes, value)
    }

    fn account(capacity: u64, records: usize) -> ResourceCreditAccountV1 {
        ResourceCreditAccountV1::new(bytes(capacity), records).unwrap()
    }

    #[derive(Debug, Eq, PartialEq)]
    struct StateSnapshot {
        usage: ResourceCreditUsageV1,
        owners: Vec<Option<(u64, ResourceVectorV1, Phase)>>,
        free: Vec<usize>,
        next_owner: u64,
    }

    fn snapshot(account: &ResourceCreditAccountV1) -> StateSnapshot {
        let usage = account.usage();
        let state = account.0.lock();
        StateSnapshot {
            usage,
            owners: state
                .records
                .iter()
                .map(|record| record.map(|record| (record.owner, record.charge, record.phase)))
                .collect(),
            free: state.free.clone(),
            next_owner: state.next_owner,
        }
    }

    #[test]
    fn batch_uses_existing_account_records_and_independent_tokens() {
        let account = account(12, 3);
        let charges = [bytes(2), bytes(3), bytes(7)];
        let members = account.reserve_batch(&charges).unwrap();
        assert_eq!(account.usage().used, bytes(12));
        assert_eq!(account.usage().reserved_records, 3);
        for (index, member) in members.iter().enumerate() {
            let token = member.token.as_ref().unwrap();
            assert!(Arc::ptr_eq(&token.account, &account.0));
            assert_eq!(token.owner, index as u64 + 1);
            let state = account.0.lock();
            assert_eq!(state.records[token.slot].unwrap().charge, charges[index]);
        }
        let mut members = members.into_vec().into_iter();
        let first = members.next().unwrap().retain();
        drop(members.next().unwrap());
        assert_eq!(account.usage().used, bytes(9));
        let third = members.next().unwrap().retain();
        third.release_after_disposal().unwrap();
        assert_eq!(account.usage().used, bytes(2));
        first.release_after_rejection().unwrap();
        assert_eq!(account.usage().used, bytes(0));
        assert_eq!(account.usage().reserved_records, 0);
        assert_eq!(account.usage().retained_records, 0);
    }

    #[test]
    fn late_member_capacity_and_final_dimension_failure_leave_all_state_unchanged() {
        let account = account(8, 3);
        let before = snapshot(&account);
        for charges in [
            [bytes(2), bytes(3), bytes(4)],
            [
                bytes(2),
                bytes(3),
                ResourceVectorV1::ZERO.with(Kind::AllocationRecords, 1),
            ],
        ] {
            assert!(matches!(
                account.reserve_batch(&charges),
                Err(ResourceCreditErrorV1::Capacity)
            ));
            assert_eq!(snapshot(&account), before);
        }
    }

    #[test]
    fn cross_member_arithmetic_overflow_has_no_partial_debit() {
        let account = account(u64::MAX, 2);
        let before = snapshot(&account);
        assert!(matches!(
            account.reserve_batch(&[bytes(u64::MAX), bytes(1)]),
            Err(ResourceCreditErrorV1::Capacity)
        ));
        assert_eq!(snapshot(&account), before);
    }

    #[test]
    fn complete_record_roster_must_be_available_before_any_member_is_admitted() {
        let account = account(8, 3);
        let existing = account.reserve(bytes(1)).unwrap();
        let before = snapshot(&account);
        assert!(matches!(
            account.reserve_batch(&[bytes(1); 3]),
            Err(ResourceCreditErrorV1::RecordCapacity)
        ));
        assert_eq!(snapshot(&account), before);
        drop(existing);
    }

    #[test]
    fn complete_owner_interval_is_checked_before_generation_assignment() {
        let account = account(8, 3);
        account.0.lock().next_owner = u64::MAX - 2;
        let before = snapshot(&account);
        assert!(matches!(
            account.reserve_batch(&[bytes(1); 3]),
            Err(ResourceCreditErrorV1::GenerationExhausted)
        ));
        assert_eq!(snapshot(&account), before);
        let members = account.reserve_batch(&[bytes(1); 2]).unwrap();
        assert_eq!(members[0].token.as_ref().unwrap().owner, u64::MAX - 2);
        assert_eq!(members[1].token.as_ref().unwrap().owner, u64::MAX - 1);
        assert_eq!(account.0.lock().next_owner, u64::MAX);
        drop(members);
        assert_eq!(account.usage().used, bytes(0));
    }

    #[test]
    fn invalid_member_count_is_atomic_but_corrupt_zero_owner_poisons() {
        let account = account(8, 3);
        let before = snapshot(&account);
        for charges in [
            Vec::new(),
            vec![bytes(0); MAX_RESOURCE_CREDIT_BATCH_MEMBERS_V1 + 1],
        ] {
            assert!(matches!(
                account.reserve_batch(&charges),
                Err(ResourceCreditErrorV1::InvalidRecordCapacity)
            ));
            assert_eq!(snapshot(&account), before);
        }
        account.0.lock().next_owner = 0;
        let mut before = snapshot(&account);
        assert!(matches!(
            account.reserve_batch(&[bytes(1)]),
            Err(ResourceCreditErrorV1::Invariant)
        ));
        before.usage.poisoned = true;
        assert_eq!(snapshot(&account), before);
    }

    #[test]
    fn corrupt_record_counters_poison_without_debit_or_owner_advance() {
        for reserved in [1, usize::MAX] {
            let account = account(8, 3);
            account.0.lock().reserved = reserved;
            let mut before = snapshot(&account);
            assert!(matches!(
                account.reserve_batch(&[bytes(1)]),
                Err(ResourceCreditErrorV1::Invariant)
            ));
            before.usage.poisoned = true;
            assert_eq!(snapshot(&account), before);
        }
        let account = account(8, 3);
        account.0.lock().free.push(0);
        let mut before = snapshot(&account);
        assert!(matches!(
            account.reserve_batch(&[bytes(1)]),
            Err(ResourceCreditErrorV1::Invariant)
        ));
        before.usage.poisoned = true;
        assert_eq!(snapshot(&account), before);
    }

    #[test]
    fn duplicate_or_invalid_free_slots_poison_without_issuing_any_token() {
        for free in [vec![0, 1, 1], vec![0, 1, 3]] {
            let account = account(8, 3);
            account.0.lock().free = free;
            let mut before = snapshot(&account);
            assert!(matches!(
                account.reserve_batch(&[bytes(1); 3]),
                Err(ResourceCreditErrorV1::Invariant)
            ));
            before.usage.poisoned = true;
            assert_eq!(snapshot(&account), before);
            assert!(matches!(
                account.reserve(bytes(1)),
                Err(ResourceCreditErrorV1::Invariant)
            ));
            assert!(matches!(
                account.reserve_batch(&[bytes(1)]),
                Err(ResourceCreditErrorV1::Invariant)
            ));
        }
    }

    #[test]
    fn occupied_slot_in_free_roster_cannot_overwrite_an_existing_owner() {
        let account = account(8, 3);
        let existing = account.reserve(bytes(2)).unwrap();
        let slot = existing.token.as_ref().unwrap().slot;
        let original = account.0.lock().free.clone();
        account.0.lock().free = vec![slot, 2];
        let mut before = snapshot(&account);
        assert!(matches!(
            account.reserve_batch(&[bytes(1); 2]),
            Err(ResourceCreditErrorV1::Invariant)
        ));
        before.usage.poisoned = true;
        assert_eq!(snapshot(&account), before);
        account.0.lock().free = original;
        drop(existing);
        account.0.lock().quarantine_anchor = None;
    }

    #[test]
    fn zero_cost_members_still_consume_independent_owner_records() {
        let account = account(0, 2);
        let members = account.reserve_batch(&[bytes(0); 2]).unwrap();
        assert_eq!(account.usage().used, bytes(0));
        assert_eq!(account.usage().reserved_records, 2);
        assert!(matches!(
            account.reserve_batch(&[bytes(0)]),
            Err(ResourceCreditErrorV1::RecordCapacity)
        ));
        drop(members);
        assert_eq!(account.usage().reserved_records, 0);
    }

    #[test]
    fn quarantined_member_keeps_only_its_charge_after_sibling_disposal() {
        let account = account(8, 2);
        let mut members = account
            .reserve_batch(&[bytes(3), bytes(5)])
            .unwrap()
            .into_vec()
            .into_iter();
        let first = members.next().unwrap().retain();
        let second = members.next().unwrap().retain();
        drop(first);
        second.release_after_disposal().unwrap();
        assert_eq!(account.usage().used, bytes(3));
        assert_eq!(account.usage().quarantined_records, 1);
        assert_eq!(account.usage().retained_records, 0);
        // Test resources are synthetic; remove only their process-lifetime anchor.
        account.0.lock().quarantine_anchor = None;
    }

    #[test]
    fn equal_capacity_accounts_cannot_exchange_batch_member_credits() {
        let first = account(8, 2);
        let second = account(8, 2);
        let a = first.reserve_batch(&[bytes(3), bytes(5)]).unwrap();
        let b = second.reserve_batch(&[bytes(3), bytes(5)]).unwrap();
        assert!(!Arc::ptr_eq(
            &a[0].token.as_ref().unwrap().account,
            &b[0].token.as_ref().unwrap().account
        ));
        drop(a);
        assert_eq!(first.usage().used, bytes(0));
        assert_eq!(second.usage().used, bytes(8));
        drop(b);
    }

    #[test]
    fn concurrent_batches_conserve_complete_rosters() {
        let account = account(8, 8);
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let account = account.clone();
                scope.spawn(move || {
                    for _ in 0..100 {
                        let members = account.reserve_batch(&[bytes(1); 2]).unwrap();
                        let usage = account.usage();
                        assert!(usage.used.get(Kind::ResidentDeviceAllocationBytes) <= 8);
                        assert_eq!(
                            usage.used.get(Kind::ResidentDeviceAllocationBytes),
                            (usage.reserved_records + usage.retained_records) as u64
                        );
                        for member in members.into_vec() {
                            member.retain().release_after_disposal().unwrap();
                        }
                    }
                });
            }
        });
        assert_eq!(account.usage().used, bytes(0));
        assert_eq!(account.usage().reserved_records, 0);
        assert_eq!(account.usage().retained_records, 0);
        assert_eq!(
            MAX_RESOURCE_CREDIT_BATCH_MEMBERS_V1,
            MAX_RESOURCE_CREDIT_RECORDS_V1
        );
    }

    #[test]
    fn concurrent_scalar_and_batch_admission_share_one_ledger_and_owner_arena() {
        let account = account(6, 6);
        std::thread::scope(|scope| {
            for worker in 0..4 {
                let account = account.clone();
                scope.spawn(move || {
                    for _ in 0..100 {
                        if worker < 2 {
                            let member = account.reserve(bytes(1)).unwrap().retain();
                            let usage = account.usage();
                            assert_eq!(
                                usage.used.get(Kind::ResidentDeviceAllocationBytes),
                                (usage.reserved_records + usage.retained_records) as u64
                            );
                            assert!(usage.used.get(Kind::ResidentDeviceAllocationBytes) <= 6);
                            member.release_after_disposal().unwrap();
                        } else {
                            let members = account.reserve_batch(&[bytes(1); 2]).unwrap();
                            let usage = account.usage();
                            assert_eq!(
                                usage.used.get(Kind::ResidentDeviceAllocationBytes),
                                (usage.reserved_records + usage.retained_records) as u64
                            );
                            assert!(usage.used.get(Kind::ResidentDeviceAllocationBytes) <= 6);
                            for member in members.into_vec() {
                                member.retain().release_after_disposal().unwrap();
                            }
                        }
                    }
                });
            }
        });
        assert_eq!(account.usage().used, bytes(0));
        assert_eq!(account.usage().reserved_records, 0);
        assert_eq!(account.usage().retained_records, 0);
        assert_eq!(account.usage().quarantined_records, 0);
    }
}
