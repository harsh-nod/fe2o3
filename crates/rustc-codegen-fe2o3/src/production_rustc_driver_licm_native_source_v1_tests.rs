//! Ordinary Cargo/rustc through the real owning LICM native route.
//! Missing source admission or the admitted reference runtime is never success.
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    VerifiedCanonicalKernelIrModuleV12 as Graph,
};

#[path = "production_rustc_driver_licm_native_context_v1_tests.rs"]
mod context;
#[path = "production_rustc_driver_licm_native_pairs_v1_tests.rs"]
mod pairs;
#[path = "production_rustc_driver_licm_native_protocol_v1_tests.rs"]
mod protocol;
#[path = "production_rustc_driver_licm_native_sim_v1_tests.rs"]
mod sim;
use protocol::*;

#[path = "production_rustc_driver_refined_forwarding_source_v1_tests.rs"]
mod refined_forwarding;

const REQUEST: &str = "FE2O3_TEST_LICM_NATIVE_SOURCE_V1";
const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::licm_native_source::licm_native_source_child";
const BASE: &str = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn profile(target: &str) -> Result<Profile, String> {
    match target {
        "gfx942" => Ok(Profile::Gfx942),
        "gfx950" => Ok(Profile::Gfx950),
        _ => Err("closed source LICM target".into()),
    }
}
fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn source_stamps() -> Vec<(String, [u8; 32])> {
    [
        "Cargo.lock".into(),
        format!("{BASE}/Cargo.toml"),
        format!("{BASE}/src/lib.rs"),
        format!("{BASE}/src/licm_native.rs"),
    ]
    .into_iter()
    .map(|name: String| {
        let hash = digest(&std::fs::read(workspace().join(&name)).unwrap());
        (name, hash)
    })
    .collect()
}
fn work_limit() -> usize {
    crate::production_canonical_phase_policy_v1::WORK_LIMIT
        .try_into()
        .unwrap()
}
fn storage_limit() -> usize {
    crate::production_canonical_phase_policy_v1::STORAGE_LIMIT
}

