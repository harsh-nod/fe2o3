// Conditional composition of the actual fold and actual per-input validator.
// The environment supplies independent possible helper returns for each input;
// unused returns are not observations. Only the reached prefix becomes a ghost
// receipt/call trace. Native journal, credit-account and interior-mutability
// refinement, allocation, unwinding, Rust compilation and ISA remain outside.
#![allow(unused_macros)]
include!("producer_input_validate_definitions_v1.rs");
mod producer_input_fold_spec_v1;
use producer_input_fold_spec_v1 as fold;

verus! {
type Source = Seq<Returns>;

spec fn source_len(source: Source) -> nat { source.len() }

spec fn source_answers<C, L, P, D, R>(_owner: &Owner<C, L, P, D, R>,
    source: Source, index: usize, _active: usize, _queued: usize) -> Returns
    recommends index < source.len(),
{ source[index as int] }
}
include!("producer_input_composition_logic_v1.rs");

verus! {
struct Composition<'a, C, L, P, D, R> {
    owner: &'a Owner<C, L, P, D, R>, id: RuntimeSubmissionIdV1,
    consumer: ContextWriterKeyV1, launch: bool, returns: &'a [Returns],
    consumed: Ghost<nat>, receipts: Ghost<Seq<Receipt>>, calls: Ghost<Seq<Call>>,
}

impl<'a, C, L, P, D, R> Composition<'a, C, L, P, D, R> {
    closed spec fn original(&self) -> Seq<Receipt> {
        prefix(self.owner, self.id, self.consumer, self.launch, self.returns@,
            self.owner.root.inputs@.len() as int).receipts
    }

    closed spec fn wf(&self) -> bool {
        &&& self.returns@.len() == self.owner.root.inputs@.len() <= usize::MAX
        &&& self.consumed@ <= self.owner.root.inputs@.len()
        &&& self.receipts@ == prefix(self.owner, self.id, self.consumer, self.launch,
            self.returns@, self.consumed@ as int).receipts
        &&& self.calls@ == prefix(self.owner, self.id, self.consumer, self.launch,
            self.returns@, self.consumed@ as int).calls
    }

    fn new(owner: &'a Owner<C, L, P, D, R>, id: RuntimeSubmissionIdV1,
        consumer: ContextWriterKeyV1, launch: bool, answers: &'a [Returns]) -> (out: Self)
        requires answers@.len() == owner.root.inputs@.len(),
        ensures out.wf(), out.owner == owner, out.id == id, out.consumer == consumer,
            out.launch == launch, out.returns == answers, out.consumed@ == 0,
            out.receipts@ == Seq::<Receipt>::empty(), out.calls@ == Seq::<Call>::empty(),
    {
        proof { vstd::std_specs::vec::axiom_spec_len(&owner.root.inputs); }
        let ghost initial_consumed = 0nat;
        Self { owner, id, consumer, launch, returns: answers,
            consumed: Ghost(initial_consumed), receipts: Ghost(Seq::empty()), calls: Ghost(Seq::empty()) }
    }

    fn input_count(&self) -> (out: usize)
        requires self.wf(),
        ensures out == self.original().len(), out == self.owner.root.inputs@.len(),
    {
        proof { prefix_facts(self.owner, self.id, self.consumer, self.launch,
            self.returns@, self.owner.root.inputs@.len() as int); }
        self.owner.root.inputs.len()
    }

    fn active_count(&self) -> (out: usize)
        ensures out == self.owner.root.references@.len(),
    { self.owner.root.references.len() }

    fn queued_count(&self) -> (out: usize)
        ensures out == self.owner.root.queued_references@.len(),
    { self.owner.root.queued_references.len() }

    fn validate(&mut self, index: usize, active: &mut usize, queued: &mut usize) -> (out: StatusResult)
        requires old(self).wf(), index == old(self).consumed@,
            index < old(self).owner.root.inputs@.len(),
            *old(active) == fold::active_before(old(self).original(), index as int),
            *old(queued) == fold::queued_before(old(self).original(), index as int),
            vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),
            vstd::std_specs::hash::obeys_key_model::<RuntimeAllocationIdV1>(),
        ensures final(self).wf(), final(self).owner == old(self).owner,
            final(self).id == old(self).id, final(self).consumer == old(self).consumer,
            final(self).launch == old(self).launch, final(self).returns == old(self).returns,
            final(self).original() == old(self).original(), final(self).consumed@ == index + 1,
            final(self).receipts@ == final(self).original().take(index as int + 1),
            *final(active) == fold::active_before(final(self).original(), index as int + 1),
            *final(queued) == fold::queued_before(final(self).original(), index as int + 1),
            fold_status_result(out) == final(self).original()[index as int].result,
            final(self).receipts@ == old(self).receipts@.push(receipt(
                old(self).owner.root.inputs@[index as int].request, *old(active), *old(queued),
                *final(active), *final(queued), out,
                outcome(old(self).owner, old(self).id, old(self).consumer, old(self).launch,
                    index, *old(active), *old(queued), old(self).returns@[index as int]).calls,
                old(self).returns@[index as int].credit)),
    {
        let ghost active_before = *active;
        let ghost queued_before = *queued;
        proof {
            prefix_facts(self.owner, self.id, self.consumer, self.launch,
                self.returns@, self.owner.root.inputs@.len() as int);
            prefix_facts(self.owner, self.id, self.consumer, self.launch, self.returns@, index as int);
            prefix_extends(self.owner, self.id, self.consumer, self.launch, self.returns@,
                index as int, self.owner.root.inputs@.len() as int);
            prefix_counts(self.original(), index as int);
        }
        let answers = self.returns[index];
        let mut validation = Observations { owner: self.owner, returns: answers, calls: Ghost(Seq::empty()) };
        let result = validation.validate(self.id, self.consumer, self.launch, index, active, queued);
        proof {
            self.receipts@ = self.receipts@.push(receipt(self.owner.root.inputs@[index as int].request,
                active_before, queued_before, *active, *queued, result, validation.calls@, answers.credit));
            self.calls@ = self.calls@ + validation.calls@;
            self.consumed@ = (index + 1) as nat;
            prefix_facts(self.owner, self.id, self.consumer, self.launch, self.returns@, index as int + 1);
            prefix_extends(self.owner, self.id, self.consumer, self.launch, self.returns@,
                index as int + 1, self.owner.root.inputs@.len() as int);
            prefix_counts(self.original(), index as int + 1);
            reveal_with_fuel(prefix, 2);
            assert(self.receipts@[index as int] == self.original()[index as int]);
        }
        result
    }

