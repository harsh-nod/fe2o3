// V4-J1: constructor contents and bounded writer issuance refinement.
// Derived from the reviewed V15 development candidate; see the qualification receipt.
// Rust storage allocation, physical capacity, unwind and Context integration are separate.
use vstd::prelude::*;

verus! {

#[derive(Clone, Copy)]
pub enum WriterKindV1 { Synchronous, Submission }

#[derive(Clone, Copy)]
pub struct WriterKeyV1 {
    pub context_generation: u64,
    pub local: u64,
    pub kind: WriterKindV1,
}

#[derive(Clone, Copy)]
pub struct WriterReferenceV1 {
    pub slot: usize,
    pub key: WriterKeyV1,
}

#[derive(Clone, Copy)]
pub enum WriterEntryV1 {
    Reserved(WriterKeyV1),
    Pending { key: WriterKeyV1, head: Option<usize>, count: usize },
    Unknown { key: WriterKeyV1, head: Option<usize>, count: usize },
}

#[derive(Clone, Copy)]
pub enum JournalErrorV1 {
    InvalidContextGeneration,
    InvalidCapacity,
    ForeignContext,
    InvalidWriterId,
    WriterReplay,
    WriterCapacity,
    InvalidReference,
    InvalidState,
}

#[derive(Clone, Copy)]
pub struct RegisterPlanV1 { pub slot: usize, pub reserved_count: usize }

#[derive(Clone, Copy)]
pub struct AbortPlanV1 { pub reserved_count: usize }

pub open spec fn issuable_id_v1(value: u64) -> bool {
    value != 0 && value != u64::MAX
}

pub open spec fn same_kind_v1(left: WriterKindV1, right: WriterKindV1) -> bool {
    match (left, right) {
        (WriterKindV1::Synchronous, WriterKindV1::Synchronous) => true,
        (WriterKindV1::Submission, WriterKindV1::Submission) => true,
        _ => false,
    }
}

pub open spec fn same_key_v1(left: WriterKeyV1, right: WriterKeyV1) -> bool {
    left.context_generation == right.context_generation
        && left.local == right.local && same_kind_v1(left.kind, right.kind)
}

pub open spec fn constructor_admission_v1(
    context: u64, allocation_capacity: usize, writer_capacity: usize,
) -> Option<JournalErrorV1> {
    if !issuable_id_v1(context) {
        Some(JournalErrorV1::InvalidContextGeneration)
    } else if allocation_capacity == 0 || allocation_capacity > 1_048_576
        || writer_capacity == 0 || writer_capacity > 1_048_576 {
        Some(JournalErrorV1::InvalidCapacity)
    } else { None }
}

pub open spec fn register_decision_v1(
    context: u64, watermark: u64, reserved_count: usize, writer_capacity: usize,
    writers: Seq<Option<WriterEntryV1>>, free: Seq<usize>, key: WriterKeyV1,
) -> Result<RegisterPlanV1, JournalErrorV1> {
    if key.context_generation != context { Err(JournalErrorV1::ForeignContext) }
    else if !issuable_id_v1(key.local) { Err(JournalErrorV1::InvalidWriterId) }
    else if key.local <= watermark { Err(JournalErrorV1::WriterReplay) }
    else if free.len() == 0 { Err(JournalErrorV1::WriterCapacity) }
    else {
        let slot = free[free.len() - 1];
        if slot >= writers.len() || writers[slot as int].is_some() {
            Err(JournalErrorV1::InvalidState)
        } else if reserved_count == usize::MAX || reserved_count >= writer_capacity {
            Err(JournalErrorV1::InvalidState)
        } else {
            Ok(RegisterPlanV1 { slot, reserved_count: (reserved_count + 1) as usize })
        }
    }
}

pub open spec fn lookup_decision_v1(
    context: u64, writers: Seq<Option<WriterEntryV1>>, reference: WriterReferenceV1,
) -> Result<WriterKeyV1, JournalErrorV1> {
    if reference.slot >= writers.len() { Err(JournalErrorV1::InvalidReference) }
    else {
        match writers[reference.slot as int] {
            Some(WriterEntryV1::Reserved(key)) => {
                if same_key_v1(key, reference.key) && key.context_generation == context {
                    Ok(key)
                } else { Err(JournalErrorV1::InvalidReference) }
            },
            _ => Err(JournalErrorV1::InvalidReference),
        }
    }
}

pub open spec fn abort_decision_v1(
    context: u64, reserved_count: usize, writer_capacity: usize,
    writers: Seq<Option<WriterEntryV1>>, free_len: usize, free_capacity: usize,
    reference: WriterReferenceV1,
) -> Result<AbortPlanV1, JournalErrorV1> {
    match lookup_decision_v1(context, writers, reference) {
        Err(error) => Err(error),
        Ok(_) => {
            if free_len >= writer_capacity || free_len >= free_capacity {
                Err(JournalErrorV1::InvalidState)
            } else if reserved_count == 0 {
                Err(JournalErrorV1::InvalidState)
            } else {
                Ok(AbortPlanV1 { reserved_count: (reserved_count - 1) as usize })
            }
        },
    }
}

pub fn issuable_id_exec_v1(value: u64) -> (result: bool)
    ensures result == issuable_id_v1(value),
{
    value != 0 && value != u64::MAX
}

pub fn same_key_exec_v1(left: WriterKeyV1, right: WriterKeyV1) -> (result: bool)
    ensures result == same_key_v1(left, right),
{
    let same_kind = match (left.kind, right.kind) {
        (WriterKindV1::Synchronous, WriterKindV1::Synchronous) => true,
        (WriterKindV1::Submission, WriterKindV1::Submission) => true,
        _ => false,
    };
    left.context_generation == right.context_generation
        && left.local == right.local && same_kind
}

pub fn constructor_preflight_v1(
    context: u64, allocation_capacity: usize, writer_capacity: usize,
) -> (result: Option<JournalErrorV1>)
    ensures result == constructor_admission_v1(context, allocation_capacity, writer_capacity),
{
    if !issuable_id_exec_v1(context) {
        Some(JournalErrorV1::InvalidContextGeneration)
    } else if allocation_capacity == 0 || allocation_capacity > 1_048_576
        || writer_capacity == 0 || writer_capacity > 1_048_576 {
        Some(JournalErrorV1::InvalidCapacity)
    } else { None }
}

pub fn register_preflight_v1(
    context: u64, watermark: u64, reserved_count: usize, writer_capacity: usize,
    writers: &[Option<WriterEntryV1>], free: &[usize], key: WriterKeyV1,
) -> (result: Result<RegisterPlanV1, JournalErrorV1>)
    ensures result == register_decision_v1(
        context, watermark, reserved_count, writer_capacity, writers@, free@, key),
{
    if key.context_generation != context { return Err(JournalErrorV1::ForeignContext); }
    if !issuable_id_exec_v1(key.local) { return Err(JournalErrorV1::InvalidWriterId); }
    if key.local <= watermark { return Err(JournalErrorV1::WriterReplay); }
    if free.len() == 0 { return Err(JournalErrorV1::WriterCapacity); }
    let slot = free[free.len() - 1];
    if slot >= writers.len() { return Err(JournalErrorV1::InvalidState); }
    if writers[slot].is_some() { return Err(JournalErrorV1::InvalidState); }
    let next_count = match reserved_count.checked_add(1) {
        Some(value) => value,
        None => return Err(JournalErrorV1::InvalidState),
    };
    if next_count > writer_capacity { return Err(JournalErrorV1::InvalidState); }
    Ok(RegisterPlanV1 { slot, reserved_count: next_count })
}

pub fn lookup_reserved_v1(
    context: u64, writers: &[Option<WriterEntryV1>], reference: WriterReferenceV1,
) -> (result: Result<WriterKeyV1, JournalErrorV1>)
    ensures result == lookup_decision_v1(context, writers@, reference),
{
    if reference.slot >= writers.len() { return Err(JournalErrorV1::InvalidReference); }
    match writers[reference.slot] {
        Some(WriterEntryV1::Reserved(key)) => {
            if same_key_exec_v1(key, reference.key) && key.context_generation == context {
                Ok(key)
            } else { Err(JournalErrorV1::InvalidReference) }
        },
        _ => Err(JournalErrorV1::InvalidReference),
    }
}

pub fn abort_preflight_v1(
    context: u64, reserved_count: usize, writer_capacity: usize,
    writers: &[Option<WriterEntryV1>], free_len: usize, free_capacity: usize,
    reference: WriterReferenceV1,
) -> (result: Result<AbortPlanV1, JournalErrorV1>)
    ensures result == abort_decision_v1(
        context, reserved_count, writer_capacity, writers@, free_len, free_capacity, reference),
{
    match lookup_reserved_v1(context, writers, reference) {
        Ok(_) => {},
        Err(error) => return Err(error),
    }
    if free_len >= writer_capacity || free_len >= free_capacity {
        return Err(JournalErrorV1::InvalidState);
    }
    let next_count = match reserved_count.checked_sub(1) {
        Some(value) => value,
        None => return Err(JournalErrorV1::InvalidState),
    };
    Ok(AbortPlanV1 { reserved_count: next_count })
}


#[derive(Clone, Copy)]
pub struct AllocationKeyV1 { pub context_generation: u64, pub local: u64 }

#[derive(Clone, Copy)]
pub struct DeviceKeyV1 { pub context_generation: u64, pub local: u64 }

#[derive(Clone, Copy)]
pub struct AllocationReferenceV1 { pub slot: usize, pub key: AllocationKeyV1 }

#[derive(Clone, Copy)]
pub struct AllocationEntryV1 {
    pub key: AllocationKeyV1,
    pub device: DeviceKeyV1,
    pub byte_extent: u64,
    pub attempt_epoch: u64,
    pub content_lineage: u64,
    pub pending_member: Option<usize>,
}

#[derive(Clone, Copy)]
pub struct MemberEntryV1 {
    pub writer: WriterReferenceV1,
    pub allocation: AllocationReferenceV1,
    pub prior_lineage: u64,
    pub attempt_epoch: u64,
    pub next: Option<usize>,
}

#[derive(Clone, Copy)]
pub struct BeginMemberPlanV1 {
    pub member_slot: usize,
    pub allocation: AllocationReferenceV1,
    pub prior_lineage: u64,
    pub attempt_epoch: u64,
}

// These fields are retained verbatim by every issuance transition.
pub struct OtherJournalStateV1 {
    pub allocation_capacity: usize,
    pub allocations: Seq<Option<AllocationEntryV1>>,
    pub allocation_free: Seq<usize>,
    pub members: Seq<Option<MemberEntryV1>>,
    pub member_free: Seq<usize>,
    pub scratch: Seq<Option<BeginMemberPlanV1>>,
    pub allocations_storage: usize,
    pub allocation_free_storage: usize,
    pub members_storage: usize,
    pub member_free_storage: usize,
    pub scratch_storage: usize,
}

pub struct JournalStateV1 {
    pub context_generation: u64,
    pub writer_capacity: usize,
    pub registration_watermark: u64,
    pub reserved_count: usize,
    pub writers: Seq<Option<WriterEntryV1>>,
    pub free: Seq<usize>,
    pub writers_storage: usize,
    pub free_storage: usize,
    pub other: OtherJournalStateV1,
    pub successful_registrations: Seq<WriterReferenceV1>,
}

pub open spec fn occupied_v1(entry: Option<WriterEntryV1>) -> bool { entry.is_some() }

pub open spec fn reserved_v1(entry: Option<WriterEntryV1>) -> bool {
    match entry { Some(WriterEntryV1::Reserved(_)) => true, _ => false }
}

pub open spec fn writer_key_v1(entry: WriterEntryV1) -> WriterKeyV1 {
    match entry {
        WriterEntryV1::Reserved(key) => key,
        WriterEntryV1::Pending { key, head: _, count: _ } => key,
        WriterEntryV1::Unknown { key, head: _, count: _ } => key,
    }
}

pub open spec fn occupied_prefix_v1(writers: Seq<Option<WriterEntryV1>>, end: int) -> int
    decreases end,
{
    if end <= 0 { 0 }
    else { occupied_prefix_v1(writers, end - 1) + if occupied_v1(writers[end - 1]) { 1int } else { 0int } }
}

pub open spec fn reserved_prefix_v1(writers: Seq<Option<WriterEntryV1>>, end: int) -> int
    decreases end,
{
    if end <= 0 { 0 }
    else { reserved_prefix_v1(writers, end - 1) + if reserved_v1(writers[end - 1]) { 1int } else { 0int } }
}

// This weak predicate permits the malformed logical states used by guard witnesses.
pub open spec fn representation_valid_v1(state: JournalStateV1) -> bool {
    &&& state.writers.len() <= state.writers_storage
    &&& state.free.len() <= state.free_storage
    &&& state.other.allocations.len() <= state.other.allocations_storage
    &&& state.other.allocation_free.len() <= state.other.allocation_free_storage
    &&& state.other.members.len() <= state.other.members_storage
    &&& state.other.member_free.len() <= state.other.member_free_storage
    &&& state.other.scratch.len() <= state.other.scratch_storage
}

pub open spec fn partition_v1(state: JournalStateV1) -> bool {
    &&& forall |i: int| 0 <= i < state.free.len() ==> state.free[i] < state.writers.len()
    &&& forall |i: int, j: int| 0 <= i < j < state.free.len() ==> state.free[i] != state.free[j]
    &&& forall |slot: int| 0 <= slot < state.writers.len()
        ==> (state.writers[slot].is_none() == state.free.contains(slot as usize))
    &&& occupied_prefix_v1(state.writers, state.writers.len() as int) + state.free.len()
        == state.writer_capacity
}

pub open spec fn history_valid_v1(state: JournalStateV1) -> bool {
    let history = state.successful_registrations;
    &&& (if history.len() == 0 { state.registration_watermark == 0 }
        else { state.registration_watermark == history[history.len() - 1].key.local })
    &&& forall |i: int| 0 <= i < history.len() ==> {
        &&& history[i].slot < state.writer_capacity
        &&& history[i].key.context_generation == state.context_generation
        &&& issuable_id_v1(history[i].key.local)
    }
    &&& forall |i: int, j: int| 0 <= i < j < history.len()
        ==> history[i].key.local < history[j].key.local
    &&& forall |slot: int| #![trigger state.writers[slot]] 0 <= slot < state.writers.len() ==> match state.writers[slot] {
        Some(entry) => {
            &&& exists |i: int| 0 <= i < history.len()
                && history[i].slot == slot && same_key_v1(history[i].key, writer_key_v1(entry))
            &&& forall |i: int| 0 <= i < history.len() && history[i].slot == slot
                ==> history[i].key.local <= writer_key_v1(entry).local
        },
        None => true,
    }
}

