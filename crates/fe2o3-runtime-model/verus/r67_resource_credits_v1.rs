// Executable fixed-vector arithmetic and record-state decisions, with reviewed
// correspondence to src/r67_resource_credits.rs. Mutex linearization, native
// charge extraction, actual disposal and whole account refinement are external.
use vstd::prelude::*;
use vstd::assert_seqs_equal;
verus! {
pub open spec fn reserve_fits_v1(used: Seq<u64>, charge: Seq<u64>, capacity: Seq<u64>) -> bool {
    &&& used.len() == 19 && charge.len() == 19 && capacity.len() == 19
    &&& forall|i: int| 0 <= i < 19 ==> used[i] as int + charge[i] as int <= capacity[i] as int
}

pub open spec fn release_fits_v1(used: Seq<u64>, charge: Seq<u64>) -> bool {
    used.len() == 19 && charge.len() == 19
        && (forall|i: int| 0 <= i < 19 ==> charge[i] <= used[i])
}

pub fn resource_reserve_v1(used: [u64; 19], charge: [u64; 19], capacity: [u64; 19]) -> (result: Option<[u64; 19]>)
    ensures
        result.is_some() == reserve_fits_v1(used@, charge@, capacity@),
        match result {
            Some(next) => forall|i: int| 0 <= i < 19 ==> next@[i] as int == used@[i] as int + charge@[i] as int,
            None => true,
        },
{
    let mut next = [0u64; 19];
    let mut index = 0usize;
    while index < 19
        invariant
            0 <= index <= 19,
            forall|i: int| 0 <= i < index ==> next@[i] as int == used@[i] as int + charge@[i] as int,
            forall|i: int| 0 <= i < index ==> next@[i] <= capacity@[i],
        decreases 19 - index,
    {
        let value = match used[index].checked_add(charge[index]) {
            Some(value) => value,
            None => return None,
        };
        if value > capacity[index] { return None; }
        next[index] = value;
        index += 1;
    }
    Some(next)
}

pub fn resource_release_v1(used: [u64; 19], charge: [u64; 19]) -> (result: Option<[u64; 19]>)
    ensures
        result.is_some() == release_fits_v1(used@, charge@),
        match result {
            Some(next) => forall|i: int| 0 <= i < 19 ==> next@[i] as int == used@[i] as int - charge@[i] as int,
            None => true,
        },
{
    let mut next = [0u64; 19];
    let mut index = 0usize;
    while index < 19
        invariant
            0 <= index <= 19,
            forall|i: int| 0 <= i < index ==> next@[i] as int == used@[i] as int - charge@[i] as int,
            forall|i: int| 0 <= i < index ==> charge@[i] <= used@[i],
        decreases 19 - index,
    {
        next[index] = match used[index].checked_sub(charge[index]) {
            Some(value) => value,
            None => return None,
        };
        index += 1;
    }
    Some(next)
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PhaseV1 { Reserved, Retained, Quarantined, Vacant }
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActionV1 { Retain, CancelUnissued, ReleaseRejected, ReleaseDisposed, Quarantine }

pub open spec fn transition_v1(actual: u64, expected: u64, phase: PhaseV1, action: ActionV1) -> Option<PhaseV1> {
    if actual == 0 || actual != expected { None }
    else {
        match (phase, action) {
            (PhaseV1::Reserved, ActionV1::Retain) => Some(PhaseV1::Retained),
            (PhaseV1::Reserved, ActionV1::CancelUnissued)
            | (PhaseV1::Retained, ActionV1::ReleaseRejected)
            | (PhaseV1::Retained, ActionV1::ReleaseDisposed) => Some(PhaseV1::Vacant),
            (PhaseV1::Retained, ActionV1::Quarantine) => Some(PhaseV1::Quarantined),
            _ => None,
        }
    }
}

pub fn credit_transition_v1(actual: u64, expected: u64, phase: PhaseV1, action: ActionV1) -> (result: Option<PhaseV1>)
    ensures result == transition_v1(actual, expected, phase, action),
{
    if actual == 0 || actual != expected { return None; }
    match (phase, action) {
        (PhaseV1::Reserved, ActionV1::Retain) => Some(PhaseV1::Retained),
        (PhaseV1::Reserved, ActionV1::CancelUnissued)
        | (PhaseV1::Retained, ActionV1::ReleaseRejected)
        | (PhaseV1::Retained, ActionV1::ReleaseDisposed) => Some(PhaseV1::Vacant),
        (PhaseV1::Retained, ActionV1::Quarantine) => Some(PhaseV1::Quarantined),
        _ => None,
    }
}

pub proof fn admitted_vector_has_every_credit_v1(used: Seq<u64>, charge: Seq<u64>, capacity: Seq<u64>, i: int)
    requires reserve_fits_v1(used, charge, capacity), 0 <= i < 19,
    ensures used[i] as int + charge[i] as int <= capacity[i] as int,
{}

pub proof fn vector_reserve_release_conserves_v1(used: Seq<u64>, charge: Seq<u64>, next: Seq<u64>, released: Seq<u64>)
    requires used.len() == 19, charge.len() == 19, next.len() == 19, released.len() == 19,
        forall|i: int| 0 <= i < 19 ==> next[i] as int == used[i] as int + charge[i] as int,
        forall|i: int| 0 <= i < 19 ==> released[i] as int == next[i] as int - charge[i] as int,
    ensures released == used,
{
    assert_seqs_equal!(released, used);
}

pub proof fn transition_requires_exact_live_owner_v1(actual: u64, expected: u64, phase: PhaseV1, action: ActionV1)
    requires transition_v1(actual, expected, phase, action).is_some(),
    ensures actual != 0, actual == expected,
{}

pub proof fn retained_cannot_cancel_unissued_v1(actual: u64, expected: u64)
    ensures transition_v1(actual, expected, PhaseV1::Retained, ActionV1::CancelUnissued) == None,
{}

pub proof fn quarantine_is_never_a_refund_v1(actual: u64, expected: u64, phase: PhaseV1)
    ensures transition_v1(actual, expected, phase, ActionV1::Quarantine) != Some(PhaseV1::Vacant),
{}

pub proof fn disposed_record_cannot_refund_twice_v1(actual: u64, expected: u64, action: ActionV1)
    ensures transition_v1(actual, expected, PhaseV1::Vacant, action) == None,
{}

pub proof fn quarantined_record_cannot_be_reused_v1(actual: u64, expected: u64, action: ActionV1)
    ensures transition_v1(actual, expected, PhaseV1::Quarantined, action) == None,
{}
}
