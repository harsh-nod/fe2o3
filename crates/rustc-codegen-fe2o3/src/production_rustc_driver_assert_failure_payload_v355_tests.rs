//! Genuine rustc failure operands; no mutated MIR or executed proof authority.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssertMessageV1, SemanticBinaryOpV1, SemanticScalarTypeV1, SemanticTypeShapeV1,
    SemanticUnwindActionV1,
};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::mixed_licm_tests::worker_orchestration_tests::predicated_publication_v90_tests::assert_failure_payload_tests::assert_failure_payload_child";
const CASES: &[(&str, &str)] = &[("dynamic", "seed + other"), ("duplicate", "seed + seed")];
const PROFILES: &[(u8, u8)] = &[(0, 0), (3, 0)];

fn program(expression: &str) -> String {
    let mut source = "use fe2o3_device::{WriteOnlyDisjointSlice, kernel, thread};\n".to_owned();
    for name in ["failure_first", "failure_second"] {
        source.push_str(&format!(
            r#"
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn {name}(seed: u32, other: u32, mut output: WriteOnlyDisjointSlice<u32>) {{
    let value = {expression};
    let _ = output.write(thread::index_1d(), value);
}}
"#
        ));
    }
    source
}

#[derive(Debug, Serialize)]
struct FailureRow {
    root: usize,
    function: u32,
    block: u32,
    success: u32,
    cut: usize,
    variables: [u32; 2],
    use_ordinals: [u32; 2],
    kill_ordinals: [u32; 2],
    success_live_in_failure_variables: usize,
    success_uses_of_failure_variables: usize,
}

