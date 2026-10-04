//! Protected F children reuse the original preparation/provenance parser and
//! only its existing output-directory relocation, never source-option changes.
use super::*;

#[path = "production_pipeline_conditional_final_results_v1.rs"]
mod records;

const F_CHILD: &str = "production_pipeline::checked_output_policy6_v1::conditional_prefix_v1::tests::genuine_conditional_f_prefix_child";
const F_RESULTS: &str = "FE2O3_CONDITIONAL_F_PREFIX_RESULTS_V1";
const F_ARGS: &str = "FE2O3_CONDITIONAL_F_PREFIX_CHILD_ARGS";
const F_ARGS_SHA256: &str = "FE2O3_CONDITIONAL_F_PREFIX_CHILD_ARGS_SHA256";
const F_TARGET: &str = "FE2O3_CONDITIONAL_F_PREFIX_CHILD_TARGET";
const F_MODE: &str = "FE2O3_CONDITIONAL_F_PREFIX_CHILD_MODE";
const F_RESULT: &str = "FE2O3_CONDITIONAL_F_PREFIX_CHILD_RESULT";

fn sha256(bytes: &[u8]) -> String {
    crate::encode_hex(&Sha256::digest(bytes))
}

fn run(suite: &str, modes: &[&str]) {
    let root = PathBuf::from(env::var_os(INPUTS).expect("prepared Vecadd inputs"));
    let configured = env::var_os(F_RESULTS);
    let scratch = configured
        .is_none()
        .then(|| crate::test_temp_dir::TestTempDir::create("fe2o3-conditional-final-results"));
    let results = configured
        .map(PathBuf::from)
        .unwrap_or_else(|| scratch.as_ref().unwrap().path().join("results"));
    let before = checked_preparation(&root).unwrap();
    let preparation = read_bounded(&root.join("preparation.json"), JSON_CAP).unwrap();
    assert!(
        !results.starts_with(&root)
            && !root.starts_with(&results)
            && !results.starts_with(&before.workspace),
        "disjoint immutable source/input and fresh result roots"
    );
    fresh_directory(&results).unwrap();
    let mut rows = Vec::new();
    for invocation in &before.invocations {
        let input = root.join(&invocation.target);
        let target_output = results.join(&invocation.target);
        fresh_directory(&target_output).unwrap();
        for mode in modes {
            let output = target_output.join(mode);
            fresh_directory(&output).unwrap();
            let compiler_output = output.join("compiler-output");
            fresh_directory(&compiler_output).unwrap();
            fresh_directory(&output.join("tmp")).unwrap();
            empty_output(&input.join("compiler-output")).unwrap();
            let captured = read_bounded(&input.join("args.json"), TEXT_CAP).unwrap();
            let (args, request) = replay_arguments(
                &captured,
                &invocation.request,
                &before.sysroot,
                &compiler_output,
            )
            .unwrap();
            new_file(&output.join("args.json"), &args).unwrap();
            write_json(&output.join("request.json"), &request).unwrap();
            let environment: Environment = serde_json::from_slice(
                &read_bounded(&input.join("environment.json"), TEXT_CAP).unwrap(),
            )
            .unwrap();
            let mut command = Command::new(env::current_exe().unwrap());
            command
                .env_clear()
                .envs(
                    environment
                        .into_iter()
                        .map(|(key, value)| (key, OsString::from_vec(value))),
                )
                .current_dir(&invocation.cwd)
                .env("TMPDIR", output.join("tmp"))
                .env(F_ARGS, output.join("args.json"))
                .env(F_ARGS_SHA256, sha256(&args))
                .env(F_TARGET, invocation.target.strip_prefix("gfx").unwrap())
                .env(F_MODE, mode)
                .env(F_RESULT, output.join("child.json"))
                .args([
                    "--exact",
                    F_CHILD,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ]);
            progress::clear_inherited_jobserver(&mut command);
            let completion = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                checked(&mut command, &output, "child", None)
            }));
            // Preserve the source/input/output audit even if the bounded child
            // launcher refused. No aggregate success record is written then.
            empty_output(&input.join("compiler-output")).unwrap();
            dep_info_only(&compiler_output).unwrap();
            assert_eq!(
                args,
                read_bounded(&output.join("args.json"), TEXT_CAP).unwrap()
            );
            assert_eq!(
                invocation.files,
                tree(&input).unwrap(),
                "child changed prepared inputs"
            );
            assert_eq!(before.source, bounded_source_stamps().unwrap());
            let stdout = completion.unwrap_or_else(|panic| std::panic::resume_unwind(panic));
            let text = std::str::from_utf8(&stdout).unwrap();
            assert_eq!(text.matches(&format!("test {F_CHILD} ... ok")).count(), 1);
            assert_eq!(
                text.matches("test result: ok. 1 passed; 0 failed; 0 ignored;")
                    .count(),
                1
            );
            assert_eq!(
                stdout,
                read_bounded(&output.join("child.stdout"), 16 * 1024 * 1024).unwrap()
            );
            let stderr = read_bounded(&output.join("child.stderr"), 16 * 1024 * 1024).unwrap();
            let child =
                records::decode_child(&read_bounded(&output.join("child.json"), 65_536).unwrap())
                    .unwrap();
            child.check(&invocation.target, mode).unwrap();
            rows.push(records::Row {
                target: invocation.target.clone(),
                mode: (*mode).into(),
                captured_args_sha256: sha256(&captured),
                replay_args_sha256: sha256(&args),
                child,
                exit_code: 0,
                stdout_sha256: sha256(&stdout),
                stderr_sha256: sha256(&stderr),
            });
        }
    }
    assert_eq!(
        preparation,
        read_bounded(&root.join("preparation.json"), JSON_CAP).unwrap()
    );
    // Rechecks executable, rustc/sysroot support, Cargo provenance, source
    // stamps, full captured dependency trees and original normalized argv/env.
    assert_eq!(
        json(&before).unwrap(),
        json(&checked_preparation(&root).unwrap()).unwrap()
    );
    let report = records::Matrix {
        schema: records::SCHEMA.into(),
        suite: suite.into(),
        preparation_sha256: sha256(&preparation),
        rows,
        source_unchanged: true,
        inputs_unchanged: true,
        tools_unchanged: true,
        qualification_credit: false,
        grants_artifact_or_launch_authority: false,
        native_output_emitted: false,
    };
    report.check().unwrap();
    write_json(&results.join("report.json"), &report).unwrap();
    eprintln!(
        "FE2O3_CONDITIONAL_FINAL_RESULTS_V1 {}",
        serde_json::to_string(&report).unwrap()
    );
}

#[test]
#[ignore = "prepared original source/captured environment and admitted proof runtime; no Cargo or GPU"]
fn actual_prepared_conditional_f_prefix_both_targets_and_fixed6_separation() {
    run("main", &records::MAIN_MODES);
}

#[test]
#[ignore = "genuine protected proof execution before injected late refusal; no proof stand-ins"]
fn actual_prepared_conditional_f_prefix_late_failures_never_install() {
    run("late", &records::LATE_MODES);
}
