use std::process::Command;

#[test]
fn authenticated_runner_checks_edges_proof_and_mutations() {
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
    assert!(
        stdout
            .contains("PASS: Slice 4 LDS edge alpha/beta model verified (101 verified, 0 errors)")
    );
    for mutation in [
        "lds_edges_lane_skips_barrier_wrong",
        "lds_edges_unguarded_tail_load_wrong",
        "lds_edges_unguarded_tail_store_wrong",
        "lds_edges_alpha_beta_wrong",
        "lds_edges_k_tail_coverage_wrong",
    ] {
        assert!(
            stdout.contains(&format!(
                "XFAIL: {mutation} rejected at the expected proof obligation"
            )),
            "missing rejection for {mutation}"
        );
    }
}