pub open spec fn issuance_invariant_v1(state: JournalStateV1) -> bool {
    &&& representation_valid_v1(state)
    &&& constructor_admission_v1(
        state.context_generation, state.other.allocation_capacity, state.writer_capacity).is_none()
    &&& state.writers.len() == state.writer_capacity
    &&& state.other.allocations.len() == state.other.allocation_capacity
    &&& state.other.members.len() == state.other.allocation_capacity
    &&& state.other.scratch.len() == state.other.allocation_capacity
    &&& state.writers_storage >= state.writer_capacity
    &&& state.free_storage >= state.writer_capacity
    &&& state.other.allocations_storage >= state.other.allocation_capacity
    &&& state.other.allocation_free_storage >= state.other.allocation_capacity
    &&& state.other.members_storage >= state.other.allocation_capacity
    &&& state.other.member_free_storage >= state.other.allocation_capacity
    &&& state.other.scratch_storage >= state.other.allocation_capacity
    &&& partition_v1(state)
    &&& state.reserved_count == reserved_prefix_v1(state.writers, state.writers.len() as int)
    &&& history_valid_v1(state)
}

pub open spec fn register_successor_v1(
    state: JournalStateV1, key: WriterKeyV1, plan: RegisterPlanV1,
) -> JournalStateV1 {
    JournalStateV1 {
        context_generation: state.context_generation,
        writer_capacity: state.writer_capacity,
        registration_watermark: key.local,
        reserved_count: plan.reserved_count,
        writers: state.writers.update(plan.slot as int, Some(WriterEntryV1::Reserved(key))),
        free: state.free.subrange(0, state.free.len() - 1),
        writers_storage: state.writers_storage,
        free_storage: state.free_storage,
        other: state.other,
        successful_registrations: state.successful_registrations.push(
            WriterReferenceV1 { slot: plan.slot, key }),
    }
}

// Only vector contents and scalar fields are related here. Physical Vec storage,
// allocator behavior, source binding and whole-call unwind behavior remain external.
pub open spec fn register_execution_relation_v1(
    context: u64, writer_capacity: usize, key: WriterKeyV1,
    before_writers: Seq<Option<WriterEntryV1>>, before_free: Seq<usize>,
    before_watermark: u64, before_count: usize,
    after_writers: Seq<Option<WriterEntryV1>>, after_free: Seq<usize>,
    after_watermark: u64, after_count: usize,
    result: Result<WriterReferenceV1, JournalErrorV1>,
) -> bool {
    match register_decision_v1(context, before_watermark, before_count, writer_capacity,
        before_writers, before_free, key) {
        Err(error) => {
            &&& result == Err(error)
            &&& after_writers == before_writers
            &&& after_free == before_free
            &&& after_watermark == before_watermark
            &&& after_count == before_count
        },
        Ok(plan) => {
            &&& result == Ok(WriterReferenceV1 { slot: plan.slot, key })
            &&& after_writers == before_writers.update(plan.slot as int, Some(WriterEntryV1::Reserved(key)))
            &&& after_free == before_free.subrange(0, before_free.len() - 1)
            &&& after_watermark == key.local
            &&& after_count == plan.reserved_count
        },
    }
}

// No issuance invariant is required to prove exact malformed-state rejection.
pub fn register_writer_exec_v1(
    context: u64, writer_capacity: usize, key: WriterKeyV1,
    writers: &mut Vec<Option<WriterEntryV1>>, free: &mut Vec<usize>,
    watermark: &mut u64, reserved_count: &mut usize,
) -> (result: Result<WriterReferenceV1, JournalErrorV1>)
    ensures register_execution_relation_v1(context, writer_capacity, key,
        old(writers)@, old(free)@, *old(watermark), *old(reserved_count),
        final(writers)@, final(free)@, *final(watermark), *final(reserved_count), result),
{
    let plan = match register_preflight_v1(context, *watermark, *reserved_count,
        writer_capacity, writers.as_slice(), free.as_slice(), key) {
        Err(error) => return Err(error),
        Ok(plan) => plan,
    };
    proof {
        assert(free@.len() > 0);
        assert(plan.slot < writers@.len());
    }
    let _ = free.pop();
    writers[plan.slot] = Some(WriterEntryV1::Reserved(key));
    *reserved_count = plan.reserved_count;
    *watermark = key.local;
    Ok(WriterReferenceV1 { slot: plan.slot, key })
}

// Storage labels and untouched journal fields are framed ghost inputs, not
// observations of actual Vec capacity or addresses. Registration history is ghost.
pub open spec fn register_execution_projection_v1(
    pre: JournalStateV1, writers: Seq<Option<WriterEntryV1>>, free: Seq<usize>,
    watermark: u64, reserved_count: usize, result: Result<WriterReferenceV1, JournalErrorV1>,
) -> JournalStateV1 {
    JournalStateV1 {
        context_generation: pre.context_generation, writer_capacity: pre.writer_capacity,
        registration_watermark: watermark, reserved_count, writers, free,
        writers_storage: pre.writers_storage, free_storage: pre.free_storage, other: pre.other,
        successful_registrations: match result {
            Ok(reference) => pre.successful_registrations.push(reference),
            Err(_) => pre.successful_registrations,
        },
    }
}

pub proof fn register_execution_refines_v1(
    pre: JournalStateV1, key: WriterKeyV1,
    writers: Seq<Option<WriterEntryV1>>, free: Seq<usize>, watermark: u64,
    reserved_count: usize, result: Result<WriterReferenceV1, JournalErrorV1>,
)
    requires register_execution_relation_v1(pre.context_generation, pre.writer_capacity, key,
        pre.writers, pre.free, pre.registration_watermark, pre.reserved_count,
        writers, free, watermark, reserved_count, result),
    ensures
        register_execution_projection_v1(pre, writers, free, watermark, reserved_count, result)
            == step_v1(pre, ActionV1::Register(key)).0,
        match result {
            Ok(reference) => step_v1(pre, ActionV1::Register(key)).1 == OutcomeV1::Registered(reference),
            Err(error) => step_v1(pre, ActionV1::Register(key)).1 == OutcomeV1::Rejected(error),
        },
        issuance_invariant_v1(pre) ==> issuance_invariant_v1(
            register_execution_projection_v1(pre, writers, free, watermark, reserved_count, result)),
{
    let post = register_execution_projection_v1(pre, writers, free, watermark, reserved_count, result);
    match register_decision_v1(pre.context_generation, pre.registration_watermark,
        pre.reserved_count, pre.writer_capacity, pre.writers, pre.free, key) {
        Err(_) => { assert(post == pre); },
        Ok(plan) => {
            assert(post == register_successor_v1(pre, key, plan));
            if issuance_invariant_v1(pre) { register_preserves_v1(pre, key, plan); }
        },
    }
}

pub open spec fn abort_successor_v1(
    state: JournalStateV1, reference: WriterReferenceV1, plan: AbortPlanV1,
) -> JournalStateV1 {
    JournalStateV1 {
        context_generation: state.context_generation,
        writer_capacity: state.writer_capacity,
        registration_watermark: state.registration_watermark,
        reserved_count: plan.reserved_count,
        writers: state.writers.update(reference.slot as int, None),
        free: state.free.push(reference.slot),
        writers_storage: state.writers_storage,
        free_storage: state.free_storage,
        other: state.other,
        successful_registrations: state.successful_registrations,
    }
}

