use std::ffi::CString;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

pub const TUTORIAL_RUNTIME_SEMANTIC_OBSERVATION_SCHEMA_V1: &str =
    "fe2o3-tutorial-runtime-semantic-observation-v1";
pub const MAX_TUTORIAL_RUNTIME_SEMANTIC_BYTES_V1: usize = 512 * 1024 * 1024;
pub const MAX_TUTORIAL_RUNTIME_SEMANTIC_REGIONS_V1: usize = 256;

mod semantic_scalar_sealed {
    pub trait Sealed {}

    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for i32 {}
    impl Sealed for f32 {}
    impl Sealed for f64 {}
    impl Sealed for fe2o3_device::Bf16 {}
}

/// Primitive, padding-free value whose complete object representation is semantic data.
pub trait TutorialRuntimeSemanticScalarV1: Copy + semantic_scalar_sealed::Sealed {}

impl TutorialRuntimeSemanticScalarV1 for u8 {}
impl TutorialRuntimeSemanticScalarV1 for u16 {}
impl TutorialRuntimeSemanticScalarV1 for u32 {}
impl TutorialRuntimeSemanticScalarV1 for i32 {}
impl TutorialRuntimeSemanticScalarV1 for f32 {}
impl TutorialRuntimeSemanticScalarV1 for f64 {}
impl TutorialRuntimeSemanticScalarV1 for fe2o3_device::Bf16 {}

/// Returns the complete native object representation of reviewed padding-free scalar values.
pub fn tutorial_runtime_semantic_bytes_v1<T: TutorialRuntimeSemanticScalarV1>(
    values: &[T],
) -> &[u8] {
    // SAFETY: the sealed implementations are padding-free scalar types, the byte extent is
    // exact, and a shared byte view cannot mutate or outlive `values`.
    unsafe { std::slice::from_raw_parts(values.as_ptr().cast(), std::mem::size_of_val(values)) }
}

const OUTPUT_ENV: &str = "FE2O3_TUTORIAL_RUNTIME_SEMANTIC_OBSERVATION_OUTPUT";
const SCRATCH_ENV: &str = "FE2O3_TUTORIAL_HARDWARE_SCRATCH";
const ARTIFACT_ENV: &str = "FE2O3_TUTORIAL_HARDWARE_ARTIFACT_INPUT";
const DRIVER_ENV: &str = "FE2O3_TUTORIAL_HARDWARE_DRIVER_INPUT";
const RUNTIME_ENV: &str = "FE2O3_TUTORIAL_HARDWARE_RUNTIME_INPUT";
const MAX_IDENTITY_FILE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ARTIFACT_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TutorialRuntimeLaunchIdentityV1<'a> {
    pub target: &'a str,
    pub kernel_symbols: &'a [&'a str],
    pub grid: [u32; 3],
    pub workgroup: [u32; 3],
    pub dynamic_lds_bytes: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct TutorialRuntimeSemanticRegionsV1<'a> {
    pub inputs_before: &'a [&'a [u8]],
    pub inputs_after: &'a [&'a [u8]],
    pub canaries_before: &'a [&'a [u8]],
    pub canaries_after: &'a [&'a [u8]],
    pub padding_before: &'a [&'a [u8]],
    pub padding_after: &'a [&'a [u8]],
    pub expected_output: &'a [&'a [u8]],
    pub observed_output: &'a [&'a [u8]],
}

