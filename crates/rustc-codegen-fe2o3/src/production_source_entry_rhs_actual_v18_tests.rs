use super::*;

const ENTRY_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_v18_tests::entry_rhs_actual_v18_tests::source_owned_entry_rhs_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct EntryObservation {
    source: [u8; 32],
    graph: [u8; 32],
    requests: usize,
    root_arguments: usize,
    caller_arguments: usize,
    foreign_original_refused: bool,
    same_type_rhs_refused: bool,
}

#[derive(Clone, Debug)]
struct EntrySession {
    mode: u8,
    source: [u8; 32],
    graph: [u8; 32],
    counts: Option<[usize; 3]>,
}

struct EntryCallbacks {
    mode: u8,
    result: Option<Result<EntrySession, String>>,
}

impl Callbacks for EntryCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            // There is exactly one capture in this TyCtxt. The parent child
            // runner gives each positive/hostile check a fresh rustc session.
            let transaction = transaction_in_active_session_v1(tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled)?;
            if self.mode == 0 {
                let original = transaction.consume_source_owned_v18(|view, budget| {
                        let source = *view.source_ssa(budget)?.source_semantic_sha256();
                        let graph = *view.canonical(budget)?.identity().digest();
                        let counts = crate::production_ranked_projection_v1::inspect_actual_source_entry_consumer_v18(
                            view, budget, 0)?;
                        budget.reserve_storage(std::mem::size_of::<([u8; 32], [u8; 32], [usize; 3])>())?;
                        Ok((source, graph, counts))
                    }).map_err(|error| format!("actual typed entry positive: {error:?}"))?;
                return Ok(EntrySession { mode: 0, source: original.0,
                    graph: original.1, counts: Some(original.2) });
            }
            let expected = match self.mode {
                1 => "source scalar original declaration is foreign to semantic owner",
                2 => "actual scalar expression differs from its original source value",
                _ => return Err("unknown actual entry check mode".to_owned()),
            };
            let identity = std::cell::Cell::new(None);
            let observed = std::cell::Cell::new(false);
            let refused = transaction.consume_source_owned_v18(|view, budget| {
                // Copy-only test identity, not a Prepared/source owner or an
                // authority-bearing value used by another compiler session.
                identity.set(Some((*view.source_ssa(budget)?.source_semantic_sha256(),
                    *view.canonical(budget)?.identity().digest())));
                let result = crate::production_ranked_projection_v1::inspect_actual_source_entry_consumer_v18(
                    view, budget, self.mode);
                assert!(matches!(&result, Err(ViewError::Binding(detail)) if *detail == expected),
                    "actual typed entry negative {}: {result:?}", self.mode);
                observed.set(true);
                result
            });
            assert!(observed.get());
            assert!(matches!(refused, Err(error) if matches!(*error,
                crate::production_pipeline::ProductionPipelineError::SourceOwnedEntrance(
                    ViewError::Binding(detail)) if detail == expected)));
            let (source, graph) = identity.get().ok_or("hostile actual entry source was not observed")?;
            Ok(EntrySession { mode: self.mode, source, graph, counts: None })
        })());
        Compilation::Stop
    }
}

fn join_entry_sessions(sessions: [EntrySession; 3]) -> Result<EntryObservation, String> {
    let [positive, foreign, rhs] = sessions;
    if (positive.mode, foreign.mode, rhs.mode) != (0, 1, 2)
        || foreign.counts.is_some() || rhs.counts.is_some()
        || positive.source != foreign.source || positive.source != rhs.source
        || positive.graph != foreign.graph || positive.graph != rhs.graph
    { return Err("actual entry sessions changed exact source, graph or check census".to_owned()); }
    let [requests, root_arguments, caller_arguments] = positive.counts
        .ok_or("positive actual entry session lacks completed requests")?;
    if requests == 0 || caller_arguments == 0
        || root_arguments.checked_add(caller_arguments) != Some(requests)
    { return Err("positive actual entry session has an incomplete request census".to_owned()); }
    Ok(EntryObservation { source: positive.source, graph: positive.graph, requests,
        root_arguments, caller_arguments, foreign_original_refused: true, same_type_rhs_refused: true })
}