// The capacity argument is an external observation. This relation proves
// contents and scalar updates, not physical allocation or whole-call unwind.
pub open spec fn abort_execution_relation_v1(
    context: u64, writer_capacity: usize, observed_free_capacity: usize,
    reference: WriterReferenceV1,
    before_writers: Seq<Option<WriterEntryV1>>, before_free: Seq<usize>,
    before_watermark: u64, before_count: usize,
    after_writers: Seq<Option<WriterEntryV1>>, after_free: Seq<usize>,
    after_watermark: u64, after_count: usize,
    result: Result<(), JournalErrorV1>,
) -> bool {
    match abort_decision_v1(context, before_count, writer_capacity,
        before_writers, before_free.len() as usize, observed_free_capacity, reference) {
        Err(error) => {
            &&& result == Err(error)
            &&& after_writers == before_writers
            &&& after_free == before_free
            &&& after_watermark == before_watermark
            &&& after_count == before_count
        },
        Ok(plan) => {
            &&& result == Ok(())
            &&& after_writers == before_writers.update(reference.slot as int, None)
            &&& after_free == before_free.push(reference.slot)
            &&& after_watermark == before_watermark
            &&& after_count == plan.reserved_count
        },
    }
}

// Malformed references, counts and headroom remain in the executable contract.
pub fn abort_reserved_exec_v1(
    context: u64, writer_capacity: usize, observed_free_capacity: usize,
    reference: WriterReferenceV1,
    writers: &mut Vec<Option<WriterEntryV1>>, free: &mut Vec<usize>,
    watermark: &mut u64, reserved_count: &mut usize,
) -> (result: Result<(), JournalErrorV1>)
    ensures abort_execution_relation_v1(context, writer_capacity, observed_free_capacity, reference,
        old(writers)@, old(free)@, *old(watermark), *old(reserved_count),
        final(writers)@, final(free)@, *final(watermark), *final(reserved_count), result),
{
    let plan = match abort_preflight_v1(context, *reserved_count, writer_capacity,
        writers.as_slice(), free.len(), observed_free_capacity, reference) {
        Err(error) => return Err(error),
        Ok(plan) => plan,
    };
    proof {
        assert(reference.slot < writers@.len());
    }
    writers[reference.slot] = None;
    free.push(reference.slot);
    *reserved_count = plan.reserved_count;
    Ok(())
}

// Aborts preserve successful registration history and every other arena.
pub open spec fn abort_execution_projection_v1(
    pre: JournalStateV1, writers: Seq<Option<WriterEntryV1>>, free: Seq<usize>,
    watermark: u64, reserved_count: usize,
) -> JournalStateV1 {
    JournalStateV1 {
        context_generation: pre.context_generation, writer_capacity: pre.writer_capacity,
        registration_watermark: watermark, reserved_count, writers, free,
        writers_storage: pre.writers_storage, free_storage: pre.free_storage, other: pre.other,
        successful_registrations: pre.successful_registrations,
    }
}

pub proof fn abort_execution_refines_v1(
    pre: JournalStateV1, observed_free_capacity: usize, reference: WriterReferenceV1,
    writers: Seq<Option<WriterEntryV1>>, free: Seq<usize>, watermark: u64,
    reserved_count: usize, result: Result<(), JournalErrorV1>,
)
    requires
        observed_free_capacity == pre.free_storage,
        abort_execution_relation_v1(pre.context_generation, pre.writer_capacity, observed_free_capacity,
            reference, pre.writers, pre.free, pre.registration_watermark, pre.reserved_count,
            writers, free, watermark, reserved_count, result),
    ensures
        abort_execution_projection_v1(pre, writers, free, watermark, reserved_count)
            == step_v1(pre, ActionV1::Abort(reference)).0,
        match result {
            Ok(()) => step_v1(pre, ActionV1::Abort(reference)).1 == OutcomeV1::Aborted,
            Err(error) => step_v1(pre, ActionV1::Abort(reference)).1 == OutcomeV1::Rejected(error),
        },
        issuance_invariant_v1(pre) ==> issuance_invariant_v1(
            abort_execution_projection_v1(pre, writers, free, watermark, reserved_count)),
{
    let post = abort_execution_projection_v1(pre, writers, free, watermark, reserved_count);
    match abort_decision_v1(pre.context_generation, pre.reserved_count, pre.writer_capacity,
        pre.writers, pre.free.len() as usize, observed_free_capacity, reference) {
        Err(_) => { assert(post == pre); },
        Ok(plan) => {
            assert(post == abort_successor_v1(pre, reference, plan));
            if issuance_invariant_v1(pre) { abort_preserves_v1(pre, reference, plan); }
        },
    }
}

pub enum ActionV1 { Register(WriterKeyV1), Lookup(WriterReferenceV1), Abort(WriterReferenceV1) }

pub enum OutcomeV1 {
    Registered(WriterReferenceV1),
    Found(WriterKeyV1),
    Aborted,
    Rejected(JournalErrorV1),
}

pub open spec fn step_v1(state: JournalStateV1, action: ActionV1) -> (JournalStateV1, OutcomeV1) {
    match action {
        ActionV1::Register(key) => match register_decision_v1(
            state.context_generation, state.registration_watermark, state.reserved_count,
            state.writer_capacity, state.writers, state.free, key) {
            Ok(plan) => (register_successor_v1(state, key, plan),
                OutcomeV1::Registered(WriterReferenceV1 { slot: plan.slot, key })),
            Err(error) => (state, OutcomeV1::Rejected(error)),
        },
        ActionV1::Lookup(reference) => match lookup_decision_v1(
            state.context_generation, state.writers, reference) {
            Ok(key) => (state, OutcomeV1::Found(key)),
            Err(error) => (state, OutcomeV1::Rejected(error)),
        },
        ActionV1::Abort(reference) => match abort_decision_v1(
            state.context_generation, state.reserved_count, state.writer_capacity,
            state.writers, state.free.len() as usize, state.free_storage, reference) {
            Ok(plan) => (abort_successor_v1(state, reference, plan), OutcomeV1::Aborted),
            Err(error) => (state, OutcomeV1::Rejected(error)),
        },
    }
}

pub proof fn lookup_and_rejection_frame_v1(state: JournalStateV1, action: ActionV1)
    ensures
        match step_v1(state, action).1 {
            OutcomeV1::Found(_) | OutcomeV1::Rejected(_) => step_v1(state, action).0 == state,
            _ => true,
        },
{
}

pub proof fn unchanged_other_state_v1(state: JournalStateV1, action: ActionV1)
    ensures step_v1(state, action).0.other == state.other,
{
}


pub struct StorageCapacitiesV1 {
    pub writers: usize,
    pub free: usize,
    pub allocations: usize,
    pub allocation_free: usize,
    pub members: usize,
    pub member_free: usize,
    pub scratch: usize,
}

pub open spec fn storage_admission_v1(
    allocation_capacity: usize, writer_capacity: usize, storage: StorageCapacitiesV1,
) -> bool {
    &&& storage.writers >= writer_capacity
    &&& storage.free >= writer_capacity
    &&& storage.allocations >= allocation_capacity
    &&& storage.allocation_free >= allocation_capacity
    &&& storage.members >= allocation_capacity
    &&& storage.member_free >= allocation_capacity
    &&& storage.scratch >= allocation_capacity
}

pub open spec fn reverse_free_v1(capacity: usize) -> Seq<usize> {
    Seq::new(capacity as nat, |i: int| (capacity as int - 1 - i) as usize)
}

// This state is conditional on successful reservation of all seven Rust vectors.
pub open spec fn initial_state_v1(
    context: u64, allocation_capacity: usize, writer_capacity: usize, storage: StorageCapacitiesV1,
) -> JournalStateV1 {
    JournalStateV1 {
        context_generation: context,
        writer_capacity,
        registration_watermark: 0,
        reserved_count: 0,
        writers: Seq::new(writer_capacity as nat, |i: int| None),
        free: reverse_free_v1(writer_capacity),
        writers_storage: storage.writers,
        free_storage: storage.free,
        other: OtherJournalStateV1 {
            allocation_capacity,
            allocations: Seq::new(allocation_capacity as nat, |i: int| None),
            allocation_free: reverse_free_v1(allocation_capacity),
            members: Seq::new(allocation_capacity as nat, |i: int| None),
            member_free: reverse_free_v1(allocation_capacity),
            scratch: Seq::new(allocation_capacity as nat, |i: int| None),
            allocations_storage: storage.allocations,
            allocation_free_storage: storage.allocation_free,
            members_storage: storage.members,
            member_free_storage: storage.member_free,
            scratch_storage: storage.scratch,
        },
        successful_registrations: Seq::empty(),
    }
}


// Constructor contents only: allocation failure and production resize/iterator
// correspondence are not modeled by these executable loops.
pub fn vacant_contents_exec_v1<T>(capacity: usize) -> (result: Vec<Option<T>>)
    ensures result@ == Seq::new(capacity as nat, |i: int| None::<T>),
{
    let mut slots: Vec<Option<T>> = Vec::new();
    while slots.len() < capacity
        invariant
            slots@.len() <= capacity,
            forall |i: int| 0 <= i < slots@.len() ==> #[trigger] slots@[i] == None::<T>,
        decreases capacity - slots@.len(),
    {
        slots.push(None);
    }
    proof {
        assert(slots@ =~= Seq::new(capacity as nat, |i: int| None::<T>));
    }
    slots
}

pub fn free_contents_exec_v1(capacity: usize) -> (result: Vec<usize>)
    ensures result@ == reverse_free_v1(capacity),
{
    let mut free: Vec<usize> = Vec::new();
    let mut next = capacity;
    while next > 0
        invariant
            next <= capacity,
            free@.len() + next as nat == capacity as nat,
            forall |i: int| 0 <= i < free@.len()
                ==> #[trigger] free@[i] == (capacity as int - 1 - i) as usize,
        decreases next,
    {
        next = next - 1;
        free.push(next);
    }
    proof {
        assert(free@ =~= reverse_free_v1(capacity));
    }
    free
}

pub struct JournalContentsV1 {
    pub context_generation: u64,
    pub allocation_capacity: usize,
    pub writer_capacity: usize,
    pub registration_watermark: u64,
    pub reserved_count: usize,
    pub writers: Vec<Option<WriterEntryV1>>,
    pub free: Vec<usize>,
    pub allocations: Vec<Option<AllocationEntryV1>>,
    pub allocation_free: Vec<usize>,
    pub members: Vec<Option<MemberEntryV1>>,
    pub member_free: Vec<usize>,
    pub scratch: Vec<Option<BeginMemberPlanV1>>,
}

pub open spec fn constructor_contents_initialized_v1(
    contents: JournalContentsV1, context: u64, allocation_capacity: usize, writer_capacity: usize,
) -> bool {
    &&& contents.context_generation == context
    &&& contents.allocation_capacity == allocation_capacity
    &&& contents.writer_capacity == writer_capacity
    &&& contents.registration_watermark == 0
    &&& contents.reserved_count == 0
    &&& contents.writers@ == Seq::new(writer_capacity as nat, |i: int| None)
    &&& contents.free@ == reverse_free_v1(writer_capacity)
    &&& contents.allocations@ == Seq::new(allocation_capacity as nat, |i: int| None)
    &&& contents.allocation_free@ == reverse_free_v1(allocation_capacity)
    &&& contents.members@ == Seq::new(allocation_capacity as nat, |i: int| None)
    &&& contents.member_free@ == reverse_free_v1(allocation_capacity)
    &&& contents.scratch@ == Seq::new(allocation_capacity as nat, |i: int| None)
}

