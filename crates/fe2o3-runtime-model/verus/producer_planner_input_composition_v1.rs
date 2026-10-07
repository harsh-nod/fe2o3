// Included inside concrete_composition by the source-bound qualification root.
// This is a present-root observation projection, not a mutable Context adapter:
// root selection, writer quarantine effects, credit locks and settlement remain
// outside this theorem. No producer status or live result is supplied by callers.
mod completion_input {
    use vstd::prelude::*;
    use vstd::std_specs::core::IndexSpecImpl;
    include!("context_completion_reconciliation_graph_v1.rs");
    include!("context_completion_reconciliation_validation_v1.rs");
    include!("context_completion_reconciliation_effects_v1.rs");
}

verus! {

type PlannerInputResult = Result<Option<completion_input::ContextProducerReadStatusV1>,
    completion_input::RuntimeValidationErrorV1>;
type ProjectedInput = Result<Option<completion_input::ContextProducerReadStatusV1>,
    completion_input::JournalObservationErrorV1>;

spec fn planner_id(id: RuntimeSubmissionIdV1) -> completion_input::RuntimeSubmissionIdV1 {
    completion_input::RuntimeSubmissionIdV1 {
        context_generation: id.context_generation, local: id.local,
    }
}

spec fn planner_status(status: ContextProducerReadStatusV1)
    -> completion_input::ContextProducerReadStatusV1 {
    match status {
        ContextProducerReadStatusV1::Pending => completion_input::ContextProducerReadStatusV1::Pending,
        ContextProducerReadStatusV1::Success => completion_input::ContextProducerReadStatusV1::Success,
        ContextProducerReadStatusV1::NoEffect => completion_input::ContextProducerReadStatusV1::NoEffect,
        ContextProducerReadStatusV1::Unknown => completion_input::ContextProducerReadStatusV1::Unknown,
    }
}

spec fn projected_input(result: StatusResult) -> ProjectedInput {
    match result {
        Ok(status) => Ok(Some(planner_status(status))),
        Err(_) => Err(completion_input::JournalObservationErrorV1 { code: 0 }),
    }
}

spec fn planner_input_result(result: StatusResult) -> PlannerInputResult {
    match result {
        Ok(status) => Ok(Some(planner_status(status))),
        Err(_) => Err(completion_input::RuntimeValidationErrorV1::InvalidBackendDescription),
    }
}

fn project_input(result: StatusResult) -> (out: ProjectedInput)
    ensures out == projected_input(result),
{
    match result {
        Ok(status) => Ok(Some(match status {
            ContextProducerReadStatusV1::Pending => completion_input::ContextProducerReadStatusV1::Pending,
            ContextProducerReadStatusV1::Success => completion_input::ContextProducerReadStatusV1::Success,
            ContextProducerReadStatusV1::NoEffect => completion_input::ContextProducerReadStatusV1::NoEffect,
            ContextProducerReadStatusV1::Unknown => completion_input::ContextProducerReadStatusV1::Unknown,
        })),
        Err(_) => Err(completion_input::JournalObservationErrorV1 { code: 0 }),
    }
}

// Every non-input field is retained exactly. The assignment is a projection of
// one observed return, never a production journal or completion-state mutation.
spec fn input_projection(before: completion_input::CompletionTableV1,
    after: completion_input::CompletionTableV1, id: completion_input::RuntimeSubmissionIdV1,
    input: ProjectedInput) -> bool {
    &&& before.wf() && before.contains(id)
    &&& after.nodes@ == before.nodes@.update(before.slot(id),
        completion_input::CompletionNodeV1 { input, ..before.node(id) })
}

#[verifier::spinoff_prover]
fn install_observed_input(context: &mut completion_input::CompletionProjectionV1,
    id: completion_input::RuntimeSubmissionIdV1, input: ProjectedInput)
    requires old(context).submissions.wf(), old(context).submissions.contains(id),
    ensures final(context).submissions.wf(), final(context).submissions.contains(id),
        input_projection(old(context).submissions, final(context).submissions, id, input),
        final(context).submissions.slot(id) == old(context).submissions.slot(id),
        final(context).submissions.node(id).input == input,
        final(context).quarantined == old(context).quarantined,
        final(context).ordinary_checked == old(context).ordinary_checked,
        final(context).custody_checked == old(context).custody_checked,
        final(context).peer_checked == old(context).peer_checked,
        final(context).producer_checked == old(context).producer_checked,
        final(context).unsafe_settlement_attempt == old(context).unsafe_settlement_attempt,
        final(context).rejection == old(context).rejection,
{
    let ghost before = context.submissions;
    let index = context.submissions.find(id).unwrap();
    context.submissions.nodes[index].input = input;
    proof {
        assert forall|i: int| 0 <= i < before.nodes@.len() implies
            context.submissions.nodes@[i].id == before.nodes@[i].id by {};
        assert(context.submissions.wf());
        context.submissions.slot_unique(id, index as int);
    }
}

impl<'a, C, L, P, D, R, V> Composition<'a, C, L, P, D, R, V> {
    #[verifier::spinoff_prover]
    fn reconcile_planner_input(&mut self, planner: &mut completion_input::CompletionProjectionV1)
        -> (out: PlannerInputResult)
        requires old(self).wf(), old(self).consumed@ == 0,
            old(planner).submissions.wf(),
            old(planner).submissions.contains(planner_id(old(self).id)),
            vstd::std_specs::hash::obeys_key_model::<RuntimeSubmissionIdV1>(),
            vstd::std_specs::hash::obeys_key_model::<RuntimeAllocationIdV1>(),
        ensures final(self).wf(), final(self).same_binding(old(self)),
            final(self).original() == old(self).original(),
            final(self).consumed@ == fold::reached(old(self).original(), 0),
            final(self).receipts@ == old(self).original().take(final(self).consumed@ as int),
            final(self).calls@ == prefix(&old(self).owner_view(), old(self).id, old(self).consumer,
                old(self).launch, old(self).source(), final(self).consumed@ as int).calls,
            final(planner).submissions.wf(),
            final(planner).submissions.contains(planner_id(old(self).id)),
            exists|actual: StatusResult|
                fold_status_result(actual) == fold::fold_result(old(self).original(), 0,
                    fold::ContextProducerReadStatusV1::Success,
                    old(self).root.references@.len() as usize,
                    old(self).root.queued_references@.len() as usize,
                    ContextVersionJournalErrorV1::InvalidReference)
                && out == planner_input_result(actual)
                && input_projection(old(planner).submissions, final(planner).submissions,
                    planner_id(old(self).id), projected_input(actual)),
            final(planner).quarantined == (old(planner).quarantined || out.is_err()),
            final(planner).rejection == match out {
                Ok(_) => old(planner).rejection, Err(error) => Some(error),
            },
            final(planner).ordinary_checked == old(planner).ordinary_checked,
            final(planner).custody_checked == old(planner).custody_checked,
            final(planner).peer_checked == old(planner).peer_checked,
            final(planner).producer_checked == old(planner).producer_checked,
            final(planner).unsafe_settlement_attempt == old(planner).unsafe_settlement_attempt,
    {
        let result = self.reconcile();
        let id = completion_input::RuntimeSubmissionIdV1 {
            context_generation: self.id.context_generation, local: self.id.local,
        };
        let input = project_input(result);
        install_observed_input(planner, id, input);
        let output = planner.directed_input_status_v1(id);
        proof {
            assert(output == planner_input_result(result));
            assert(input_projection(old(planner).submissions, planner.submissions,
                planner_id(old(self).id), projected_input(result)));
        }
        output
    }
}

// A succeeded backend and exhausted, successful dependencies cannot turn an
// unknown/pending/no-effect/error producer result into a success-ready gate.
proof fn observed_input_success_gate(before: completion_input::CompletionTableV1,
    after: completion_input::CompletionTableV1, id: completion_input::RuntimeSubmissionIdV1,
    actual: StatusResult)
    requires input_projection(before, after, id, projected_input(actual)),
        after.wf(), after.contains(id), after.slot(id) == before.slot(id),
    ensures completion_input::success_ready(after, id)
        ==> actual == Ok(ContextProducerReadStatusV1::Success),
        after.node(id).state.is_some()
            && after.node(id).state.unwrap().cursor == after.node(id).dependencies@.len()
            && completion_input::quiescent_ready(after, id)
            ==> actual == Ok(ContextProducerReadStatusV1::Unknown),
{}

}
