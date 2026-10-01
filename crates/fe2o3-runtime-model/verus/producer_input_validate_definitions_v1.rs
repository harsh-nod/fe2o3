// Conditional refinement of the actual per-input production body. Every helper
// answer below describes one reached call, not a frozen journal or account.
// The five native forwarding methods, lazy credit method and complete compared
// value schemas are source-calibrated separately. Their native semantics are
// not established by this theorem. Local immutable storage is framed; shared
// Arc interiors, locking, allocation, unwinding and native freshness are not.
// Membership scans use the actual bounded slice bodies below. Pinned slice
// indexing and structural comparison contracts remain library/compiler trust.
include!("producer_input_runtime_declarations_v1.rs");
include!("producer_input_journal_comparison_declarations_v1.rs");
include!("producer_input_outcome_spec_v1.rs");

verus! {
struct Observations<'a, C, L, P, D, R> {
    owner: &'a Owner<C, L, P, D, R>, returns: Returns, calls: Ghost<Seq<Call>>,
}

impl<'a, C, L, P, D, R> Observations<'a, C, L, P, D, R> {
    fn observe_active_lookup(&mut self, reference: ContextProducerReadReferenceV1)
        -> (out: Result<ContextProducerReadV1, ContextVersionJournalErrorV1>)
        ensures out == old(self).returns.active_lookup,
            final(self).owner == old(self).owner, final(self).returns == old(self).returns,
            final(self).calls@ == old(self).calls@.push(Call::ActiveLookup(reference)),
    { proof { self.calls@ = self.calls@.push(Call::ActiveLookup(reference)); } self.returns.active_lookup }

    fn observe_active_status(&mut self, reference: ContextProducerReadReferenceV1) -> (out: StatusResult)
        ensures out == old(self).returns.active_status,
            final(self).owner == old(self).owner, final(self).returns == old(self).returns,
            final(self).calls@ == old(self).calls@.push(Call::ActiveStatus(reference)),
    { proof { self.calls@ = self.calls@.push(Call::ActiveStatus(reference)); } self.returns.active_status }

    fn observe_queued_lookup(&mut self, reference: ContextQueuedProducerReadReferenceV1)
        -> (out: Result<ContextQueuedProducerReadV1, ContextVersionJournalErrorV1>)
        ensures out == old(self).returns.queued_lookup,
            final(self).owner == old(self).owner, final(self).returns == old(self).returns,
            final(self).calls@ == old(self).calls@.push(Call::QueuedLookup(reference)),
    { proof { self.calls@ = self.calls@.push(Call::QueuedLookup(reference)); } self.returns.queued_lookup }

    fn observe_queued_status(&mut self, reference: ContextQueuedProducerReadReferenceV1) -> (out: StatusResult)
        ensures out == old(self).returns.queued_status,
            final(self).owner == old(self).owner, final(self).returns == old(self).returns,
            final(self).calls@ == old(self).calls@.push(Call::QueuedStatus(reference)),
    { proof { self.calls@ = self.calls@.push(Call::QueuedStatus(reference)); } self.returns.queued_status }

    fn observe_expected_credit(&mut self, allocation: RuntimeAllocationIdV1, device: RuntimeDeviceIdV1, bytes: u64) -> (out: bool)
        ensures out == old(self).returns.credit,
            final(self).owner == old(self).owner, final(self).returns == old(self).returns,
            final(self).calls@ == old(self).calls@.push(Call::Credit(allocation, device, bytes)),
    { proof { self.calls@ = self.calls@.push(Call::Credit(allocation, device, bytes)); } self.returns.credit }

    fn observe_live(&mut self, allocation: RuntimeAllocationIdV1, record: &AllocationRecordV1)
        -> (out: Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1>)
        ensures out == old(self).returns.live,
            final(self).owner == old(self).owner, final(self).returns == old(self).returns,
            final(self).calls@ == old(self).calls@.push(Call::Live(allocation, *record)),
    { proof { self.calls@ = self.calls@.push(Call::Live(allocation, *record)); } self.returns.live }

    fn validate(&mut self, id: RuntimeSubmissionIdV1, consumer: ContextWriterKeyV1,
        launch: bool, index: usize, active: &mut usize, queued: &mut usize) -> (out: StatusResult)
        requires index < old(self).owner.root.inputs@.len(),
            vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),
            vstd::std_specs::hash::obeys_key_model::<RuntimeAllocationIdV1>(),
        ensures final(self).owner == old(self).owner,
            final(self).returns == old(self).returns,
            out == outcome(old(self).owner, id, consumer, launch, index, *old(active), *old(queued), old(self).returns).result,
            *final(active) == outcome(old(self).owner, id, consumer, launch, index, *old(active), *old(queued), old(self).returns).active,
            *final(queued) == outcome(old(self).owner, id, consumer, launch, index, *old(active), *old(queued), old(self).returns).queued,
            final(self).calls@ == old(self).calls@ + outcome(old(self).owner, id, consumer, launch, index, *old(active), *old(queued), old(self).returns).calls,
    {
        broadcast use trace_push_after_prefix;
        let owner = self.owner;
        let context = &owner.context;
        let root = &owner.root;
        proof {
            if index > 0 {
                allocation_order_correspondence(
                    root.inputs@[index as int - 1].source.region.allocation,
                    root.inputs@[index as int].source.region.allocation,
                );
            }
        }
        producer_input_validate_body!(verus_exec_expr, context, root, id,
            consumer, launch, index, active, queued, self)
    }
}
}