pub open spec fn constructor_contents_relation_v1(
    context: u64, allocation_capacity: usize, writer_capacity: usize,
    result: Result<JournalContentsV1, JournalErrorV1>,
) -> bool {
    match constructor_admission_v1(context, allocation_capacity, writer_capacity) {
        Some(error) => result == Err(error),
        None => match result {
            Ok(contents) => constructor_contents_initialized_v1(
                contents, context, allocation_capacity, writer_capacity),
            Err(_) => false,
        },
    }
}

pub fn constructor_contents_exec_v1(
    context: u64, allocation_capacity: usize, writer_capacity: usize,
) -> (result: Result<JournalContentsV1, JournalErrorV1>)
    ensures constructor_contents_relation_v1(context, allocation_capacity, writer_capacity, result),
{
    if let Some(error) = constructor_preflight_v1(context, allocation_capacity, writer_capacity) {
        return Err(error);
    }
    let writers = vacant_contents_exec_v1(writer_capacity);
    let free = free_contents_exec_v1(writer_capacity);
    let allocations = vacant_contents_exec_v1(allocation_capacity);
    let allocation_free = free_contents_exec_v1(allocation_capacity);
    let members = vacant_contents_exec_v1(allocation_capacity);
    let member_free = free_contents_exec_v1(allocation_capacity);
    let scratch = Vec::new();
    Ok(JournalContentsV1 {
        context_generation: context, allocation_capacity, writer_capacity,
        registration_watermark: 0, reserved_count: 0,
        writers, free, allocations, allocation_free, members, member_free, scratch,
    })
}

// Every concrete field is projected. Only storage labels and issuance history
// remain ghost inputs; successful construction starts with empty ghost history.
pub open spec fn constructor_contents_projection_v1(
    contents: JournalContentsV1, storage: StorageCapacitiesV1,
) -> JournalStateV1 {
    JournalStateV1 {
        context_generation: contents.context_generation,
        writer_capacity: contents.writer_capacity,
        registration_watermark: contents.registration_watermark,
        reserved_count: contents.reserved_count,
        writers: contents.writers@,
        free: contents.free@,
        writers_storage: storage.writers,
        free_storage: storage.free,
        other: OtherJournalStateV1 {
            allocation_capacity: contents.allocation_capacity,
            allocations: contents.allocations@,
            allocation_free: contents.allocation_free@,
            members: contents.members@,
            member_free: contents.member_free@,
            scratch: contents.scratch@,
            allocations_storage: storage.allocations,
            allocation_free_storage: storage.allocation_free,
            members_storage: storage.members,
            member_free_storage: storage.member_free,
            scratch_storage: storage.scratch,
        },
        successful_registrations: Seq::empty(),
    }
}

pub proof fn constructor_contents_projection_exact_v1(
    contents: JournalContentsV1, context: u64, allocation_capacity: usize,
    writer_capacity: usize, storage: StorageCapacitiesV1,
)
    requires constructor_contents_initialized_v1(contents, context, allocation_capacity, writer_capacity),
    ensures constructor_contents_projection_v1(contents, storage)
        == initial_state_v1(context, allocation_capacity, writer_capacity, storage),
{
}

pub proof fn constructor_contents_refines_v1(
    context: u64, allocation_capacity: usize, writer_capacity: usize,
    result: Result<JournalContentsV1, JournalErrorV1>, storage: StorageCapacitiesV1,
)
    requires constructor_contents_relation_v1(context, allocation_capacity, writer_capacity, result),
    ensures
        match result {
            Err(error) => constructor_admission_v1(context, allocation_capacity, writer_capacity) == Some(error),
            Ok(contents) => {
                &&& constructor_admission_v1(context, allocation_capacity, writer_capacity).is_none()
                &&& constructor_contents_projection_v1(contents, storage)
                    == initial_state_v1(context, allocation_capacity, writer_capacity, storage)
                &&& (storage_admission_v1(allocation_capacity, writer_capacity, storage)
                    ==> issuance_invariant_v1(constructor_contents_projection_v1(contents, storage)))
            },
        },
{
    match result {
        Err(_) => {},
        Ok(contents) => {
            constructor_contents_projection_exact_v1(contents, context, allocation_capacity, writer_capacity, storage);
            if storage_admission_v1(allocation_capacity, writer_capacity, storage) {
                initialized_journal_invariant_v1(context, allocation_capacity, writer_capacity, storage);
            }
        },
    }
}

// Frame actual proof-side fields rather than copying unrelated state from a
// ghost prestate. This is a contents contract, not a physical storage contract.
pub open spec fn issuance_contents_frame_v1(before: JournalContentsV1, after: JournalContentsV1) -> bool {
    &&& after.context_generation == before.context_generation
    &&& after.allocation_capacity == before.allocation_capacity
    &&& after.writer_capacity == before.writer_capacity
    &&& after.allocations@ == before.allocations@
    &&& after.allocation_free@ == before.allocation_free@
    &&& after.members@ == before.members@
    &&& after.member_free@ == before.member_free@
    &&& after.scratch@ == before.scratch@
}

pub fn register_contents_exec_v1(journal: &mut JournalContentsV1, key: WriterKeyV1)
    -> (result: Result<WriterReferenceV1, JournalErrorV1>)
    ensures
        issuance_contents_frame_v1(*old(journal), *final(journal)),
        register_execution_relation_v1(old(journal).context_generation, old(journal).writer_capacity, key,
            old(journal).writers@, old(journal).free@, old(journal).registration_watermark,
            old(journal).reserved_count, final(journal).writers@, final(journal).free@,
            final(journal).registration_watermark, final(journal).reserved_count, result),
{
    register_writer_exec_v1(journal.context_generation, journal.writer_capacity, key,
        &mut journal.writers, &mut journal.free,
        &mut journal.registration_watermark, &mut journal.reserved_count)
}

pub fn abort_contents_exec_v1(
    journal: &mut JournalContentsV1, observed_free_capacity: usize, reference: WriterReferenceV1,
) -> (result: Result<(), JournalErrorV1>)
    ensures
        issuance_contents_frame_v1(*old(journal), *final(journal)),
        abort_execution_relation_v1(old(journal).context_generation, old(journal).writer_capacity,
            observed_free_capacity, reference, old(journal).writers@, old(journal).free@,
            old(journal).registration_watermark, old(journal).reserved_count,
            final(journal).writers@, final(journal).free@, final(journal).registration_watermark,
            final(journal).reserved_count, result),
{
    abort_reserved_exec_v1(journal.context_generation, journal.writer_capacity, observed_free_capacity,
        reference, &mut journal.writers, &mut journal.free,
        &mut journal.registration_watermark, &mut journal.reserved_count)
}

pub open spec fn issuance_contents_projection_v1(
    contents: JournalContentsV1, storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>,
) -> JournalStateV1 {
    JournalStateV1 { successful_registrations: history, ..constructor_contents_projection_v1(contents, storage) }
}

pub proof fn register_contents_refines_v1(
    before: JournalContentsV1, after: JournalContentsV1, key: WriterKeyV1,
    result: Result<WriterReferenceV1, JournalErrorV1>, storage: StorageCapacitiesV1,
    history: Seq<WriterReferenceV1>,
)
    requires
        issuance_contents_frame_v1(before, after),
        register_execution_relation_v1(before.context_generation, before.writer_capacity, key,
            before.writers@, before.free@, before.registration_watermark, before.reserved_count,
            after.writers@, after.free@, after.registration_watermark, after.reserved_count, result),
    ensures
        issuance_contents_projection_v1(after, storage, match result {
            Ok(reference) => history.push(reference), Err(_) => history,
        }) == step_v1(issuance_contents_projection_v1(before, storage, history), ActionV1::Register(key)).0,
        issuance_invariant_v1(issuance_contents_projection_v1(before, storage, history))
            ==> issuance_invariant_v1(issuance_contents_projection_v1(after, storage, match result {
                Ok(reference) => history.push(reference), Err(_) => history,
            })),
{
    let pre = issuance_contents_projection_v1(before, storage, history);
    register_execution_refines_v1(pre, key, after.writers@, after.free@,
        after.registration_watermark, after.reserved_count, result);
}

pub proof fn abort_contents_refines_v1(
    before: JournalContentsV1, after: JournalContentsV1, reference: WriterReferenceV1,
    result: Result<(), JournalErrorV1>, storage: StorageCapacitiesV1, history: Seq<WriterReferenceV1>,
)
    requires
        issuance_contents_frame_v1(before, after),
        abort_execution_relation_v1(before.context_generation, before.writer_capacity, storage.free,
            reference, before.writers@, before.free@, before.registration_watermark, before.reserved_count,
            after.writers@, after.free@, after.registration_watermark, after.reserved_count, result),
    ensures
        issuance_contents_projection_v1(after, storage, history)
            == step_v1(issuance_contents_projection_v1(before, storage, history), ActionV1::Abort(reference)).0,
        issuance_invariant_v1(issuance_contents_projection_v1(before, storage, history))
            ==> issuance_invariant_v1(issuance_contents_projection_v1(after, storage, history)),
{
    let pre = issuance_contents_projection_v1(before, storage, history);
    abort_execution_refines_v1(pre, storage.free, reference, after.writers@, after.free@,
        after.registration_watermark, after.reserved_count, result);
}

pub proof fn prefix_bounds_v1(writers: Seq<Option<WriterEntryV1>>, end: int)
    requires 0 <= end <= writers.len(),
    ensures
        0 <= reserved_prefix_v1(writers, end),
        reserved_prefix_v1(writers, end) <= occupied_prefix_v1(writers, end),
        occupied_prefix_v1(writers, end) <= end,
    decreases end,
{
    if end > 0 {
        prefix_bounds_v1(writers, end - 1);
        assert(reserved_v1(writers[end - 1]) ==> occupied_v1(writers[end - 1]));
    }
}

pub proof fn empty_prefixes_v1(capacity: usize, end: int)
    requires 0 <= end <= capacity,
    ensures
        occupied_prefix_v1(Seq::new(capacity as nat, |i: int| None), end) == 0,
        reserved_prefix_v1(Seq::new(capacity as nat, |i: int| None), end) == 0,
    decreases end,
{
    if end > 0 {
        empty_prefixes_v1(capacity, end - 1);
        let writers: Seq<Option<WriterEntryV1>> = Seq::new(capacity as nat, |i: int| None);
        assert(writers[end - 1].is_none());
    }
}

pub proof fn prefix_update_v1(
    writers: Seq<Option<WriterEntryV1>>, slot: int, value: Option<WriterEntryV1>, end: int,
)
    requires 0 <= slot < writers.len(), 0 <= end <= writers.len(),
    ensures
        occupied_prefix_v1(writers.update(slot, value), end) == occupied_prefix_v1(writers, end)
            + if slot < end {
                (if occupied_v1(value) { 1int } else { 0int })
                - (if occupied_v1(writers[slot]) { 1int } else { 0int })
            } else { 0int },
        reserved_prefix_v1(writers.update(slot, value), end) == reserved_prefix_v1(writers, end)
            + if slot < end {
                (if reserved_v1(value) { 1int } else { 0int })
                - (if reserved_v1(writers[slot]) { 1int } else { 0int })
            } else { 0int },
    decreases end,
{
    if end > 0 {
        prefix_update_v1(writers, slot, value, end - 1);
        assert(writers.update(slot, value)[end - 1]
            == if slot == end - 1 { value } else { writers[end - 1] });
    }
}

