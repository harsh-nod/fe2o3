//! Additive ladder: an external normal-library process publishes the seed, then
//! unchanged actual-source callbacks observe a no-edit and a register-only edit.
//! This qualifies only the bounded API output, not production re-admission or native proof.
use super::*;
use crate::production_rustc_driver_v1::lower_hex_v1;
use std::io::Read as _;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};

#[path = "source_bitselect_headless_prefix_v1_tests.rs"]
mod prefix;

const HEADLESS_OUTPUT: &str = "FE2O3_TEST_SOURCE_HEADLESS_MACHINE_OUTPUT";
pub(super) const CONSUMER: &str = "FE2O3_TEST_SOURCE_HEADLESS_CONSUMER";
const NORMAL_PREFIX: &str = "FE2O3_HEADLESS_NORMAL_PUBLISHED ";
const CONSUMER_BYTE_CAP: u64 = 256 * 1024 * 1024;

// A bounded local file observation, not an attestation of its build or source.
pub(super) fn consumer_observation(path: &Path) -> Value {
    assert!(path.is_absolute());
    assert_eq!(path.canonicalize().unwrap(), path);
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .unwrap();
    let before = file.metadata().unwrap();
    assert!(before.is_file() && before.mode() & 0o111 != 0);
    assert!(before.len() > 0 && before.len() <= CONSUMER_BYTE_CAP);
    let mut digest = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        total = total.checked_add(count as u64).unwrap();
        assert!(total <= CONSUMER_BYTE_CAP);
        digest.update(&buffer[..count]);
    }
    let after = file.metadata().unwrap();
    assert_eq!(total, before.len());
    assert_eq!(
        (
            before.dev(),
            before.ino(),
            before.len(),
            before.mtime(),
            before.mtime_nsec(),
            before.ctime(),
            before.ctime_nsec()
        ),
        (
            after.dev(),
            after.ino(),
            after.len(),
            after.mtime(),
            after.mtime_nsec(),
            after.ctime(),
            after.ctime_nsec()
        )
    );
    json!({"path":path,"bytes":total,"sha256":lower_hex_v1(&digest.finalize()),
        "device":after.dev(),"inode":after.ino(),"mtime":after.mtime(),"mtime_nsec":after.mtime_nsec(),
        "ctime":after.ctime(),"ctime_nsec":after.ctime_nsec(),"authenticated_build":false})
}

pub(super) fn publish_with_normal_consumer(directory: &Path, consumer: &Path) -> Value {
    let actual = super::super::derive_roundtrip(directory, "positive", "baseline");
    let relative = paths::relative_case(directory, "positive");
    let source = relative.join("original.rs");
    let candidate = relative.join("candidate.rs");
    let absolute = paths::absolute_case(directory, "positive");
    let absolute_source = absolute.join("original.rs");
    let absolute_candidate = absolute.join("candidate.rs");
    assert!(!absolute_candidate.try_exists().unwrap());
    paths::write_new(
        &directory.join("positive/headless-normal.invocation.json"),
        &serde_json::to_vec_pretty(&actual).unwrap(),
    );
    let binary = consumer_observation(consumer);
    let mut child = Command::new(consumer);
    let stdout = checked(
        sanitized(&mut child)
            .current_dir(repository())
            .arg(&source)
            .arg(&candidate)
            .arg(&actual.source_sha256)
            .arg("--")
            .args(&actual.args)
            .env(CRATE_BINDING_ID_ENV_V1, &actual.crate_binding)
            .env(
                CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                &actual.cargo_observation,
            )
            .env("CARGO_MANIFEST_DIR", fixture())
            .env("CARGO_PKG_NAME", PACKAGE)
            .env("CARGO_PKG_VERSION", "0.1.0")
            .env("CARGO_CRATE_NAME", CRATE_NAME),
        directory,
        "headless-normal-publication",
        None,
    );
    assert_eq!(consumer_observation(consumer), binary);
    assert_eq!(
        super::super::derive_roundtrip(directory, "positive", "baseline"),
        actual
    );
    assert_eq!(hash(&absolute_source), actual.source_sha256);
    let bytes = read_bounded(&absolute_candidate, 128 * 1024).unwrap();
    assert!(!bytes.is_empty());
    let candidate_sha = lower_hex_v1(&Sha256::digest(&bytes));
    let text = std::str::from_utf8(&stdout).unwrap();
    let rows: Vec<_> = text
        .lines()
        .filter(|line| line.starts_with(NORMAL_PREFIX))
        .collect();
    assert_eq!(
        rows,
        [format!(
            "{NORMAL_PREFIX}{} {} {}",
            actual.source_sha256,
            candidate_sha,
            bytes.len()
        )]
    );
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    json!({"invocation":actual,"consumer_file_observation":binary,
        "candidate_sha256":candidate_sha,"candidate_bytes":bytes.len(),
        "normal_library_process_published":true,"fixed_register_plan":[4,5,0,1,2],
        "public_driver_return_observed":true,"private_publish_callback_used":false,
        "source_authentication_claim":false,"fresh_frontend_admitted":false,"production_resume":false})
}

