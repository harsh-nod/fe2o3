//! Root-built fixed artifacts, observed before/after every CLI call; not source custody.
use super::*;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
const R: &str = "/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr";
pub(super) const INPUT_ENV: &str = "FE2O3_TEST_ORDERED_COMPOSITION_CLI_BUILD_INPUT_V1";
#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Pin {
    path: String,
    bytes: u64,
    sha256: String,
}
#[derive(Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Build {
    schema: String,
    extractor: Pin,
    backend: Pin,
    build_receipt: Pin,
}
pub(super) fn extractor() -> PathBuf {
    Path::new(R).join("target-milestones-phase28-v19-r1/debug/fe2o3-rustc-extract")
}
fn backend() -> PathBuf {
    Path::new(R).join("target-milestones-phase28-v19-r1/debug/librustc_codegen_fe2o3.so")
}
pub(super) fn runtime_library_path() -> std::ffi::OsString {
    let library = backend();
    std::env::join_paths([
        library.parent().expect("fixed backend parent"),
        Path::new(
            "/home/harmenon/.rustup/toolchains/nightly-2026-04-03-x86_64-unknown-linux-gnu/lib",
        ),
    ])
    .expect("fixed runtime library paths")
}
fn stamp(m: &fs::Metadata) -> (u64, u64, u64, i64, i64, i64, i64) {
    (
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    )
}
fn observe(path: &Path, cap: u64) -> Pin {
    let named = fs::symlink_metadata(path).unwrap();
    assert!(
        named.is_file() && !named.file_type().is_symlink() && named.len() > 0 && named.len() <= cap
    );
    let mut file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .unwrap();
    assert_eq!(stamp(&named), stamp(&file.metadata().unwrap()));
    let mut count = 0_u64;
    let mut hash = Sha256::new();
    let mut scratch = [0_u8; 64 * 1024];
    loop {
        let n = file.read(&mut scratch).unwrap();
        if n == 0 {
            break;
        }
        count = count.checked_add(n as u64).unwrap();
        assert!(count <= named.len());
        hash.update(&scratch[..n]);
    }
    assert_eq!(count, named.len());
    assert_eq!(stamp(&named), stamp(&file.metadata().unwrap()));
    assert_eq!(stamp(&named), stamp(&fs::symlink_metadata(path).unwrap()));
    Pin {
        path: path.to_str().unwrap().into(),
        bytes: count,
        sha256: super::super::super::super::super::lower_hex_v1(&hash.finalize()),
    }
}
fn validate(build: &Build) -> Result<(), &'static str> {
    if build.schema != "fe2o3-test-ordered-composition-cli-build-input-v1" {
        return Err("build schema");
    }
    if Path::new(&build.extractor.path) != extractor()
        || Path::new(&build.backend.path) != backend()
    {
        return Err("fixed build paths");
    }
    let receipt = Path::new(&build.build_receipt.path);
    if !receipt.is_absolute()
        || !receipt.starts_with(Path::new(R).join("logs"))
        || receipt.file_name().and_then(|s| s.to_str()) != Some("receipt.json")
        || receipt.components().any(|c| {
            !matches!(
                c,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
    {
        return Err("build receipt path");
    }
    for p in [&build.extractor, &build.backend, &build.build_receipt] {
        if p.bytes == 0
            || p.sha256.len() != 64
            || p.sha256 == "0".repeat(64)
            || !p
                .sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err("build pin shape");
        }
    }
    if build.extractor.bytes > 512 * 1024 * 1024
        || build.backend.bytes > 512 * 1024 * 1024
        || build.build_receipt.bytes > 16 * 1024 * 1024
    {
        return Err("build pin bounds");
    }
    Ok(())
}
impl Build {
    pub(super) fn read() -> Self {
        let path = PathBuf::from(
            std::env::var_os(INPUT_ENV).expect("root-owned successful build input required"),
        );
        let bytes = read_bounded(&path, 16 * 1024).unwrap();
        let value: Self = serde_json::from_slice(&bytes).unwrap();
        validate(&value).unwrap();
        value.recheck();
        value
    }
    pub(super) fn recheck(&self) {
        assert_eq!(observe(&extractor(), 512 * 1024 * 1024), self.extractor);
        assert_eq!(observe(&backend(), 512 * 1024 * 1024), self.backend);
        assert_eq!(
            observe(Path::new(&self.build_receipt.path), 16 * 1024 * 1024),
            self.build_receipt
        );
    }
}
#[test]
fn fixed_binary_inputs_do_not_accept_arbitrary_executables_or_zero_pins() {
    let pin = |path: PathBuf| Pin {
        path: path.to_str().unwrap().into(),
        bytes: 1,
        sha256: "1".repeat(64),
    };
    let mut b = Build {
        schema: "fe2o3-test-ordered-composition-cli-build-input-v1".into(),
        extractor: pin(extractor()),
        backend: pin(backend()),
        build_receipt: pin(Path::new(R).join("logs/reviewed/receipt.json")),
    };
    assert!(validate(&b).is_ok());
    b.extractor.path = "/bin/true".into();
    assert_eq!(validate(&b), Err("fixed build paths"));
    b.extractor.path = extractor().to_str().unwrap().into();
    b.backend.sha256 = "0".repeat(64);
    assert_eq!(validate(&b), Err("build pin shape"));
    b.backend.sha256 = "1".repeat(64);
    b.build_receipt.path = Path::new(R)
        .join("logs/../candidate/receipt.json")
        .to_str()
        .unwrap()
        .into();
    assert_eq!(validate(&b), Err("build receipt path"));
}

#[test]
fn cli_runtime_path_is_fixed_and_replaces_an_ambient_override() {
    let runtime = runtime_library_path();
    assert_eq!(
        std::env::split_paths(&runtime).collect::<Vec<_>>(),
        vec![
            Path::new(R).join("target-milestones-phase28-v19-r1/debug"),
            PathBuf::from(
                "/home/harmenon/.rustup/toolchains/nightly-2026-04-03-x86_64-unknown-linux-gnu/lib"
            ),
        ]
    );
    assert_eq!(backend().parent(), extractor().parent());
    let mut command = Command::new(extractor());
    command.env("LD_LIBRARY_PATH", "/untrusted/ambient");
    command.env("LD_LIBRARY_PATH", runtime_library_path());
    let (_, value) = command
        .get_envs()
        .find(|(name, _)| *name == std::ffi::OsStr::new("LD_LIBRARY_PATH"))
        .expect("explicit child runtime path");
    assert_eq!(value, Some(runtime.as_os_str()));
}