pub proof fn reverse_free_partition_v1(capacity: usize)
    ensures
        reverse_free_v1(capacity).len() == capacity,
        forall |i: int| 0 <= i < capacity ==> reverse_free_v1(capacity)[i] < capacity,
        forall |i: int, j: int| 0 <= i < j < capacity
            ==> reverse_free_v1(capacity)[i] != reverse_free_v1(capacity)[j],
        forall |slot: int| #![trigger reverse_free_v1(capacity).contains(slot as usize)]
            0 <= slot < capacity ==> reverse_free_v1(capacity).contains(slot as usize),
{
    let free = reverse_free_v1(capacity);
    assert forall |i: int| 0 <= i < capacity implies free[i] < capacity by {
        assert(free[i] as int == capacity as int - 1 - i);
    }
    assert forall |i: int, j: int| 0 <= i < j < capacity implies free[i] != free[j] by {
        assert(free[i] as int == capacity as int - 1 - i);
        assert(free[j] as int == capacity as int - 1 - j);
    }
    assert forall |slot: int| #![trigger free.contains(slot as usize)]
        0 <= slot < capacity implies free.contains(slot as usize) by {
        let index = capacity as int - 1 - slot;
        assert(0 <= index < capacity);
        assert(free[index] == slot as usize);
    }
}

pub proof fn initialized_journal_invariant_v1(
    context: u64, allocation_capacity: usize, writer_capacity: usize, storage: StorageCapacitiesV1,
)
    requires
        constructor_admission_v1(context, allocation_capacity, writer_capacity).is_none(),
        storage_admission_v1(allocation_capacity, writer_capacity, storage),
    ensures
        issuance_invariant_v1(initial_state_v1(context, allocation_capacity, writer_capacity, storage)),
{
    let state = initial_state_v1(context, allocation_capacity, writer_capacity, storage);
    reverse_free_partition_v1(writer_capacity);
    empty_prefixes_v1(writer_capacity, writer_capacity as int);
    assert forall |slot: int| 0 <= slot < state.writers.len()
        implies state.writers[slot].is_none() by {}
    assert(history_valid_v1(state));
    assert(partition_v1(state));
}


pub proof fn pop_last_membership_v1(free: Seq<usize>, value: usize)
    requires
        free.len() > 0,
        forall |i: int, j: int| 0 <= i < j < free.len() ==> free[i] != free[j],
    ensures
        free.subrange(0, free.len() - 1).contains(value)
            == (free.contains(value) && value != free[free.len() - 1]),
{
    let end = free.len() as int - 1;
    let popped = free.subrange(0, end);
    if popped.contains(value) {
        let i = choose |i: int| 0 <= i < popped.len() && popped[i] == value;
        assert(free[i] == value);
        assert(free[i] != free[end]);
        assert(free.contains(value));
    }
    if free.contains(value) && value != free[end] {
        let i = choose |i: int| 0 <= i < free.len() && free[i] == value;
        assert(i != end);
        assert(0 <= i < end);
        assert(popped[i] == value);
        assert(popped.contains(value));
    }
}

pub proof fn history_key_bounded_v1(pre: JournalStateV1, i: int)
    requires history_valid_v1(pre), 0 <= i < pre.successful_registrations.len(),
    ensures pre.successful_registrations[i].key.local <= pre.registration_watermark,
{
    let history = pre.successful_registrations;
    let last = history.len() as int - 1;
    if i < last {
        assert(history[i].key.local < history[last].key.local);
    }
}

pub proof fn reserved_slot_headroom_v1(pre: JournalStateV1, slot: usize)
    requires
        issuance_invariant_v1(pre),
        slot < pre.writers.len(),
        reserved_v1(pre.writers[slot as int]),
    ensures
        pre.reserved_count > 0,
        pre.free.len() < pre.writer_capacity,
        pre.free.len() < pre.free_storage,
{
    let end = pre.writers.len() as int;
    let cleared = pre.writers.update(slot as int, None);
    prefix_update_v1(pre.writers, slot as int, None, end);
    prefix_bounds_v1(cleared, end);
    assert(occupied_v1(pre.writers[slot as int]));
    assert(reserved_prefix_v1(pre.writers, end) >= 1);
    assert(occupied_prefix_v1(pre.writers, end) >= 1);
    assert(pre.reserved_count >= 1);
    assert(pre.free.len() < pre.writer_capacity);
}

pub proof fn register_history_preserves_v1(
    pre: JournalStateV1, key: WriterKeyV1, plan: RegisterPlanV1,
)
    requires
        issuance_invariant_v1(pre),
        register_decision_v1(pre.context_generation, pre.registration_watermark,
            pre.reserved_count, pre.writer_capacity, pre.writers, pre.free, key) == Ok(plan),
    ensures history_valid_v1(register_successor_v1(pre, key, plan)),
{
    let post = register_successor_v1(pre, key, plan);
    let old = pre.successful_registrations;
    let history = post.successful_registrations;
    let end = old.len() as int;
    assert(plan.slot < pre.writers.len());
    assert(pre.writers[plan.slot as int].is_none());
    assert(key.local > pre.registration_watermark);
    assert forall |i: int| 0 <= i < history.len() implies {
        &&& history[i].slot < post.writer_capacity
        &&& history[i].key.context_generation == post.context_generation
        &&& issuable_id_v1(history[i].key.local)
    } by {
        if i < end { assert(history[i] == old[i]); }
        else { assert(i == end); assert(history[i] == WriterReferenceV1 { slot: plan.slot, key }); }
    }
    assert forall |i: int, j: int| 0 <= i < j < history.len()
        implies history[i].key.local < history[j].key.local by {
        assert(i < end);
        if j < end {
            assert(history[i] == old[i]);
            assert(history[j] == old[j]);
        } else {
            assert(j == end);
            history_key_bounded_v1(pre, i);
            assert(history[j].key == key);
        }
    }
    assert forall |slot: int| #![trigger post.writers[slot]] 0 <= slot < post.writers.len() implies match post.writers[slot] {
        Some(entry) => {
            &&& exists |i: int| 0 <= i < history.len()
                && history[i].slot == slot && same_key_v1(history[i].key, writer_key_v1(entry))
            &&& forall |i: int| 0 <= i < history.len() && history[i].slot == slot
                ==> history[i].key.local <= writer_key_v1(entry).local
        },
        None => true,
    } by {
        if let Some(entry) = post.writers[slot] {
            if slot == plan.slot {
                assert(entry == WriterEntryV1::Reserved(key));
                assert(history[end].slot == slot);
                assert(same_key_v1(history[end].key, writer_key_v1(entry)));
                assert forall |i: int| 0 <= i < history.len() && history[i].slot == slot
                    implies history[i].key.local <= writer_key_v1(entry).local by {
                    if i < end { history_key_bounded_v1(pre, i); }
                    else { assert(i == end); }
                }
            } else {
                assert(pre.writers[slot] == Some(entry));
                let witness = choose |i: int| 0 <= i < old.len()
                    && old[i].slot == slot && same_key_v1(old[i].key, writer_key_v1(entry));
                assert(history[witness] == old[witness]);
                assert forall |i: int| 0 <= i < history.len() && history[i].slot == slot
                    implies history[i].key.local <= writer_key_v1(entry).local by {
                    assert(i < end);
                    assert(history[i] == old[i]);
                }
            }
        }
    }
}

pub proof fn abort_history_preserves_v1(
    pre: JournalStateV1, reference: WriterReferenceV1, plan: AbortPlanV1,
)
    requires
        issuance_invariant_v1(pre),
        abort_decision_v1(pre.context_generation, pre.reserved_count, pre.writer_capacity,
            pre.writers, pre.free.len() as usize, pre.free_storage, reference) == Ok(plan),
    ensures history_valid_v1(abort_successor_v1(pre, reference, plan)),
{
    let post = abort_successor_v1(pre, reference, plan);
    let history = pre.successful_registrations;
    assert(reference.slot < pre.writers.len());
    assert forall |slot: int| #![trigger post.writers[slot]] 0 <= slot < post.writers.len() implies match post.writers[slot] {
        Some(entry) => {
            &&& exists |i: int| 0 <= i < history.len()
                && history[i].slot == slot && same_key_v1(history[i].key, writer_key_v1(entry))
            &&& forall |i: int| 0 <= i < history.len() && history[i].slot == slot
                ==> history[i].key.local <= writer_key_v1(entry).local
        },
        None => true,
    } by {
        if let Some(entry) = post.writers[slot] {
            assert(slot != reference.slot);
            assert(pre.writers[slot] == Some(entry));
        }
    }
}

pub proof fn register_preserves_v1(
    pre: JournalStateV1, key: WriterKeyV1, plan: RegisterPlanV1,
)
    requires
        issuance_invariant_v1(pre),
        register_decision_v1(pre.context_generation, pre.registration_watermark,
            pre.reserved_count, pre.writer_capacity, pre.writers, pre.free, key) == Ok(plan),
    ensures issuance_invariant_v1(register_successor_v1(pre, key, plan)),
{
    let post = register_successor_v1(pre, key, plan);
    let end = pre.writers.len() as int;
    assert(pre.free.len() > 0);
    assert(plan.slot == pre.free[pre.free.len() - 1]);
    assert(plan.slot < end);
    assert(pre.writers[plan.slot as int].is_none());
    prefix_bounds_v1(pre.writers, end);
    assert(pre.reserved_count <= occupied_prefix_v1(pre.writers, end));
    assert(occupied_prefix_v1(pre.writers, end) < pre.writer_capacity);
    assert(pre.reserved_count < pre.writer_capacity);
    prefix_update_v1(pre.writers, plan.slot as int, Some(WriterEntryV1::Reserved(key)), end);
    assert(occupied_prefix_v1(post.writers, end) == occupied_prefix_v1(pre.writers, end) + 1);
    assert(reserved_prefix_v1(post.writers, end) == reserved_prefix_v1(pre.writers, end) + 1);
    assert forall |i: int| 0 <= i < post.free.len() implies post.free[i] < post.writers.len() by {
        assert(post.free[i] == pre.free[i]);
    }
    assert forall |i: int, j: int| 0 <= i < j < post.free.len()
        implies post.free[i] != post.free[j] by {
        assert(post.free[i] == pre.free[i]);
        assert(post.free[j] == pre.free[j]);
    }
    assert forall |slot: int| 0 <= slot < post.writers.len()
        implies post.writers[slot].is_none() == post.free.contains(slot as usize) by {
        pop_last_membership_v1(pre.free, slot as usize);
        if slot == plan.slot { assert(post.writers[slot].is_some()); }
        else { assert(post.writers[slot] == pre.writers[slot]); }
    }
    assert(partition_v1(post));
    register_history_preserves_v1(pre, key, plan);
    assert(representation_valid_v1(post));
}

