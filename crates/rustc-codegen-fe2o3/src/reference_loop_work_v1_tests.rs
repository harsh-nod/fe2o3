use super::*;
use fe2o3_mir_model::semantic_mir_v1::HARD_MAX_VALIDATION_WORK_V1;
use std::cell::Cell;
use std::mem::size_of;

// Empty payloads isolate map-key accounting; these are not authenticated loop traces.
fn state_with_empty_traces() -> ReferenceSymbolicStateV2 {
    ReferenceSymbolicStateV2 {
        block: 0,
        environment: BTreeMap::new(),
        guard: ReferencePathPredicateV1::unconditional_v1(),
        traces: (0_u32..64)
            .map(|header| {
                let latch = header + 64;
                (
                    (header, latch),
                    ReferenceLoopTraceV2 {
                        header,
                        latch,
                        exit: Some(header + 128),
                        initial: BTreeMap::new(),
                        transitions: Vec::new(),
                        variants: Vec::new(),
                        exact_iterations: Some(0),
                        maximum_iterations: Some(0),
                    },
                )
            })
            .collect(),
    }
}

#[test]
fn state_payload_minimum_includes_every_trace_map_key() {
    let state = state_with_empty_traces();
    let minimum_payload_bytes = size_of::<ReferenceSymbolicStateV2>()
        + state.guard.clauses.len() * size_of::<ReferenceGuardClauseV1>()
        + state.traces.len() * size_of::<((u32, u32), ReferenceLoopTraceV2)>();
    let mut work = SourceClosureWorkV1::default();
    let units = ReferenceExtractionWorkV1::borrowed(&mut work)
        .state_units(&state)
        .unwrap();
    assert!(
        units >= minimum_payload_bytes,
        "state clone debit {units} undercounts {minimum_payload_bytes} payload bytes",
    );
    assert!(work.validation_work_for_test() > 0);
}

#[test]
fn state_clone_key_accounting_preserves_inherited_exact_and_one_short_work() {
    let state = state_with_empty_traces();
    let clones = Cell::new(0_usize);
    let clone_state = |work: &mut SourceClosureWorkV1| {
        let meter = ReferenceExtractionWorkV1::borrowed(work);
        let mut symbolic_work = ReferenceSymbolicWorkBudgetV2::default();
        symbolic_work.charge_state_clone_v2(&meter, &state)?;
        clones.set(clones.get() + 1);
        Ok::<_, ReferenceBindingErrorV1>(state.clone())
    };

    let mut measured = SourceClosureWorkV1::default();
    let cloned = clone_state(&mut measured).unwrap();
    assert!(cloned.traces.keys().eq(state.traces.keys()));
    let cost = measured.validation_work_for_test();
    assert!(cost > 0 && cost < HARD_MAX_VALIDATION_WORK_V1);

    let mut inherited = SourceClosureWorkV1::default();
    inherited.charge(17).unwrap();
    clone_state(&mut inherited).unwrap();
    assert_eq!(inherited.validation_work_for_test(), 17 + cost);

    let mut exact = SourceClosureWorkV1::default();
    exact
        .charge(usize::try_from(HARD_MAX_VALIDATION_WORK_V1 - cost).unwrap())
        .unwrap();
    clone_state(&mut exact).unwrap();
    assert_eq!(
        exact.validation_work_for_test(),
        HARD_MAX_VALIDATION_WORK_V1
    );
    assert_eq!(clones.get(), 3);

    let mut short = SourceClosureWorkV1::default();
    short
        .charge(usize::try_from(HARD_MAX_VALIDATION_WORK_V1 - cost + 1).unwrap())
        .unwrap();
    let error = clone_state(&mut short).unwrap_err();
    assert!(error.to_string().contains("ValidationWork"));
    assert_eq!(
        short.validation_work_for_test(),
        HARD_MAX_VALIDATION_WORK_V1 + 1,
    );
    assert_eq!(clones.get(), 3, "one-short work must refuse before cloning");
}
