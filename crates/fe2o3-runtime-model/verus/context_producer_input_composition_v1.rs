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
type Receipt = fold::InputObservation<ContextVersionJournalErrorV1>;

// The two proof roots deliberately retain their own status declaration order.
// Correspondence is by constructor, never by representation or ordinal.
spec fn fold_status(status: ContextProducerReadStatusV1) -> fold::ContextProducerReadStatusV1 {
    match status {
        ContextProducerReadStatusV1::Pending => fold::ContextProducerReadStatusV1::Pending,
        ContextProducerReadStatusV1::Success => fold::ContextProducerReadStatusV1::Success,
        ContextProducerReadStatusV1::NoEffect => fold::ContextProducerReadStatusV1::NoEffect,
        ContextProducerReadStatusV1::Unknown => fold::ContextProducerReadStatusV1::Unknown,
    }
}

spec fn fold_status_result(result: StatusResult)
    -> Result<fold::ContextProducerReadStatusV1, ContextVersionJournalErrorV1>
{
    match result { Ok(status) => Ok(fold_status(status)), Err(error) => Err(error) }
}

proof fn status_mapping_injective(left: ContextProducerReadStatusV1, right: ContextProducerReadStatusV1)
    ensures fold_status(left) == fold_status(right) <==> left == right,
{}

proof fn fold_step_error(history: Seq<Receipt>, index: int,
    aggregate: fold::ContextProducerReadStatusV1, active: usize, queued: usize,
    invalid: ContextVersionJournalErrorV1)
    requires 0 <= index < history.len(),
    ensures forall|error: ContextVersionJournalErrorV1|
        #[trigger] fold_status_result(Err(error)) == history[index].result ==>
            fold_status_result(Err(error))
                == fold::fold_result(history, index, aggregate, active, queued, invalid),
{
    reveal_with_fuel(fold::fold_result, 2);
}

spec fn credit_from_calls(calls: Seq<Call>, matched: bool) -> fold::CreditObservation
    decreases calls.len(),
{
    if calls.len() == 0 { fold::CreditObservation::NotReached }
    else {
        match calls[0] {
            Call::Credit(allocation, device, byte_len) => fold::CreditObservation::Returned {
                allocation_generation: allocation.context_generation,
                allocation_local: allocation.local,
                device_generation: device.context_generation,
                device_local: device.local,
                byte_len, matched,
            },
            _ => credit_from_calls(calls.drop_first(), matched),
        }
    }
}

spec fn receipt(request: ProducerReadRequestV1, active_before: usize, queued_before: usize,
    active_after: usize, queued_after: usize, result: StatusResult, calls: Seq<Call>, credit: bool)
    -> Receipt
{
    let (family, advanced) = match request {
        ProducerReadRequestV1::Active(_) => (fold::Family::Active, active_after != active_before),
        ProducerReadRequestV1::Queued(_) => (fold::Family::Queued, queued_after != queued_before),
    };
    fold::InputObservation {
        family, advanced, result: fold_status_result(result),
        credit: credit_from_calls(calls, credit),
    }
}

struct Prefix {
    receipts: Seq<Receipt>, calls: Seq<Call>, active: usize, queued: usize,
}

// This total ghost sequence also describes a hypothetical unreached suffix.
// Only Prefix(consumed) is retained or claimed as actually observed.
spec fn prefix<C, L, P, D, R>(owner: &Owner<C, L, P, D, R>, id: RuntimeSubmissionIdV1,
    consumer: ContextWriterKeyV1, launch: bool, answers: Seq<Returns>, end: int) -> Prefix
    recommends 0 <= end <= owner.root.inputs@.len(), answers.len() == owner.root.inputs@.len(),
    decreases end,
{
    if end <= 0 { Prefix { receipts: seq![], calls: seq![], active: 0, queued: 0 } }
    else {
        let before = prefix(owner, id, consumer, launch, answers, end - 1);
        let index = (end - 1) as usize;
        let step = outcome(owner, id, consumer, launch, index, before.active, before.queued, answers[end - 1]);
        let observed = receipt(owner.root.inputs@[end - 1].request, before.active, before.queued,
            step.active, step.queued, step.result, step.calls, answers[end - 1].credit);
        Prefix { receipts: before.receipts.push(observed), calls: before.calls + step.calls,
            active: step.active, queued: step.queued }
    }
}