struct SourceCallbacks {
    request: RequestRecord,
    calls: usize,
    result: Option<Result<Report, String>>,
}
impl Callbacks for SourceCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some((|| {
            if self.calls != 1 {
                return Err("exactly one genuine source callback".into());
            }
            let ranked = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?
            .verify_general_kernel_checks()
            .map_err(|e| format!("actual source/ranked admission prerequisite: {e:?}"))?;
            let route = ranked.checked_output_source_policy_v1();
            let expected = if self.request.subject.case.unit_local() {
                fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::UnitLocal
            } else {
                fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::RawEmpty
            };
            if route != expected {
                return Err(format!(
                    "genuine source route {route:?}, expected {expected:?}"
                ));
            }
            let baseline = self.request.baseline.as_ref();
            let w = match self.request.purpose {
                Purpose::Exact => baseline.unwrap().work,
                Purpose::WorkShort => baseline.unwrap().work - 1,
                _ => work_limit(),
            };
            let p = match self.request.purpose {
                Purpose::Exact | Purpose::WorkShort => baseline.unwrap().peak,
                Purpose::StorageShort => baseline.unwrap().peak - 1,
                _ => storage_limit(),
            };
            let sibling = vec![0x5bu8; 43];
            let sibling_bytes = std::mem::size_of_val(&sibling) + sibling.capacity();
            let mut foreign_work = Work::new(work_limit());
            let mut work = Work::new(w);
            let (mut report, failed_storage) = {
                let mut budget = Budget::new(&mut work, p);
                budget.charge_work(7).map_err(|e| e.to_string())?;
                budget
                    .reserve_storage(37 + sibling_bytes)
                    .map_err(|e| e.to_string())?;
                let floor = budget.storage();
                let ledger = budget.work_ledger_identity_v1();
                if self.request.purpose == Purpose::Exact {
                    assert!(budget.charge_work(w + 1 - budget.work()).is_err());
                    assert!(budget.reserve_storage(p + 1 - budget.storage()).is_err());
                }
                let result = ranked.lower_licm_native_with_budget_v1(&mut budget);
                let prepared_work = budget.work();
                let prepared_peak = budget.peak_storage();
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
                let mut report = Report {
                    request: self.request.clone(),
                    callback_count: self.calls,
                    measurement: None,
                    denial: None,
                    accepted_work: prepared_work,
                    accepted_peak: prepared_peak,
                    final_storage: floor,
                    failed_work: None,
                    failed_storage: None,
                    replay_work: 0,
                    hostile_replays: 0,
                    simulation: None,
                };
                match result {
                    Ok((mut owner, receipt)) => {
                        if matches!(
                            self.request.purpose,
                            Purpose::WorkShort | Purpose::StorageShort
                        ) {
                            drop(owner);
                            return Err("short native construction unexpectedly succeeded".into());
                        }
                        budget
                            .reserve_storage(receipt.retained_storage())
                            .map_err(|e| e.to_string())?;
                        assert_eq!(owner.retained_storage_floor_v1(), budget.storage());
                        assert!(!owner.grants_artifact_or_launch_authority());
                        let measurement = owner.with_source_test_observation_v1(|view| {
                            assert_eq!(view.unit_local(), self.request.subject.case.unit_local());
                            assert_eq!(
                                view.profile(),
                                profile(&self.request.subject.target).unwrap()
                            );
                            assert_eq!(view.floor(), budget.storage());
                            assert_eq!(view.source_identity(), view.preflight_identity());
                            let input = sim::Input {
                                original: view.original().unwrap(),
                                historical: view.historical_p8(),
                                promoted: view.promoted(),
                                input: view.input(),
                                output: view.output(),
                                origins: view.origins(),
                                launch: view.source_launch(),
                                semantic: view.source_semantic(),
                                kernels: view.kernels(),
                                profile: view.profile(),
                            };
                            let hoists = sim::shape(&input, self.request.subject.case);
                            assert_eq!(
                                view.incoming().len(),
                                view.preheaders()
                                    .iter()
                                    .map(|row| row.incoming().len())
                                    .sum::<usize>()
                            );
                            assert_eq!(
                                view.parameters().len(),
                                view.preheaders()
                                    .iter()
                                    .map(|row| row.parameters().len())
                                    .sum::<usize>()
                            );
                            let deleted = view.deleted_helpers();
                            Measurement {
                                subject: self.request.subject.identity(),
                                work: prepared_work,
                                peak: prepared_peak,
                                floor,
                                retained: receipt.retained_storage(),
                                llvm_bytes: owner.llvm_ir().len(),
                                llvm_sha256: digest(owner.llvm_ir().as_bytes()),
                                original: *view.original().unwrap().canonical().identity().digest(),
                                historical_p8: *view
                                    .historical_p8()
                                    .canonical()
                                    .identity()
                                    .digest(),
                                promoted: *view.promoted().canonical().identity().digest(),
                                input: *view.input().canonical().identity().digest(),
                                output: *view.output().canonical().identity().digest(),
                                source: view.source_identity(),
                                preflight: view.preflight_identity(),
                                ranked: view.ranked_identity(),
                                execution: digest(view.execution_bytes()),
                                hoists,
                                deleted_helpers: [deleted.0, deleted.1],
                            }
                        });
                        measurement.check(&self.request.subject)?;
                        report.measurement = Some(measurement);
                        if self.request.purpose == Purpose::Baseline {
                            let start = budget.work();
                            owner
                                .verify_equivalence(&mut budget)
                                .map_err(|e| format!("actual native replay: {e:?}"))?;
                            report.replay_work = budget.work() - start;
                            let scratch = dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES
                                .checked_mul(3)
                                .unwrap();
                            budget.reserve_storage(scratch).map_err(|e| e.to_string())?;
                            let native=match profile(&self.request.subject.target)? {
                                Profile::Gfx942=>dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner.output()),
                                Profile::Gfx950=>dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(owner.output()),
                            }.map_err(|e|e.to_string())?;
                            let expected =
                                dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&native)
                                    .map_err(|e| e.to_string())?;
                            assert_eq!(expected, owner.llvm_ir());
                            assert!(!owner.llvm_ir().contains(".fe2o3.kd.v1"));
                            drop((expected, native));
                            budget.release_storage(scratch).unwrap();
                            report.simulation =
                                Some(owner.with_source_test_observation_v1(|view| {
                                    sim::observe(
                                        &sim::Input {
                                            original: view.original().unwrap(),
                                            historical: view.historical_p8(),
                                            promoted: view.promoted(),
                                            input: view.input(),
                                            output: view.output(),
                                            origins: view.origins(),
                                            launch: view.source_launch(),
                                            semantic: view.source_semantic(),
                                            kernels: view.kernels(),
                                            profile: view.profile(),
                                        },
                                        self.request.subject.case,
                                    )
                                }));
                            owner.source_test_bad_llvm_v1(&mut budget).unwrap_err();
                            owner.verify_equivalence(&mut budget).unwrap();
                            owner.source_test_wrong_target_v1(&mut budget).unwrap_err();
                            owner.verify_equivalence(&mut budget).unwrap();
                            owner.source_test_missing_floor_v1(&mut budget).unwrap_err();
                            owner.verify_equivalence(&mut budget).unwrap();
                            let mut foreign = Budget::new(&mut foreign_work, storage_limit());
                            foreign.reserve_storage(budget.storage()).unwrap();
                            owner
                                .source_test_foreign_ledger_v1(&mut budget, &mut foreign)
                                .unwrap_err();
                            owner.verify_equivalence(&mut budget).unwrap();
                            assert_eq!(budget.storage(), floor + receipt.retained_storage());
                            assert!(budget.work_ledger_identity_v1() == ledger);
                            owner
                                .source_test_consume_failure_v1(
                                    &mut budget,
                                    !self.request.subject.case.motion(),
                                )
                                .unwrap_err();
                            report.hostile_replays = 5;
                        } else {
                            drop(owner);
                        }
                        budget.release_storage(receipt.retained_storage()).unwrap();
                    }
                    Err(error) => match self.request.purpose {
                        Purpose::WorkShort => {
                            let baseline = baseline.unwrap();
                            error.source_test_assert_licm_work_denial_v1(baseline.work, w);
                            report.denial = Some(Denial::Work {
                                actual: baseline.work,
                                limit: w,
                            });
                        }
                        Purpose::StorageShort => {
                            let baseline = baseline.unwrap();
                            assert_eq!(budget.failed_storage(), Some(baseline.peak));
                            let nested_error = format!("{error:?}");
                            if nested_error.len() > 8192 {
                                return Err("bounded diagnostic Storage record".into());
                            }
                            report.denial = Some(Denial::StorageObservation {
                                actual: baseline.peak,
                                limit: p,
                                nested_error,
                            });
                        }
                        _ => {
                            return Err(format!(
                                "actual source/native construction prerequisite: {error:?}"
                            ));
                        }
                    },
                }
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert!(sibling.iter().all(|byte| *byte == 0x5b));
                let failed_storage = budget.failed_storage();
                drop(sibling);
                budget.release_storage(sibling_bytes).unwrap();
                assert_eq!(budget.storage(), 37);
                budget.release_storage(37).unwrap();
                (report, failed_storage)
            };
            report.failed_work = work.failed_work();
            report.failed_storage = failed_storage;
            report.check(&self.request)?;
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess helper requires exact actual Cargo invocation and real admitted reference runtime"]
fn licm_native_source_child() {
    let args: Vec<String> = serde_json::from_slice(
        &std::fs::read(env::var_os(CHILD_ARGS).expect("parent argv")).unwrap(),
    )
    .unwrap();
    let request: RequestRecord =
        serde_json::from_str(&env::var(REQUEST).expect("parent request")).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        request.check(&args)?;
        let mut callbacks = SourceCallbacks {
            request: request.clone(),
            calls: 0,
            result: None,
        };
        rustc_driver::run_compiler(&args, &mut callbacks);
        request.check(&args)?;
        if callbacks.calls != 1 {
            return Err("exact source callback completion".into());
        }
        callbacks
            .result
            .ok_or("missing actual native callback result")?
    }))
    .unwrap_or_else(|_| Err("actual source/runtime/native callback panicked".into()));
    let success = result.is_ok();
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(env::var_os(CHILD_RESULT).expect("parent result"))
        .unwrap()
        .write_all(&serde_json::to_vec(&result).unwrap())
        .unwrap();
    assert!(success, "genuine ordinary Rust native LICM: {result:?}");
}

