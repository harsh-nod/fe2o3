use std::process::Command;

#[test]
fn authenticated_runner_mechanically_checks_slice1_lds_proofs_and_mutations() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(manifest.join("run-verus.sh"))
        .env("VERUS_TIMEOUT_SECONDS", "300")
        .output()
        .expect("run authenticated tiled-GEMM Verus helper");

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success()
        && (stderr.contains("Verus is unavailable")
            || (stderr.contains("Verus executable SHA-256")
                && stderr.contains("does not match pinned")))
    {
        eprintln!("skipping authenticated proof run because pinned Verus is unavailable: {stderr}");
        return;
    }

    assert!(
        output.status.success(),
        "runner failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        stderr,
    );

    let stdout = String::from_utf8(output.stdout).expect("runner stdout is UTF-8");
    assert!(stdout.contains("PASS: Slice 1 LDS tiled GEMM model verified (93 verified, 0 errors)"));
    assert!(stdout.contains(
        "PASS: attributed Slice 1 source-refinement evidence verified (96 verified, 0 errors)"
    ));
    assert!(stdout.contains("XFAIL: lds_epoch_wrong rejected at the expected proof obligation"));
    assert!(stdout.contains("XFAIL: lds_product_wrong rejected at the expected proof obligation"));
    assert!(stdout.contains(
        "XFAIL: lds_source_correspondence_identity_wrong rejected at the expected proof obligation"
    ));
}
