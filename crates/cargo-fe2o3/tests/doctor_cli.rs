use std::process::Command;

fn doctor(arguments: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .arg("doctor")
        .args(arguments)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .output()
        .expect("run cargo fe2o3 doctor")
}

#[test]
fn default_doctor_is_a_host_independent_kfd_first_diagnostic() {
    let output = doctor(&[]);
    assert!(
        output.status.success(),
        "doctor failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 doctor report");
    for required in [
        "fe2o3 doctor v1",
        "runtime: direct-kfd",
        "kfd-interface:",
        "direct-kfd-preflight:",
        "compiler-tools:",
        "runtime-libraries: HIP/HSA not-required-or-loaded",
        "source-export: extraction-only-no-compiler-or-hardware-authority",
        "application-execution: unavailable worker-v3-application-route-unwired",
        "overall: diagnostics-complete",
    ] {
        assert!(stdout.contains(required), "missing `{required}`:\n{stdout}");
    }
    for label in ["debugger-rocgdb", "profiler-rocprofv3"] {
        assert!(
            stdout.contains(&format!("{label}: optional-unavailable"))
                || stdout.contains(&format!("{label}: optional-present-unvalidated")),
            "invalid optional-tool status for `{label}`:\n{stdout}"
        );
    }
    assert!(!stdout.contains("optional-available"), "{stdout}");
    assert!(!stdout.contains("libamdhip64"), "{stdout}");
    assert!(!stdout.contains("rocminfo"), "{stdout}");
}

#[test]
fn execution_requirement_fails_at_the_worker_v3_boundary() {
    let output = doctor(&["--require-execution"]);
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("UTF-8 doctor report");
    let stderr = String::from_utf8(output.stderr).expect("UTF-8 doctor error");
    assert!(
        stdout.contains("application-execution: unavailable worker-v3-application-route-unwired")
    );
    assert!(stderr.contains("Worker V3 application route is not wired"));
}

#[test]
fn gfx942_requirement_is_explicitly_hardware_gated() {
    if std::env::var_os("FE2O3_REQUIRE_GFX942_DOCTOR").as_deref() != Some(std::ffi::OsStr::new("1"))
    {
        return;
    }
    let output = doctor(&["--require-gfx942"]);
    assert!(
        output.status.success(),
        "gfx942 doctor failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("target=gfx942 wave-width=64"));
}

#[test]
fn doctor_help_and_option_surface_are_closed() {
    let help = doctor(&["--help"]);
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("--require-direct-kfd"));
    assert!(help.contains("--require-tools-present"));
    assert!(!help.contains("--require-compiler"));

    let unknown = doctor(&["--unknown"]);
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown option"));
}
