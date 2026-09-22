//! Public normal-library seed joined to existing exact negative/capture consumers.
//! Test-only observations: no imported report becomes a compiler owner or proof.
use super::super::super::{debug_join, headless_machine};
use super::*;

const PUBLIC_NEGATIVE_OUTPUT: &str = "FE2O3_TEST_SOURCE_HEADLESS_NEGATIVES_OUTPUT";
const PUBLIC_DEBUG_OUTPUT: &str = "FE2O3_TEST_SOURCE_HEADLESS_DEBUG_JOIN_OUTPUT";

struct PublicSeed {
    directory: PathBuf,
    consumer: PathBuf,
    consumer_observation: Value,
    publication: Value,
    original_sha256: String,
    candidate_sha256: String,
}

impl PublicSeed {
    fn prepare(output: &str) -> Self {
        let consumer = PathBuf::from(
            std::env::var_os(headless_machine::CONSUMER)
                .expect("absolute normal-dependency consumer binary"),
        );
        let consumer_observation = headless_machine::consumer_observation(&consumer);
        let directory = PathBuf::from(std::env::var_os(output).expect("fresh absolute output"));
        assert!(directory.is_absolute());
        fs::create_dir(&directory).unwrap();
        let directory = directory.canonicalize().unwrap();
        let source_root = paths::create_root(&directory);
        require_current_source();
        super::super::super::super::prepare(&directory);
        fs::create_dir(directory.join("positive")).unwrap();
        let source = source_root.join("positive");
        fs::create_dir(&source).unwrap();
        paths::write_new(&source.join("original.rs"), FIXTURE_FILES[2].1);
        paths::write_new(&source.join("original-loader.rs"), ORIGINAL_LOADER);
        paths::write_new(&source.join("candidate-loader.rs"), CANDIDATE_LOADER);
        let original_sha256 = hash(&source.join("original.rs"));
        let publication = headless_machine::publish_with_normal_consumer(&directory, &consumer);
        let candidate_sha256 = hash(&source.join("candidate.rs"));
        assert_eq!(publication["invocation"]["source_sha256"], original_sha256);
        assert_eq!(publication["candidate_sha256"], candidate_sha256);
        assert_eq!(
            publication["consumer_file_observation"],
            consumer_observation
        );
        assert_eq!(publication["normal_library_process_published"], true);
        assert_eq!(publication["public_driver_return_observed"], true);
        assert_eq!(publication["private_publish_callback_used"], false);
        assert_eq!(publication["fresh_frontend_admitted"], false);
        assert_eq!(publication["production_resume"], false);
        let seed = Self {
            directory,
            consumer,
            consumer_observation,
            publication,
            original_sha256,
            candidate_sha256,
        };
        seed.recheck();
        seed
    }

    fn recheck(&self) {
        let source = paths::absolute_case(&self.directory, "positive");
        assert_eq!(hash(&source.join("original.rs")), self.original_sha256);
        assert_eq!(hash(&source.join("candidate.rs")), self.candidate_sha256);
        assert_eq!(
            headless_machine::consumer_observation(&self.consumer),
            self.consumer_observation
        );
        assert!(
            fs::read_dir(self.directory.join("analysis-output"))
                .unwrap()
                .next()
                .is_none()
        );
        require_current_source();
    }
}

fn require_fresh_diagnostic(fresh: &Value) {
    assert_eq!(fresh["fresh_frontend_admitted"], true);
    assert_eq!(fresh["pre_ranked_diagnostic"], true);
    assert_eq!(fresh["semantic_version"], "V32");
    assert_eq!(fresh["kernel_ir_version"], "V17");
    assert_eq!(fresh["boolean_oracle_cases"], 128);
    for field in [
        "ranked_checks",
        "functional_proof",
        "production_resume",
        "hardware_observed",
        "grants_artifact_or_launch_authority",
    ] {
        assert_eq!(fresh[field], false, "{field} remains unavailable");
    }
}

