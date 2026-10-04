#![cfg(target_os = "linux")]

#[path = "../examples/support/tutorial_fill_v1.rs"]
mod tutorial_fill_v1;

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

const KIR_IDENTITY_SHA256: &str =
    "e8f2c794a5dd4aeac63f5c820f9d5785b40b5aaff357e3f6726164fa4425f384";
const KIR_CANONICAL_BYTES: u64 = 245;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tutorial/fill-v1")
        .join(name)
}

fn run_fixture() -> Output {
    Command::new(env!("CARGO_BIN_EXE_fe2o3-kir-sim"))
        .env_clear()
        .arg("--kir-v7")
        .arg(fixture_path("kernel.kir"))
        .arg("--request")
        .arg(fixture_path("request.json"))
        .output()
        .expect("run standalone tutorial fixture")
}

#[test]
fn committed_tutorial_fixture_is_canonical_and_reproducible() {
    let kir = fs::read(fixture_path("kernel.kir")).expect("read committed KIR V7");
    assert_eq!(kir, tutorial_fill_v1::canonical_kir_v7());
    assert_eq!(kir.len() as u64, KIR_CANONICAL_BYTES);

    let expected =
        fs::read(fixture_path("expected-result.json")).expect("read committed simulation result");
    for _ in 0..2 {
        let output = run_fixture();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected);
    }

    let result: serde_json::Value =
        serde_json::from_slice(&expected).expect("parse committed simulation result");
    assert_eq!(result["schema"], "fe2o3-simulation-result-v1");
    assert_eq!(result["status"], "ok");
    assert_eq!(result["authority"], "observation_only");
    assert_eq!(result["simulated"], true);
    assert_eq!(result["hardware_observed"], false);
    assert_eq!(result["hardware_validation"], false);
    assert_eq!(result["performance_prediction"], false);
    assert_eq!(result["kir"]["sha256"], KIR_IDENTITY_SHA256);
    assert_eq!(result["kir"]["canonical_bytes"], KIR_CANONICAL_BYTES);
    assert_eq!(result["counts"]["arguments"], 1);
    assert_eq!(result["counts"]["shared_buffers"], 0);
    assert_eq!(result["counts"]["invocations_executed"], 4);
    assert_eq!(result["counts"]["workgroups_visited"], 1);
    assert_eq!(result["counts"]["scheduled_slots_visited"], 64);
    assert_eq!(result["counts"]["steps_executed"], 20);
    assert_eq!(result["counts"]["events_emitted"], 0);
    assert_eq!(
        result["schedule"]["identity"],
        "workgroup_major_local_zyx_cooperative_v1"
    );
    assert_eq!(
        result["schedule"]["transcript_sha256"],
        "a9c2892473af3eeeda6466fbaebd03672800cea54738ae80528863abd491acf3"
    );
    assert_eq!(result["schedule"]["coverage"]["decisions"], 4);
    assert_eq!(result["schedule"]["coverage"]["workgroups"], 1);
    assert_eq!(result["schedule"]["coverage"]["barrier_releases"], 0);
    assert_eq!(result["schedule"]["coverage"]["complete"], true);
    assert_eq!(
        result["conflict_assessment"]["status"],
        "no_conflicts_observed"
    );
    assert_eq!(
        result["arguments"][0]["value"]["bytes"],
        "0x11000000110000001100000011000000"
    );
    assert_eq!(result["arguments"][0]["value"]["initialized"], "0xffff");
}