pub proof fn abort_preserves_v1(
    pre: JournalStateV1, reference: WriterReferenceV1, plan: AbortPlanV1,
)
    requires
        issuance_invariant_v1(pre),
        abort_decision_v1(pre.context_generation, pre.reserved_count, pre.writer_capacity,
            pre.writers, pre.free.len() as usize, pre.free_storage, reference) == Ok(plan),
    ensures issuance_invariant_v1(abort_successor_v1(pre, reference, plan)),
{
    let post = abort_successor_v1(pre, reference, plan);
    let end = pre.writers.len() as int;
    assert(reference.slot < pre.writers.len());
    assert(reserved_v1(pre.writers[reference.slot as int]));
    reserved_slot_headroom_v1(pre, reference.slot);
    assert(!pre.free.contains(reference.slot));
    prefix_update_v1(pre.writers, reference.slot as int, None, end);
    assert(occupied_prefix_v1(post.writers, end) == occupied_prefix_v1(pre.writers, end) - 1);
    assert(reserved_prefix_v1(post.writers, end) == reserved_prefix_v1(pre.writers, end) - 1);
    assert forall |i: int| 0 <= i < post.free.len() implies post.free[i] < post.writers.len() by {
        if i < pre.free.len() { assert(post.free[i] == pre.free[i]); }
        else { assert(post.free[i] == reference.slot); }
    }
    assert forall |i: int, j: int| 0 <= i < j < post.free.len()
        implies post.free[i] != post.free[j] by {
        assert(i < pre.free.len());
        assert(post.free[i] == pre.free[i]);
        if j < pre.free.len() { assert(post.free[j] == pre.free[j]); }
        else {
            assert(post.free[j] == reference.slot);
            assert(pre.free.contains(pre.free[i]));
        }
    }
    assert forall |slot: int| 0 <= slot < post.writers.len()
        implies post.writers[slot].is_none() == post.free.contains(slot as usize) by {
        vstd::seq_lib::lemma_seq_contains_after_push(pre.free, reference.slot, slot as usize);
        if slot == reference.slot { assert(post.writers[slot].is_none()); }
        else { assert(post.writers[slot] == pre.writers[slot]); }
    }
    assert(partition_v1(post));
    abort_history_preserves_v1(pre, reference, plan);
    assert(representation_valid_v1(post));
}


pub proof fn same_key_is_structural_equality_v1(left: WriterKeyV1, right: WriterKeyV1)
    requires same_key_v1(left, right),
    ensures left == right,
{
    assert(left.kind == right.kind);
}

pub proof fn step_preserves_v1(pre: JournalStateV1, action: ActionV1)
    requires issuance_invariant_v1(pre),
    ensures issuance_invariant_v1(step_v1(pre, action).0),
{
    match action {
        ActionV1::Register(key) => match register_decision_v1(
            pre.context_generation, pre.registration_watermark, pre.reserved_count,
            pre.writer_capacity, pre.writers, pre.free, key) {
            Ok(plan) => register_preserves_v1(pre, key, plan),
            Err(_) => {},
        },
        ActionV1::Abort(reference) => match abort_decision_v1(
            pre.context_generation, pre.reserved_count, pre.writer_capacity,
            pre.writers, pre.free.len() as usize, pre.free_storage, reference) {
            Ok(plan) => abort_preserves_v1(pre, reference, plan),
            Err(_) => {},
        },
        ActionV1::Lookup(_) => {},
    }
}

pub open spec fn trace_state_v1(initial: JournalStateV1, actions: Seq<ActionV1>, end: int)
    -> JournalStateV1
    decreases end,
{
    if end <= 0 { initial }
    else { step_v1(trace_state_v1(initial, actions, end - 1), actions[end - 1]).0 }
}

pub proof fn trace_preserves_v1(initial: JournalStateV1, actions: Seq<ActionV1>, end: int)
    requires issuance_invariant_v1(initial), 0 <= end <= actions.len(),
    ensures issuance_invariant_v1(trace_state_v1(initial, actions, end)),
    decreases end,
{
    if end > 0 {
        trace_preserves_v1(initial, actions, end - 1);
        step_preserves_v1(trace_state_v1(initial, actions, end - 1), actions[end - 1]);
    }
}

pub proof fn watermark_never_decreases_v1(pre: JournalStateV1, action: ActionV1)
    ensures step_v1(pre, action).0.registration_watermark >= pre.registration_watermark,
{
}

pub proof fn rejected_old_reference_stays_rejected_v1(
    pre: JournalStateV1, action: ActionV1, reference: WriterReferenceV1,
)
    requires
        issuance_invariant_v1(pre),
        reference.key.local <= pre.registration_watermark,
        lookup_decision_v1(pre.context_generation, pre.writers, reference)
            == Err(JournalErrorV1::InvalidReference),
    ensures
        lookup_decision_v1(step_v1(pre, action).0.context_generation,
            step_v1(pre, action).0.writers, reference) == Err(JournalErrorV1::InvalidReference),
        reference.key.local <= step_v1(pre, action).0.registration_watermark,
{
    let post = step_v1(pre, action).0;
    watermark_never_decreases_v1(pre, action);
    match action {
        ActionV1::Register(key) => match register_decision_v1(
            pre.context_generation, pre.registration_watermark, pre.reserved_count,
            pre.writer_capacity, pre.writers, pre.free, key) {
            Ok(plan) => {
                assert(key.local > pre.registration_watermark);
                assert(key.local != reference.key.local);
                if reference.slot < pre.writers.len() {
                    if reference.slot == plan.slot {
                        assert(post.writers[reference.slot as int] == Some(WriterEntryV1::Reserved(key)));
                        assert(!same_key_v1(key, reference.key));
                    } else {
                        assert(post.writers[reference.slot as int] == pre.writers[reference.slot as int]);
                    }
                }
            },
            Err(_) => {},
        },
        ActionV1::Abort(aborted) => match abort_decision_v1(
            pre.context_generation, pre.reserved_count, pre.writer_capacity,
            pre.writers, pre.free.len() as usize, pre.free_storage, aborted) {
            Ok(_) => {
                if reference.slot < pre.writers.len() {
                    if reference.slot == aborted.slot { assert(post.writers[reference.slot as int].is_none()); }
                    else { assert(post.writers[reference.slot as int] == pre.writers[reference.slot as int]); }
                }
            },
            Err(_) => {},
        },
        ActionV1::Lookup(_) => {},
    }
}

pub proof fn abort_makes_reference_stale_v1(
    pre: JournalStateV1, reference: WriterReferenceV1, plan: AbortPlanV1,
)
    requires
        issuance_invariant_v1(pre),
        abort_decision_v1(pre.context_generation, pre.reserved_count, pre.writer_capacity,
            pre.writers, pre.free.len() as usize, pre.free_storage, reference) == Ok(plan),
    ensures
        lookup_decision_v1(pre.context_generation, abort_successor_v1(pre, reference, plan).writers,
            reference) == Err(JournalErrorV1::InvalidReference),
        reference.key.local <= abort_successor_v1(pre, reference, plan).registration_watermark,
{
    assert(reference.slot < pre.writers.len());
    match pre.writers[reference.slot as int] {
        Some(WriterEntryV1::Reserved(key)) => same_key_is_structural_equality_v1(key, reference.key),
        _ => { assert(false); },
    }
    assert(pre.writers[reference.slot as int] == Some(WriterEntryV1::Reserved(reference.key)));
    let history = pre.successful_registrations;
    let witness = choose |i: int| 0 <= i < history.len()
        && history[i].slot == reference.slot && same_key_v1(history[i].key, reference.key);
    history_key_bounded_v1(pre, witness);
}

pub proof fn rejected_old_reference_trace_v1(
    initial: JournalStateV1, actions: Seq<ActionV1>, end: int, reference: WriterReferenceV1,
)
    requires
        issuance_invariant_v1(initial),
        0 <= end <= actions.len(),
        reference.key.local <= initial.registration_watermark,
        lookup_decision_v1(initial.context_generation, initial.writers, reference)
            == Err(JournalErrorV1::InvalidReference),
    ensures
        issuance_invariant_v1(trace_state_v1(initial, actions, end)),
        reference.key.local <= trace_state_v1(initial, actions, end).registration_watermark,
        lookup_decision_v1(trace_state_v1(initial, actions, end).context_generation,
            trace_state_v1(initial, actions, end).writers, reference)
            == Err(JournalErrorV1::InvalidReference),
    decreases end,
{
    if end > 0 {
        rejected_old_reference_trace_v1(initial, actions, end - 1, reference);
        let pre = trace_state_v1(initial, actions, end - 1);
        rejected_old_reference_stays_rejected_v1(pre, actions[end - 1], reference);
        step_preserves_v1(pre, actions[end - 1]);
    }
}

pub proof fn aborted_reference_never_revives_v1(
    pre: JournalStateV1, reference: WriterReferenceV1, plan: AbortPlanV1,
    actions: Seq<ActionV1>, end: int,
)
    requires
        issuance_invariant_v1(pre),
        0 <= end <= actions.len(),
        abort_decision_v1(pre.context_generation, pre.reserved_count, pre.writer_capacity,
            pre.writers, pre.free.len() as usize, pre.free_storage, reference) == Ok(plan),
    ensures
        lookup_decision_v1(
            trace_state_v1(abort_successor_v1(pre, reference, plan), actions, end).context_generation,
            trace_state_v1(abort_successor_v1(pre, reference, plan), actions, end).writers,
            reference) == Err(JournalErrorV1::InvalidReference),
{
    abort_preserves_v1(pre, reference, plan);
    abort_makes_reference_stale_v1(pre, reference, plan);
    rejected_old_reference_trace_v1(abort_successor_v1(pre, reference, plan), actions, end, reference);
}


pub proof fn register_keeps_older_reserved_v1(
    pre: JournalStateV1, key: WriterKeyV1, plan: RegisterPlanV1, reference: WriterReferenceV1,
)
    requires
        issuance_invariant_v1(pre),
        register_decision_v1(pre.context_generation, pre.registration_watermark,
            pre.reserved_count, pre.writer_capacity, pre.writers, pre.free, key) == Ok(plan),
        lookup_decision_v1(pre.context_generation, pre.writers, reference) == Ok(reference.key),
    ensures
        reference.slot != plan.slot,
        reference.key.local < key.local,
        lookup_decision_v1(pre.context_generation,
            register_successor_v1(pre, key, plan).writers, reference) == Ok(reference.key),
{
    assert(reference.slot < pre.writers.len());
    assert(pre.writers[reference.slot as int] == Some(WriterEntryV1::Reserved(reference.key)));
    assert(pre.writers[plan.slot as int].is_none());
    assert(reference.slot != plan.slot);
    let history = pre.successful_registrations;
    let i = choose |i: int| 0 <= i < history.len()
        && history[i].slot == reference.slot && same_key_v1(history[i].key, reference.key);
    history_key_bounded_v1(pre, i);
    assert(reference.key.local <= pre.registration_watermark);
    assert(register_successor_v1(pre, key, plan).writers[reference.slot as int]
        == pre.writers[reference.slot as int]);
}

pub proof fn abort_keeps_other_reserved_v1(
    pre: JournalStateV1, aborted: WriterReferenceV1, plan: AbortPlanV1,
    retained: WriterReferenceV1,
)
    requires
        issuance_invariant_v1(pre),
        abort_decision_v1(pre.context_generation, pre.reserved_count, pre.writer_capacity,
            pre.writers, pre.free.len() as usize, pre.free_storage, aborted) == Ok(plan),
        lookup_decision_v1(pre.context_generation, pre.writers, retained) == Ok(retained.key),
        aborted.slot != retained.slot,
    ensures
        lookup_decision_v1(pre.context_generation,
            abort_successor_v1(pre, aborted, plan).writers, retained) == Ok(retained.key),
        abort_successor_v1(pre, aborted, plan).registration_watermark
            == pre.registration_watermark,
{
    assert(retained.slot < pre.writers.len());
    assert(abort_successor_v1(pre, aborted, plan).writers[retained.slot as int]
        == pre.writers[retained.slot as int]);
}

