//! Ordinary rustc source through ranked checks and Direct Policy4, without publication.
use super::*;
use fe2o3_kernel_ir as kir;

#[path = "production_rustc_driver_guarded_loop_read_rows_v1_tests.rs"]
mod rows;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::guarded_loop_read::guarded_loop_read_source_child";
const BASE: &str = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";
const MAX_RESPONSE_JSON_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Case {
    Fresh,
    Control,
    Stale,
    Different,
}

impl Case {
    const ALL: [Self; 4] = [Self::Fresh, Self::Control, Self::Stale, Self::Different];
    pub(super) fn feature(self) -> &'static str {
        match self {
            Self::Fresh => "guarded-loop-read",
            Self::Control => "guarded-loop-read-control",
            Self::Stale => "guarded-loop-read-stale",
            Self::Different => "guarded-loop-read-different",
        }
    }
    pub(super) fn roots(self) -> &'static [&'static str] {
        match self {
            Self::Fresh => &["guarded_loop_read"],
            Self::Control => &["guarded_loop_read_control"],
            Self::Stale => &["guarded_loop_read_stale"],
            Self::Different => &["guarded_loop_read_different"],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Phase {
    Request,
    Rustc,
    Collection,
    Ranked,
    Policy4,
    Rows,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Mismatch {
    Callback,
    Route,
    Owner,
    Roster,
    Incomplete,
    Load,
    Pointer,
    GuardIndex,
    GuardLength,
    GuardEdge,
    Phi,
    Update,
    Assert,
    NegativeGuard,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    phase: Phase,
    mismatch: Option<Mismatch>,
    detail: String,
}
type Result<T> = std::result::Result<T, Failure>;

fn fail(phase: Phase, detail: impl std::fmt::Debug) -> Failure {
    Failure {
        phase,
        mismatch: None,
        detail: format!("{detail:?}"),
    }
}
fn mismatch(kind: Mismatch, detail: impl std::fmt::Debug) -> Failure {
    Failure {
        phase: Phase::Rows,
        mismatch: Some(kind),
        detail: format!("{detail:?}"),
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stamp {
    path: String,
    sha256: [u8; 32],
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    case: Case,
    profile: String,
    args_sha256: [u8; 32],
    source: Vec<Stamp>,
    callback_count: usize,
    semantic: [u8; 32],
    input: [u8; 32],
    output: [u8; 32],
    source_rows: rows::GraphRows,
    output_rows: rows::GraphRows,
    assert_query_work: usize,
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub(super) fn check_nominal_output(
    case: Case,
    source: Option<&fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1>,
    output: &kir::VerifiedCanonicalKernelIrModuleV12,
    obligations: &[kir::FormalMemoryObligations],
    floor: usize,
) -> std::result::Result<(), String> {
    if let Some(source) = source {
        let (observed, _) = rows::source_rows(case, source, floor).map_err(|e| format!("{e:?}"))?;
        rows::check_summary(case, &observed);
        rows::check_source_successes(case, &observed);
    } else {
        assert_eq!(case, Case::Control);
    }
    let observed =
        rows::graph_rows(case, output.module(), obligations).map_err(|e| format!("{e:?}"))?;
    rows::check_summary(case, &observed);
    Ok(())
}
fn source_stamps() -> Vec<Stamp> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    [
        "Cargo.lock".to_owned(),
        format!("{BASE}/Cargo.toml"),
        format!("{BASE}/src/lib.rs"),
        format!("{BASE}/src/guarded_loop_read.rs"),
    ]
    .into_iter()
    .map(|path| Stamp {
        sha256: digest(&std::fs::read(workspace.join(&path)).unwrap()),
        path,
    })
    .collect()
}

fn request_case(args: &[String]) -> Result<Case> {
    require_canonical_overflow_checks_v1(args).map_err(|e| fail(Phase::Request, e))?;
    let selected: Vec<_> = args
        .iter()
        .filter_map(|arg| {
            Case::ALL
                .into_iter()
                .find(|case| *arg == format!("--cfg=feature=\"{}\"", case.feature()))
        })
        .collect();
    if selected.len() != 1
        || args.iter().filter(|arg| arg.starts_with("--cfg")).count() != 1
        || args.iter().any(|arg| {
            arg.starts_with("feature=")
                || (arg.starts_with("-Z")
                    && !matches!(arg.as_str(), "-Zalways-encode-mir" | "-Zunstable-options"))
                || arg.starts_with("mir-opt-level")
                || arg.starts_with("inline-mir")
        })
        || ["-Zalways-encode-mir", "-Zunstable-options"]
            .into_iter()
            .any(|expected| args.iter().filter(|arg| arg.as_str() == expected).count() != 1)
    {
        return Err(fail(
            Phase::Request,
            "one exact source feature and unchanged MIR options",
        ));
    }
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = workspace
        .join(format!("{BASE}/src/lib.rs"))
        .canonicalize()
        .map_err(|e| fail(Phase::Request, e))?;
    if args
        .iter()
        .filter(|arg| Path::new(arg).canonicalize().ok().as_ref() == Some(&source))
        .count()
        != 1
    {
        return Err(fail(
            Phase::Request,
            "exact ordinary fixture source argument",
        ));
    }
    Ok(selected[0])
}

fn response_json(result: &Result<Observation>) -> Vec<u8> {
    let bytes = serde_json::to_vec(result).unwrap();
    assert!(
        bytes.len() <= MAX_RESPONSE_JSON_BYTES,
        "ordinary-source response exceeds log bound: {} bytes, digest {:?}",
        bytes.len(),
        digest(&bytes)
    );
    bytes
}

struct SourceCallbacks {
    case: Case,
    args: Vec<String>,
    callbacks: usize,
    result: Option<Result<Observation>>,
}

impl Callbacks for SourceCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.callbacks += 1;
        self.result = Some((|| {
            if self.callbacks != 1 {
                return Err(mismatch(Mismatch::Callback, "duplicate actual callback"));
            }
            let profile = tcx
                .sess
                .opts
                .cg
                .target_cpu
                .as_deref()
                .unwrap_or(tcx.sess.target.cpu.as_ref());
            if !["gfx942", "gfx950"].contains(&profile)
                || self
                    .args
                    .iter()
                    .filter(|arg| **arg == format!("-Ctarget-cpu={profile}"))
                    .count()
                    != 1
            {
                return Err(fail(
                    Phase::Request,
                    "actual target and exact invocation disagree",
                ));
            }
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .map_err(|e| fail(Phase::Collection, e))?;
            let ranked = transaction
                .verify_general_kernel_checks()
                .map_err(|e| fail(Phase::Ranked, e))?;
            if !ranked.all_kernel_checks_are_clean() || ranked.grants_artifact_or_launch_authority()
            {
                return Err(mismatch(
                    Mismatch::Incomplete,
                    "actual ranked checks or authority",
                ));
            }
            let stage = dispatch::Stage::lower(ranked).map_err(|e| fail(Phase::Policy4, e))?;
            let dispatch::Stage::Direct(direct) = &stage else {
                return Err(mismatch(
                    Mismatch::Route,
                    "ordinary fixture must use actual Direct owner",
                ));
            };
            if !std::ptr::eq(stage.output(), stage.checked_output().owner())
                || direct.output().grants_artifact_or_launch_authority()
            {
                return Err(mismatch(
                    Mismatch::Owner,
                    "actual output owner or authority",
                ));
            }
            let source = direct.output().source_semantic_kir();
            let pre = source
                .pre_ranked_executable()
                .ok_or_else(|| mismatch(Mismatch::Owner, "no genuine connected executable"))?;
            if !std::ptr::eq(pre.module(), source.module()) {
                return Err(mismatch(Mismatch::Owner, "connected source graph changed"));
            }
            let (source_rows, assert_query_work) =
                rows::source_rows(self.case, source, stage.retained_storage_floor_v1())?;
            let output_rows =
                rows::graph_rows(self.case, stage.output().module(), stage.kernels())?;
            Ok(Observation {
                case: self.case,
                profile: profile.to_owned(),
                args_sha256: digest(&serde_json::to_vec(&self.args).unwrap()),
                source: source_stamps(),
                callback_count: self.callbacks,
                semantic: *source.semantic().semantic().semantic_sha256().as_bytes(),
                input: *pre.canonical().identity().digest(),
                output: *stage.output().canonical().identity().digest(),
                source_rows,
                output_rows,
                assert_query_work,
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "actual rustc subprocess helper; its parent request is mandatory"]
fn guarded_loop_read_source_child() {
    let request = env::var_os(CHILD_ARGS).expect("ordinary-source invocation required");
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(request).unwrap()).unwrap();
    let result = match request_case(&args) {
        Err(error) => Err(error),
        Ok(case) => {
            let mut callbacks = SourceCallbacks {
                case,
                args: args.clone(),
                callbacks: 0,
                result: None,
            };
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                rustc_driver::run_compiler(&args, &mut callbacks)
            })) {
                Ok(()) => callbacks
                    .result
                    .unwrap_or_else(|| Err(fail(Phase::Rustc, "callback absent"))),
                Err(_) => Err(fail(Phase::Rustc, "actual compiler invocation panicked")),
            }
        }
    };
    let response = PathBuf::from(env::var_os(CHILD_RESULT).expect("ordinary-source result path"));
    let bytes = response_json(&result);
    std::fs::write(&response, &bytes).unwrap();
    println!(
        "GUARDED_LOOP_READ_RESPONSE_BEGIN bytes={} sha256={:?}",
        bytes.len(),
        digest(&bytes)
    );
    println!("{}", std::str::from_utf8(&bytes).unwrap());
    println!("GUARDED_LOOP_READ_RESPONSE_END");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    assert!(
        result.is_ok(),
        "actual source refusal is not source coverage: {result:?}"
    );
    println!("{result:?}");
}

fn check(path: &Path, roots: &[&str]) {
    let report: Result<Observation> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let report =
        report.expect("genuine source callback must succeed; runtime absence is not success");
    assert_eq!(report.case.roots(), roots);
    assert_eq!(report.source, source_stamps());
    assert_eq!(report.callback_count, 1);
    let args_path = path.with_file_name(format!("{}-args.json", report.case.feature()));
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(args_path).unwrap()).unwrap();
    assert_eq!(request_case(&args).unwrap(), report.case);
    assert_eq!(
        report.args_sha256,
        digest(&serde_json::to_vec(&args).unwrap())
    );
    assert_eq!(
        args.iter()
            .filter(|arg| **arg == format!("-Ctarget-cpu={}", report.profile))
            .count(),
        1
    );
    for hash in [report.semantic, report.input, report.output] {
        assert_ne!(hash, [0; 32]);
    }
    rows::check_summary(report.case, &report.source_rows);
    rows::check_summary(report.case, &report.output_rows);
    rows::check_source_successes(report.case, &report.source_rows);
    if report.case != Case::Control {
        assert!(report.assert_query_work > 0);
    }
}

#[test]
#[ignore = "requires pinned nightly rust-src and real ordinary AMD source imports; no protected runtime or GPU"]
fn ordinary_rust_guarded_changing_index_reads_reach_direct_policy4() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let cases: Vec<_> = Case::ALL
            .into_iter()
            .map(OrdinarySourceCase::GuardedLoopRead)
            .collect();
        ordinary_rust_source_cases(
            &cases,
            profile,
            false,
            Some(SourceObserver {
                child_test: CHILD,
                check,
            }),
        );
    }
}
