// Actual rustc source into the owning assertion-history entry, before targets.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionCanonicalScalarAssertionDispositionV1 as HDisposition,
    ProductionCanonicalScalarAssertionPoliciesV1 as HView,
    ProductionCanonicalScalarFixedPointOwnerV1 as HOwner,
    ProductionCanonicalScalarSourceErrorV1 as HError,
};
use std::mem::size_of;

const H_REQUEST_ENV: &str = "FE2O3_TEST_CANONICAL_ASSERTION_HISTORY_REQUEST_V1";
const H_ARGS_ENV: &str = "FE2O3_TEST_CANONICAL_ASSERTION_HISTORY_ARGS_V1";
const H_RESULT_ENV: &str = "FE2O3_TEST_CANONICAL_ASSERTION_HISTORY_RESULT_V1";
const H_ROUTE: &str = "canonical-scalar-assertion-history-v1";
const H_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::integer_identity_source::canonical_assertion_source::canonical_assertion_history::canonical_assertion_history_source_child";
const H_RETAINED_CFG: &str = "fe2o3_canonical_assertion_history_retained";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum HistoryCase {
    ArithmeticHistoryElided,
    LiteralHistoryElided,
    MaskedSourceElided,
    MaskedAddRetained,
}
impl HistoryCase {
    fn root(self) -> &'static str {
        match self {
            Self::ArithmeticHistoryElided => "assertion_retained",
            Self::LiteralHistoryElided => "assertion_literal",
            Self::MaskedSourceElided => "assertion_masked",
            Self::MaskedAddRetained => "assertion_history_retained",
        }
    }
    fn cfg(self) -> &'static str {
        match self {
            Self::ArithmeticHistoryElided => CASE_CFGS[0],
            Self::LiteralHistoryElided => CASE_CFGS[1],
            Self::MaskedSourceElided => CASE_CFGS[2],
            Self::MaskedAddRetained => H_RETAINED_CFG,
        }
    }
}
include!("production_rustc_driver_canonical_assertion_history_protocol_v1_tests.rs");
include!("production_rustc_driver_canonical_assertion_history_observation_v1_tests.rs");

