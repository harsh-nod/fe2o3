// Concrete composition of the actual journal, validator and fold bodies.
// Only reached credit results are supplied by the environment.
// Journal answers below are ghost expressions derived from the actual borrowed
// journal and incoming reference cursors, never executable replacement answers.
include!("context_queued_query_execution_v1.rs");

mod concrete_composition {
    use super::*;
    use vstd::prelude::verus as enrollment_declarations_v1;
    include!("../src/context_version_journal/enrollment_declarations.rs");
    include!("producer_input_runtime_declarations_v1.rs");
    include!("producer_input_outcome_spec_v1.rs");
    include!("context_live_validation_definitions_v1.rs");
    include!("../../fe2o3-runtime/src/context/versions/producer_journal_observer_bodies.rs");
    mod fold { include!("producer_input_fold_spec_v1.rs"); }

    verus! {
    // Independent per reached input. Credit-lock and Arc-identity refinement
    // remain outside this actual journal and live-allocation composition.
    #[derive(Clone, Copy)]
    struct ExternalReturns {
        credit: bool,
    }

    struct Source {
        journal: ContextQueuedWriterJournalV1,
        phases: Seq<Option<AllocationPhaseV1>>,
        external: Seq<ExternalReturns>,
    }

    spec fn source_len(source: Source) -> nat { source.external.len() }

    #[verifier::opaque]
    spec fn journal_answers<R>(journal: ContextQueuedWriterJournalV1,
        phases: Seq<Option<AllocationPhaseV1>>, root: &Root<R>,
        external: ExternalReturns, index: usize, active: usize, queued: usize) -> Returns {
        let invalid = ContextVersionJournalErrorV1::InvalidReference;
        Returns {
            active_lookup: if active < root.references@.len() {
                queued_active_lookup_decision_v1(journal, root.references@[active as int])
            } else { Err(invalid) },
            active_status: if active < root.references@.len() {
                queued_active_status_decision_v1(journal, root.references@[active as int])
            } else { Err(invalid) },
            queued_lookup: if queued < root.queued_references@.len() {
                reads::lookup_decision_v1(journal, root.queued_references@[queued as int])
            } else { Err(invalid) },
            queued_status: if queued < root.queued_references@.len() {
                reads::status_decision_v1(journal, root.queued_references@[queued as int])
            } else { Err(invalid) },
            credit: external.credit,
            live: if index < root.inputs@.len() {
                live_allocation_decision(journal, phases,
                    root.inputs@[index as int].source.region.allocation,
                    root.inputs@[index as int].source.record)
            } else { Err(invalid) },
        }
    }

    proof fn journal_answers_credit<R>(journal: ContextQueuedWriterJournalV1,
        phases: Seq<Option<AllocationPhaseV1>>, root: &Root<R>,
        external: ExternalReturns, index: usize, active: usize, queued: usize)
        ensures journal_answers(journal, phases, root, external, index, active, queued).credit == external.credit,
    {
        reveal(journal_answers);
    }

    spec fn source_answers<C, L, P, D, R>(owner: &Owner<C, L, P, D, R>,
        source: Source, index: usize, active: usize, queued: usize) -> Returns
        recommends index < source.external.len(),
    {
        journal_answers(source.journal, source.phases, &owner.root,
            source.external[index as int], index, active, queued)
    }

    struct Observations<'a, C, L, P, D, R, V> {
        context: &'a Context<C, L, P, D>,
        versions: &'a Versions<V>,
        root: &'a Root<R>,
        id: RuntimeSubmissionIdV1,
        consumer: ContextWriterKeyV1,
        launch: bool,
        external: ExternalReturns,
        calls: Ghost<Seq<Call>>,
    }

    impl<'a, C, L, P, D, R, V> Observations<'a, C, L, P, D, R, V> {
        spec fn owner_view(&self) -> Owner<C, L, P, D, R> {
            Owner { context: *self.context, root: *self.root }
        }

        spec fn same_binding(&self, before: &Self) -> bool {
            self.context == before.context && self.versions == before.versions
                && self.root == before.root && self.id == before.id
                && self.consumer == before.consumer && self.launch == before.launch
                && self.external == before.external
        }

        spec fn answers(&self, index: usize, active: usize, queued: usize) -> Returns {
            journal_answers(self.versions.journal, self.versions.phases@,
                self.root, self.external, index, active, queued)
        }

        fn observe_active_lookup(&mut self, reference: ContextProducerReadReferenceV1)
            -> (result: Result<ContextProducerReadV1, ContextVersionJournalErrorV1>)
            ensures final(self).same_binding(old(self)),
                final(self).calls@ == old(self).calls@.push(Call::ActiveLookup(reference)),
                result == queued_active_lookup_decision_v1(old(self).versions.journal, reference),
        {
            let result = producer_observe_active_lookup_body_v1!(self, reference);
            proof { self.calls@ = self.calls@.push(Call::ActiveLookup(reference)); }
            result
        }

