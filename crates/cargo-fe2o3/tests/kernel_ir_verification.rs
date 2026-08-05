use std::path::Path;
use std::process::{Command, Output};

fn build(workspace: &Path, package: &str, verify: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command
        .current_dir(workspace)
        .env_remove("FE2O3_VERIFY_KERNEL_IR")
        .args(["build", "-p", package]);
    if verify {
        command.env("FE2O3_VERIFY_KERNEL_IR", "1");
    }
    command.output().expect("run the custom backend")
}

#[test]
#[ignore = "requires a working ROCm toolchain"]
fn verification_gate_accepts_rejects_and_remains_opt_in() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let artifact_dir = workspace.join("target/fe2o3");

    let supported = build(&workspace, "fe2o3-vecadd", true);
    let supported_stderr = String::from_utf8_lossy(&supported.stderr);
    assert!(
        supported.status.success(),
        "supported MIR failed verification:\n{supported_stderr}"
    );
    assert!(
        supported_stderr.contains("verified MIR kernel IR analysis: 1 kernel(s), 4 function(s)"),
        "missing successful verification diagnostic:\n{supported_stderr}"
    );
    assert!(
        supported_stderr.contains("emitted vecadd"),
        "verified kernel did not reach emission:\n{supported_stderr}"
    );

    let unsupported = build(&workspace, "fe2o3-negate", true);
    let unsupported_stderr = String::from_utf8_lossy(&unsupported.stderr);

    assert!(
        !unsupported.status.success(),
        "unsupported MIR unexpectedly built"
    );
    assert!(
        unsupported_stderr
            .contains("device artifact preflight failed: MIR kernel IR analysis failed"),
        "missing rustc fatal diagnostic:\n{unsupported_stderr}"
    );
    assert!(
        unsupported_stderr
            .contains("UnsupportedRvalue: unsupported structured MIR rvalue: Unary(Neg)"),
        "missing structured translation diagnostic:\n{unsupported_stderr}"
    );
    assert!(
        !unsupported_stderr.contains("emitted negate"),
        "legacy emission ran after failed verification:\n{unsupported_stderr}"
    );
    for extension in ["ll", "o", "hsaco"] {
        assert!(
            !artifact_dir.join(format!("negate.{extension}")).exists(),
            "rejected preflight left negate.{extension} executable state"
        );
    }

    let legacy = build(&workspace, "fe2o3-negate", false);
    let legacy_stderr = String::from_utf8_lossy(&legacy.stderr);
    assert!(
        legacy.status.success(),
        "legacy mode changed when verification was absent:\n{legacy_stderr}"
    );
    assert!(
        legacy_stderr.contains("emitted negate"),
        "legacy mode did not emit negate:\n{legacy_stderr}"
    );
    assert!(
        !legacy_stderr.contains("MIR kernel IR analysis"),
        "verification ran without an explicit opt-in:\n{legacy_stderr}"
    );
}
