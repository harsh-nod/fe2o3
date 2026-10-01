//! No-spawn setup adapters for the unchanged historical nine-case publisher.
//! The external parent owns all Cargo/child supervision and partial-stream custody.
use super::*;

const SETUP_ROOT: &str = "FE2O3_TEST_ORDERED_PUBLISH_SETUP_ROOT_V1";
const SETUP_SYSROOT: &str = "FE2O3_TEST_ORDERED_PUBLISH_SETUP_SYSROOT_V1";
const SETUP_PREFIX: &str = "FE2O3_ORDERED_COMPOSITION_PUBLISH_SETUP_V1 ";
const PACKAGES: [&str; 3] = [
    "package-original",
    "package-promoted-copy",
    "package-promoted-edit",
];

fn setup_root() -> PathBuf {
    let root = PathBuf::from(std::env::var_os(SETUP_ROOT).expect("fresh setup root"));
    assert!(root.is_absolute());
    root
}
fn save_setup(root: &Path, name: &str, value: &impl Serialize) {
    let bytes = serde_json::to_vec(value).unwrap();
    create(&root.join(name), &bytes, 256 * 1024);
}
fn emit_setup(value: &Value) {
    let text = serde_json::to_string(value).unwrap();
    assert!(text.len() <= 256 * 1024 && !text.contains('\n') && !text.contains('\r'));
    println!("\n{SETUP_PREFIX}{text}");
}

#[test]
#[ignore = "fresh root and pinned sysroot; external durable parent runs all Cargo commands"]
fn prepare_ordered_publish_packages() {
    let root = setup_root();
    assert!(!root.exists(), "never reuse a setup root");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    assert!(!root.starts_with(repository().parent().unwrap()));
    let sysroot = PathBuf::from(std::env::var_os(SETUP_SYSROOT).expect("pinned sysroot"));
    assert!(sysroot.is_absolute());
    assert_eq!(sysroot.canonicalize().unwrap(), sysroot);
    assert!(
        sysroot
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("nightly-2026-04-03-")
    );
    let source = staging::fixture_source();
    let selected_sources = super::super::inputs::current_sources();
    let workspace = read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap();
    for name in PACKAGES {
        staging::package_with_source(
            &root,
            name,
            (name == "package-original").then_some(source.as_slice()),
        );
        let prep = preparation(&root, name);
        fs::create_dir(&prep).unwrap();
        fs::create_dir(prep.join("analysis-output")).unwrap();
        create(
            &prep.join("sysroot.stdout"),
            format!("{}\n", sysroot.display()).as_bytes(),
            4096,
        );
        create(&prep.join("workspace-lock.seed"), &workspace, 1024 * 1024);
        create(&root.join(name).join("Cargo.lock"), &workspace, 1024 * 1024);
    }
    assert_eq!(staging::fixture_source(), source);
    assert_eq!(super::super::inputs::current_sources(), selected_sources);
    assert_eq!(
        read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap(),
        workspace
    );
    let facts = json!({
        "schema":"fe2o3-test-ordered-publish-package-preparation-v1",
        "packages":PACKAGES,"cases":CASES,"source_sha256":digest(&source),
        "workspace_lock_sha256":digest(&workspace),"sysroot":sysroot,
        "selected_provider_sources":selected_sources,
        "subprocesses_spawned":0,"frontend_sessions":0,
        "metadata_commands_required":6,"dependency_builds_required":1,
        "normal_checked_handoff_qualified":false,"acceptance":"preparation only",
    });
    save_setup(&root, "historical-preparation.json", &facts);
    emit_setup(&facts);
}

#[test]
#[ignore = "after external metadata/dependency setup or successful preceding publication"]
fn prepare_ordered_publish_invocation() {
    let root = setup_root();
    assert_eq!(root.canonicalize().unwrap(), root);
    assert_eq!(
        std::env::current_dir().unwrap().canonicalize().unwrap(),
        root
    );
    let case = std::env::var(CASE_ENV).expect("closed historical case");
    assert!(CASES.contains(&case.as_str()));
    let workspace = read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap();
    for name in PACKAGES {
        let prep = preparation(&root, name);
        assert_eq!(
            read_bounded(&prep.join("workspace-lock.seed"), 1024 * 1024).unwrap(),
            workspace
        );
        let resolve = read_bounded(&prep.join("resolve.stdout"), 16 * 1024 * 1024).unwrap();
        let metadata = read_bounded(&prep.join("metadata.stdout"), 16 * 1024 * 1024).unwrap();
        assert_eq!(resolve, metadata, "locked actual metadata differs");
        for feature in staging::FEATURES {
            staging::metadata_feature(&metadata, &root.join(name), feature).unwrap();
        }
        let lock = read_bounded(&root.join(name).join("Cargo.lock"), 1024 * 1024).unwrap();
        staging::closure_matches(&workspace, &lock).unwrap();
    }
    assert_eq!(
        read_bounded(
            &root.join("package-original").join(staging::LEAF),
            64 * 1024
        )
        .unwrap(),
        staging::fixture_source(),
    );
    // Original derivation binds actual feature/package/source, original provider
    // subset, all metadata/lock/sysroot inputs and the fresh dependency tree.
    // It is not a whole compiler-closure attestation; root pins that separately.
    let record = derive(&root, &case);
    save_setup(&root, &format!("{case}.invocation.json"), &record);
    emit_setup(&json!({
        "schema":"fe2o3-test-ordered-publish-invocation-preparation-v1",
        "case":case,"invocation":record,"subprocesses_spawned":0,"frontend_sessions":0,
        "normal_checked_handoff_qualified":false,"acceptance":"invocation only",
    }));
}