        fn observe_active_status(&mut self, reference: ContextProducerReadReferenceV1) -> (result: StatusResult)
            ensures final(self).same_binding(old(self)),
                final(self).calls@ == old(self).calls@.push(Call::ActiveStatus(reference)),
                result == queued_active_status_decision_v1(old(self).versions.journal, reference),
        {
            let result = producer_observe_active_status_body_v1!(self, reference);
            proof { self.calls@ = self.calls@.push(Call::ActiveStatus(reference)); }
            result
        }

        fn observe_queued_lookup(&mut self, reference: ContextQueuedProducerReadReferenceV1)
            -> (result: Result<ContextQueuedProducerReadV1, ContextVersionJournalErrorV1>)
            ensures final(self).same_binding(old(self)),
                final(self).calls@ == old(self).calls@.push(Call::QueuedLookup(reference)),
                result == reads::lookup_decision_v1(old(self).versions.journal, reference),
        {
            let result = producer_observe_queued_lookup_body_v1!(self, reference);
            proof { self.calls@ = self.calls@.push(Call::QueuedLookup(reference)); }
            result
        }

        fn observe_queued_status(&mut self, reference: ContextQueuedProducerReadReferenceV1) -> (result: StatusResult)
            ensures final(self).same_binding(old(self)),
                final(self).calls@ == old(self).calls@.push(Call::QueuedStatus(reference)),
                result == reads::status_decision_v1(old(self).versions.journal, reference),
        {
            let result = producer_observe_queued_status_body_v1!(self, reference);
            proof { self.calls@ = self.calls@.push(Call::QueuedStatus(reference)); }
            result
        }

        fn observe_expected_credit(&mut self, allocation: RuntimeAllocationIdV1,
            device: RuntimeDeviceIdV1, bytes: u64) -> (out: bool)
            ensures out == old(self).external.credit, final(self).same_binding(old(self)),
                final(self).calls@ == old(self).calls@.push(Call::Credit(allocation, device, bytes)),
        {
            proof { self.calls@ = self.calls@.push(Call::Credit(allocation, device, bytes)); }
            self.external.credit
        }

        fn observe_live(&mut self, allocation: RuntimeAllocationIdV1, record: &AllocationRecordV1)
            -> (out: Result<ContextAllocationReferenceV1, ContextVersionJournalErrorV1>)
            ensures out == live_allocation_decision(old(self).versions.journal,
                old(self).versions.phases@, allocation, *record), final(self).same_binding(old(self)),
                final(self).calls@ == old(self).calls@.push(Call::Live(allocation, *record)),
        {
            let result = self.versions.validate_live(allocation, record);
            proof { self.calls@ = self.calls@.push(Call::Live(allocation, *record)); }
            result
        }

