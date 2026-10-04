use super::*;
use crate::compiler_descriptor::source_abi_v1::entry_packing::scalar_tests::{
    Qualification, SYMBOLS,
};
const SCALAR_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::source_abi::packing_scalars::entry_packing_scalar_child";

#[derive(Debug, Serialize, Deserialize)]
struct ScalarReport {
    profile: String,
    qualification: Qualification,
}
struct ScalarCapture {
    callbacks: usize,
    result: Option<Result<ScalarReport, String>>,
}
impl Callbacks for ScalarCapture {
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
                .entry_packing_scalar_qualification_v1(&mut budget)
                .map_err(|error| format!("{error:?}"))?;
            assert_eq!(budget.storage(), 29);
            Ok(ScalarReport {
                qualification,
                profile: tcx
                    .sess
                    .opts
                    .cg
                    .target_cpu
                    .as_deref()
                    .unwrap_or(tcx.sess.target.cpu.as_ref())
                    .to_owned(),
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess helper; parent supplies exact six-root Cargo invocation"]
fn entry_packing_scalar_child() {
    let Some(path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = ScalarCapture {
        callbacks: 0,
        result: None,
    };
    let completed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustc_driver::run_compiler(&args, &mut callbacks)
    }));
    let result = if completed.is_err() {
        Err("scalar packing callback panicked".into())
    } else {
        callbacks
            .result
            .unwrap_or_else(|| Err("scalar packing callback did not execute".into()))
    };
    std::fs::write(
        env::var_os(CHILD_RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "genuine scalar packing: {result:?}");
}

#[test]
#[ignore = "genuine Cargo/rustc scalar packing; requires pinned production toolchain"]
fn entry_packing_empty_small_and_mixed_scalar_roots_keep_actual_abi_on_both_profiles() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-entry-packing-scalars");
    let mut completed = 0;
    for target in ["gfx942", "gfx950"] {
        let mut selected = fixture(&workspace, "entry_packing_scalars", target);
        selected.compiler_input.kernel_symbols =
            SYMBOLS.iter().map(|name| (*name).into()).collect();
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
                .args(["--exact", SCALAR_CHILD, "--ignored", "--nocapture"]);
            progress::clear_inherited_jobserver(&mut command);
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{target} opt{optimization}: {}",
                corpus_cargo::diagnostics(&output)
            );
            let report: Result<ScalarReport, String> =
                serde_json::from_slice(&std::fs::read(response).unwrap()).unwrap();
            let report = report.unwrap();
            assert_eq!(report.profile, target);
            let q = report.qualification;
            assert_eq!(
                (
                    q.roots,
                    q.logical,
                    q.physical,
                    q.source_paths,
                    q.resource_cuts
                ),
                (6, 10, 10, 10, 2)
            );
            assert_eq!(
                q.rows
                    .iter()
                    .map(|row| row.name.as_str())
                    .collect::<Vec<_>>(),
                SYMBOLS
            );
            for row in q.rows {
                let (sizes, offsets, explicit, hidden, segment): (&[u16], &[u32], u32, u32, u32) =
                    match row.name.as_str() {
                        "entry_packing_empty" => (&[], &[], 0, 0, 256),
                        "entry_packing_u8" => (&[1], &[0], 1, 8, 264),
                        "entry_packing_u16" => (&[2], &[0], 2, 8, 264),
                        "entry_packing_u32" => (&[4], &[0], 4, 8, 264),
                        "entry_packing_three_u32" => (&[4, 4, 4], &[0, 4, 8], 12, 16, 272),
                        "entry_packing_mixed" => (&[1, 2, 4, 8], &[0, 2, 4, 8], 16, 16, 272),
                        other => panic!("foreign reported root {other}"),
                    };
                assert_eq!(row.sizes, sizes);
                assert_eq!(row.device_offsets, offsets);
                assert_eq!(row.source_offsets, vec![0; sizes.len()]);
                assert_eq!(
                    (
                        row.explicit_bytes,
                        row.hidden_start,
                        row.segment_bytes,
                        row.segment_alignment
                    ),
                    (explicit, hidden, segment, 8)
                );
                completed += 1;
            }
            eprintln!(
                "ENTRY PACKING SCALARS {target} opt{optimization}: six original signatures and hidden tails; descriptor/launch pending"
            );
        }
    }
    assert_eq!(completed, 24);
}
