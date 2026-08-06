use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_artifacts::{
    AbiLayout, ArtifactContainerV1, BlockSize, BundleIndexV1, CodeObjectFormat, CodeObjectIdentity,
    CodeObjectPayload, CompilerIdentity, DigestAlgorithm, DigestBytes, Dimensions, Endianness,
    IdentityText, KernelEntry, LaunchContract, ManifestV1, Name, PointerWidth, TargetIdentity,
    ToolIdentity,
};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct TempFile(PathBuf);

impl TempFile {
    fn with_bytes(label: &str, bytes: &[u8]) -> Self {
        loop {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = env::temp_dir().join(format!(
                "cargo-fe2o3-inspect-{label}-{}-{id}.bin",
                process::id()
            ));
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    use std::io::Write as _;
                    file.write_all(bytes).expect("write inspect fixture");
                    return Self(path);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("failed to create inspect fixture: {error}"),
            }
        }
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn text(value: &str) -> IdentityText {
    IdentityText::new(value).expect("valid identity text")
}

fn name(value: &str) -> Name {
    Name::new(value).expect("valid name")
}

fn manifest(payload: &[u8]) -> ManifestV1 {
    let object_digest = DigestAlgorithm::Sha256.calculate(payload).bytes();
    let target = TargetIdentity::new(
        text("amdgcn-amd-amdhsa"),
        text("gfx1151"),
        PointerWidth::Bits64,
        Endianness::Little,
        vec![],
    )
    .expect("valid target");
    let object = CodeObjectIdentity::new(
        object_digest,
        CodeObjectFormat::NativeExecutable,
        payload.len() as u64,
    )
    .expect("valid object");
    let launch = LaunchContract::new(
        1,
        BlockSize::Any,
        Dimensions::new(65_535, 1, 1).expect("valid grid"),
        0,
        0,
    )
    .expect("valid launch");
    let abi = AbiLayout::new(0, 1, PointerWidth::Bits64, vec![]).expect("valid empty ABI");
    let kernel = KernelEntry::new(
        DigestBytes::from_bytes([0x11; 32]),
        name("fixture"),
        name("fixture.kd"),
        DigestBytes::from_bytes([0x22; 32]),
        DigestBytes::from_bytes([0x33; 32]),
        object_digest,
        vec![],
        launch,
        abi,
    )
    .expect("valid kernel");
    ManifestV1::new(
        CompilerIdentity::new(text("rustc"), text("1.94.0")),
        ToolIdentity::new(text("fe2o3"), text("0.1.0")),
        target,
        vec![object],
        vec![kernel],
    )
    .expect("valid manifest")
}

fn run_inspect(path: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args(["inspect", path.to_str().expect("UTF-8 fixture path")])
        .output()
        .expect("run inspect")
}

#[test]
fn auto_inspects_manifest_container_and_bundle_fixtures() {
    let payload = b"not-an-executable-fixture";
    let manifest = manifest(payload);
    let container = ArtifactContainerV1::new(
        manifest.clone(),
        DigestAlgorithm::Sha256,
        vec![
            CodeObjectPayload::from_bytes(DigestAlgorithm::Sha256, payload.to_vec())
                .expect("valid payload"),
        ],
    )
    .expect("valid container");
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container))
        .expect("valid bundle index");

    for (label, bytes, expected) in [
        ("manifest", manifest.to_bytes(), "format: fe2o3-manifest-v1"),
        (
            "container",
            container.to_bytes(),
            "format: fe2o3-container-v1",
        ),
        ("bundle", bundle.to_bytes(), "format: fe2o3-bundle-index-v1"),
    ] {
        let fixture = TempFile::with_bytes(label, &bytes);
        let output = run_inspect(&fixture.0);
        assert!(
            output.status.success(),
            "{label} stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("UTF-8 inspect output");
        assert!(stdout.contains(expected), "{label} output: {stdout}");
        assert!(stdout.contains("authority: descriptive-only"));
        assert!(stdout.contains("architecture=gfx1151"));
        assert!(stdout.contains("symbol=fixture.kd"));
        if label == "container" {
            assert!(stdout.contains("abi-fields=0\ndigest-algorithm: sha256"));
        }
    }
}

#[test]
fn malformed_and_unknown_inputs_fail_without_execution() {
    let malformed = TempFile::with_bytes("malformed", b"FE2O3AM\0");
    let output = run_inspect(&malformed.0);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid fe2o3 manifest"));

    let unknown = TempFile::with_bytes("unknown", b"plain text is not an artifact");
    let output = run_inspect(&unknown.0);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized input magic"));
}

#[test]
fn explicit_format_mismatch_fails_closed() {
    let fixture = TempFile::with_bytes("mismatch", &manifest(b"payload").to_bytes());
    let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args([
            "inspect",
            "--format=hsaco",
            fixture.0.to_str().expect("UTF-8 fixture path"),
        ])
        .output()
        .expect("run inspect");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid HSACO"));
}
