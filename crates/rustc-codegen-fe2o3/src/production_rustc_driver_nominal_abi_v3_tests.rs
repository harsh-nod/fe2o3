//! Real ordinary rustc callback, through nominal capture and the inert ABI entry.
use super::guarded_loop_read::Case;
use super::*;
#[path = "production_rustc_driver_nominal_loop_unroll_v3_tests.rs"]
mod native_unroll;
use crate::compiler_descriptor::nominal_v3::NominalDescriptorErrorV3 as E;
use fe2o3_kernel_descriptor::{DESCRIPTOR_QUERY_STORAGE_V3, SourceTypeDescriptorV3 as Kind};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::nominal_abi_v3::nominal_abi_source_child";
const BASE: &str = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";
const CASES: [Case; 4] = [Case::Fresh, Case::Control, Case::Stale, Case::Different];
const WORK: usize = 100_000_000;
const STORAGE: usize = 256 * 1024 * 1024;
type R<T> = std::result::Result<T, Failure>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum Phase {
    Request,
    Rustc,
    Collection,
    Factory,
    Replay,
    Rows,
    Floor,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    phase: Phase,
    detail: String,
}
fn fail(phase: Phase, error: impl std::fmt::Debug) -> Failure {
    Failure {
        phase,
        detail: format!("{error:?}"),
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
    callback_count: usize,
    args_sha256: [u8; 32],
    source: Vec<Stamp>,
    descriptor: [u8; 32],
    descriptor_bytes: usize,
    source_kinds: [u8; 3],
    component_counts: [usize; 3],
    work: usize,
    peak: usize,
    receipt: usize,
    floor: usize,
    native_authority: bool,
    artifact_authority: bool,
    replay_work: usize,
    replay_peak: usize,
    short_storage_work: usize,
    short_storage_prior_peak: usize,
    short_storage_error: String,
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
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

fn selected_feature(args: &[String]) -> R<Case> {
    require_canonical_overflow_checks_v1(args).map_err(|e| fail(Phase::Request, e))?;
    let selected: Vec<_> = args
        .iter()
        .filter_map(|arg| {
            CASES
                .into_iter()
                .find(|case| *arg == format!("--cfg=feature=\"{}\"", case.feature()))
        })
        .collect();
    if selected.len() != 1
        || args.iter().filter(|arg| arg.starts_with("--cfg")).count() != 1
        || args.iter().any(|arg| {
            arg.starts_with("feature=")
                || arg.starts_with("mir-opt-level")
                || arg.starts_with("inline-mir")
                || (arg.starts_with("-Z")
                    && !matches!(arg.as_str(), "-Zalways-encode-mir" | "-Zunstable-options"))
        })
        || ["-Zalways-encode-mir", "-Zunstable-options"]
            .into_iter()
            .any(|flag| args.iter().filter(|arg| arg.as_str() == flag).count() != 1)
    {
        return Err(fail(
            Phase::Request,
            "one exact ordinary feature and unchanged MIR controls",
        ));
    }
    Ok(selected[0])
}
fn request_case(args: &[String]) -> R<Case> {
    let case = selected_feature(args)?;
    let expected = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(format!("{BASE}/src/lib.rs"))
        .canonicalize()
        .map_err(|e| fail(Phase::Request, e))?;
    if args
        .iter()
        .filter(|arg| Path::new(arg).canonicalize().ok().as_ref() == Some(&expected))
        .count()
        != 1
    {
        return Err(fail(Phase::Request, "one exact ordinary fixture source"));
    }
    Ok(case)
}

struct SourceCallbacks {
    args: Vec<String>,
    case: Case,
    callbacks: usize,
    result: Option<R<Observation>>,
}
impl Callbacks for SourceCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.callbacks += 1;
        self.result = Some((|| {
            if self.callbacks != 1 {
                return Err(fail(Phase::Rustc, "exactly one actual callback"));
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
                return Err(fail(Phase::Request, "actual target/invocation mismatch"));
            }
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .map_err(|e| fail(Phase::Collection, e))?;
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.charge_work(17).unwrap();
            // The returned observation arrays stay paid across replay probes;
            // the rest of this inherited floor remains a nonzero sibling.
            assert!(53 > std::mem::size_of::<[u8; 3]>() + std::mem::size_of::<[usize; 3]>());
            budget.reserve_storage(53).unwrap();
            let (owner, receipt) = transaction
                .prepare_nominal_source_abi_v3(&mut budget)
                .map_err(|e| fail(Phase::Factory, e))?;
            if budget.storage() != 53
                || owner.retained_storage_floor_v1() != 53 + receipt.retained_storage()
            {
                return Err(fail(Phase::Floor, "factory floor/receipt mismatch"));
            }
            budget
                .reserve_storage(receipt.retained_storage())
                .map_err(|e| fail(Phase::Floor, e))?;
            let floor = budget.storage();
            owner
                .verify_equivalence(&mut budget)
                .map_err(|e| fail(Phase::Replay, e))?;
            let (source_kinds, component_counts) = owner.with_checked_table(&mut budget, |table, budget| {
                const CALLER: usize = std::mem::size_of::<fe2o3_kernel_descriptor::KernelDescriptorRefV3<'static, 'static>>()
                    + std::mem::size_of::<fe2o3_kernel_descriptor::ArgumentCursorV3<'static, 'static>>()
                    + std::mem::size_of::<fe2o3_kernel_descriptor::LogicalArgumentRefV3<'static, 'static>>()
                    + std::mem::size_of::<fe2o3_kernel_descriptor::SourceTypeRecordV3>()
                    + std::mem::size_of::<fe2o3_kernel_descriptor::DeviceLayoutRecordV1>()
                    + std::mem::size_of::<fe2o3_kernel_descriptor::PhysicalComponentV3>()
                    + std::mem::size_of::<[u8; 3]>()
                    + std::mem::size_of::<[usize; 3]>();
                budget.reserve_storage(DESCRIPTOR_QUERY_STORAGE_V3 + CALLER)?;
                if table.kernel_count() != 1 || table.device_target().as_amd_target_id().processor() != profile {
                    return Err(E::Mismatch("actual root/target descriptor roster"));
                }
                let kernel = table.kernel(0, &mut |w| budget.charge_work(w)).map_err(E::Wire)?;
                if kernel.entry_name() != self.case.roots()[0] || kernel.argument_count() != 3 {
                    return Err(E::Mismatch("actual fixture root/signature"));
                }
                let mut kinds = [0; 3];
                let mut counts = [0; 3];
                let mut arguments = kernel.arguments();
                for i in 0..3 {
                    let argument = arguments.next(&mut |w| budget.charge_work(w)).map_err(E::Wire)?
                        .ok_or(E::Mismatch("complete actual argument cursor"))?;
                    if argument.source_index() as usize != i { return Err(E::Mismatch("actual source ordinal")); }
                    let source = table.source_type(argument.source_type(), &mut |w| budget.charge_work(w)).map_err(E::Wire)?;
                    kinds[i] = match source.descriptor() {
                        Kind::SharedSlice(fe2o3_kernel_descriptor::ScalarTypeV1::U32) => 2,
                        Kind::Usize => 5,
                        Kind::DisjointSlice(fe2o3_kernel_descriptor::ScalarTypeV1::U32) => 3,
                        _ => return Err(E::Mismatch("genuine nominal/slice source kind")),
                    };
                    counts[i] = argument.component_count();
                    if i == 1 {
                        let layout = table.device_layout(argument.device_layout(), &mut |w| budget.charge_work(w)).map_err(E::Wire)?;
                        let component = argument.component(0, &mut |w| budget.charge_work(w)).map_err(E::Wire)?;
                        if layout.descriptor().size_bytes() != 8 || layout.descriptor().alignment_bytes() != 8
                            || component.kind != fe2o3_kernel_descriptor::PhysicalAbiComponentKind::ScalarByValue(fe2o3_kernel_descriptor::ScalarTypeV1::U64)
                            || component.offset != 16 || component.size != 8 || component.alignment != 8 {
                            return Err(E::Mismatch("nominal source and exact physical packing"));
                        }
                    }
                }
                if arguments.next(&mut |w| budget.charge_work(w)).map_err(E::Wire)?.is_some() {
                    return Err(E::Mismatch("no extra argument"));
                }
                Ok((kinds, counts))
            }).map_err(|e| fail(Phase::Rows, e))?;
            if budget.storage() != floor
                || source_kinds != [2, 5, 3]
                || component_counts != [2, 1, 2]
            {
                return Err(fail(
                    Phase::Rows,
                    "complete source shape and preserved floor",
                ));
            }
            let replay = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.charge_work(17).unwrap();
                budget.reserve_storage(floor).unwrap();
                let result = owner.verify_equivalence(&mut budget);
                assert_eq!(budget.storage(), floor);
                (
                    result,
                    budget.work(),
                    budget.peak_storage(),
                    budget.failed_storage(),
                )
            };
            let (full, replay_work, replay_peak, failed) = replay(WORK, STORAGE);
            full.map_err(|e| fail(Phase::Replay, e))?;
            assert_eq!(failed, None);
            assert!(replay_peak > floor);
            let (exact, exact_work, exact_peak, failed) = replay(replay_work, replay_peak);
            exact.map_err(|e| fail(Phase::Replay, e))?;
            assert_eq!(
                (exact_work, exact_peak, failed),
                (replay_work, replay_peak, None)
            );
            let (short, accepted, _, failed) = replay(replay_work - 1, replay_peak);
            let Err(E::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(error),
            )) = short
            else {
                return Err(fail(
                    Phase::Replay,
                    "exact final one-unit replay Work refusal",
                ));
            };
            assert_eq!(
                (error.actual(), error.limit(), accepted, failed),
                (replay_work, replay_work - 1, replay_work - 1, None)
            );
            let (short, short_storage_work, short_storage_prior_peak, failed) =
                replay(replay_work, replay_peak - 1);
            assert!(short.is_err());
            assert_eq!(failed, Some(replay_peak));
            // This is a retained observation, not the later exact phase oracle.
            let short_storage_error = format!("{short:?}");
            let result = Observation {
                case: self.case,
                profile: profile.to_owned(),
                callback_count: self.callbacks,
                args_sha256: digest(&serde_json::to_vec(&self.args).unwrap()),
                source: source_stamps(),
                descriptor: digest(owner.canonical_bytes()),
                descriptor_bytes: owner.canonical_bytes().len(),
                source_kinds,
                component_counts,
                work: budget.work(),
                peak: budget.peak_storage(),
                receipt: receipt.retained_storage(),
                floor,
                native_authority: owner.authenticates_execution(),
                artifact_authority: owner.grants_artifact_or_launch_authority(),
                replay_work,
                replay_peak,
                short_storage_work,
                short_storage_prior_peak,
                short_storage_error,
            };
            drop(owner);
            budget.release_storage(receipt.retained_storage()).unwrap();
            if budget.storage() != 53 {
                return Err(fail(Phase::Floor, "drop before receipt refund"));
            }
            Ok(result)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "genuine parent-managed rustc child, not a no-environment success"]
fn nominal_abi_source_child() {
    let args_path = PathBuf::from(env::var_os(CHILD_ARGS).expect("actual parent request"));
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(args_path).unwrap()).unwrap();
    let result = request_case(&args).and_then(|case| {
        let mut callback = SourceCallbacks {
            args: args.clone(),
            case,
            callbacks: 0,
            result: None,
        };
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rustc_driver::run_compiler(&args, &mut callback)
        })) {
            Ok(()) => callback
                .result
                .unwrap_or_else(|| Err(fail(Phase::Rustc, "callback absent"))),
            Err(_) => Err(fail(Phase::Rustc, "actual compiler panicked")),
        }
    });
    let bytes = serde_json::to_vec(&result).unwrap();
    assert!(bytes.len() <= 1024 * 1024);
    std::fs::write(
        PathBuf::from(env::var_os(CHILD_RESULT).expect("actual parent response")),
        &bytes,
    )
    .unwrap();
    println!(
        "NOMINAL_ABI_V3_RESPONSE_BEGIN bytes={} sha256={:?}",
        bytes.len(),
        digest(&bytes)
    );
    println!("{}", std::str::from_utf8(&bytes).unwrap());
    println!("NOMINAL_ABI_V3_RESPONSE_END");
    std::io::Write::flush(&mut std::io::stdout()).unwrap();
    assert!(
        result.is_ok(),
        "source refusal is not qualification: {result:?}"
    );
}