    fn reconcile(&mut self) -> (out: StatusResult)
        requires old(self).wf(), old(self).consumed@ == 0,
            vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),
            vstd::std_specs::hash::obeys_key_model::<RuntimeAllocationIdV1>(),
        ensures final(self).wf(), final(self).owner == old(self).owner,
            final(self).id == old(self).id, final(self).consumer == old(self).consumer,
            final(self).launch == old(self).launch, final(self).returns == old(self).returns,
            final(self).original() == old(self).original(),
            final(self).consumed@ == fold::reached(old(self).original(), 0),
            final(self).receipts@ == old(self).original().take(final(self).consumed@ as int),
            final(self).calls@ == prefix(old(self).owner, old(self).id, old(self).consumer,
                old(self).launch, old(self).returns@, final(self).consumed@ as int).calls,
            fold_status_result(out) == fold::fold_result(old(self).original(), 0,
                fold::ContextProducerReadStatusV1::Success,
                old(self).owner.root.references@.len() as usize,
                old(self).owner.root.queued_references@.len() as usize,
                ContextVersionJournalErrorV1::InvalidReference),
    {
        let ghost before = *self;
        let invalid_reference = ContextVersionJournalErrorV1::InvalidReference;
        proof {
            vstd::std_specs::vec::axiom_spec_len(&self.owner.root.references);
            vstd::std_specs::vec::axiom_spec_len(&self.owner.root.queued_references);
            prefix_facts(self.owner, self.id, self.consumer, self.launch,
                self.returns@, self.owner.root.inputs@.len() as int);
            prefix_extends(self.owner, self.id, self.consumer, self.launch,
                self.returns@, 0, self.owner.root.inputs@.len() as int);
            assert(self.receipts@ == Seq::<Receipt>::empty());
            assert(before.original().take(0) =~= Seq::<Receipt>::empty());
        }
        producer_input_fold_body!(verus_exec_expr, self, invalid_reference,
            (index, aggregate, active_index, queued_index, input_count),
            [invariant
                before == *old(self), self.wf(), self.owner == before.owner,
                invalid_reference == ContextVersionJournalErrorV1::InvalidReference,
                vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),
                vstd::std_specs::hash::obeys_key_model::<RuntimeAllocationIdV1>(),
                self.owner.root.references@.len() <= usize::MAX,
                self.owner.root.queued_references@.len() <= usize::MAX,
                self.id == before.id, self.consumer == before.consumer,
                self.launch == before.launch, self.returns == before.returns,
                self.original() == before.original(),
                input_count == before.original().len(), index <= input_count,
                self.consumed@ == index,
                self.receipts@ == before.original().take(index as int),
                active_index == fold::active_before(before.original(), index as int),
                queued_index == fold::queued_before(before.original(), index as int),
                fold::fold_result(before.original(), 0, fold::ContextProducerReadStatusV1::Success,
                    before.owner.root.references@.len() as usize,
                    before.owner.root.queued_references@.len() as usize, invalid_reference)
                    == fold::fold_result(before.original(), index as int, fold_status(aggregate),
                        before.owner.root.references@.len() as usize,
                        before.owner.root.queued_references@.len() as usize, invalid_reference),
                fold::reached(before.original(), 0) == fold::reached(before.original(), index as int),
             decreases input_count - index],
            [proof {
                reveal_with_fuel(fold::fold_result, 2);
                reveal_with_fuel(fold::reached, 2);
                fold_step_error(before.original(), index as int, fold_status(aggregate),
                    before.owner.root.references@.len() as usize,
                    before.owner.root.queued_references@.len() as usize, invalid_reference);
            }],
            [])
    }
}
}