#[test]
#[ignore = "normal external consumer plus pinned actual callbacks; serialized Cargo; fresh output"]
fn actual_source_headless_machine_ladder() {
    let consumer = PathBuf::from(
        std::env::var_os(CONSUMER).expect("absolute normal-dependency consumer binary"),
    );
    let consumer_before = consumer_observation(&consumer);
    let directory =
        PathBuf::from(std::env::var_os(HEADLESS_OUTPUT).expect("fresh absolute output"));
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
    let publication = publish_with_normal_consumer(&directory, &consumer);
    let candidate_sha = hash(&case.join("candidate.rs"));
    let default = run_machine_child(&directory, "default");
    assert_eq!(publication["candidate_sha256"], candidate_sha);
    assert_eq!(default["invocation"]["source_sha256"], candidate_sha);
    let edits = fixture_cases::run_edits(&directory);
    let edited_sha = hash(&case.join("edited.rs"));
    assert_ne!(edited_sha, candidate_sha);
    let edited = run_machine_child(&directory, "edited");
    let repeated = run_machine_child(&directory, "repeat");
    assert_eq!(edited["invocation"]["source_sha256"], edited_sha);
    assert_eq!(
        edited["observation"], repeated["observation"],
        "fresh actual callback repeat"
    );
    for identity in ["kernel_ir_sha256", "semantic_sha256", "candidate_sha256"] {
        assert_ne!(
            default["observation"]["fresh"][identity], edited["observation"]["fresh"][identity],
            "changed actual source/registers require fresh identities"
        );
    }
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
    assert_eq!(consumer_observation(&consumer), consumer_before);
    require_current_source();
    let (source_files, source_bytes) = fixture_cases::footprint(&directory);
    let report = json!({
        "normal_consumer_publication":publication,"edit_publications":edits,"observations":observations,
        "normal_library_publication_processes":1,"actual_fresh_callbacks":6,"successful_fresh_callbacks":3,
        "positive_whole_kernel_simulation_runs":90,"additional_negative_simulation_is_bounded":true,
        "exact_refusals":3,"fresh_owner_version":"V17","instruction_profile":"unchanged exact three-op bitselect",
        "source_directory":paths::relative_root(&directory),"source_files":source_files,"source_bytes":source_bytes,
        "source_file_limit":10,"source_byte_limit":10*128*1024,
        "original_sha256":original_sha,"unedited_candidate_sha256":candidate_sha,"edited_candidate_sha256":edited_sha,
        "old_evidence_reused":false,"ranked_checks":false,"source_authentication_claim":false,
        "fixed_production_policy_modified":false,"production_resume":false,"native_qualified":false,
        "functional_proof":false,"hardware_observed":false,"grants_artifact_or_launch_authority":false,
    });
    let bytes = serde_json::to_vec_pretty(&report).unwrap();
    assert!(bytes.len() <= 512 * 1024);
    paths::write_new(&directory.join("observation.json"), &bytes);
    eprintln!(
        "normal-library source candidate/register-edit feasibility: {}",
        directory.display()
    );
}