fn check(path: &Path, roots: &[&str]) {
    let result: R<Observation> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let report = result.expect("actual nominal source continuation must succeed");
    assert_eq!(report.case.roots(), roots);
    assert_eq!(report.callback_count, 1);
    assert_eq!(report.source, source_stamps());
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
    assert_eq!(report.source_kinds, [2, 5, 3]);
    assert_eq!(report.component_counts, [2, 1, 2]);
    assert_ne!(report.descriptor, [0; 32]);
    assert!(report.descriptor_bytes > 48 && report.work > 17 && report.peak >= report.floor);
    assert_eq!(report.floor, 53 + report.receipt);
    assert!(!report.native_authority && !report.artifact_authority);
    assert!(report.replay_work > 17 && report.replay_peak > report.floor);
    assert!(report.short_storage_error.starts_with("Err("));
}

#[test]
#[ignore = "requires pinned rust-src and eight actual ordinary AMD rustc children, not a protected runtime or GPU"]
fn ordinary_rust_nominal_abi_v3_reaches_inert_source_agreement() {
    for profile in [
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
        fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let cases: Vec<_> = CASES
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

fn feature_args(case: Case) -> Vec<String> {
    vec![
        "rustc".into(),
        format!("--cfg=feature=\"{}\"", case.feature()),
        "-Zalways-encode-mir".into(),
        "-Zunstable-options".into(),
        "-Coverflow-checks=on".into(),
    ]
}
#[test]
fn nominal_abi_request_accepts_exact_joined_parent_features() {
    for case in CASES {
        assert_eq!(selected_feature(&feature_args(case)).unwrap(), case);
    }
}
#[test]
fn nominal_abi_request_refuses_duplicate_split_and_changed_mir_controls() {
    let base = feature_args(Case::Fresh);
    for extra in [
        base[1].clone(),
        "--cfg".into(),
        "feature=\"guarded-loop-read\"".into(),
        "-Zmir-opt-level=0".into(),
        "-Zinline-mir=no".into(),
    ] {
        let mut args = base.clone();
        args.push(extra);
        assert!(matches!(
            selected_feature(&args),
            Err(Failure {
                phase: Phase::Request,
                ..
            })
        ));
    }
}
#[test]
fn nominal_abi_request_keeps_exact_eight_child_roster() {
    let mut names = std::collections::BTreeSet::new();
    for profile in ["gfx942", "gfx950"] {
        for case in CASES {
            assert!(names.insert((profile, case.feature(), case.roots()[0])));
        }
    }
    assert_eq!(names.len(), 8);
    assert_eq!(
        CHILD,
        "production_rustc_driver_v1::checked_output_source_v1_tests::nominal_abi_v3::nominal_abi_source_child"
    );
}
