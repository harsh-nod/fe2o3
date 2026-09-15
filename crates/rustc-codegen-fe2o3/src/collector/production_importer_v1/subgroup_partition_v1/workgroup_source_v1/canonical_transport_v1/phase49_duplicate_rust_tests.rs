//! Rust-rejection tests, separate from success-required source/SSA imports.
use super::*;
#[path = "phase49_duplicate_diagnostics.rs"]
mod diagnostics;
const DIAGNOSTIC_CHILD: &str = "FE2O3_PHASE_DUPLICATE_RUST_CHILD_V1";
const BIND: &str = "let _lease = phase.bind_reusable_lds(&mut storage);";
fn invalid_source() -> String {
    assert_eq!(source_tests::SOURCE.matches(BIND).count(), 2);
    source_tests::SOURCE.replacen(BIND,
        "let _lease = phase.bind_reusable_lds(&mut storage); let _other = phase.bind_reusable_lds(&mut storage);", 1)
}

#[derive(Default)]
struct RustRejectProbe {
    configured: usize,
    after_analysis: usize,
}
impl Callbacks for RustRejectProbe {
    fn config(&mut self, config: &mut Config) {
        self.configured += 1;
        config.input = Input::Str {
            name: FileName::Custom("phase49_duplicate_rust_source.rs".into()),
            input: invalid_source(),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, _: TyCtxt<'tcx>) -> Compilation {
        self.after_analysis += 1;
        // There is deliberately no importer invocation in this callback.
        panic!("duplicate mutable phase storage unexpectedly reached after_analysis");
    }
}

fn rust_reject(test: &str, cpu: &'static str) {
    run(test, |original| {
        if let Some(child) = std::env::var_os(DIAGNOSTIC_CHILD) {
            assert_eq!(child, std::ffi::OsStr::new(test));
            let mut args = original.to_vec();
            let mut replaced = 0;
            for arg in &mut args {
                if arg == "-Ctarget-cpu=gfx950" {
                    *arg = format!("-Ctarget-cpu={cpu}");
                    replaced += 1;
                }
            }
            assert_eq!(replaced, 1);
            assert!(
                !args
                    .iter()
                    .any(|a| a.starts_with("--error-format") || a.starts_with("--json"))
            );
            args.push("--error-format=json".into());
            let mut probe = RustRejectProbe::default();
            // This catches only rustc's FatalErrorMarker. Ordinary assertion
            // failures and ICE panics continue unwinding and fail the child.
            let outcome =
                rustc_driver::catch_fatal_errors(|| rustc_driver::run_compiler(&args, &mut probe));
            assert!(
                outcome.is_err(),
                "duplicate phase mutable borrow must fail Rust compilation"
            );
            assert_eq!(probe.configured, 1);
            assert_eq!(probe.after_analysis, 0);
            eprintln!(
                "{}",
                serde_json::json!({"$message_type":diagnostics::SUMMARY,
                "test":test,"cpu":cpu,"configured":probe.configured,
                "after_analysis":probe.after_analysis,"fatal":outcome.is_err()})
            );
            return;
        }
        // Reuse the authenticated parent harness environment, registration,
        // repo cwd and scratch output directory. Never build dependencies.
        let output = crate::process_execution::capture_output(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    test,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(DIAGNOSTIC_CHILD, test)
                .current_dir(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "Rust diagnostic child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        diagnostics::check(&output.stderr, test, cpu, &invalid_source())
            .unwrap_or_else(|e| panic!("{e}:\n{}", String::from_utf8_lossy(&output.stderr)));
    });
}
macro_rules! rust_case {
    ($name:ident,$cpu:literal) => {
        #[test]
        #[ignore = "requires cached authenticated AMD metadata; Rust rejection only; no Cargo"]
        fn $name() {
            rust_reject(
                concat!(module_path!(), "::", stringify!($name))
                    .strip_prefix("rustc_codegen_fe2o3::")
                    .unwrap(),
                $cpu,
            );
        }
    };
}
rust_case!(phase49_duplicate_allocation_rust_reject_gfx942, "gfx942");
rust_case!(phase49_duplicate_allocation_rust_reject_gfx950, "gfx950");