#[test]
#[ignore = "normal public seed plus existing exact fresh resource/boundary refusals; fresh output"]
fn actual_source_headless_negative_ladder() {
    let seed = PublicSeed::prepare(PUBLIC_NEGATIVE_OUTPUT);
    let positive = super::super::super::run_machine_child(&seed.directory, "default");
    assert_eq!(
        positive["invocation"]["source_sha256"],
        seed.candidate_sha256
    );
    require_fresh_diagnostic(&positive["observation"]["fresh"]);
    assert_eq!(
        positive["observation"]["whole_kernel_simulation"]["runs"],
        30
    );
    assert_eq!(
        positive["observation"]["whole_kernel_simulation"]["output_and_canaries_checked"],
        true
    );
    assert_eq!(positive["observation"]["old_evidence_reused"], false);
    let publications = run_negative_publications(&seed.directory);
    let observations: Vec<_> = NEGATIVES
        .iter()
        .map(|case| run_negative(&seed.directory, case.name))
        .collect();
    for (case, observation) in NEGATIVES.iter().zip(&observations) {
        assert_eq!(observation["selector"], case.name);
        assert_eq!(observation["diagnostic"], case.expected);
        assert_eq!(observation["actual_rustc_callbacks"], 1);
        assert_eq!(observation["genuine_collected_transaction"], true);
        assert_eq!(observation["fresh_method_entries"], 1);
        assert_eq!(observation["candidate_rejected"], true);
        assert_eq!(observation["llvm_written"], false);
        assert!(
            !seed
                .directory
                .join("positive")
                .join(format!("{}.ll", case.name))
                .exists()
        );
    }
    seed.recheck();
    let (source_files, source_bytes) = source_footprint(&seed.directory);
    let report = json!({
        "kind":"public_seeded_source_candidate_negative_observation_v1",
        "normal_consumer_publication":seed.publication,
        "positive":positive,"publications":publications,"observations":observations,
        "normal_library_publication_processes":1,"actual_fresh_callbacks":5,
        "actual_frontend_callbacks_total":6,"positive_whole_kernel_simulation_runs":30,
        "exact_refusals":4,"physical_resource_refusals":2,"program_boundary_refusals":2,
        "source_directory":paths::relative_root(&seed.directory),
        "source_files":source_files,"source_bytes":source_bytes,
        "source_file_limit":NEGATIVE_SOURCE_FILES,"source_byte_limit":NEGATIVE_SOURCE_BYTES,
        "original_sha256":seed.original_sha256,"generated_candidate_sha256":seed.candidate_sha256,
        "original_and_seed_bytes_unchanged":true,"consumer_file_observation_unchanged":true,
        "seed_private_publish_callback_used":false,"pre_ranked_diagnostic":true,
        "ranked_checks":false,"functional_proof":false,"proof_invalidation_qualified":false,
        "source_authentication_claim":false,"old_evidence_reused_as_authority":false,
        "portable_capture_import":false,"production_resume":false,
        "native_inspected":false,"hardware_observed":false,
        "grants_artifact_or_launch_authority":false,
    });
    let bytes = serde_json::to_vec_pretty(&report).unwrap();
    assert!(bytes.len() <= 512 * 1024);
    paths::write_new(
        &seed.directory.join("headless-negative-observation.json"),
        &bytes,
    );
}