struct HistoryCallbacks {
    request: HistoryRequest,
    callbacks: usize,
    result: Option<Result<HistoryObservation, String>>,
}
impl Callbacks for HistoryCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.callbacks += 1;
        self.result = Some(if self.callbacks == 1 {
            history_observe(tcx, &self.request)
        } else {
            Err("duplicate compiler callback".into())
        });
        Compilation::Stop
    }
}
#[test]
#[ignore = "strict consuming subprocess helper; absent parent inputs fail"]
fn canonical_assertion_history_source_child() {
    let path = |key| PathBuf::from(env::var_os(key).expect("strict history parent paths"));
    let request: HistoryRequest = read_json(&path(H_REQUEST_ENV), INPUT_CAP).unwrap();
    let invocation: Invocation = read_json(&path(H_ARGS_ENV), INPUT_CAP).unwrap();
    let result_path = path(H_RESULT_ENV);
    assert!(!result_path.exists());
    let mut callbacks = HistoryCallbacks {
        request: request.clone(),
        callbacks: 0,
        result: None,
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        history_check_request(&request, &invocation)?;
        rustc_driver::run_compiler(&invocation.executed, &mut callbacks);
        require(callbacks.callbacks == 1, "one actual compiler callback")?;
        let row = callbacks.result.take().ok_or("missing callback")??;
        history_check_request(&request, &invocation)?;
        history_validate(&request, &row)?;
        Ok(row)
    }))
    .unwrap_or_else(|_| Err("compiler or history callback panicked".into()));
    let success = result.is_ok();
    let report = HistoryReport {
        request,
        callbacks: callbacks.callbacks,
        result,
    };
    if success {
        history_audit_protocol(&report.request, &invocation, &report);
    }
    write_json(&result_path, &report, REPORT_CAP).unwrap();
    assert!(success, "strict canonical assertion history: {report:?}");
}
fn history_fixture(case: HistoryCase, target: Target) -> corpus::Fixture {
    let mut fixture = super::fixture(AssertionCase::RetainedArithmeticOpt0, target);
    fixture.fixture_id = format!("canonical-assertion-history-{case:?}-{}", target.cpu());
    fixture.compiler_input.kernel_symbols = vec![case.root().into()];
    fixture
}
fn history_qualify(case: HistoryCase) {
    let scratch = crate::test_temp_dir::TestTempDir::create("canonical-assertion-history");
    for target in [Target::Gfx942, Target::Gfx950] {
        let directory = scratch.path().join(target.cpu());
        std::fs::create_dir(&directory).unwrap();
        let captured = corpus_cargo::capture(
            &workspace(),
            &history_fixture(case, target),
            &directory,
            &scratch.path().join("cargo-target"),
        )
        .unwrap();
        let invocation = Invocation {
            executed: history_executed(&captured.args, case).unwrap(),
            captured: captured.args,
        };
        let request = HistoryRequest {
            schema: 1,
            route: H_ROUTE.into(),
            run_id: format!("{}-{case:?}-{}", directory.display(), std::process::id()),
            case,
            target,
            captured: args_hash(&invocation.captured),
            executed: args_hash(&invocation.executed),
            cwd: captured.cwd.clone(),
            source: stamps().unwrap(),
        };
        let request_path = directory.join("request.json");
        let args_path = directory.join("invocation.json");
        let result_path = directory.join("report.json");
        write_json(&request_path, &request, INPUT_CAP).unwrap();
        write_json(&args_path, &invocation, INPUT_CAP).unwrap();
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .env_clear()
            .envs(captured.environment.iter().cloned())
            .current_dir(&captured.cwd);
        for (key, _) in &captured.environment {
            if key.to_string_lossy().starts_with("FE2O3_TEST_") {
                command.env_remove(key);
            }
        }
        command
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER");
        progress::clear_inherited_jobserver(&mut command);
        census::configure(&mut command, None);
        simulation::configure_child(&mut command, None);
        command
            .env(H_REQUEST_ENV, &request_path)
            .env(H_ARGS_ENV, &args_path)
            .env(H_RESULT_ENV, &result_path)
            .args(["--exact", H_CHILD, "--ignored", "--nocapture"]);
        let output = command.output().unwrap();
        let bytes = std::fs::File::open(&result_path).ok().map(|file| {
            let mut bytes = Vec::new();
            file.take(REPORT_CAP as u64 + 1)
                .read_to_end(&mut bytes)
                .unwrap();
            bytes
        });
        let observed = history_decode(output.status.code(), bytes.as_deref(), &request)
            .unwrap_or_else(|error| {
                panic!(
                    "assertion history {case:?}/{}: {error}\n{}",
                    target.cpu(),
                    corpus_cargo::diagnostics(&output),
                )
            });
        assert_eq!(request.source, stamps().unwrap());
        println!(
            "CANONICAL_ASSERTION_HISTORY {} {case:?} callbacks=1 rounds={} assertions={} definitions={} pending=19",
            target.cpu(),
            observed.rounds.len(),
            observed.rows.assertions.len(),
            observed.rows.definitions.len(),
        );
    }
}
#[test]
#[ignore = "strict real rustc assertion history, both collection profiles"]
fn ordinary_rust_canonical_assertion_history_arithmetic_elision_both_profiles() {
    history_qualify(HistoryCase::ArithmeticHistoryElided);
}
#[test]
#[ignore = "strict real rustc assertion history, both collection profiles"]
fn ordinary_rust_canonical_assertion_history_literal_selection_both_profiles() {
    history_qualify(HistoryCase::LiteralHistoryElided);
}
#[test]
#[ignore = "strict real rustc assertion history, both collection profiles"]
fn ordinary_rust_canonical_assertion_history_masked_source_elision_both_profiles() {
    history_qualify(HistoryCase::MaskedSourceElided);
}
#[test]
#[ignore = "strict real rustc assertion history, both collection profiles"]
fn ordinary_rust_canonical_assertion_history_masked_add_retained_both_profiles() {
    history_qualify(HistoryCase::MaskedAddRetained);
}