#[derive(Debug)]
pub enum TutorialRuntimeSemanticObservationErrorV1 {
    InvalidLaunch(&'static str),
    InvalidRegions(&'static str),
    EnvironmentMissing(&'static str),
    InvalidPath(&'static str),
    FileTooLarge(&'static str),
    Io(io::Error),
}

impl fmt::Display for TutorialRuntimeSemanticObservationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLaunch(message) => write!(formatter, "invalid launch identity: {message}"),
            Self::InvalidRegions(message) => {
                write!(formatter, "invalid runtime semantic regions: {message}")
            }
            Self::EnvironmentMissing(name) => {
                write!(formatter, "required environment is absent: {name}")
            }
            Self::InvalidPath(name) => {
                write!(formatter, "{name} is not a canonical protected path")
            }
            Self::FileTooLarge(name) => write!(formatter, "{name} exceeds its byte limit"),
            Self::Io(error) => write!(
                formatter,
                "runtime semantic observation I/O failed: {error}"
            ),
        }
    }
}

impl std::error::Error for TutorialRuntimeSemanticObservationErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for TutorialRuntimeSemanticObservationErrorV1 {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Clone, Debug)]
struct ObservationPathsV1 {
    output: PathBuf,
    scratch: PathBuf,
    artifact: PathBuf,
    driver: PathBuf,
    runtime: PathBuf,
}

impl ObservationPathsV1 {
    fn from_environment() -> Result<Option<Self>, TutorialRuntimeSemanticObservationErrorV1> {
        let Some(output) = std::env::var_os(OUTPUT_ENV) else {
            return Ok(None);
        };
        Ok(Some(Self {
            output: PathBuf::from(output),
            scratch: environment_path(SCRATCH_ENV)?,
            artifact: environment_path(ARTIFACT_ENV)?,
            driver: environment_path(DRIVER_ENV)?,
            runtime: environment_path(RUNTIME_ENV)?,
        }))
    }
}

fn environment_path(
    name: &'static str,
) -> Result<PathBuf, TutorialRuntimeSemanticObservationErrorV1> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .ok_or(TutorialRuntimeSemanticObservationErrorV1::EnvironmentMissing(name))
}

/// Publishes one observation only when the authenticated tutorial runner requests it.
///
/// Every equality bit in the document is derived here from retained bytes. A caller cannot
/// supply success booleans or digests. `Ok(false)` means this is an ordinary, non-qualification
/// run and no output environment was present.
pub fn publish_tutorial_runtime_semantic_observation_v1(
    launch: TutorialRuntimeLaunchIdentityV1<'_>,
    regions: TutorialRuntimeSemanticRegionsV1<'_>,
) -> Result<bool, TutorialRuntimeSemanticObservationErrorV1> {
    let Some(paths) = ObservationPathsV1::from_environment()? else {
        return Ok(false);
    };
    publish(&paths, launch, regions)?;
    Ok(true)
}

fn publish(
    paths: &ObservationPathsV1,
    launch: TutorialRuntimeLaunchIdentityV1<'_>,
    regions: TutorialRuntimeSemanticRegionsV1<'_>,
) -> Result<(), TutorialRuntimeSemanticObservationErrorV1> {
    validate_launch(launch)?;
    validate_pair("inputs", regions.inputs_before, regions.inputs_after, false)?;
    validate_pair(
        "canaries",
        regions.canaries_before,
        regions.canaries_after,
        true,
    )?;
    validate_pair(
        "padding",
        regions.padding_before,
        regions.padding_after,
        false,
    )?;
    validate_pair(
        "output",
        regions.expected_output,
        regions.observed_output,
        true,
    )?;

    let scratch = canonical_directory(&paths.scratch, SCRATCH_ENV)?;
    if !paths.output.is_absolute()
        || paths.output.parent() != Some(scratch.as_path())
        || paths.output.file_name().is_none()
        || paths.output.exists()
        || fs::symlink_metadata(&paths.output).is_ok()
    {
        return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidPath(
            OUTPUT_ENV,
        ));
    }

    let artifact = digest_regular_file(&paths.artifact, ARTIFACT_ENV, MAX_ARTIFACT_BYTES)?;
    let driver = digest_regular_file(&paths.driver, DRIVER_ENV, MAX_IDENTITY_FILE_BYTES)?;
    let runtime = digest_regular_file(&paths.runtime, RUNTIME_ENV, MAX_IDENTITY_FILE_BYTES)?;
    let input_before = digest_regions(b"fe2o3-runtime-input-v1\0", launch, regions.inputs_before)?;
    let input_after = digest_regions(b"fe2o3-runtime-input-v1\0", launch, regions.inputs_after)?;
    let canary_before = digest_regions(
        b"fe2o3-runtime-canary-v1\0",
        launch,
        regions.canaries_before,
    )?;
    let canary_after =
        digest_regions(b"fe2o3-runtime-canary-v1\0", launch, regions.canaries_after)?;
    let padding_before = digest_regions(
        b"fe2o3-runtime-padding-v1\0",
        launch,
        regions.padding_before,
    )?;
    let padding_after =
        digest_regions(b"fe2o3-runtime-padding-v1\0", launch, regions.padding_after)?;
    let expected_output = digest_regions(
        b"fe2o3-runtime-output-v1\0",
        launch,
        regions.expected_output,
    )?;
    let observed_output = digest_regions(
        b"fe2o3-runtime-output-v1\0",
        launch,
        regions.observed_output,
    )?;

    let document = encode_document(
        launch,
        artifact,
        driver,
        runtime,
        input_before,
        input_after,
        canary_before,
        canary_after,
        padding_before,
        padding_after,
        expected_output,
        observed_output,
    );
    atomic_publish(&paths.output, &scratch, document.as_bytes())
}

