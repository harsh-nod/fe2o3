// The complete actual journal/query closure is shared, not duplicated or modeled
// by supplied answers. Native owner field projections are source-bound separately.
include!("context_queued_query_execution_v1.rs");

mod native_observers {
    use super::*;
    include!("../../fe2o3-runtime/src/context/versions/producer_journal_observer_bodies.rs");

    verus! {

    // The journal has its actual complete declaration type. Unread versions,
    // Context, retained root and submission identity values remain opaque; they
    // are not absent or implicitly Copy. This is not an object-layout theorem.
    struct Versions<V> { journal: ContextQueuedWriterJournalV1, unread: V }

    enum QueryCall {
        ActiveLookup(ContextProducerReadReferenceV1),
        ActiveStatus(ContextProducerReadReferenceV1),
        QueuedLookup(ContextQueuedProducerReadReferenceV1),
        QueuedStatus(ContextQueuedProducerReadReferenceV1),
    }

    struct Observations<'a, C, V, R, I> {
        context: &'a C,
        versions: &'a Versions<V>,
        root: &'a R,
        id: I,
        consumer: ContextWriterKeyV1,
        launch: bool,
        calls: Ghost<Seq<QueryCall>>,
    }

    spec fn frame<C, V, R, I>(before: Observations<C, V, R, I>,
        after: Observations<C, V, R, I>) -> bool {
        after.context == before.context
            && after.versions == before.versions
            && after.root == before.root
            && after.id == before.id
            && after.consumer == before.consumer
            && after.launch == before.launch
    }

    impl<C, V, R, I> Observations<'_, C, V, R, I> {
        fn observe_active_lookup(&mut self, reference: ContextProducerReadReferenceV1)
            -> (result: Result<ContextProducerReadV1, ContextVersionJournalErrorV1>)
            ensures
                frame(*old(self), *final(self)),
                final(self).calls@ == old(self).calls@.push(QueryCall::ActiveLookup(reference)),
                result == queued_active_lookup_decision_v1(old(self).versions.journal, reference),
        {
            let result = producer_observe_active_lookup_body_v1!(self, reference);
            proof { self.calls@ = self.calls@.push(QueryCall::ActiveLookup(reference)); }
            result
        }

        fn observe_active_status(&mut self, reference: ContextProducerReadReferenceV1)
            -> (result: Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1>)
            ensures
                frame(*old(self), *final(self)),
                final(self).calls@ == old(self).calls@.push(QueryCall::ActiveStatus(reference)),
                result == queued_active_status_decision_v1(old(self).versions.journal, reference),
        {
            let result = producer_observe_active_status_body_v1!(self, reference);
            proof { self.calls@ = self.calls@.push(QueryCall::ActiveStatus(reference)); }
            result
        }

        fn observe_queued_lookup(&mut self, reference: ContextQueuedProducerReadReferenceV1)
            -> (result: Result<ContextQueuedProducerReadV1, ContextVersionJournalErrorV1>)
            ensures
                frame(*old(self), *final(self)),
                final(self).calls@ == old(self).calls@.push(QueryCall::QueuedLookup(reference)),
                result == reads::lookup_decision_v1(old(self).versions.journal, reference),
        {
            let result = producer_observe_queued_lookup_body_v1!(self, reference);
            proof { self.calls@ = self.calls@.push(QueryCall::QueuedLookup(reference)); }
            result
        }

        fn observe_queued_status(&mut self, reference: ContextQueuedProducerReadReferenceV1)
            -> (result: Result<ContextProducerReadStatusV1, ContextVersionJournalErrorV1>)
            ensures
                frame(*old(self), *final(self)),
                final(self).calls@ == old(self).calls@.push(QueryCall::QueuedStatus(reference)),
                result == reads::status_decision_v1(old(self).versions.journal, reference),
        {
            let result = producer_observe_queued_status_body_v1!(self, reference);
            proof { self.calls@ = self.calls@.push(QueryCall::QueuedStatus(reference)); }
            result
        }
    }

    }
}
