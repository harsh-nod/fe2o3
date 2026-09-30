// Rust and Verus compile the complete queued owner declaration tokens.
context_queued_writer_declarations_v1! {
/// One whole-allocation successor binding. None reserves an idle destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextQueuedWriteV1 {
    pub destination: ContextAllocationWriteV1,
    pub predecessor: Option<ContextWriterReferenceV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextQueuedWriterStatusV1 {
    Waiting,
    Ready,
    Blocked,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Prerequisite {
    Pending,
    Success { epoch: u64, lineage: u64 },
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Active,
    Queued,
    Unknown,
}

#[derive(Clone, Copy, Debug)]
struct Root {
    writer: ContextWriterReferenceV1,
    head: Option<usize>,
    count: usize,
    phase: Phase,
    read_head: Option<usize>,
    read_count: usize,
}

#[derive(Clone, Copy, Debug)]
struct Member {
    writer: ContextWriterReferenceV1,
    request: ContextQueuedWriteV1,
    prerequisite: Prerequisite,
    next_writer: Option<usize>,
    previous_allocation: Option<usize>,
    next_allocation: Option<usize>,
}

/// Bounded exclusive writer queues. No mutable or immutable inner projection is
/// exposed: even read-only availability queries must respect queued destinations.
/// Construction reserves O(A + W + R + M) metadata; M bounds all active/queued
/// destination records. Writer admission and activation are O(k); settlement is
/// O(k + r) for its k destinations and r attached queued reads. There is no heap
/// allocation after construction. Cancellation unlinks only its own roster
/// and marks immediate descendants failed without reparenting them.
///
/// Queued writers remain Reserved in the inner journal until activation. Their
/// registered identity is burned on cancellation, but no attempt epoch is burned
/// before activation. Successful logical activation is separate from physical
/// execution; an adapter must authenticate all dependencies and outcome premises.
/// This owner has no formal-refinement or device-ordering qualification yet.
///
/// ```compile_fail
/// use fe2o3_runtime_model::{ContextQueuedWriterJournalV1, ContextProducerReadJournalV1};
/// let mut owner = ContextQueuedWriterJournalV1::new(1, 4, 4, 4, 16).unwrap();
/// let bypass: &mut ContextProducerReadJournalV1 = &mut *owner;
/// ```
/// ```compile_fail
/// use fe2o3_runtime_model::{ContextQueuedWriterJournalV1, ContextProducerReadJournalV1};
/// let owner = ContextQueuedWriterJournalV1::new(1, 4, 4, 4, 16).unwrap();
/// let bypass: &ContextProducerReadJournalV1 = &*owner;
/// ```
#[derive(Debug)]
pub struct ContextQueuedWriterJournalV1 {
    inner: ContextProducerReadJournalV1,
    roots: Vec<Option<Root>>,
    members: Vec<Option<Member>>,
    free: Vec<usize>,
    heads: Vec<Option<usize>>,
    tails: Vec<Option<usize>>,
    queued_counts: Vec<usize>,
    scratch: Vec<ContextAllocationWriteV1>,
    queued_reads: Vec<Option<reads::Reservation>>,
    free_reads: Vec<usize>,
    read_counts: Vec<usize>,
    next_read_incarnation: u64,
    read_producer_scratch: Vec<ContextWriterReferenceV1>,
    disposal_terminal: bool,
    #[cfg(test)]
    disposal_fault: Option<(usize, bool)>,
}

}
