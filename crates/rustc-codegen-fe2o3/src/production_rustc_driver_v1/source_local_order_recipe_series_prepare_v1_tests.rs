//! Ignored preparation only: one original fixture, current invocation derivation,
//! no Cargo/rustc child, frontend, recipe decoding or compiler-owner admission.
use super::*;
use crate::production_rustc_driver_v1::gfx942_inline_value_qualification_v30_tests::invocation_for_fixture_source_with_dependencies;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::OpenOptionsExt;

const PREPARE_CONFIG_ENV: &str = "FE2O3_RECIPE_SERIES_PREPARE_CONFIG";
const PREPARE_SHA_ENV: &str = "FE2O3_RECIPE_SERIES_PREPARE_SHA256";
const PREPARE_SCHEMA: &str = "fe2o3-recipe-series-prepare-v1";
const PREPARE_PREFIX: &str = "FE2O3_RECIPE_SERIES_PREPARED_V1 ";
const INPUT_CAP: usize = 16 * 1024;
const CAPTURE_CAP: usize = 16 * 1024 * 1024;
// Setup-only selected metadata envelope; fixed-chunk hashing does not retain it.
// The actual core metadata is larger than the separate text-capture envelope.
const ARTIFACT_CAP: u64 = 128 * 1024 * 1024;
const RECORD_CAP: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Preference {
    SourceOrder,
    ReverseReady,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preparation {
    schema: String,
    directory: PathBuf,
    dependency_directory: PathBuf,
    metadata_sha256: String,
    sysroot_sha256: String,
    dependencies_stdout_sha256: String,
    preference: Preference,
}
fn require_hash(value: &str) {
    assert_eq!(value.len(), 64, "expected lowercase64 SHA256");
    assert!(
        value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
}
fn current_directory(path: &Path) {
    assert!(path.is_absolute() && path.as_os_str().len() <= 4096);
    assert!(fs::symlink_metadata(path).unwrap().file_type().is_dir());
    assert_eq!(path.canonicalize().unwrap(), path);
}
fn pinned_bytes(path: &Path, expected: &str, cap: usize) -> Vec<u8> {
    require_hash(expected);
    assert!(path.is_absolute() && path.as_os_str().len() <= 4096);
    assert_eq!(path.canonicalize().unwrap(), path);
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.is_file() && metadata.nlink() == 1 && metadata.len() > 0);
    let bytes = read_bounded(path, cap).unwrap();
    assert_eq!(
        crate::production_rustc_driver_v1::lower_hex_v1(&Sha256::digest(&bytes)),
        expected
    );
    bytes
}
fn check_captures(input: &Preparation) {
    let _ = pinned_bytes(
        &input.directory.join("metadata.stdout"),
        &input.metadata_sha256,
        CAPTURE_CAP,
    );
    let _ = pinned_bytes(
        &input.directory.join("sysroot.stdout"),
        &input.sysroot_sha256,
        4096,
    );
    let _ = pinned_bytes(
        &input.dependency_directory.join("dependencies.stdout"),
        &input.dependencies_stdout_sha256,
        CAPTURE_CAP,
    );
}
fn selected_path(args: &[String], name: &str) -> PathBuf {
    assert!(matches!(name, "fe2o3_device" | "noprelude:core"));
    let prefix = format!("{name}=");
    let selected = args
        .windows(2)
        .filter_map(|pair| {
            (pair[0] == "--extern")
                .then(|| pair[1].strip_prefix(&prefix))
                .flatten()
        })
        .collect::<Vec<_>>();
    let [path] = selected.as_slice() else {
        panic!("exactly one current selected metadata artifact");
    };
    PathBuf::from(*path)
}
fn artifact_digest(
    reader: &mut impl std::io::Read,
    expected: u64,
    cap: u64,
) -> std::io::Result<[u8; 32]> {
    if expected == 0 || expected > cap {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "metadata size envelope",
        ));
    }
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut remaining = expected;
    while remaining != 0 {
        let limit = remaining.min(buffer.len() as u64) as usize;
        let read = reader.read(&mut buffer[..limit])?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "metadata truncated",
            ));
        }
        hash.update(&buffer[..read]);
        remaining -= read as u64;
    }
    if reader.read(&mut [0_u8; 1])? != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "metadata grew",
        ));
    }
    Ok(hash.finalize().into())
}
fn same_artifact(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    a.is_file()
        && b.is_file()
        && a.nlink() == 1
        && b.nlink() == 1
        && a.dev() == b.dev()
        && a.ino() == b.ino()
        && a.mode() == b.mode()
        && a.len() == b.len()
        && a.mtime() == b.mtime()
        && a.mtime_nsec() == b.mtime_nsec()
        && a.ctime() == b.ctime()
        && a.ctime_nsec() == b.ctime_nsec()
}
fn artifact(args: &[String], name: &str, dependency_directory: &Path) -> Value {
    let path = selected_path(args, name);
    assert_eq!(path.canonicalize().unwrap(), path);
    assert!(
        path.starts_with(
            dependency_directory
                .join("dependencies")
                .canonicalize()
                .unwrap()
        )
    );
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(&path)
        .unwrap();
    let metadata = file.metadata().unwrap();
    assert!(same_artifact(
        &metadata,
        &fs::symlink_metadata(&path).unwrap()
    ));
    let digest = artifact_digest(&mut file, metadata.len(), ARTIFACT_CAP).unwrap();
    assert!(same_artifact(&metadata, &file.metadata().unwrap()));
    assert!(same_artifact(
        &metadata,
        &fs::symlink_metadata(&path).unwrap()
    ));
    assert_eq!(path.canonicalize().unwrap(), path);
    json!({"name":name,"path":path,"bytes":metadata.len(),
        "sha256":crate::production_rustc_driver_v1::lower_hex_v1(&digest)})
}

