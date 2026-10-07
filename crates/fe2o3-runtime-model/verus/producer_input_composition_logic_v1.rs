// Shared pure fold-prefix logic. Each root defines Source and its concrete
// source_len/source_answers functions; no abstract contracts or axioms are added.
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
    consumer: ContextWriterKeyV1, launch: bool, answers: Source, end: int) -> Prefix
    recommends 0 <= end <= owner.root.inputs@.len(), source_len(answers) == owner.root.inputs@.len(),
    decreases end,
{
    if end <= 0 { Prefix { receipts: seq![], calls: seq![], active: 0, queued: 0 } }
    else {
        let before = prefix(owner, id, consumer, launch, answers, end - 1);
        let index = (end - 1) as usize;
        let observed_answers = source_answers(owner, answers, index, before.active, before.queued);
        let step = outcome(owner, id, consumer, launch, index, before.active, before.queued, observed_answers);
        let observed = receipt(owner.root.inputs@[end - 1].request, before.active, before.queued,
            step.active, step.queued, step.result, step.calls, observed_answers.credit);
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
    consumer: ContextWriterKeyV1, launch: bool, answers: Source, end: int)
    requires 0 <= end <= owner.root.inputs@.len() <= usize::MAX,
        source_len(answers) == owner.root.inputs@.len(),
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
            before.active, before.queued, source_answers(owner, answers, (end - 1) as usize, before.active, before.queued));
        reveal_with_fuel(prefix, 2);
        let after = prefix(owner, id, consumer, launch, answers, end);
        assert(after.receipts.len() == end);
        prefix_counts(after.receipts, end - 1);
        assert(after.receipts.take(end - 1) =~= before.receipts);
    }
}

proof fn prefix_extends<C, L, P, D, R>(owner: &Owner<C, L, P, D, R>, id: RuntimeSubmissionIdV1,
    consumer: ContextWriterKeyV1, launch: bool, answers: Source, end: int, total: int)
    requires 0 <= end <= total <= owner.root.inputs@.len() <= usize::MAX,
        source_len(answers) == owner.root.inputs@.len(),
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

}
