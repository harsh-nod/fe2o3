//! Opt-in ordinary source -> candidate -> register edit -> fresh V17 observation.
use super::*;

#[test]
#[ignore = "pinned actual callbacks; serialized Cargo; fresh task-owned output"]
fn actual_source_candidate_machine_ladder() {
    let directory = PathBuf::from(std::env::var_os(OUTPUT).expect("fresh absolute output"));
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let source_root = paths::create_root(&directory);
    require_current_source();
    super::super::prepare(&directory);
    fs::create_dir(directory.join("positive")).unwrap();
    let case = source_root.join("positive");
    fs::create_dir(&case).unwrap();
    paths::write_new(&case.join("original.rs"), FIXTURE_FILES[2].1);
    paths::write_new(&case.join("original-loader.rs"), ORIGINAL_LOADER);
    paths::write_new(&case.join("candidate-loader.rs"), CANDIDATE_LOADER);
    let original_sha = hash(&case.join("original.rs"));
    let baseline = super::super::run_child(&directory, "positive", "baseline");
    assert_eq!(
        baseline["observation"]["baseline"]["kernel_ir_version"],
        "V8"
    );
    let candidate_sha = hash(&case.join("candidate.rs"));
    let default = run_machine_child(&directory, "default");
    assert_eq!(
        baseline["observation"]["candidate_sha256"],
        default["observation"]["fresh"]["candidate_sha256"]
    );
    let edits = fixture_cases::run_edits(&directory);
    let edited_sha = hash(&case.join("edited.rs"));
    assert_ne!(edited_sha, candidate_sha);
    let edited = run_machine_child(&directory, "edited");
    let repeated = run_machine_child(&directory, "repeat");
    assert_eq!(
        edited["observation"], repeated["observation"],
        "fresh actual callback repeat"
    );
    assert_ne!(
        default["observation"]["fresh"]["kernel_ir_sha256"],
        edited["observation"]["fresh"]["kernel_ir_sha256"],
        "new explicit registers must produce a new actual canonical identity"
    );
    assert_ne!(
        default["observation"]["fresh"]["semantic_sha256"],
        edited["observation"]["fresh"]["semantic_sha256"]
    );
    assert_ne!(
        default["observation"]["llvm_sha256"],
        edited["observation"]["llvm_sha256"]
    );
    assert_eq!(
        default["observation"]["register_plan"],
        json!([4, 5, 0, 1, 2])
    );
    assert_eq!(
        edited["observation"]["register_plan"],
        json!([32, 33, 34, 35, 36])
    );
    assert_eq!(
        default["observation"]["fresh"]["descriptors"],
        edited["observation"]["fresh"]["descriptors"]
    );
    let mut observations = vec![default, edited, repeated];
    for selector in ["wrong-plan", "wrong-output", "stale-edited"] {
        observations.push(run_machine_child(&directory, selector));
    }
    assert_eq!(hash(&case.join("original.rs")), original_sha);
    assert_eq!(hash(&case.join("candidate.rs")), candidate_sha);
    assert_eq!(hash(&case.join("edited.rs")), edited_sha);
    require_current_source();
    let (source_files, source_bytes) = fixture_cases::footprint(&directory);
    let report = json!({
        "baseline":baseline,"edit_publications":edits,"observations":observations,
        "actual_callbacks":7,"successful_fresh_callbacks":3,"positive_whole_kernel_simulation_runs":90,
        "additional_negative_simulation_is_bounded":true,
        "exact_refusals":3,"baseline_owner_version":"V8","fresh_owner_version":"V17",
        "source_directory":paths::relative_root(&directory),
        "source_files":source_files,"source_bytes":source_bytes,
        "source_file_limit":10,"source_byte_limit":10*128*1024,
        "original_sha256":original_sha,"unedited_candidate_sha256":candidate_sha,
        "edited_candidate_sha256":edited_sha,
        "native_inspection_input":directory.join("positive/edited.ll"),
        "native_observer_arguments":["three","used"],
        "native_inspection_pending":true,"old_evidence_reused":false,
        "fixed_production_policy_modified":false,"production_resume":false,
        "functional_proof":false,"hardware_observed":false,
        "grants_artifact_or_launch_authority":false,
    });
    let bytes = serde_json::to_vec_pretty(&report).unwrap();
    assert!(bytes.len() <= 512 * 1024);
    paths::write_new(&directory.join("observation.json"), &bytes);
    eprintln!(
        "source-candidate machine feasibility: {}",
        directory.display()
    );
}