#[test]
fn recipe_series_artifact_digest_streams_short_reads_and_chunk_boundaries() {
    struct Short<'a>(&'a [u8]);
    impl std::io::Read for Short<'_> {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            let n = out.len().min(self.0.len()).min(3);
            out[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }
    let bytes = vec![17_u8; 64 * 1024 + 7];
    let expected: [u8; 32] = Sha256::digest(&bytes).into();
    assert_eq!(
        artifact_digest(&mut Short(&bytes), bytes.len() as u64, ARTIFACT_CAP).unwrap(),
        expected
    );
    assert_eq!(
        artifact_digest(
            &mut bytes.as_slice(),
            bytes.len() as u64,
            bytes.len() as u64
        )
        .unwrap(),
        expected
    );
    assert_eq!(ARTIFACT_CAP, 134_217_728);
}

#[test]
fn recipe_series_artifact_digest_refuses_empty_oversized_truncated_and_growing() {
    use std::io::ErrorKind;
    let bytes = [1_u8, 2, 3, 4];
    for expected in [0, 5] {
        let mut reader = std::io::Cursor::new(bytes);
        assert_eq!(
            artifact_digest(&mut reader, expected, 4)
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidData
        );
        assert_eq!(reader.position(), 0);
    }
    assert_eq!(
        artifact_digest(&mut &bytes[..3], 4, 4).unwrap_err().kind(),
        ErrorKind::UnexpectedEof
    );
    assert_eq!(
        artifact_digest(&mut bytes.as_slice(), 3, 4)
            .unwrap_err()
            .kind(),
        ErrorKind::InvalidData
    );
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(ErrorKind::Other, "original read error"))
        }
    }
    assert_eq!(
        artifact_digest(&mut Broken, 1, 1).unwrap_err().to_string(),
        "original read error"
    );
}
fn create_config(
    source: &str,
    source_sha256: &str,
    args: &[String],
    preference: Preference,
) -> Value {
    let (order, relation) = match preference {
        Preference::SourceOrder => ("source_order", "xor_before_or"),
        Preference::ReverseReady => ("reverse_ready", "or_before_xor"),
    };
    json!({"schema":"fe2o3-recipe-series-input-v1","mode":"ordinary",
        "source":source,"source_sha256":source_sha256,
        "intent":{"action":"create","order":order,"relation":relation,
            "strength":"exact","binding":"exact_revision"},
        "rustc_args":args,"oracle":null})
}
fn prepare(input: Preparation) -> Value {
    assert_eq!(input.schema, PREPARE_SCHEMA);
    current_directory(&input.directory);
    current_directory(&input.dependency_directory);
    assert_eq!(std::env::current_dir().unwrap(), repository());
    require_current_source();
    check_captures(&input);
    let source_root = paths::create_root(&input.directory);
    let source_directory = source_root.join("positive");
    fs::create_dir(&source_directory).unwrap();
    let text = fixture_cases::source("positive");
    assert!(!text.is_empty() && text.len() <= 64 * 1024);
    paths::write_new(&source_directory.join("source.rs"), text.as_bytes());
    let source = fixture_cases::absolute(&input.directory, "positive");
    let relative = fixture_cases::relative(&input.directory, "positive");
    fs::create_dir(input.directory.join("analysis-output")).unwrap();
    let (args, crate_binding, cargo_observation) = invocation_for_fixture_source_with_dependencies(
        (&input.directory, &input.dependency_directory),
        &fixture(),
        PACKAGE,
        CRATE_NAME,
        Some(FEATURES[0]),
        &source,
        "gfx942",
    );
    let selected = [
        artifact(&args, "fe2o3_device", &input.dependency_directory),
        artifact(&args, "noprelude:core", &input.dependency_directory),
    ];
    let source_sha256 = hash(&source);
    assert_eq!(read_bounded(&source, 64 * 1024).unwrap(), text.as_bytes());
    check_captures(&input);
    require_current_source();
    let environment = std::collections::BTreeMap::from([
        (CRATE_BINDING_ID_ENV_V1.to_owned(), crate_binding.clone()),
        (
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2.to_owned(),
            cargo_observation.clone(),
        ),
        (
            "CARGO_MANIFEST_DIR".to_owned(),
            fixture().to_str().unwrap().to_owned(),
        ),
        ("CARGO_PKG_NAME".to_owned(), PACKAGE.to_owned()),
        ("CARGO_PKG_VERSION".to_owned(), "0.1.0".to_owned()),
        ("CARGO_CRATE_NAME".to_owned(), CRATE_NAME.to_owned()),
    ]);
    assert_eq!(fs::read_dir(&source_root).unwrap().take(2).count(), 1);
    assert_eq!(fs::read_dir(&source_directory).unwrap().take(2).count(), 1);
    assert!(
        fs::read_dir(input.directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    json!({"schema":"fe2o3-recipe-series-prepared-v1",
        "directory":input.directory,"dependency_directory":input.dependency_directory,
        "repository_cwd":repository(),"fixture":fixture(),
        "source_relative":relative,"source_absolute":source,"source_bytes":text.len(),
        "source_sha256":source_sha256,
        "fixture_sha256":FIXTURE_FILES.map(|(path,_)|hash(&fixture().join(path))),
        "metadata_sha256":input.metadata_sha256,"sysroot_sha256":input.sysroot_sha256,
        "dependencies_stdout_sha256":input.dependencies_stdout_sha256,
        "selected_metadata_artifacts":selected,"rustc_args":args,
        "crate_binding":crate_binding,"cargo_observation":cargo_observation,
        "required_child_environment":environment,
        "ordinary_create_config":create_config(relative.to_str().unwrap(),&source_sha256,&args,input.preference),
        "freshness_claim":"derived from supplied current captures; root owns fresh build/tool/input qualification",
        "compiler_frontends_entered":0,"recipe_created":false,"child_processes_spawned":0,
        "grants_artifact_or_launch_authority":false})
}
#[test]
#[ignore = "root-pinned fresh metadata/sysroot/dependencies; creates one task-owned source, no child spawning"]
fn actual_source_local_order_recipe_series_prepare_v1() {
    let config =
        PathBuf::from(std::env::var_os(PREPARE_CONFIG_ENV).expect("pinned preparation config"));
    let expected = std::env::var(PREPARE_SHA_ENV).expect("pinned preparation config SHA256");
    let bytes = pinned_bytes(&config, &expected, INPUT_CAP);
    let input: Preparation = serde_json::from_slice(&bytes).unwrap();
    let directory = input.directory.clone();
    let report = prepare(input);
    let bytes = serde_json::to_vec(&report).unwrap();
    assert!(bytes.len() <= RECORD_CAP);
    let path = directory.join("recipe-series-prepared.json");
    paths::write_new(&path, &bytes);
    assert_eq!(read_bounded(&path, RECORD_CAP).unwrap(), bytes);
    println!("\n{PREPARE_PREFIX}{}", std::str::from_utf8(&bytes).unwrap());
}
#[test]
fn recipe_series_preparation_uses_original_positive_source_and_closed_create() {
    let source = fixture_cases::source("positive");
    assert!(source.contains("let result = (a ^ b) & (c | d);"));
    assert!(source.contains("required = [64, 1, 1], max = [64, 1, 1]"));
    for (preference, order, relation) in [
        (Preference::SourceOrder, "source_order", "xor_before_or"),
        (Preference::ReverseReady, "reverse_ready", "or_before_xor"),
    ] {
        let config = create_config("target/source.rs", &"11".repeat(32), &[], preference);
        assert_eq!(config["mode"], "ordinary");
        assert_eq!(config["oracle"], Value::Null);
        assert_eq!(config["intent"]["order"], order);
        assert_eq!(config["intent"]["relation"], relation);
        assert_eq!(config["intent"]["strength"], "exact");
        assert_eq!(config["intent"]["binding"], "exact_revision");
    }
}
#[test]
fn recipe_series_preparation_selects_exact_two_current_extern_roles() {
    let args = vec![
        "rustc".into(),
        "--extern".into(),
        "fe2o3_device=/fresh/device.rmeta".into(),
        "--extern".into(),
        "noprelude:core=/fresh/core.rmeta".into(),
    ];
    assert_eq!(
        selected_path(&args, "fe2o3_device"),
        Path::new("/fresh/device.rmeta")
    );
    assert_eq!(
        selected_path(&args, "noprelude:core"),
        Path::new("/fresh/core.rmeta")
    );
    assert!(std::panic::catch_unwind(|| selected_path(&[], "fe2o3_device")).is_err());
    let mut duplicate = args.clone();
    duplicate.extend(["--extern".into(), "fe2o3_device=/other.rmeta".into()]);
    assert!(std::panic::catch_unwind(|| selected_path(&duplicate, "fe2o3_device")).is_err());
}
#[test]
fn recipe_series_preparation_config_rejects_extra_or_unbounded_actions() {
    let valid = r#"{"schema":"fe2o3-recipe-series-prepare-v1","directory":"/fresh/case","dependency_directory":"/fresh/deps","metadata_sha256":"","sysroot_sha256":"","dependencies_stdout_sha256":"","preference":"source_order"}"#;
    assert!(serde_json::from_str::<Preparation>(valid).is_ok());
    assert!(
        serde_json::from_str::<Preparation>(&valid.replace("source_order", "caller_passes"))
            .is_err()
    );
    assert!(
        serde_json::from_str::<Preparation>(
            &valid.replace("\"preference\"", "\"unrecognized\":0,\"preference\"")
        )
        .is_err()
    );
}
