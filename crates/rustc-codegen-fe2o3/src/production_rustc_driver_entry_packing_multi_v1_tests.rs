use super::*;
use crate::compiler_descriptor::source_abi_v1::entry_packing::multi_tests::Qualification;
const MULTI_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::source_abi::packing_multi::entry_packing_multi_child";

#[derive(Debug, Serialize, Deserialize)]
struct MultiReport {
    profile: String,
    qualification: Qualification,
}
struct MultiCapture {
    callbacks: usize,
    result: Option<Result<MultiReport, String>>,
}
impl Callbacks for MultiCapture {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.callbacks += 1;
        self.result = Some((|| {
            assert_eq!(self.callbacks, 1);
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let ranked = transaction
                .verify_general_kernel_checks()
                .map_err(|error| format!("{error:?}"))?;
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(29).unwrap();
            let qualification = ranked
                .entry_packing_multi_qualification_v1(&mut budget)
                .map_err(|error| format!("{error:?}"))?;
            assert_eq!(budget.storage(), 29);
            Ok(MultiReport {
                profile: tcx
                    .sess
                    .opts
                    .cg
                    .target_cpu
                    .as_deref()
                    .unwrap_or(tcx.sess.target.cpu.as_ref())
                    .to_owned(),
                qualification,
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess helper; parent supplies exact two-root Cargo invocation"]
fn entry_packing_multi_child() {
    let Some(path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = MultiCapture {
        callbacks: 0,
        result: None,
    };
    let completed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustc_driver::run_compiler(&args, &mut callbacks)
    }));
    let result = if completed.is_err() {
        Err("two-root packing callback panicked".to_owned())
    } else {
        callbacks
            .result
            .unwrap_or_else(|| Err("two-root packing callback did not execute".into()))
    };
    std::fs::write(
        env::var_os(CHILD_RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "genuine two-root packing: {result:?}");
}

#[test]
#[ignore = "genuine Cargo/rustc two-root source packing; requires pinned production toolchain"]
fn entry_packing_two_roots_refuse_late_capture_without_partial_consumer_on_both_profiles() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-entry-packing-two-roots");
    let mut completed = 0;
    for target in ["gfx942", "gfx950"] {
        let mut selected = fixture(&workspace, "aggregate_pair_struct", target);
        selected.fixture_id = format!("{target}-aggregate-two-roots");
        selected.compiler_input.features =
            vec!["aggregate_nested".into(), "aggregate_pair_struct".into()];
        selected.compiler_input.kernel_symbols =
            vec!["aggregate_nested".into(), "aggregate_pair_struct".into()];
        let directory = scratch.path().join(&selected.fixture_id);
        std::fs::create_dir(&directory).unwrap();
        let captured = corpus_cargo::capture(
            &workspace,
            &selected,
            &directory,
            &scratch.path().join(target),
        )
        .unwrap();
        for (optimization, mir) in [(0, 0), (3, 2)] {
            let mut args = captured.args.clone();
            args.extend([
                format!("-Copt-level={optimization}"),
                format!("-Zmir-opt-level={mir}"),
            ]);
            let request = directory.join(format!("args-{optimization}.json"));
            let response = directory.join(format!("result-{optimization}.json"));
            std::fs::write(&request, serde_json::to_vec(&args).unwrap()).unwrap();
            let mut command = Command::new(env::current_exe().unwrap());
            command
                .env_clear()
                .envs(captured.environment.clone())
                .current_dir(&captured.cwd)
                .env_remove("RUSTC_WRAPPER")
                .env_remove("RUSTC_WORKSPACE_WRAPPER")
                .env_remove(CHILD_PROOF_PROBE)
                .env_remove(CASE)
                .env(CHILD_ARGS, &request)
                .env(CHILD_RESULT, &response)
                .args(["--exact", MULTI_CHILD, "--ignored", "--nocapture"]);
            progress::clear_inherited_jobserver(&mut command);
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{target} opt{optimization}: {}",
                corpus_cargo::diagnostics(&output)
            );
            let report: Result<MultiReport, String> =
                serde_json::from_slice(&std::fs::read(response).unwrap()).unwrap();
            let report = report.unwrap();
            assert_eq!(report.profile, target);
            assert_eq!(
                report.qualification,
                Qualification {
                    roots: 2,
                    logical: 6,
                    physical: 10,
                    components: 12,
                    later_root_refusals: 4,
                    complete_first_roots_observed: 4,
                    refused_consumers_entered: 0,
                    resource_cuts: 2
                }
            );
            completed += 1;
            eprintln!(
                "ENTRY PACKING TWO ROOTS {target} opt{optimization}: complete original roster; four later-root refusals, no partial consumer; descriptor/launch pending"
            );
        }
    }
    assert_eq!(completed, 4);
}