pub open spec fn witness_key_v1(local: u64) -> WriterKeyV1 {
    WriterKeyV1 { context_generation: 7, local, kind: WriterKindV1::Synchronous }
}

pub open spec fn witness_reference_v1(slot: usize, local: u64) -> WriterReferenceV1 {
    WriterReferenceV1 { slot, key: witness_key_v1(local) }
}

pub open spec fn witness_storage_v1(a: usize, w: usize) -> StorageCapacitiesV1 {
    StorageCapacitiesV1 {
        writers: w, free: w, allocations: a, allocation_free: a,
        members: a, member_free: a, scratch: a,
    }
}

pub open spec fn witness_initial_v1(a: usize, w: usize) -> JournalStateV1 {
    initial_state_v1(7, a, w, witness_storage_v1(a, w))
}

pub open spec fn witness_reuse_result_v1() -> JournalStateV1 {
    let first = step_v1(witness_initial_v1(1, 1), ActionV1::Register(witness_key_v1(41))).0;
    let cleared = step_v1(first, ActionV1::Abort(witness_reference_v1(0, 41))).0;
    step_v1(cleared, ActionV1::Register(witness_key_v1(44))).0
}

pub proof fn capacity_one_reuse_witness_v1()
    ensures
        issuance_invariant_v1(witness_reuse_result_v1()),
        witness_reuse_result_v1().reserved_count == 1,
        witness_reuse_result_v1().registration_watermark == 44,
        witness_reuse_result_v1().successful_registrations.len() == 2,
        lookup_decision_v1(7, witness_reuse_result_v1().writers,
            witness_reference_v1(0, 41)) == Err(JournalErrorV1::InvalidReference),
        lookup_decision_v1(7, witness_reuse_result_v1().writers,
            witness_reference_v1(0, 44)) == Ok(witness_key_v1(44)),
{
    let initial = witness_initial_v1(1, 1);
    initialized_journal_invariant_v1(7, 1, 1, witness_storage_v1(1, 1));
    let key41 = witness_key_v1(41);
    let ref41 = witness_reference_v1(0, 41);
    assert(step_v1(initial, ActionV1::Register(key41)).1 == OutcomeV1::Registered(ref41));
    step_preserves_v1(initial, ActionV1::Register(key41));
    let first = step_v1(initial, ActionV1::Register(key41)).0;
    assert(step_v1(first, ActionV1::Abort(ref41)).1 == OutcomeV1::Aborted);
    step_preserves_v1(first, ActionV1::Abort(ref41));
    let cleared = step_v1(first, ActionV1::Abort(ref41)).0;
    assert(cleared.registration_watermark == 41);
    assert(cleared.successful_registrations == first.successful_registrations);
    let key44 = witness_key_v1(44);
    assert(step_v1(cleared, ActionV1::Register(key44)).1
        == OutcomeV1::Registered(witness_reference_v1(0, 44)));
    step_preserves_v1(cleared, ActionV1::Register(key44));
    aborted_reference_never_revives_v1(first, ref41, AbortPlanV1 { reserved_count: 0 },
        seq![ActionV1::Register(key44)], 1);
}

pub open spec fn witness_gaps_result_v1() -> JournalStateV1 {
    let first = step_v1(witness_initial_v1(1, 3), ActionV1::Register(witness_key_v1(41))).0;
    let second = step_v1(first, ActionV1::Register(witness_key_v1(44))).0;
    step_v1(second, ActionV1::Abort(witness_reference_v1(1, 44))).0
}

pub proof fn gaps_and_older_reference_witness_v1()
    ensures
        issuance_invariant_v1(witness_gaps_result_v1()),
        witness_gaps_result_v1().registration_watermark == 44,
        witness_gaps_result_v1().reserved_count == 1,
        lookup_decision_v1(7, witness_gaps_result_v1().writers, witness_reference_v1(0, 41))
            == Ok(witness_key_v1(41)),
        lookup_decision_v1(7, witness_gaps_result_v1().writers, witness_reference_v1(1, 42))
            == Err(JournalErrorV1::InvalidReference),
        step_v1(witness_gaps_result_v1(), ActionV1::Register(witness_key_v1(42))).1
            == OutcomeV1::Rejected(JournalErrorV1::WriterReplay),
        step_v1(witness_gaps_result_v1(), ActionV1::Register(witness_key_v1(42))).0
            == witness_gaps_result_v1(),
{
    let initial = witness_initial_v1(1, 3);
    initialized_journal_invariant_v1(7, 1, 3, witness_storage_v1(1, 3));
    let ref41 = witness_reference_v1(0, 41);
    assert(step_v1(initial, ActionV1::Register(ref41.key)).1 == OutcomeV1::Registered(ref41));
    step_preserves_v1(initial, ActionV1::Register(ref41.key));
    let first = step_v1(initial, ActionV1::Register(ref41.key)).0;
    let ref44 = witness_reference_v1(1, 44);
    assert(step_v1(first, ActionV1::Register(ref44.key)).1 == OutcomeV1::Registered(ref44));
    register_keeps_older_reserved_v1(first, ref44.key,
        RegisterPlanV1 { slot: 1, reserved_count: 2 }, ref41);
    step_preserves_v1(first, ActionV1::Register(ref44.key));
    let second = step_v1(first, ActionV1::Register(ref44.key)).0;
    assert(lookup_decision_v1(7, second.writers, witness_reference_v1(1, 42))
        == Err(JournalErrorV1::InvalidReference));
    assert(step_v1(second, ActionV1::Register(witness_key_v1(42))).1
        == OutcomeV1::Rejected(JournalErrorV1::WriterReplay));
    assert(step_v1(second, ActionV1::Abort(ref44)).1 == OutcomeV1::Aborted);
    abort_keeps_other_reserved_v1(second, ref44, AbortPlanV1 { reserved_count: 1 }, ref41);
    step_preserves_v1(second, ActionV1::Abort(ref44));
    assert(witness_gaps_result_v1().successful_registrations == second.successful_registrations);
}

pub open spec fn witness_last_id_result_v1() -> JournalStateV1 {
    let issued = step_v1(witness_initial_v1(1, 1),
        ActionV1::Register(witness_key_v1(0xffff_ffff_ffff_fffe))).0;
    step_v1(issued, ActionV1::Abort(witness_reference_v1(0, 0xffff_ffff_ffff_fffe))).0
}

pub proof fn last_valid_id_witness_v1()
    ensures
        issuance_invariant_v1(witness_last_id_result_v1()),
        witness_last_id_result_v1().registration_watermark == 0xffff_ffff_ffff_fffe,
        witness_last_id_result_v1().reserved_count == 0,
        witness_last_id_result_v1().successful_registrations.len() == 1,
        step_v1(witness_last_id_result_v1(), ActionV1::Register(witness_key_v1(u64::MAX))).1
            == OutcomeV1::Rejected(JournalErrorV1::InvalidWriterId),
        step_v1(witness_last_id_result_v1(), ActionV1::Register(witness_key_v1(u64::MAX))).0
            == witness_last_id_result_v1(),
        step_v1(witness_last_id_result_v1(), ActionV1::Register(witness_key_v1(44))).1
            == OutcomeV1::Rejected(JournalErrorV1::WriterReplay),
{
    let initial = witness_initial_v1(1, 1);
    initialized_journal_invariant_v1(7, 1, 1, witness_storage_v1(1, 1));
    let reference = witness_reference_v1(0, 0xffff_ffff_ffff_fffe);
    assert(step_v1(initial, ActionV1::Register(reference.key)).1 == OutcomeV1::Registered(reference));
    step_preserves_v1(initial, ActionV1::Register(reference.key));
    let issued = step_v1(initial, ActionV1::Register(reference.key)).0;
    assert(step_v1(issued, ActionV1::Register(witness_key_v1(u64::MAX))).1
        == OutcomeV1::Rejected(JournalErrorV1::InvalidWriterId));
    assert(step_v1(issued, ActionV1::Register(witness_key_v1(u64::MAX))).0 == issued);
    assert(step_v1(issued, ActionV1::Abort(reference)).1 == OutcomeV1::Aborted);
    step_preserves_v1(issued, ActionV1::Abort(reference));
}

// This is an inhabited mixed prestate, not a J1-only path to Pending or Unknown.
pub open spec fn witness_mixed_initial_v1() -> JournalStateV1 {
    let context_generation = 7u64;
    let allocation0 = AllocationKeyV1 { context_generation, local: 11 };
    let allocation1 = AllocationKeyV1 { context_generation, local: 12 };
    let device = DeviceKeyV1 { context_generation, local: 1 };
    let pending = witness_reference_v1(1, 44);
    let unknown = witness_reference_v1(2, 47);
    JournalStateV1 {
        context_generation, writer_capacity: 4, registration_watermark: 47, reserved_count: 1,
        writers: seq![
            Some(WriterEntryV1::Reserved(witness_key_v1(41))),
            Some(WriterEntryV1::Pending { key: pending.key, head: Some(0), count: 1 }),
            Some(WriterEntryV1::Unknown { key: unknown.key, head: Some(1), count: 1 }),
            None,
        ],
        free: seq![3usize],
        writers_storage: 4,
        free_storage: 4,
        successful_registrations: seq![witness_reference_v1(0, 41), pending, unknown],
        other: OtherJournalStateV1 {
            allocation_capacity: 2,
            allocations: seq![
                Some(AllocationEntryV1 { key: allocation0, device, byte_extent: 16,
                    attempt_epoch: 1, content_lineage: 0, pending_member: Some(0) }),
                Some(AllocationEntryV1 { key: allocation1, device, byte_extent: 16,
                    attempt_epoch: 1, content_lineage: 0, pending_member: Some(1) }),
            ],
            allocation_free: Seq::empty(),
            members: seq![
                Some(MemberEntryV1 { writer: pending,
                    allocation: AllocationReferenceV1 { slot: 0, key: allocation0 },
                    prior_lineage: 0, attempt_epoch: 1, next: None }),
                Some(MemberEntryV1 { writer: unknown,
                    allocation: AllocationReferenceV1 { slot: 1, key: allocation1 },
                    prior_lineage: 0, attempt_epoch: 1, next: None }),
            ],
            member_free: Seq::empty(),
            scratch: seq![None, None],
            allocations_storage: 2, allocation_free_storage: 2,
            members_storage: 2, member_free_storage: 2, scratch_storage: 2,
        },
    }
}

pub open spec fn witness_mixed_result_v1() -> JournalStateV1 {
    let first = step_v1(witness_mixed_initial_v1(), ActionV1::Register(witness_key_v1(50))).0;
    let cleared = step_v1(first, ActionV1::Abort(witness_reference_v1(0, 41))).0;
    step_v1(cleared, ActionV1::Register(witness_key_v1(53))).0
}

