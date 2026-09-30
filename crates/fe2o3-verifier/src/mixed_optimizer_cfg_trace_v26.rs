//! Conditional finite-trace composition. The producer must still establish the
//! local step premise and initial relation for its exact source/output graphs.
//! This lemma alone does not upgrade a block-simulation receipt.

pub(super) const PRELUDE: &str = r#"
// One step is an entire matched basic block. An intra-block rewrite may erase
// instructions, but the admitted Policy9 statement does not erase CFG blocks.
struct CfgStepV26<S, E> {
    state: S,
    events: Seq<E>,
    halted: bool,
}

struct CfgTraceV26<S, E> {
    state: S,
    events: Seq<E>,
    halted: bool,
    steps: nat,
}

open spec fn cfg_trace_v26<S, E>(
    step: spec_fn(S) -> CfgStepV26<S, E>,
    state: S,
    fuel: nat,
) -> CfgTraceV26<S, E>
    decreases fuel,
{
    if fuel == 0 {
        CfgTraceV26 { state, events: Seq::empty(), halted: false, steps: 0 }
    } else {
        let first = step(state);
        if first.halted {
            CfgTraceV26 {
                state: first.state,
                events: first.events,
                halted: true,
                steps: 1,
            }
        } else {
            let tail = cfg_trace_v26(step, first.state, (fuel - 1) as nat);
            CfgTraceV26 {
                state: tail.state,
                events: first.events + tail.events,
                halted: tail.halted,
                steps: 1 + tail.steps,
            }
        }
    }
}

open spec fn cfg_step_simulates_v26<N, O, E>(
    step_n: spec_fn(N) -> CfgStepV26<N, E>,
    step_o: spec_fn(O) -> CfgStepV26<O, E>,
    related: spec_fn(N, O) -> bool,
) -> bool {
    forall|n: N, o: O| #[trigger] related(n, o) ==>
        step_n(n).events == step_o(o).events
        && step_n(n).halted == step_o(o).halted
        && related(step_n(n).state, step_o(o).state)
}

proof fn cfg_trace_bound_v26<S, E>(
    step: spec_fn(S) -> CfgStepV26<S, E>,
    state: S,
    fuel: nat,
)
    ensures
        cfg_trace_v26(step, state, fuel).steps <= fuel,
        cfg_trace_v26(step, state, fuel).steps < fuel ==>
            cfg_trace_v26(step, state, fuel).halted,
        cfg_trace_v26(step, state, fuel).halted ==>
            cfg_trace_v26(step, state, fuel).steps > 0,
    decreases fuel,
{
    if fuel > 0 {
        let first = step(state);
        if !first.halted {
            cfg_trace_bound_v26(step, first.state, (fuel - 1) as nat);
        }
    }
}

// Conditional theorem only: every concrete generator must prove the quantified
// local-step premise. Merely emitting this theorem is not that instantiation.
proof fn cfg_finite_trace_refinement_v26<N, O, E>(
    step_n: spec_fn(N) -> CfgStepV26<N, E>,
    step_o: spec_fn(O) -> CfgStepV26<O, E>,
    related: spec_fn(N, O) -> bool,
    n: N,
    o: O,
    fuel: nat,
)
    requires
        related(n, o),
        cfg_step_simulates_v26(step_n, step_o, related),
    ensures
        cfg_trace_v26(step_n, n, fuel).events ==
            cfg_trace_v26(step_o, o, fuel).events,
        cfg_trace_v26(step_n, n, fuel).halted ==
            cfg_trace_v26(step_o, o, fuel).halted,
        cfg_trace_v26(step_n, n, fuel).steps ==
            cfg_trace_v26(step_o, o, fuel).steps,
        related(
            cfg_trace_v26(step_n, n, fuel).state,
            cfg_trace_v26(step_o, o, fuel).state,
        ),
    decreases fuel,
{
    if fuel > 0 {
        let first_n = step_n(n);
        let first_o = step_o(o);
        assert(first_n.events == first_o.events);
        assert(first_n.halted == first_o.halted);
        assert(related(first_n.state, first_o.state));
        if !first_n.halted {
            cfg_finite_trace_refinement_v26(
                step_n, step_o, related,
                first_n.state, first_o.state, (fuel - 1) as nat,
            );
        }
    }
}
"#;