proof fn outcome_cursor_frame<C, L, P, D, R>(owner: &Owner<C, L, P, D, R>, id: RuntimeSubmissionIdV1,
    consumer: ContextWriterKeyV1, launch: bool, index: usize, active: usize, queued: usize, answers: Returns)
    requires index < owner.root.inputs@.len(), active < usize::MAX, queued < usize::MAX,
    ensures
        match owner.root.inputs@[index as int].request {
            ProducerReadRequestV1::Active(_) => {
                let step = outcome(owner, id, consumer, launch, index, active, queued, answers);
                step.queued == queued && (step.active == active || step.active == active + 1)
            },
            ProducerReadRequestV1::Queued(_) => {
                let step = outcome(owner, id, consumer, launch, index, active, queued, answers);
                step.active == active && (step.queued == queued || step.queued == queued + 1)
            },
        },
{}

proof fn prefix_facts<C, L, P, D, R>(owner: &Owner<C, L, P, D, R>, id: RuntimeSubmissionIdV1,
    consumer: ContextWriterKeyV1, launch: bool, answers: Seq<Returns>, end: int)
    requires 0 <= end <= owner.root.inputs@.len() <= usize::MAX,
        answers.len() == owner.root.inputs@.len(),
    ensures
        prefix(owner, id, consumer, launch, answers, end).receipts.len() == end,
        prefix(owner, id, consumer, launch, answers, end).active
            == fold::active_before(prefix(owner, id, consumer, launch, answers, end).receipts, end),
        prefix(owner, id, consumer, launch, answers, end).queued
            == fold::queued_before(prefix(owner, id, consumer, launch, answers, end).receipts, end),
        prefix(owner, id, consumer, launch, answers, end).active <= end,
        prefix(owner, id, consumer, launch, answers, end).queued <= end,
    decreases end,
{
    if end > 0 {
        prefix_facts(owner, id, consumer, launch, answers, end - 1);
        let before = prefix(owner, id, consumer, launch, answers, end - 1);
        outcome_cursor_frame(owner, id, consumer, launch, (end - 1) as usize,
            before.active, before.queued, answers[end - 1]);
        reveal_with_fuel(prefix, 2);
        let after = prefix(owner, id, consumer, launch, answers, end);
        assert(after.receipts.len() == end);
        prefix_counts(after.receipts, end - 1);
        assert(after.receipts.take(end - 1) =~= before.receipts);
    }
}

proof fn prefix_extends<C, L, P, D, R>(owner: &Owner<C, L, P, D, R>, id: RuntimeSubmissionIdV1,
    consumer: ContextWriterKeyV1, launch: bool, answers: Seq<Returns>, end: int, total: int)
    requires 0 <= end <= total <= owner.root.inputs@.len() <= usize::MAX,
        answers.len() == owner.root.inputs@.len(),
    ensures prefix(owner, id, consumer, launch, answers, total).receipts.take(end)
        == prefix(owner, id, consumer, launch, answers, end).receipts,
    decreases total - end,
{
    prefix_facts(owner, id, consumer, launch, answers, total);
    if end < total {
        prefix_extends(owner, id, consumer, launch, answers, end, total - 1);
        reveal_with_fuel(prefix, 2);
        assert(prefix(owner, id, consumer, launch, answers, total).receipts.take(end)
            =~= prefix(owner, id, consumer, launch, answers, total - 1).receipts.take(end));
    } else {
        assert(prefix(owner, id, consumer, launch, answers, total).receipts.take(end)
            =~= prefix(owner, id, consumer, launch, answers, total).receipts);
    }
}

proof fn prefix_counts<E>(history: Seq<fold::InputObservation<E>>, end: int)
    requires 0 <= end <= history.len(),
    ensures fold::active_before(history.take(end), end) == fold::active_before(history, end),
        fold::queued_before(history.take(end), end) == fold::queued_before(history, end),
    decreases end,
{
    if end > 0 {
        prefix_counts(history, end - 1);
        prefix_counts(history.take(end), end - 1);
        assert(history.take(end).take(end - 1) =~= history.take(end - 1));
    }
}

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