#[test]
#[ignore = "process helper; requires exact real-source parent arguments"]
fn source_owned_entry_rhs_child() {
    let Some(path) = env::var_os(ARGS) else { return; };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut sessions = Vec::with_capacity(3);
    for mode in 0..3 {
        let mut callbacks = EntryCallbacks { mode, result: None };
        rustc_driver::run_compiler(&args, &mut callbacks);
        sessions.push(callbacks.result.expect("actual entry RHS rustc callback did not run"));
    }
    let result = sessions.into_iter().collect::<Result<Vec<_>, _>>()
        .and_then(|rows| join_entry_sessions(rows.try_into()
            .map_err(|_| "actual entry session count changed".to_owned())?));
    std::fs::write(env::var_os(RESULT).expect("entry RHS report path"),
        serde_json::to_vec(&result).unwrap()).unwrap();
    assert!(result.is_ok(), "actual typed entry/backend consumer: {result:?}");
}

#[test]
fn actual_entry_session_join_requires_exact_positive_negative_identity_and_census() {
    let genuine = [
        EntrySession { mode: 0, source: [3; 32], graph: [7; 32], counts: Some([3, 1, 2]) },
        EntrySession { mode: 1, source: [3; 32], graph: [7; 32], counts: None },
        EntrySession { mode: 2, source: [3; 32], graph: [7; 32], counts: None },
    ];
    let observed = join_entry_sessions(genuine.clone()).unwrap();
    assert_eq!((observed.requests, observed.root_arguments, observed.caller_arguments), (3, 1, 2));
    assert!(observed.foreign_original_refused && observed.same_type_rhs_refused);
    for fault in 0..12 {
        let mut wrong = genuine.clone();
        match fault {
            0 => wrong[0].mode = 1,
            1 => wrong[1].mode = 2,
            2 => wrong[2].mode = 1,
            3 => wrong[1].source[0] ^= 1,
            4 => wrong[2].source[0] ^= 1,
            5 => wrong[1].graph[0] ^= 1,
            6 => wrong[2].graph[0] ^= 1,
            7 => wrong[0].counts = None,
            8 => wrong[1].counts = Some([3, 1, 2]),
            9 => wrong[2].counts = Some([3, 1, 2]),
            10 => wrong[0].counts = Some([3, 3, 0]),
            11 => wrong[0].counts = Some([3, 1, 1]),
            _ => unreachable!(),
        }
        assert!(join_entry_sessions(wrong).is_err(), "session substitution {fault}");
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_rust_workgroup_helper_entry_rhs_runs_original_and_optimized_source_checks() {
    run_actual_sources::<EntryObservation>(
        &[("workgroup", "let _ = ctx.with_workgroup(|_wg| seed);"),
          ("arithmetic", "let _ = ctx.with_workgroup(|_wg| seed.wrapping_add(3));"),
          ("workgroup", "let _ = ctx.with_workgroup(|_wg| seed);")],
        &[(0, 0), (3, 2)], ENTRY_CHILD, "SOURCE_TYPED_ENTRY_RHS_V18", |body| format!(r#"
use fe2o3_device::{{kernel, KernelContext}};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn lifecycle_probe(mut ctx: KernelContext<'_>, seed: u32) {{ {body} }}
"#),
        |_, _, label, observation, previous| {
            assert!(observation.requests > 0 && observation.caller_arguments > 0);
            assert_eq!(observation.requests, observation.root_arguments + observation.caller_arguments);
            assert!(observation.foreign_original_refused && observation.same_type_rhs_refused);
            if let Some(old) = previous.get(label) { assert_eq!(old, &observation); }
            else { previous.insert(label.to_owned(), observation); }
        },
    );
}
