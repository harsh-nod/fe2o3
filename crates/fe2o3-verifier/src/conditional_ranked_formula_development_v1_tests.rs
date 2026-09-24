//! Opt-in source/solver regression, never a protected execution receipt.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, ConditionalTotalViewAddressDomainV1 as Domain,
};
use fe2o3_pliron::ProductionConditionalRuntimePremiseV1 as Premise;
use std::{os::unix::fs::DirBuilderExt, path::PathBuf, process::Command};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        use rand_core::RngCore;
        let nonce = rand_core::OsRng.next_u64();
        let root = std::env::temp_dir().join(format!(
            "fe2o3-conditional-verus-{}-{nonce:016x}",
            std::process::id()
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        Self(root)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn development_source(wrong_value: bool) -> String {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 4 * SOURCE_LIMIT);
    let mut out = BoundedSource::new().unwrap();
    out.write_str("use vstd::prelude::*;\nverus! {\n").unwrap();
    out.write_str(ranked_effect_formula_replay_prelude_v2())
        .unwrap();
    out.write_str(
        &crate::functional_refinement_receipt_v2::conditional_formula_development_source_v1(
            wrong_value,
        ),
    )
    .unwrap();
    let premises = [
        Premise::D1Launch,
        Premise::OutputWithinGlobalX { parameter: 2 },
        Premise::WritableOutput { parameter: 2 },
        Premise::RepresentableAddress {
            parameter: 2,
            domain: Domain::GuardedOutput,
            element_bytes: 4,
            alignment: 4,
        },
        Premise::ReadableInput {
            parameter: 0,
            domain: Domain::GuardedOutput,
        },
        Premise::SeparateInputOutput {
            input: 0,
            output: 2,
        },
        Premise::RepresentableAddress {
            parameter: 0,
            domain: Domain::GlobalLaunch,
            element_bytes: 4,
            alignment: 4,
        },
        Premise::ReadableInput {
            parameter: 1,
            domain: Domain::GlobalLaunch,
        },
        Premise::SeparateInputOutput {
            input: 1,
            output: 2,
        },
        Premise::RepresentableAddress {
            parameter: 1,
            domain: Domain::GlobalLaunch,
            element_bytes: 4,
            alignment: 4,
        },
    ];
    source::append_premise_theorem(&mut out, &premises, 2, &mut budget).unwrap();
    out.write_str("}\n").unwrap();
    out.0
}

#[test]
#[ignore = "explicit user-writable development Verus; no protected-runtime or hardware credit"]
fn conditional_formula_development_verus_accepts_and_rejects_mutations() {
    let verus = PathBuf::from(
        std::env::var_os("FE2O3_DEVELOPMENT_VERUS_BIN")
            .expect("set an explicit development Verus binary"),
    );
    assert!(verus.is_absolute() && verus.is_file());
    let rustup = PathBuf::from(
        std::env::var_os("FE2O3_DEVELOPMENT_RUSTUP_BIN")
            .expect("set an explicit development rustup binary"),
    );
    assert!(rustup.is_absolute() && rustup.is_file());
    let executable_path = std::env::join_paths(
        std::iter::once(rustup.parent().unwrap().to_path_buf()).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )),
    )
    .unwrap();
    let scratch = Scratch::new();
    for (name, contents, accepted) in [
        ("positive", development_source(false), true),
        ("wrong_value", development_source(true), false),
        (
            "missing_coverage",
            format!(
                "{}\nverus! {{ proof fn missing_coverage(n: int, g: int) requires 0 <= n, 0 <= g, ensures n <= g, {{}} }}\n",
                development_source(false)
            ),
            false,
        ),
    ] {
        let path = scratch.0.join(format!("{name}.rs"));
        std::fs::write(&path, contents).unwrap();
        let result = Command::new("/usr/bin/timeout")
            .args(["--kill-after=5s", "60s"])
            .arg(&verus)
            .arg(&path)
            .args([
                "--crate-type",
                "lib",
                "--multiple-errors",
                "3",
                "--num-threads",
                "1",
                "--no-cheating",
            ])
            .env("RUSTUP_TOOLCHAIN", "1.97.1")
            .env("PATH", &executable_path)
            .current_dir(&scratch.0)
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert_eq!(
            result.status.success(),
            accepted,
            "{name}: {}\n{stdout}\n{stderr}",
            result.status
        );
        if accepted {
            assert!(stdout.contains("0 errors"), "{stdout}\n{stderr}");
        } else {
            assert_eq!(result.status.code(), Some(1), "{stderr}");
            assert!(
                stderr.contains("assertion failed")
                    || stderr.contains("postcondition not satisfied"),
                "{stderr}"
            );
        }
    }
}