fn fixture(case: Case, target: &str) -> corpus::Fixture {
    let hash = |path: &str| {
        digest(&std::fs::read(workspace().join(path)).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let manifest = format!("{BASE}/Cargo.toml");
    corpus::Fixture {
        fixture_id: format!("{target}-{}", case.feature()),
        target: target.into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hash(&manifest),
            package_manifest: manifest,
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hash("Cargo.lock"),
            source_paths: vec![
                format!("{BASE}/src/lib.rs"),
                format!("{BASE}/src/licm_native.rs"),
            ],
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features: vec![case.feature().into()],
            kernel_symbols: vec!["licm_native".into()],
        },
    }
}
fn child(
    captured: &corpus_cargo::Captured,
    request: &RequestRecord,
    directory: &Path,
    output: &mut impl std::io::Write,
) -> (Report, Completed) {
    let args_path = directory.join(format!("args-{}.json", request.ordinal));
    let result_path = directory.join(format!("result-{}.json", request.ordinal));
    std::fs::write(&args_path, serde_json::to_vec(&captured.args).unwrap()).unwrap();
    let mut command = Command::new(env::current_exe().unwrap());
    command
        .env_clear()
        .envs(captured.environment.iter().cloned())
        .current_dir(&captured.cwd)
        .env_remove("RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove(CHILD_PROOF_PROBE)
        .env(CHILD_ARGS, &args_path)
        .env(CHILD_RESULT, &result_path)
        .env(REQUEST, serde_json::to_string(request).unwrap())
        .args(["--exact", CHILD, "--ignored", "--nocapture"]);
    progress::clear_inherited_jobserver(&mut command);
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "actual source child: {}",
        corpus_cargo::diagnostics(&result)
    );
    let stdout = std::str::from_utf8(&result.stdout).expect("genuine libtest stdout");
    assert_eq!(
        stdout
            .matches("test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured;")
            .count(),
        1
    );
    assert_eq!(stdout.matches(&format!("test {CHILD} ... ok")).count(), 1);
    let report: Result<Report, String> =
        serde_json::from_slice(&std::fs::read(&result_path).unwrap()).unwrap();
    let report = report.unwrap();
    report.check(request).unwrap();
    assert_eq!(source_stamps(), request.subject.sources);
    assert_eq!(
        digest(&std::fs::read(env::current_exe().unwrap()).unwrap()),
        request.subject.test_binary_sha256
    );
    assert_eq!(
        digest(&std::fs::read(&captured.args[0]).unwrap()),
        request.subject.rustc_sha256
    );
    let completion =
        write_observation(output, request, &report, &result.stdout, &result.stderr).unwrap();
    (report, completion)
}
fn run_parent(resource: bool) {
    let root = workspace();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-ordinary-licm-native");
    let mut completions = Vec::new();
    let stderr = std::io::stderr();
    let mut output = stderr.lock();
    for target in ["gfx942", "gfx950"] {
        for case in Case::ALL {
            let fixture = fixture(case, target);
            let directory = scratch.path().join(&fixture.fixture_id);
            std::fs::create_dir(&directory).unwrap();
            let mut captured =
                corpus_cargo::capture(&root, &fixture, &directory, &scratch.path().join(target))
                    .unwrap();
            captured.args.push("-Zmir-opt-level=0".into());
            captured.args.push("-Zinline-mir=no".into());
            let subject = Subject {
                case,
                target: target.into(),
                args_sha256: digest(&serde_json::to_vec(&captured.args).unwrap()),
                cfg: captured.cfg.clone(),
                environment_sha256: environment_digest(captured.environment.clone()).unwrap(),
                cwd: captured.cwd.clone(),
                rustc_sha256: digest(&std::fs::read(&captured.args[0]).unwrap()),
                test_binary_sha256: digest(&std::fs::read(env::current_exe().unwrap()).unwrap()),
                sources: source_stamps(),
            };
            let mut request = RequestRecord {
                ordinal: completions.len() + 1,
                purpose: if resource {
                    Purpose::Measure
                } else {
                    Purpose::Baseline
                },
                subject,
                baseline: None,
            };
            let (report, completion) = child(&captured, &request, &directory, &mut output);
            completions.push(completion);
            if resource {
                request.baseline = Some(report.measurement.unwrap());
                for purpose in [Purpose::Exact, Purpose::WorkShort, Purpose::StorageShort] {
                    request.ordinal = completions.len() + 1;
                    request.purpose = purpose;
                    let (_, completion) = child(&captured, &request, &directory, &mut output);
                    completions.push(completion);
                }
            }
        }
    }
    assert_eq!(completions.len(), if resource { 32 } else { 8 });
    write_completion(&mut output, resource, &completions).unwrap();
}

#[test]
#[ignore = "requires real ordinary source admission and admitted protected runtime; missing prerequisites are hard failures"]
fn ordinary_rust_licm_native_motion_noop_direct_unitlocal_both_profiles() {
    run_parent(false);
}

#[test]
#[ignore = "requires real source/runtime; Storage-short is diagnostic until independently reviewed exact-phase successor"]
fn ordinary_rust_licm_native_full_wrapper_exact_and_short_resources() {
    run_parent(true);
}