#[test]
#[ignore = "normal public seed plus existing actual dual-session capture identity checks; fresh output"]
fn actual_source_headless_debug_join_ladder() {
    let seed = PublicSeed::prepare(PUBLIC_DEBUG_OUTPUT);
    // The existing bounded editor also writes two unused negative source/loader
    // pairs. Keep charging all ten leaves without counting those as executions.
    let edits = super::super::run_edits(&seed.directory);
    let source = paths::absolute_case(&seed.directory, "positive");
    let edited_sha256 = hash(&source.join("edited.rs"));
    assert_ne!(seed.candidate_sha256, edited_sha256);
    let joined = debug_join::run_pair_child(&seed.directory);
    assert_eq!(joined["actual_rustc_callbacks"], 2);
    assert_eq!(joined["fixed_environment_sequential_sessions"], 2);
    assert_eq!(joined["positive_whole_kernel_oracle_runs"], 60);
    assert_eq!(joined["actual_debug_capture_runs"], 2);
    assert_eq!(joined["exact_source_identity_mismatch_refusals"], 3);
    assert_eq!(joined["old_evidence_used_only_as_negative_input"], true);
    assert_eq!(joined["public_bundle_created"], false);
    assert_eq!(joined["portable_capture_import"], false);
    let pair = joined["observations"].as_array().unwrap();
    assert_eq!(pair.len(), 2);
    for (index, (observation, expected_sha256)) in pair
        .iter()
        .zip([&seed.candidate_sha256, &edited_sha256])
        .enumerate()
    {
        assert_eq!(observation["invocation"]["source_sha256"], *expected_sha256);
        let observed = &observation["observation"];
        require_fresh_diagnostic(&observed["fresh"]);
        assert_eq!(observed["whole_kernel_simulation"]["runs"], 30);
        assert_eq!(observed["actual_captures"], 1);
        assert_eq!(observed["same_live_v17_owner"], true);
        assert_eq!(observed["actual_lane_zero_before_after"], true);
        assert_eq!(observed["complete_transcript"], true);
        assert_eq!(observed["capture_output_init_and_canaries_checked"], true);
        assert_eq!(observed["capture_input_immutable"], true);
        assert_eq!(
            observed["exact_source_identity_mismatch_refusals"],
            if index == 0 { 0 } else { 3 }
        );
    }
    for identity in ["candidate_sha256", "semantic_sha256", "kernel_ir_sha256"] {
        assert_ne!(
            pair[0]["observation"]["fresh"][identity],
            pair[1]["observation"]["fresh"][identity]
        );
    }
    assert_ne!(
        pair[0]["observation"]["catalog_identity"],
        pair[1]["observation"]["catalog_identity"]
    );
    assert_eq!(hash(&source.join("edited.rs")), edited_sha256);
    seed.recheck();
    let (source_files, source_bytes) = super::super::footprint(&seed.directory);
    let report = json!({
        "kind":"public_seeded_source_candidate_debug_join_observation_v1",
        "normal_consumer_publication":seed.publication,"edit_publications":edits,"joined":joined,
        "normal_library_publication_processes":1,"actual_fresh_callbacks":2,
        "actual_frontend_callbacks_total":3,"positive_whole_kernel_oracle_runs":60,
        "actual_debug_capture_runs":2,"exact_source_identity_mismatch_refusals":3,
        "failed_bindings_remain_unbound":true,"current_catalog_rebind_checked":true,
        "source_directory":paths::relative_root(&seed.directory),
        "source_files":source_files,"source_bytes":source_bytes,
        "source_file_limit":10,"source_byte_limit":10*128*1024,
        "unused_generated_negative_source_files":4,
        "original_sha256":seed.original_sha256,"generated_candidate_sha256":seed.candidate_sha256,
        "edited_sha256":edited_sha256,"original_seed_and_edited_bytes_unchanged":true,
        "consumer_file_observation_unchanged":true,"seed_private_publish_callback_used":false,
        "old_evidence_used_only_as_negative_input":true,"old_evidence_reused_as_authority":false,
        "pre_ranked_diagnostic":true,"ranked_checks":false,"functional_proof":false,
        "proof_invalidation_qualified":false,"source_authentication_claim":false,
        "public_v17_source_map_support":false,"portable_capture_import":false,
        "llvm_or_native_emission":false,"physical_register_observations":false,
        "resource_lifetime_observations":false,"production_resume":false,"hardware_observed":false,
        "grants_artifact_or_launch_authority":false,
    });
    let bytes = serde_json::to_vec_pretty(&report).unwrap();
    assert!(bytes.len() <= 512 * 1024);
    paths::write_new(
        &seed.directory.join("headless-debug-join-observation.json"),
        &bytes,
    );
}