fn validate_launch(
    launch: TutorialRuntimeLaunchIdentityV1<'_>,
) -> Result<(), TutorialRuntimeSemanticObservationErrorV1> {
    if launch.target.is_empty()
        || launch.target.len() > 64
        || !launch
            .target
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidLaunch(
            "target is not a bounded lowercase identifier",
        ));
    }
    if launch.kernel_symbols.is_empty()
        || launch.kernel_symbols.len() > MAX_TUTORIAL_RUNTIME_SEMANTIC_REGIONS_V1
    {
        return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidLaunch(
            "kernel symbol roster is empty or too large",
        ));
    }
    let mut previous = None;
    for symbol in launch.kernel_symbols {
        if symbol.is_empty()
            || symbol.len() > 256
            || !symbol.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-' | b'$')
            })
            || previous.is_some_and(|value| value >= symbol)
        {
            return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidLaunch(
                "kernel symbols are not bounded, canonical, sorted, and unique",
            ));
        }
        previous = Some(symbol);
    }
    let threads = launch
        .workgroup
        .into_iter()
        .try_fold(1_u32, u32::checked_mul);
    if launch.grid.contains(&0)
        || launch.workgroup.contains(&0)
        || !matches!(threads, Some(1..=1024))
    {
        return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidLaunch(
            "grid or workgroup dimensions are invalid",
        ));
    }
    Ok(())
}

fn validate_pair(
    label: &'static str,
    before: &[&[u8]],
    after: &[&[u8]],
    require_nonempty: bool,
) -> Result<(), TutorialRuntimeSemanticObservationErrorV1> {
    validate_regions(label, before, require_nonempty)?;
    validate_regions(label, after, require_nonempty)?;
    if before.len() != after.len()
        || before
            .iter()
            .zip(after)
            .any(|(left, right)| left.len() != right.len() || left != right)
    {
        return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidRegions(
            "before/after or expected/observed bytes differ",
        ));
    }
    Ok(())
}

fn validate_regions(
    label: &'static str,
    regions: &[&[u8]],
    require_nonempty: bool,
) -> Result<(), TutorialRuntimeSemanticObservationErrorV1> {
    if regions.len() > MAX_TUTORIAL_RUNTIME_SEMANTIC_REGIONS_V1 {
        return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidRegions(
            "region count exceeds the limit",
        ));
    }
    let total = regions
        .iter()
        .try_fold(0_usize, |total, region| total.checked_add(region.len()));
    if total.is_none_or(|total| {
        total > MAX_TUTORIAL_RUNTIME_SEMANTIC_BYTES_V1 || (require_nonempty && total == 0)
    }) {
        return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidRegions(
            label,
        ));
    }
    Ok(())
}

