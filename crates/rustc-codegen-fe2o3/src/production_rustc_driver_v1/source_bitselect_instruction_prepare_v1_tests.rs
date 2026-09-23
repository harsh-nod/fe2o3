//! Preparation only for the external normal-consumer instruction-edit lane.
//! Reuses actual root-owned Cargo observations; does not call a promotion,
//! private freshness observer, normal exporter, simulator, LLVM or native worker.
use super::*;

const PREPARE_OUTPUT: &str = "FE2O3_TEST_SOURCE_INSTRUCTION_PREPARE_OUTPUT";

#[test]
#[ignore = "prepare-only pinned real Cargo dependencies; root supervisor; fresh absolute output"]
fn actual_source_instruction_edit_prepare() {
    let directory = PathBuf::from(
        std::env::var_os(PREPARE_OUTPUT).expect("fresh absolute preparation directory"),
    );
    assert!(directory.is_absolute());
    assert_eq!(
        directory.parent().unwrap().canonicalize().unwrap(),
        directory.parent().unwrap()
    );
    fs::create_dir(&directory).unwrap();
    assert_eq!(directory.canonicalize().unwrap(), directory);
    assert_eq!(std::env::current_dir().unwrap(), repository());
    // Validates the <=96-byte safe basename and creates a fresh source root
    // below the current repository; no previous phase/source directory is reused.
    let source_root = paths::create_root(&directory);
    require_current_source();
    prepare(&directory);
    fs::create_dir(directory.join("positive")).unwrap();
    let source_case = source_root.join("positive");
    fs::create_dir(&source_case).unwrap();
    paths::write_new(&source_case.join("original.rs"), FIXTURE_FILES[2].1);
    paths::write_new(&source_case.join("original-loader.rs"), ORIGINAL_LOADER);
    let actual = derive_roundtrip(&directory, "positive", "baseline");
    let encoded = serde_json::to_vec_pretty(&actual).unwrap();
    assert!(!encoded.is_empty() && encoded.len() <= 64 * 1024);
    paths::write_new(
        &directory.join("positive/headless-normal.invocation.json"),
        &encoded,
    );
    assert_eq!(derive_roundtrip(&directory, "positive", "baseline"), actual);
    assert_eq!(fs::read_dir(&source_case).unwrap().count(), 2);
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    require_current_source();
    // The external script must itself invoke promote_once with this exact
    // record and then use normal tools; preparation is not acceptance evidence.
}