        #[verifier::spinoff_prover]
        fn validate(&mut self, index: usize, active: &mut usize, queued: &mut usize) -> (out: StatusResult)
            requires index < old(self).root.inputs@.len(),
                vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),
                vstd::std_specs::hash::obeys_key_model::<RuntimeAllocationIdV1>(),
            ensures final(self).same_binding(old(self)),
                out == outcome(&old(self).owner_view(), old(self).id, old(self).consumer,
                    old(self).launch, index, *old(active), *old(queued),
                    old(self).answers(index, *old(active), *old(queued))).result,
                *final(active) == outcome(&old(self).owner_view(), old(self).id, old(self).consumer,
                    old(self).launch, index, *old(active), *old(queued),
                    old(self).answers(index, *old(active), *old(queued))).active,
                *final(queued) == outcome(&old(self).owner_view(), old(self).id, old(self).consumer,
                    old(self).launch, index, *old(active), *old(queued),
                    old(self).answers(index, *old(active), *old(queued))).queued,
                final(self).calls@ == old(self).calls@ + outcome(&old(self).owner_view(),
                    old(self).id, old(self).consumer, old(self).launch, index,
                    *old(active), *old(queued), old(self).answers(index, *old(active), *old(queued))).calls,
        {
            broadcast use trace_push_after_prefix;
            proof { reveal(journal_answers); }
            let context = self.context;
            let root = self.root;
            let id = self.id;
            let consumer = self.consumer;
            let launch = self.launch;
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

    include!("producer_input_composition_logic_v1.rs");

    verus! {
    struct Composition<'a, C, L, P, D, R, V> {
        context: &'a Context<C, L, P, D>,
        versions: &'a Versions<V>,
        root: &'a Root<R>,
        id: RuntimeSubmissionIdV1,
        consumer: ContextWriterKeyV1,
        launch: bool,
        returns: &'a [ExternalReturns],
        consumed: Ghost<nat>,
        receipts: Ghost<Seq<Receipt>>,
        calls: Ghost<Seq<Call>>,
    }

    impl<'a, C, L, P, D, R, V> Composition<'a, C, L, P, D, R, V> {
        spec fn owner_view(&self) -> Owner<C, L, P, D, R> {
            Owner { context: *self.context, root: *self.root }
        }

        spec fn source(&self) -> Source {
            Source { journal: self.versions.journal, phases: self.versions.phases@, external: self.returns@ }
        }

        spec fn same_binding(&self, before: &Self) -> bool {
            self.context == before.context && self.versions == before.versions
                && self.root == before.root && self.id == before.id
                && self.consumer == before.consumer && self.launch == before.launch
                && self.returns == before.returns
        }

        closed spec fn original(&self) -> Seq<Receipt> {
            prefix(&self.owner_view(), self.id, self.consumer, self.launch, self.source(),
                self.root.inputs@.len() as int).receipts
        }

        closed spec fn wf(&self) -> bool {
            &&& self.returns@.len() == self.root.inputs@.len() <= usize::MAX
            &&& self.consumed@ <= self.root.inputs@.len()
            &&& self.receipts@ == prefix(&self.owner_view(), self.id, self.consumer, self.launch,
                self.source(), self.consumed@ as int).receipts
            &&& self.calls@ == prefix(&self.owner_view(), self.id, self.consumer, self.launch,
                self.source(), self.consumed@ as int).calls
        }

        fn new(context: &'a Context<C, L, P, D>, versions: &'a Versions<V>, root: &'a Root<R>,
            id: RuntimeSubmissionIdV1, consumer: ContextWriterKeyV1, launch: bool,
            answers: &'a [ExternalReturns]) -> (out: Self)
            requires answers@.len() == root.inputs@.len(),
            ensures out.wf(), out.context == context, out.versions == versions, out.root == root,
                out.id == id, out.consumer == consumer, out.launch == launch, out.returns == answers,
                out.consumed@ == 0, out.receipts@ == Seq::<Receipt>::empty(), out.calls@ == Seq::<Call>::empty(),
        {
            proof { vstd::std_specs::vec::axiom_spec_len(&root.inputs); }
            let ghost initial_consumed = 0nat;
            Self { context, versions, root, id, consumer, launch, returns: answers,
                consumed: Ghost(initial_consumed), receipts: Ghost(Seq::empty()), calls: Ghost(Seq::empty()) }
        }

        fn input_count(&self) -> (out: usize)
            requires self.wf(),
            ensures out == self.original().len(), out == self.root.inputs@.len(),
        {
            proof { prefix_facts(&self.owner_view(), self.id, self.consumer, self.launch,
                self.source(), self.root.inputs@.len() as int); }
            self.root.inputs.len()
        }

        fn active_count(&self) -> (out: usize)
            ensures out == self.root.references@.len(),
        { self.root.references.len() }

        fn queued_count(&self) -> (out: usize)
            ensures out == self.root.queued_references@.len(),
        { self.root.queued_references.len() }

        #[verifier::spinoff_prover]
        fn validate(&mut self, index: usize, active: &mut usize, queued: &mut usize) -> (out: StatusResult)
            requires old(self).wf(), index == old(self).consumed@,
                index < old(self).root.inputs@.len(),
                *old(active) == fold::active_before(old(self).original(), index as int),
                *old(queued) == fold::queued_before(old(self).original(), index as int),
                vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),
                vstd::std_specs::hash::obeys_key_model::<RuntimeAllocationIdV1>(),
            ensures final(self).wf(), final(self).same_binding(old(self)),
                final(self).original() == old(self).original(), final(self).consumed@ == index + 1,
                final(self).receipts@ == final(self).original().take(index as int + 1),
                *final(active) == fold::active_before(final(self).original(), index as int + 1),
                *final(queued) == fold::queued_before(final(self).original(), index as int + 1),
                fold_status_result(out) == final(self).original()[index as int].result,
                final(self).receipts@ == old(self).receipts@.push(receipt(
                    old(self).root.inputs@[index as int].request, *old(active), *old(queued),
                    *final(active), *final(queued), out,
                    outcome(&old(self).owner_view(), old(self).id, old(self).consumer, old(self).launch,
                        index, *old(active), *old(queued),
                        source_answers(&old(self).owner_view(), old(self).source(), index,
                            *old(active), *old(queued))).calls,
                    old(self).returns@[index as int].credit)),
        {
            let ghost active_before = *active;
            let ghost queued_before = *queued;
            proof {
                prefix_facts(&self.owner_view(), self.id, self.consumer, self.launch,
                    self.source(), self.root.inputs@.len() as int);
                prefix_facts(&self.owner_view(), self.id, self.consumer, self.launch, self.source(), index as int);
                prefix_extends(&self.owner_view(), self.id, self.consumer, self.launch, self.source(),
                    index as int, self.root.inputs@.len() as int);
                prefix_counts(self.original(), index as int);
            }
            let external = self.returns[index];
            let mut validation = Observations { context: self.context, versions: self.versions,
                root: self.root, id: self.id, consumer: self.consumer, launch: self.launch,
                external, calls: Ghost(Seq::empty()) };
            // Record the reached receipt even when validation returns an error.
            let result = validation.validate(index, active, queued);
            proof {
                journal_answers_credit(self.versions.journal, self.versions.phases@, self.root, external,
                    index, active_before, queued_before);
                self.receipts@ = self.receipts@.push(receipt(self.root.inputs@[index as int].request,
                    active_before, queued_before, *active, *queued, result, validation.calls@, external.credit));
                self.calls@ = self.calls@ + validation.calls@;
                self.consumed@ = (index + 1) as nat;
                prefix_facts(&self.owner_view(), self.id, self.consumer, self.launch, self.source(), index as int + 1);
                prefix_extends(&self.owner_view(), self.id, self.consumer, self.launch, self.source(),
                    index as int + 1, self.root.inputs@.len() as int);
                prefix_counts(self.original(), index as int + 1);
                reveal_with_fuel(prefix, 2);
                assert(self.receipts@[index as int] == self.original()[index as int]);
            }
            result
        }

        #[verifier::spinoff_prover]
        fn reconcile(&mut self) -> (out: StatusResult)
            requires old(self).wf(), old(self).consumed@ == 0,
                vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),
                vstd::std_specs::hash::obeys_key_model::<RuntimeAllocationIdV1>(),
            ensures final(self).wf(), final(self).same_binding(old(self)),
                final(self).original() == old(self).original(),
                final(self).consumed@ == fold::reached(old(self).original(), 0),
                final(self).receipts@ == old(self).original().take(final(self).consumed@ as int),
                final(self).calls@ == prefix(&old(self).owner_view(), old(self).id, old(self).consumer,
                    old(self).launch, old(self).source(), final(self).consumed@ as int).calls,
                fold_status_result(out) == fold::fold_result(old(self).original(), 0,
                    fold::ContextProducerReadStatusV1::Success,
                    old(self).root.references@.len() as usize,
                    old(self).root.queued_references@.len() as usize,
                    ContextVersionJournalErrorV1::InvalidReference),
        {
            let ghost before = *self;
            let invalid_reference = ContextVersionJournalErrorV1::InvalidReference;
            proof {
                vstd::std_specs::vec::axiom_spec_len(&self.root.references);
                vstd::std_specs::vec::axiom_spec_len(&self.root.queued_references);
                prefix_facts(&self.owner_view(), self.id, self.consumer, self.launch,
                    self.source(), self.root.inputs@.len() as int);
                prefix_extends(&self.owner_view(), self.id, self.consumer, self.launch,
                    self.source(), 0, self.root.inputs@.len() as int);
                assert(self.receipts@ == Seq::<Receipt>::empty());
                assert(before.original().take(0) =~= Seq::<Receipt>::empty());
            }
            producer_input_fold_body!(verus_exec_expr, self, invalid_reference,
                (index, aggregate, active_index, queued_index, input_count),
                [invariant
                    before == *old(self), self.wf(), self.same_binding(&before),
                    invalid_reference == ContextVersionJournalErrorV1::InvalidReference,
                    vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),
                    vstd::std_specs::hash::obeys_key_model::<RuntimeAllocationIdV1>(),
                    self.root.references@.len() <= usize::MAX,
                    self.root.queued_references@.len() <= usize::MAX,
                    self.original() == before.original(),
                    input_count == before.original().len(), index <= input_count,
                    self.consumed@ == index,
                    self.receipts@ == before.original().take(index as int),
                    active_index == fold::active_before(before.original(), index as int),
                    queued_index == fold::queued_before(before.original(), index as int),
                    fold::fold_result(before.original(), 0, fold::ContextProducerReadStatusV1::Success,
                        before.root.references@.len() as usize,
                        before.root.queued_references@.len() as usize, invalid_reference)
                        == fold::fold_result(before.original(), index as int, fold_status(aggregate),
                            before.root.references@.len() as usize,
                            before.root.queued_references@.len() as usize, invalid_reference),
                    fold::reached(before.original(), 0) == fold::reached(before.original(), index as int),
                 decreases input_count - index],
                [proof {
                    reveal_with_fuel(fold::fold_result, 2);
                    reveal_with_fuel(fold::reached, 2);
                    fold_step_error(before.original(), index as int, fold_status(aggregate),
                        before.root.references@.len() as usize,
                        before.root.queued_references@.len() as usize, invalid_reference);
                }],
                [])
        }
    }
    }
}