fn digest_regions(
    domain: &[u8],
    launch: TutorialRuntimeLaunchIdentityV1<'_>,
    regions: &[&[u8]],
) -> Result<[u8; 32], TutorialRuntimeSemanticObservationErrorV1> {
    validate_regions("digest", regions, false)?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((launch.target.len() as u64).to_le_bytes());
    digest.update(launch.target.as_bytes());
    digest.update((launch.kernel_symbols.len() as u64).to_le_bytes());
    for symbol in launch.kernel_symbols {
        digest.update((symbol.len() as u64).to_le_bytes());
        digest.update(symbol.as_bytes());
    }
    for dimension in launch.grid {
        digest.update(dimension.to_le_bytes());
    }
    for dimension in launch.workgroup {
        digest.update(dimension.to_le_bytes());
    }
    digest.update(launch.dynamic_lds_bytes.to_le_bytes());
    digest.update((regions.len() as u64).to_le_bytes());
    for region in regions {
        digest.update((region.len() as u64).to_le_bytes());
        digest.update(region);
    }
    Ok(digest.finalize().into())
}

fn digest_regular_file(
    path: &Path,
    name: &'static str,
    limit: u64,
) -> Result<[u8; 32], TutorialRuntimeSemanticObservationErrorV1> {
    if !path.is_absolute() || fs::canonicalize(path).ok().as_deref() != Some(path) {
        return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidPath(name));
    }
    let mut file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)?;
    let before = file.metadata()?;
    if !before.file_type().is_file() || before.nlink() != 1 || before.len() == 0 {
        return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidPath(name));
    }
    if before.len() > limit {
        return Err(TutorialRuntimeSemanticObservationErrorV1::FileTooLarge(
            name,
        ));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(before.len()).unwrap_or(0));
    Read::by_ref(&mut file)
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    let current = fs::symlink_metadata(path)?;
    if bytes.len() as u64 != before.len()
        || before.dev() != after.dev()
        || before.ino() != after.ino()
        || before.len() != after.len()
        || before.nlink() != after.nlink()
        || current.file_type().is_symlink()
        || current.dev() != after.dev()
        || current.ino() != after.ino()
        || current.len() != after.len()
        || current.nlink() != after.nlink()
    {
        return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidPath(name));
    }
    Ok(Sha256::digest(bytes).into())
}

fn canonical_directory(
    path: &Path,
    name: &'static str,
) -> Result<PathBuf, TutorialRuntimeSemanticObservationErrorV1> {
    if !path.is_absolute() || fs::canonicalize(path).ok().as_deref() != Some(path) {
        return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidPath(name));
    }
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidPath(name));
    }
    Ok(path.to_path_buf())
}

#[allow(clippy::too_many_arguments)]
fn encode_document(
    launch: TutorialRuntimeLaunchIdentityV1<'_>,
    artifact: [u8; 32],
    driver: [u8; 32],
    runtime: [u8; 32],
    input_before: [u8; 32],
    input_after: [u8; 32],
    canary_before: [u8; 32],
    canary_after: [u8; 32],
    padding_before: [u8; 32],
    padding_after: [u8; 32],
    expected_output: [u8; 32],
    observed_output: [u8; 32],
) -> String {
    let symbols = launch
        .kernel_symbols
        .iter()
        .map(|symbol| format!("\"{symbol}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        concat!(
            "{{\"artifactSha256\":\"{}\",",
            "\"checks\":{{\"canariesChecked\":true,\"completeOutputChecked\":true,",
            "\"inputsUnchangedChecked\":true,\"paddingChecked\":true,\"timedOut\":false}},",
            "\"driverIdentitySha256\":\"{}\",\"kernelSymbols\":[{}],",
            "\"observedIdentities\":{{\"canaryAfterSha256\":\"{}\",",
            "\"canaryBeforeSha256\":\"{}\",\"expectedOutputSha256\":\"{}\",",
            "\"inputAfterSha256\":\"{}\",\"inputBeforeSha256\":\"{}\",",
            "\"observedOutputSha256\":\"{}\",\"paddingAfterSha256\":\"{}\",",
            "\"paddingBeforeSha256\":\"{}\"}},\"runtimeIdentitySha256\":\"{}\",",
            "\"schema\":\"{}\",\"target\":\"{}\"}}\n"
        ),
        hex(artifact),
        hex(driver),
        symbols,
        hex(canary_after),
        hex(canary_before),
        hex(expected_output),
        hex(input_after),
        hex(input_before),
        hex(observed_output),
        hex(padding_after),
        hex(padding_before),
        hex(runtime),
        TUTORIAL_RUNTIME_SEMANTIC_OBSERVATION_SCHEMA_V1,
        launch.target,
    )
}

