//! Fixed resource-vector arithmetic and exact record-transition decisions.
//!
//! Values are model-only. Native cost extraction, disposal observations, account
//! identity, synchronization and whole-executor ownership remain adapter duties.

macro_rules! resource_vector_declarations_v1 {
    ($($items:tt)*) => { $($items)* };
}
include!("resource_vector_declarations.rs");
include!("resource_vector_bodies.rs");

macro_rules! resource_vector_rust_expr {
    ($body:expr) => {
        $body
    };
}

/// Distinct accounting units; neither logical bytes nor slots imply residency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum R67ResourceKindV1 {
    LogicalPayloadBytes,
    RequestedAllocationBytes,
    ResidentHostAllocationBytes,
    ResidentDeviceAllocationBytes,
    ExecutableHostImageBytes,
    ExecutableDeviceBytes,
    ControlResidentBytes,
    QueueResidentBytes,
    SignalResidentBytes,
    KernargResidentBytes,
    QueueSlots,
    SignalSlots,
    KernargSlots,
    OperationSlots,
    ReplyBytes,
    ReplyCells,
    TerminalRecordBytes,
    QuarantineBookkeepingBytes,
    AllocationRecords,
}

impl R67ResourceVectorV1 {
    pub const ZERO: Self = resource_vector_zero_body_v1!();

    pub const fn get(self, kind: R67ResourceKindV1) -> u64 {
        self.counts[kind as usize]
    }

    pub const fn with(mut self, kind: R67ResourceKindV1, count: u64) -> Self {
        self.counts[kind as usize] = count;
        self
    }

    pub const fn counts(&self) -> &[u64; R67_RESOURCE_DIMENSIONS_V1] {
        &self.counts
    }
}

#[allow(clippy::question_mark)] // Explicit matches are shared with Verus.
pub fn r67_resource_reserve_v1(
    used: R67ResourceVectorV1,
    charge: R67ResourceVectorV1,
    capacity: R67ResourceVectorV1,
) -> Option<R67ResourceVectorV1> {
    resource_vector_reserve_body_v1!(
        resource_vector_rust_expr,
        used,
        charge,
        capacity,
        next,
        index,
        []
    )
}

#[allow(clippy::question_mark)] // Explicit matches are shared with Verus.
pub fn r67_resource_release_v1(
    used: R67ResourceVectorV1,
    charge: R67ResourceVectorV1,
) -> Option<R67ResourceVectorV1> {
    resource_vector_release_body_v1!(resource_vector_rust_expr, used, charge, next, index, [])
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R67CreditPhaseV1 {
    Reserved,
    Retained,
    Quarantined,
    Vacant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R67CreditActionV1 {
    Retain,
    CancelUnissued,
    ReleaseRejected,
    ReleaseDisposed,
    Quarantine,
}

pub const fn r67_credit_transition_v1(
    actual_owner: u64,
    expected_owner: u64,
    phase: R67CreditPhaseV1,
    action: R67CreditActionV1,
) -> Option<R67CreditPhaseV1> {
    if actual_owner == 0 || actual_owner != expected_owner {
        return None;
    }
    match (phase, action) {
        (R67CreditPhaseV1::Reserved, R67CreditActionV1::Retain) => Some(R67CreditPhaseV1::Retained),
        (R67CreditPhaseV1::Reserved, R67CreditActionV1::CancelUnissued)
        | (R67CreditPhaseV1::Retained, R67CreditActionV1::ReleaseRejected)
        | (R67CreditPhaseV1::Retained, R67CreditActionV1::ReleaseDisposed) => {
            Some(R67CreditPhaseV1::Vacant)
        }
        (R67CreditPhaseV1::Retained, R67CreditActionV1::Quarantine) => {
            Some(R67CreditPhaseV1::Quarantined)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_admission_and_release_check_every_dimension_before_returning() {
        let used = R67ResourceVectorV1 { counts: [3; 19] };
        let charge = R67ResourceVectorV1 { counts: [5; 19] };
        let capacity = R67ResourceVectorV1 { counts: [8; 19] };
        let admitted = r67_resource_reserve_v1(used, charge, capacity).unwrap();
        assert_eq!(r67_resource_release_v1(admitted, charge), Some(used));
        for index in 0..19 {
            let mut insufficient = capacity;
            insufficient.counts[index] -= 1;
            assert_eq!(r67_resource_reserve_v1(used, charge, insufficient), None);
            let mut underflow = admitted;
            underflow.counts[index] = 4;
            assert_eq!(r67_resource_release_v1(underflow, charge), None);
            let mut overflow = used;
            overflow.counts[index] = u64::MAX;
            assert_eq!(r67_resource_reserve_v1(overflow, charge, capacity), None);
        }
        assert_eq!(used.counts, [3; 19]);
    }

    #[test]
    fn vector_boundary_arithmetic_matches_wide_model() {
        let values = [0, 1, u64::MAX / 2, u64::MAX - 1, u64::MAX];
        for used in values {
            for charge in values {
                for capacity in values {
                    let u = R67ResourceVectorV1 { counts: [used; 19] };
                    let c = R67ResourceVectorV1 {
                        counts: [charge; 19],
                    };
                    let cap = R67ResourceVectorV1 {
                        counts: [capacity; 19],
                    };
                    let expected = (used as u128 + charge as u128 <= capacity as u128).then(|| {
                        R67ResourceVectorV1 {
                            counts: [used.wrapping_add(charge); 19],
                        }
                    });
                    assert_eq!(r67_resource_reserve_v1(u, c, cap), expected);
                }
            }
        }
    }

    #[test]
    fn record_transitions_reject_stale_owner_and_quarantine_refunds() {
        use R67CreditActionV1 as A;
        use R67CreditPhaseV1 as P;
        for phase in [P::Reserved, P::Retained, P::Quarantined, P::Vacant] {
            for action in [
                A::Retain,
                A::CancelUnissued,
                A::ReleaseRejected,
                A::ReleaseDisposed,
                A::Quarantine,
            ] {
                assert_eq!(r67_credit_transition_v1(1, 2, phase, action), None);
                assert_eq!(r67_credit_transition_v1(0, 0, phase, action), None);
                if matches!(phase, P::Quarantined | P::Vacant) {
                    assert_eq!(r67_credit_transition_v1(1, 1, phase, action), None);
                }
            }
        }
        assert_eq!(
            r67_credit_transition_v1(1, 1, P::Retained, A::CancelUnissued),
            None
        );
        assert_eq!(
            r67_credit_transition_v1(1, 1, P::Reserved, A::Retain),
            Some(P::Retained)
        );
        assert_eq!(
            r67_credit_transition_v1(1, 1, P::Retained, A::Quarantine),
            Some(P::Quarantined)
        );
    }
}