fn observe_failure_payloads(
    candidate: Candidate<'_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<Observation, Error> {
    let source = candidate.source(budget)?;
    let semantic = source.source_semantic(budget)?;
    let owner = source.source_ssa(budget)?;
    source.check_original_source(owner, budget)?;
    assert!(std::ptr::eq(owner.source_semantic(), semantic));
    assert_eq!(source.root_count(budget)?, 2);
    assert_eq!(semantic.roots().len(), 2);
    let captured = owner
        .occurrences_v1()
        .expect("retained original occurrences");
    for (root, function) in semantic.roots().iter().copied().enumerate() {
        budget.charge_work(1)?;
        assert_eq!(source.instance(root, 0, budget)?, (function, None));
        let body = &semantic.functions()[function.index() as usize];
        let occurrences = captured.function(function).unwrap();
        assert!(std::ptr::eq(occurrences.owner(), owner));
        assert_eq!(occurrences.function(), function);
        assert_eq!(occurrences.block_count_v299(), body.blocks().len());
        let function_plan = owner.plan_for_function(function).unwrap();
        assert_eq!(function_plan.function(), function);
        assert_eq!(function_plan.function_identity(), body.identity());
        let plan = function_plan.plan();
        let mut assertions = 0;
        for (block_index, block) in body.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            let SemanticTerminatorKindV1::Assert {
                condition,
                expected,
                message:
                    SemanticAssertMessageV1::Overflow {
                        operation,
                        left,
                        right,
                    },
                target,
                unwind,
            } = block.terminator().kind()
            else {
                continue;
            };
            assertions += 1;
            assert_eq!(*operation, SemanticBinaryOpV1::Add);
            assert!(!expected);
            assert_eq!(*unwind, SemanticUnwindActionV1::Unreachable);
            let block_id = SsaBlockIdV1::new(block_index as u32);
            let site = Site::Terminator { block: block_id };
            let events = occurrences.block_events_v299(block_id).unwrap();
            let cut = occurrences.terminal_failure_start(block_id).unwrap();
            assert!(cut > 0);
            assert_eq!(events.len(), cut + 4);
            let (SemanticOperandV1::Move(condition) | SemanticOperandV1::Copy(condition)) =
                condition
            else {
                panic!("dynamic checked-add condition must retain its source place");
            };
            budget.charge_work(events.len())?;
            let mut condition_events = events.iter().filter(|event| {
                event.site() == site
                    && event.operand() == OperandRole::AssertCondition
                    && event.role() == EventRole::BaseUse
            });
            let condition_use = condition_events.next().unwrap();
            assert!(condition_events.next().is_none());
            // The condition may project the flag from an unpromoted checked
            // aggregate; only the scalar diagnostic operands require promotion.
            assert_eq!(
                condition_use.event().variable(),
                SsaVariableIdV1::new(condition.local().index())
            );
            assert!((condition_use.ordinal() as usize) < cut);
            assert!(condition_use.is_reachable());
            let mut row = FailureRow {
                root,
                function: function.index(),
                block: block_id.get(),
                success: target.target().index(),
                cut,
                variables: [0; 2],
                use_ordinals: [0; 2],
                kill_ordinals: [0; 2],
                success_live_in_failure_variables: 0,
                success_uses_of_failure_variables: 0,
            };
            for (operand, actual) in [left, right].into_iter().enumerate() {
                budget.charge_work(1)?;
                let SemanticOperandV1::Move(place) = actual else {
                    panic!("MIR0 must retain an actual moved overflow payload: {actual:?}");
                };
                assert!(place.projections().is_empty());
                assert_eq!(
                    semantic.types()[place.ty().index() as usize].shape(),
                    &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32,
                    })
                );
                let used = exact_event(
                    events,
                    site,
                    OperandRole::AssertMessage(operand as u32),
                    EventRole::BaseUse,
                    place.local(),
                    budget,
                )?;
                let killed = exact_event(
                    events,
                    site,
                    OperandRole::AssertMessage(operand as u32),
                    EventRole::MoveKill,
                    place.local(),
                    budget,
                )?;
                assert!(used.is_reachable() && killed.is_reachable());
                assert_eq!(used.ordinal() as usize, cut + 2 * operand);
                assert_eq!(killed.ordinal(), used.ordinal() + 1);
                let Some(SsaResolvedEventV1::Use { variable, value }) = used.resolved() else {
                    panic!("authentic failure payload must resolve to a use");
                };
                assert_eq!(variable, SsaVariableIdV1::new(place.local().index()));
                assert_eq!(
                    killed.resolved(),
                    Some(SsaResolvedEventV1::Kill {
                        variable,
                        previous: Some(value),
                    })
                );
                for event in [used, killed] {
                    assert_eq!(
                        plan.resolved_event(block_id, event.ordinal()).copied(),
                        event.resolved()
                    );
                }
                row.variables[operand] = variable.get();
                row.use_ordinals[operand] = used.ordinal();
                row.kill_ordinals[operand] = killed.ordinal();
            }
            let success_events = occurrences
                .block_events_v299(SsaBlockIdV1::new(target.target().index()))
                .unwrap();
            budget.charge_work(success_events.len())?;
            // rustc may create fresh panic temporaries. This is an observation,
            // not a claim that a moved failure variable is live on success.
            row.success_uses_of_failure_variables = success_events
                .iter()
                .filter(|event| {
                    matches!(event.resolved(), Some(SsaResolvedEventV1::Use { variable, .. })
                    if row.variables.contains(&variable.get()))
                })
                .count();
            let live_in = plan
                .live_in(SsaBlockIdV1::new(target.target().index()))
                .unwrap();
            budget.charge_work(live_in.len())?;
            row.success_live_in_failure_variables = live_in
                .iter()
                .filter(|variable| row.variables.contains(&variable.get()))
                .count();
            println!(
                "ACTUAL_ASSERT_FAILURE_PAYLOAD_V355 {}",
                serde_json::to_string(&row).unwrap()
            );
        }
        assert_eq!(
            assertions, 1,
            "each actual root must retain its dynamic checked add"
        );
    }
    // Preserve the existing final graph, source binding, descriptor, model, and
    // execution-refusal checks on the exact same default V90 candidate/account.
    super::observe(candidate, budget)
}

#[test]
#[ignore = "private actual-rustc child; invoked only by the parent fixture"]
fn assert_failure_payload_child() {
    super::child_with_consumer(true, observe_failure_payloads as Consumer);
}

#[test]
fn failure_payload_source_roster_keeps_dynamic_and_duplicate_inputs_at_mir_zero() {
    assert_eq!(
        CASES,
        &[("dynamic", "seed + other"), ("duplicate", "seed + seed")]
    );
    assert_eq!(PROFILES, &[(0, 0), (3, 0)]);
    for (_, expression) in CASES {
        let source = program(expression);
        assert_eq!(source.matches("#[kernel(").count(), 2);
        assert_eq!(
            source
                .matches(&format!("let value = {expression};"))
                .count(),
            2
        );
        assert_eq!(
            source
                .matches("output.write(thread::index_1d(), value)")
                .count(),
            2
        );
        assert!(!source.contains("wrapping_") && !source.contains("unsafe"));
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src/rustc-dev; genuine AMD source/SSA and default V90 account checks, not executed proof"]
fn actual_default_boundary_retains_nonreturning_assert_failure_payloads() {
    run_actual_sources::<Observation>(
        CASES,
        PROFILES,
        CHILD,
        "ACTUAL_DEFAULT_ASSERT_FAILURE_PAYLOAD_V355",
        program,
        |_, _, _, report, _| super::validate("fresh", &report),
    );
}