// Literal field census: JSON punctuation, every key, fixed scalar spellings and
// at most six escaped bytes per input string byte. No measured report baseline.
fn wire_array(count: usize, element: usize) -> Result<usize, String> {
    add(2, mul(count, add(element, 1)?)?)
}
fn wire_object(keys: &[&str], values: &[usize]) -> Result<usize, String> {
    require(keys.len() == values.len(), "literal JSON field census")?;
    keys.iter().zip(values).try_fold(2usize, |n, (key, value)| {
        add(n, add(add(key.len(), 4)?, *value)?)
    })
}
fn wire_text(text: &str) -> Result<usize, String> {
    add(2, mul(6, text.len())?)
}
fn wire_texts(texts: &[String]) -> Result<usize, String> {
    texts
        .iter()
        .try_fold(2, |n, s| add(n, add(1, wire_text(s)?)?))
}
fn history_wire_bound(row: &HistoryObservation, budget: &mut Budget<'_>) -> Result<usize, String> {
    budget
        .charge_work(add(
            512,
            add(
                row.roots.len(),
                add(
                    row.original.symbols.len(),
                    add(
                        row.original.kernels.len(),
                        add(row.final_graph.symbols.len(), row.final_graph.kernels.len())?,
                    )?,
                )?,
            )?,
        )?)
        .map_err(|e| e.to_string())?;
    // Binary digit count also bounds decimal digits without assuming usize width.
    let u = usize::BITS.max(64) as usize;
    let a2 = wire_array(2, u)?;
    let a3 = wire_array(3, u)?;
    let a4 = wire_array(4, u)?;
    let a5 = wire_array(5, u)?;
    let digest = wire_array(32, 3)?;
    let subject = wire_object(&["digest", "bytes"], &[digest, u])?;
    let origin = wire_object(&["file", "bytes", "start", "end"], &[digest, a2, a2, a2])?;
    let edge = wire_object(
        &["coordinate", "target", "target_id", "payload"],
        &[a3, a2, u, a2],
    )?;
    let assertion = wire_object(
        &[
            "span",
            "association",
            "source",
            "helper",
            "origin",
            "expansion",
            "expected",
            "message",
            "signed_literal",
            "semantic_success",
            "condition",
            "definition",
            "success",
            "failure",
            "proof",
        ],
        &[u, u, a3, 5, origin, origin, 5, 64, 5, u, a3, a5, a3, a3, 64],
    )?;
    let place = wire_object(&["Retained"], &[a3])?
        .max(wire_object(&["Internal"], &[a3])?)
        .max(9);
    let hist_assertion = wire_object(
        &[
            "original",
            "disposition",
            "condition",
            "definition",
            "failure",
            "success",
            "selection",
            "removal",
        ],
        &[assertion, u, a3, a5, a3, place, a2, a2],
    )?;
    let span = wire_object(
        &["association", "block", "kind", "source", "operations"],
        &[u, a2, u, a4, a2],
    )?;
    let operation = wire_object(
        &["output", "original", "synthesis"],
        &[a3, a3, add(5, add(u, add(5, a5)?)?)?],
    )?;
    let descendant = wire_object(&["original", "output", "retained"], &[a5, a5, 5])?;
    let segment = wire_object(&["output", "original", "connector"], &[a2, a2, a3])?;
    let block = wire_object(&["original", "placement", "reachable"], &[a2, a3, 5])?;
    let control = wire_object(&["original", "placement", "executable"], &[a3, place, 5])?;
    let pair = wire_object(&["call", "declaration", "incoming"], &[a3, u, a2])?;
    let incoming = wire_object(
        &["condition", "definition", "success", "failure", "expected"],
        &[a3, a5, a3, a3, 5],
    )?;
    let definition = wire_object(
        &[
            "ordinal",
            "function",
            "history_function",
            "stages",
            "paired",
            "clean",
            "floor",
            "invocation",
            "denied",
            "panicked",
        ],
        &[u, u, u, wire_array(9, u)?, u, 5, a3, a3, 5, 5],
    )?;
    let round = wire_object(
        &[
            "ordinal",
            "input",
            "integer",
            "output",
            "changed",
            "integer_passes",
            "scalar_passes",
            "maps",
            "executions",
            "occurrences",
        ],
        &[
            u,
            subject,
            subject,
            subject,
            5,
            wire_array(2, u)?,
            wire_array(8, u)?,
            wire_array(2, digest)?,
            wire_array(2, digest)?,
            wire_array(2, wire_array(9, u)?)?,
        ],
    )?;
    let graph = |g: &GraphRow| {
        wire_object(
            &[
                "subject",
                "edges",
                "payloads",
                "definitions",
                "symbols",
                "kernels",
                "counts",
                "memory",
            ],
            &[
                subject,
                wire_array(g.edges.len(), edge)?,
                wire_array(g.payloads.len(), u)?,
                wire_array(g.definitions.len(), u)?,
                wire_texts(&g.symbols)?,
                wire_texts(&g.kernels)?,
                wire_array(6, u)?,
                a3,
            ],
        )
    };
    let r = &row.rows;
    let rows = wire_object(
        &[
            "original",
            "assertions",
            "spans",
            "functions",
            "operations",
            "descendants",
            "segments",
            "blocks",
            "controls",
            "uses",
            "edges",
            "arguments",
            "pairs",
            "incoming",
            "definitions",
            "final_edges",
            "final_payloads",
        ],
        &[
            wire_array(r.original.len(), assertion)?,
            wire_array(r.assertions.len(), hist_assertion)?,
            wire_array(r.spans.len(), span)?,
            wire_array(r.functions.len(), a2)?,
            wire_array(r.operations.len(), operation)?,
            wire_array(r.descendants.len(), descendant)?,
            wire_array(r.segments.len(), segment)?,
            wire_array(r.blocks.len(), block)?,
            wire_array(r.controls.len(), control)?,
            wire_array(r.uses.len(), wire_array(3, a5)?)?,
            wire_array(r.edges.len(), wire_array(2, a3)?)?,
            wire_array(r.arguments.len(), wire_array(2, a4)?)?,
            wire_array(r.pairs.len(), pair)?,
            wire_array(r.incoming.len(), incoming)?,
            wire_array(r.definitions.len(), definition)?,
            wire_array(r.final_edges.len(), edge)?,
            wire_array(r.final_payloads.len(), u)?,
        ],
    )?;
    let sizes = wire_object(
        &[
            "assertions",
            "spans",
            "associations",
            "functions",
            "operations",
            "descendants",
            "segments",
            "blocks",
            "controls",
            "uses",
            "edges",
            "arguments",
            "pairs",
            "incoming",
            "definitions",
        ],
        &[u; 15],
    )?;
    let roots = row.roots.iter().try_fold(2, |n, root| {
        add(
            n,
            add(
                1,
                wire_object(
                    &["name", "function", "body", "entry"],
                    &[
                        wire_text(&root.name)?,
                        digest,
                        digest,
                        wire_text(&root.entry)?,
                    ],
                )?,
            )?,
        )
    })?;
    wire_object(
        &[
            "actual_target",
            "fixture",
            "roots",
            "original",
            "after",
            "final_graph",
            "policy_subject",
            "rounds",
            "rows",
            "sizes",
            "semantic",
            "source_assertions",
            "owner_floors",
            "diagnostics_floor",
            "restored_floor",
            "source_resources",
            "policy_resources",
            "no_denial",
            "pending",
            "complete",
            "authority",
            "final_source_storage",
            "protocol_storage",
            "work",
        ],
        &[
            64,
            origin,
            roots,
            graph(&row.original)?,
            subject,
            graph(&row.final_graph)?,
            subject,
            wire_array(row.rounds.len(), round)?,
            rows,
            sizes,
            a3,
            u,
            a3,
            u,
            u,
            a3,
            a3,
            5,
            u,
            5,
            5,
            u,
            u,
            u,
        ],
    )
}