fn hex(bytes: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn atomic_publish(
    output: &Path,
    scratch: &Path,
    bytes: &[u8],
) -> Result<(), TutorialRuntimeSemanticObservationErrorV1> {
    let file_name =
        output
            .file_name()
            .ok_or(TutorialRuntimeSemanticObservationErrorV1::InvalidPath(
                OUTPUT_ENV,
            ))?;
    let mut temporary = None;
    for attempt in 0..64_u32 {
        let candidate = scratch.join(format!(
            ".{}.{}.{}.tmp",
            file_name.to_string_lossy(),
            std::process::id(),
            attempt
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
            .open(&candidate)
        {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    let (temporary_path, mut file) = temporary.ok_or_else(|| {
        TutorialRuntimeSemanticObservationErrorV1::Io(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "cannot reserve observation temporary file",
        ))
    })?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        let metadata = file.metadata()?;
        if !metadata.file_type().is_file()
            || metadata.nlink() != 1
            || metadata.mode() & 0o777 != 0o600
            || metadata.len() != bytes.len() as u64
        {
            return Err(TutorialRuntimeSemanticObservationErrorV1::InvalidPath(
                OUTPUT_ENV,
            ));
        }
        drop(file);
        rename_no_replace(&temporary_path, output)?;
        File::open(scratch)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn rename_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    let source = CString::new(source.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "source path contains NUL"))?;
    let destination = CString::new(destination.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "output path contains NUL"))?;
    // SAFETY: both C strings are valid for the duration of this no-replace rename syscall.
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn create(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "fe2o3-runtime-observation-{label}-{}-{}",
                std::process::id(),
                std::thread::current().name().unwrap_or("unnamed")
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }

        fn paths(&self) -> ObservationPathsV1 {
            let artifact = self.write("kernel.hsaco", b"exact compiler artifact");
            let driver = self.write("driver.identity", b"exact driver identity");
            let runtime = self.write("runtime.identity", b"exact runtime identity");
            ObservationPathsV1 {
                output: self.0.join("observation.json"),
                scratch: self.0.clone(),
                artifact,
                driver,
                runtime,
            }
        }

        fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, bytes).unwrap();
            path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn launch<'a>(symbols: &'a [&'a str]) -> TutorialRuntimeLaunchIdentityV1<'a> {
        TutorialRuntimeLaunchIdentityV1 {
            target: "gfx950",
            kernel_symbols: symbols,
            grid: [2, 1, 1],
            workgroup: [256, 1, 1],
            dynamic_lds_bytes: 0,
        }
    }

    #[test]
    fn exact_regions_produce_equal_domain_bound_identities() {
        let symbols = ["kernel"];
        let bytes = [b"input".as_slice(), b"second".as_slice()];
        assert_eq!(
            digest_regions(b"domain\0", launch(&symbols), &bytes).unwrap(),
            digest_regions(b"domain\0", launch(&symbols), &bytes).unwrap()
        );
        assert_ne!(
            digest_regions(b"domain\0", launch(&symbols), &bytes).unwrap(),
            digest_regions(b"other\0", launch(&symbols), &bytes).unwrap()
        );
    }

    #[test]
    fn changed_or_partial_regions_fail_closed() {
        assert!(validate_pair("input", &[b"abc"], &[b"abd"], true).is_err());
        assert!(validate_pair("output", &[b"abc"], &[b"ab"], true).is_err());
        assert!(validate_pair("canary", &[], &[], true).is_err());
    }

    #[test]
    fn launch_roster_and_geometry_are_canonical() {
        assert!(validate_launch(launch(&["a", "b"])).is_ok());
        assert!(validate_launch(launch(&["b", "a"])).is_err());
        assert!(validate_launch(launch(&["a", "a"])).is_err());
        let symbols = ["a"];
        let mut invalid = launch(&symbols);
        invalid.workgroup = [1025, 1, 1];
        assert!(validate_launch(invalid).is_err());
    }

    #[test]
    fn document_is_canonical_and_has_no_unreported_fields() {
        let symbols = ["kernel"];
        let document = encode_document(
            launch(&symbols),
            [1; 32],
            [2; 32],
            [3; 32],
            [4; 32],
            [4; 32],
            [5; 32],
            [5; 32],
            [6; 32],
            [6; 32],
            [7; 32],
            [7; 32],
        );
        assert!(document.ends_with("\n"));
        assert!(document.starts_with("{\"artifactSha256\":"));
        assert!(document.contains("\"kernelSymbols\":[\"kernel\"]"));
        assert!(!document.contains(' '));
    }

    #[test]
    fn publication_is_atomic_and_binds_real_regions_and_identity_files() {
        let directory = TestDirectory::create("publish");
        let paths = directory.paths();
        let symbols = ["kernel"];
        let input = b"input".as_slice();
        let canary = b"canary".as_slice();
        let padding = b"padding".as_slice();
        let output = b"complete output".as_slice();
        publish(
            &paths,
            launch(&symbols),
            TutorialRuntimeSemanticRegionsV1 {
                inputs_before: &[input],
                inputs_after: &[input],
                canaries_before: &[canary],
                canaries_after: &[canary],
                padding_before: &[padding],
                padding_after: &[padding],
                expected_output: &[output],
                observed_output: &[output],
            },
        )
        .unwrap();

        let document = fs::read_to_string(&paths.output).unwrap();
        assert!(document.ends_with("\n"));
        assert!(document.contains("\"completeOutputChecked\":true"));
        assert!(document.contains("\"kernelSymbols\":[\"kernel\"]"));
        assert!(document.contains("\"target\":\"gfx950\""));
        assert_eq!(
            fs::symlink_metadata(&paths.output).unwrap().mode() & 0o777,
            0o600
        );
        assert!(fs::read_dir(&directory.0).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
    }

    #[test]
    fn semantic_mismatch_publishes_nothing() {
        let directory = TestDirectory::create("mismatch");
        let paths = directory.paths();
        let symbols = ["kernel"];
        let error = publish(
            &paths,
            launch(&symbols),
            TutorialRuntimeSemanticRegionsV1 {
                inputs_before: &[b"input"],
                inputs_after: &[b"changed"],
                canaries_before: &[b"canary"],
                canaries_after: &[b"canary"],
                padding_before: &[],
                padding_after: &[],
                expected_output: &[b"expected"],
                observed_output: &[b"observed"],
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            TutorialRuntimeSemanticObservationErrorV1::InvalidRegions(_)
        ));
        assert!(!paths.output.exists());
    }

    #[test]
    fn linked_or_replaced_identity_input_is_rejected() {
        let directory = TestDirectory::create("identity-link");
        let mut paths = directory.paths();
        let linked_driver = directory.0.join("linked-driver.identity");
        symlink(&paths.driver, &linked_driver).unwrap();
        paths.driver = linked_driver;
        let symbols = ["kernel"];
        let error = publish(
            &paths,
            launch(&symbols),
            TutorialRuntimeSemanticRegionsV1 {
                inputs_before: &[],
                inputs_after: &[],
                canaries_before: &[b"canary"],
                canaries_after: &[b"canary"],
                padding_before: &[],
                padding_after: &[],
                expected_output: &[b"output"],
                observed_output: &[b"output"],
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            TutorialRuntimeSemanticObservationErrorV1::InvalidPath(DRIVER_ENV)
        ));
        assert!(!paths.output.exists());
    }
}