pub proof fn mixed_phase_frame_witness_v1()
    ensures
        issuance_invariant_v1(witness_mixed_initial_v1()),
        issuance_invariant_v1(witness_mixed_result_v1()),
        witness_mixed_result_v1().other == witness_mixed_initial_v1().other,
        witness_mixed_result_v1().writers[1] == witness_mixed_initial_v1().writers[1],
        witness_mixed_result_v1().writers[2] == witness_mixed_initial_v1().writers[2],
        witness_mixed_result_v1().reserved_count == 2,
        witness_mixed_result_v1().registration_watermark == 53,
        lookup_decision_v1(7, witness_mixed_result_v1().writers, witness_reference_v1(0, 41))
            == Err(JournalErrorV1::InvalidReference),
        lookup_decision_v1(7, witness_mixed_result_v1().writers, witness_reference_v1(3, 50))
            == Ok(witness_key_v1(50)),
{
    let initial = witness_mixed_initial_v1();
    reveal_with_fuel(occupied_prefix_v1, 5);
    reveal_with_fuel(reserved_prefix_v1, 5);
    assert(issuance_invariant_v1(initial));
    let key50 = witness_key_v1(50);
    assert(step_v1(initial, ActionV1::Register(key50)).1
        == OutcomeV1::Registered(witness_reference_v1(3, 50)));
    step_preserves_v1(initial, ActionV1::Register(key50));
    let first = step_v1(initial, ActionV1::Register(key50)).0;
    assert(first.reserved_count == 2);
    let old = witness_reference_v1(0, 41);
    assert(step_v1(first, ActionV1::Abort(old)).1 == OutcomeV1::Aborted);
    step_preserves_v1(first, ActionV1::Abort(old));
    let cleared = step_v1(first, ActionV1::Abort(old)).0;
    assert(cleared.reserved_count == 1);
    let key53 = witness_key_v1(53);
    assert(step_v1(cleared, ActionV1::Register(key53)).1
        == OutcomeV1::Registered(witness_reference_v1(0, 53)));
    register_keeps_older_reserved_v1(cleared, key53,
        RegisterPlanV1 { slot: 0, reserved_count: 2 }, witness_reference_v1(3, 50));
    step_preserves_v1(cleared, ActionV1::Register(key53));
}

pub open spec fn witness_with_count_v1(state: JournalStateV1, count: usize) -> JournalStateV1 {
    JournalStateV1 {
        context_generation: state.context_generation, writer_capacity: state.writer_capacity,
        registration_watermark: state.registration_watermark, reserved_count: count,
        writers: state.writers, free: state.free, writers_storage: state.writers_storage,
        free_storage: state.free_storage, other: state.other,
        successful_registrations: state.successful_registrations,
    }
}

pub proof fn malformed_count_guards_witness_v1()
    ensures
        representation_valid_v1(witness_with_count_v1(witness_initial_v1(1, 1), usize::MAX)),
        !issuance_invariant_v1(witness_with_count_v1(witness_initial_v1(1, 1), usize::MAX)),
        step_v1(witness_with_count_v1(witness_initial_v1(1, 1), usize::MAX),
            ActionV1::Register(witness_key_v1(41))).1
            == OutcomeV1::Rejected(JournalErrorV1::InvalidState),
        step_v1(witness_with_count_v1(witness_initial_v1(1, 1), usize::MAX),
            ActionV1::Register(witness_key_v1(41))).0
            == witness_with_count_v1(witness_initial_v1(1, 1), usize::MAX),
        step_v1(witness_with_count_v1(witness_initial_v1(1, 1), 1),
            ActionV1::Register(witness_key_v1(41))).1
            == OutcomeV1::Rejected(JournalErrorV1::InvalidState),
{
    let initial = witness_initial_v1(1, 1);
    empty_prefixes_v1(1, 1);
    let bound = witness_with_count_v1(initial, 1);
    assert(representation_valid_v1(bound));
    assert(!issuance_invariant_v1(bound));
    assert(step_v1(bound, ActionV1::Register(witness_key_v1(41))).1
        == OutcomeV1::Rejected(JournalErrorV1::InvalidState));
    assert(step_v1(bound, ActionV1::Register(witness_key_v1(41))).0 == bound);
    let reference = witness_reference_v1(0, 41);
    assert(step_v1(initial, ActionV1::Register(reference.key)).1 == OutcomeV1::Registered(reference));
    let issued = step_v1(initial, ActionV1::Register(reference.key)).0;
    let underflow = witness_with_count_v1(issued, 0);
    assert(representation_valid_v1(underflow));
    assert(underflow.writers == initial.writers.update(0, Some(WriterEntryV1::Reserved(reference.key))));
    prefix_update_v1(initial.writers, 0, Some(WriterEntryV1::Reserved(reference.key)), 1);
    assert(reserved_prefix_v1(underflow.writers, 1) == 1);
    assert(!issuance_invariant_v1(underflow));
    assert(lookup_decision_v1(7, underflow.writers, reference) == Ok(reference.key));
    assert(underflow.free.len() < underflow.writer_capacity);
    assert(underflow.free.len() < underflow.free_storage);
    assert(step_v1(underflow, ActionV1::Abort(reference)).1
        == OutcomeV1::Rejected(JournalErrorV1::InvalidState));
    assert(step_v1(underflow, ActionV1::Abort(reference)).0 == underflow);
}


pub proof fn constructor_guard_witnesses_v1()
    ensures
        constructor_admission_v1(0, 0, 0) == Some(JournalErrorV1::InvalidContextGeneration),
        constructor_admission_v1(u64::MAX, 0, 0) == Some(JournalErrorV1::InvalidContextGeneration),
        constructor_admission_v1(7, 0, 1) == Some(JournalErrorV1::InvalidCapacity),
        constructor_admission_v1(7, 1, 0) == Some(JournalErrorV1::InvalidCapacity),
        constructor_admission_v1(7, 1_048_577, 1) == Some(JournalErrorV1::InvalidCapacity),
        constructor_admission_v1(7, 1, 1_048_577) == Some(JournalErrorV1::InvalidCapacity),
        constructor_admission_v1(1, 1, 1).is_none(),
        constructor_admission_v1(0xffff_ffff_ffff_fffe, 1, 1).is_none(),
        constructor_admission_v1(7, 1_048_576, 1_048_576).is_none(),
{
}

pub proof fn identity_and_registration_guard_witnesses_v1()
    ensures
        register_decision_v1(7, 44, 0, 1, Seq::empty(), Seq::empty(),
            WriterKeyV1 { context_generation: 8, local: 0, kind: WriterKindV1::Synchronous })
            == Err(JournalErrorV1::ForeignContext),
        register_decision_v1(7, 44, 0, 1, Seq::empty(), Seq::empty(), witness_key_v1(0))
            == Err(JournalErrorV1::InvalidWriterId),
        register_decision_v1(7, 44, 0, 1, Seq::empty(), Seq::empty(), witness_key_v1(u64::MAX))
            == Err(JournalErrorV1::InvalidWriterId),
        register_decision_v1(7, 44, 0, 1, Seq::empty(), Seq::empty(), witness_key_v1(41))
            == Err(JournalErrorV1::WriterReplay),
        register_decision_v1(7, 44, 0, 1, Seq::empty(), Seq::empty(), witness_key_v1(45))
            == Err(JournalErrorV1::WriterCapacity),
        lookup_decision_v1(7, seq![Some(WriterEntryV1::Reserved(witness_key_v1(41)))],
            witness_reference_v1(0, 41)) == Ok(witness_key_v1(41)),
        lookup_decision_v1(7, seq![Some(WriterEntryV1::Reserved(witness_key_v1(41)))],
            witness_reference_v1(0, 42)) == Err(JournalErrorV1::InvalidReference),
        lookup_decision_v1(7, seq![Some(WriterEntryV1::Reserved(witness_key_v1(41)))],
            WriterReferenceV1 { slot: 0, key: WriterKeyV1 {
                context_generation: 7, local: 41, kind: WriterKindV1::Submission } })
            == Err(JournalErrorV1::InvalidReference),
        lookup_decision_v1(7, seq![Some(WriterEntryV1::Reserved(witness_key_v1(41)))],
            WriterReferenceV1 { slot: 0, key: WriterKeyV1 {
                context_generation: 8, local: 41, kind: WriterKindV1::Synchronous } })
            == Err(JournalErrorV1::InvalidReference),
        lookup_decision_v1(7, seq![Some(WriterEntryV1::Reserved(WriterKeyV1 {
            context_generation: 7, local: 41, kind: WriterKindV1::Submission }))],
            WriterReferenceV1 { slot: 0, key: WriterKeyV1 {
                context_generation: 7, local: 41, kind: WriterKindV1::Submission } })
            == Ok(WriterKeyV1 { context_generation: 7, local: 41, kind: WriterKindV1::Submission }),
        lookup_decision_v1(8, seq![Some(WriterEntryV1::Reserved(witness_key_v1(41)))],
            witness_reference_v1(0, 41)) == Err(JournalErrorV1::InvalidReference),
        lookup_decision_v1(7, seq![Some(WriterEntryV1::Reserved(witness_key_v1(41)))],
            witness_reference_v1(1, 41)) == Err(JournalErrorV1::InvalidReference),
        lookup_decision_v1(7, seq![Some(WriterEntryV1::Pending {
            key: witness_key_v1(41), head: None, count: 0 })], witness_reference_v1(0, 41))
            == Err(JournalErrorV1::InvalidReference),
        lookup_decision_v1(7, seq![Some(WriterEntryV1::Unknown {
            key: witness_key_v1(41), head: None, count: 0 })], witness_reference_v1(0, 41))
            == Err(JournalErrorV1::InvalidReference),
{
}

pub proof fn malformed_slot_and_headroom_witnesses_v1()
    ensures
        register_decision_v1(7, 0, 0, 1, seq![None], seq![1usize], witness_key_v1(41))
            == Err(JournalErrorV1::InvalidState),
        register_decision_v1(7, 41, 1, 2,
            seq![Some(WriterEntryV1::Reserved(witness_key_v1(41))), None],
            seq![0usize], witness_key_v1(44)) == Err(JournalErrorV1::InvalidState),
        abort_decision_v1(7, 1, 1, seq![Some(WriterEntryV1::Reserved(witness_key_v1(41)))],
            1, 2, witness_reference_v1(0, 41)) == Err(JournalErrorV1::InvalidState),
        abort_decision_v1(7, 1, 2, seq![Some(WriterEntryV1::Reserved(witness_key_v1(41))), None],
            1, 1, witness_reference_v1(0, 41)) == Err(JournalErrorV1::InvalidState),
        abort_decision_v1(7, 1, 2, seq![Some(WriterEntryV1::Reserved(witness_key_v1(41))), None],
            1, 2, witness_reference_v1(0, 41)) == Ok(AbortPlanV1 { reserved_count: 0 }),
{
    // The two headroom failures have valid lookup and nonzero count; neither masks the other.
    assert(lookup_decision_v1(7, seq![Some(WriterEntryV1::Reserved(witness_key_v1(41)))],
        witness_reference_v1(0, 41)) == Ok(witness_key_v1(41)));
    assert(lookup_decision_v1(7, seq![Some(WriterEntryV1::Reserved(witness_key_v1(41))), None],
        witness_reference_v1(0, 41)) == Ok(witness_key_v1(41)));
    assert(1usize < 2usize);
}

} // verus!
