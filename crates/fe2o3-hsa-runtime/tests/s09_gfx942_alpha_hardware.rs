use std::fmt;

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
use std::{
    collections::HashSet,
    fs::File,
    io::{Read, Seek, SeekFrom},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::MetadataExt,
    },
    path::Path,
};

#[cfg(feature = "hardware-test-hooks")]
use fe2o3_amd_target::FeatureState;
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_artifacts::{DigestAlgorithm, PayloadDigest};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_core::{DeviceBuffer, GpuContext};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_host::{
    HsaKernelResolutionObservationV1, HsaLaunchGeometryV1, ReviewedHsaExecutableLifecycleAdapterV1,
    ReviewedHsaImplicitKernargAdapterV1,
};
#[cfg(feature = "hardware-test-hooks")]
use fe2o3_hsa_runtime::{
    ReviewedHsaExecutableV1, ReviewedHsaKernelV1, ReviewedHsaRuntimeAdapterV1,
};

const WORKGROUP_SIZE: usize = 256;
const COV6_IMPLICIT_BYTES: usize = 256;
const PHYSICAL_COV6_KERNARG_ALIGNMENT: u64 = 8;
const REVIEWED_HSA_MINIMUM_KERNARG_ALIGNMENT: u64 = 16;
const EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT: u64 =
    reviewed_hsa_resolution_alignment(PHYSICAL_COV6_KERNARG_ALIGNMENT);
const ALPHA_EXPLICIT_BYTES: usize = 40;
const GUARD_PREFIX_ELEMENTS: usize = 8;
const GUARD_SUFFIX_ELEMENTS: usize = 11;
#[cfg(feature = "hardware-test-hooks")]
const HARDWARE_LENGTHS: [usize; 5] = [1, 255, 256, 257, 1023];
#[cfg(feature = "hardware-test-hooks")]
const INPUT_PREFIX: f32 = 12_345.0;
#[cfg(feature = "hardware-test-hooks")]
const INPUT_SUFFIX: f32 = -23_456.0;
const OUTPUT_PREFIX: f32 = 56_789.0;
const OUTPUT_SUFFIX: f32 = -67_890.0;
#[cfg(feature = "hardware-test-hooks")]
const OUTPUT_FILL: f32 = 9_876.0;
const S09_ARTIFACT_FACTS: &str = concat!(
    "format=fe2o3-s09-artifact-facts-v1\n",
    "object_format=elf64-amdgpu\n",
    "arch=amdgcn\n",
    "target=gfx942:xnack-\n",
    "optimization=O0\n",
    "source_path=crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs\n",
    "kernel=alpha:alpha.kd\n",
);

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
const MAX_PINNED_FILE_BYTES: u64 = 64 * 1024 * 1024;

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
const REQUIRED_MEMFD_SEALS: libc::c_int =
    libc::F_SEAL_SEAL | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_WRITE;

#[cfg(feature = "hardware-test-hooks")]
type BoxError = Box<dyn std::error::Error>;

const fn reviewed_hsa_resolution_alignment(physical_alignment: u64) -> u64 {
    if physical_alignment > REVIEWED_HSA_MINIMUM_KERNARG_ALIGNMENT {
        physical_alignment
    } else {
        REVIEWED_HSA_MINIMUM_KERNARG_ALIGNMENT
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LengthMismatch {
    expected: usize,
    actual: usize,
}

impl fmt::Display for LengthMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "alpha output has length {}, but input has length {}",
            self.actual, self.expected
        )
    }
}

impl std::error::Error for LengthMismatch {}

fn parse_sha256(hex: &str) -> Result<[u8; 32], String> {
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("the pinned SHA-256 must be 64 lowercase hex digits".to_owned());
    }
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
            .map_err(|_| "the pinned SHA-256 is malformed".to_owned())?;
    }
    Ok(bytes)
}

fn require_declared_digest(actual: [u8; 32], declared: &str) -> Result<(), String> {
    let expected = parse_sha256(declared)?;
    if actual != expected {
        return Err("file does not match its pinned SHA-256".to_owned());
    }
    Ok(())
}

fn validate_artifact_facts(facts: &[u8]) -> Result<(), String> {
    if facts != S09_ARTIFACT_FACTS.as_bytes() {
        return Err(
            "artifact facts are not exact gfx942:xnack- COV6 alpha/alpha.kd facts".to_owned(),
        );
    }
    Ok(())
}

fn alpha_explicit_kernarg(
    scale: f32,
    input_pointer: u64,
    input_len: usize,
    output_pointer: u64,
    output_len: usize,
) -> Result<[u8; ALPHA_EXPLICIT_BYTES], LengthMismatch> {
    if input_len != output_len {
        return Err(LengthMismatch {
            expected: input_len,
            actual: output_len,
        });
    }
    let mut bytes = [0; ALPHA_EXPLICIT_BYTES];
    put_u32(&mut bytes, 0, scale.to_bits());
    put_u64(&mut bytes, 8, input_pointer);
    put_u64(&mut bytes, 16, input_len as u64);
    put_u64(&mut bytes, 24, output_pointer);
    put_u64(&mut bytes, 32, output_len as u64);
    Ok(bytes)
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn grid_x(length: usize) -> Result<u32, &'static str> {
    if length == 0 {
        return Err("the S09 controller does not dispatch an empty domain");
    }
    u32::try_from(length.div_ceil(WORKGROUP_SIZE))
        .map_err(|_| "the rounded grid exceeds the gfx942 launch contract")
}

fn alpha_input(length: usize) -> Vec<f32> {
    (0..length)
        .map(|index| ((index % 31) as i32 - 15) as f32 * 0.25)
        .collect()
}

fn alpha_oracle(scale: f32, input: &[f32]) -> Vec<f32> {
    input.iter().map(|value| scale * value).collect()
}

fn guarded(body: &[f32], prefix: f32, suffix: f32) -> Vec<f32> {
    let mut values = Vec::with_capacity(GUARD_PREFIX_ELEMENTS + body.len() + GUARD_SUFFIX_ELEMENTS);
    values.extend(std::iter::repeat_n(prefix, GUARD_PREFIX_ELEMENTS));
    values.extend_from_slice(body);
    values.extend(std::iter::repeat_n(suffix, GUARD_SUFFIX_ELEMENTS));
    values
}

fn verify_guarded(
    actual: &[f32],
    expected_body: &[f32],
    prefix: f32,
    suffix: f32,
) -> Result<(), String> {
    let expected_len = GUARD_PREFIX_ELEMENTS + expected_body.len() + GUARD_SUFFIX_ELEMENTS;
    if actual.len() != expected_len {
        return Err(format!(
            "guarded allocation length changed: expected {expected_len}, got {}",
            actual.len()
        ));
    }
    if let Some(index) = actual[..GUARD_PREFIX_ELEMENTS]
        .iter()
        .position(|value| *value != prefix)
    {
        return Err(format!("prefix canary changed at element {index}"));
    }
    let body_end = GUARD_PREFIX_ELEMENTS + expected_body.len();
    if let Some(index) = actual[GUARD_PREFIX_ELEMENTS..body_end]
        .iter()
        .zip(expected_body)
        .position(|(actual, expected)| actual != expected)
    {
        return Err(format!("body differs from CPU oracle at element {index}"));
    }
    if let Some(index) = actual[body_end..].iter().position(|value| *value != suffix) {
        return Err(format!("suffix canary changed at element {index}"));
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn require(condition: bool, message: impl Into<String>) -> Result<(), BoxError> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn parse_exact_proc_fd_path(path: &std::path::Path) -> Option<(u32, i32)> {
    fn canonical_decimal(value: &str, allow_zero: bool) -> bool {
        !value.is_empty()
            && value.bytes().all(|byte| byte.is_ascii_digit())
            && (value == "0" || !value.starts_with('0'))
            && (allow_zero || value != "0")
    }

    let components: Vec<_> = path.to_str()?.split('/').collect();
    let ["", "proc", pid, "fd", descriptor] = components.as_slice() else {
        return None;
    };
    if !canonical_decimal(pid, false) || !canonical_decimal(descriptor, true) {
        return None;
    }
    Some((pid.parse().ok()?, descriptor.parse().ok()?))
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
fn process_parent_and_start_time(pid: u32) -> Result<(u32, u64), BoxError> {
    let path = format!("/proc/{pid}/stat");
    let state = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read snapshot owner state {path}: {error}"))?;
    let closing = state
        .rfind(") ")
        .ok_or_else(|| format!("snapshot owner state is malformed: {path}"))?;
    let fields: Vec<_> = state[closing + 2..].split_whitespace().collect();
    require(
        fields.len() >= 20,
        format!("snapshot owner state is truncated: {path}"),
    )?;
    let parent = fields[1]
        .parse()
        .map_err(|_| format!("snapshot owner parent is malformed: {path}"))?;
    let start_time = fields[19]
        .parse()
        .map_err(|_| format!("snapshot owner starttime is malformed: {path}"))?;
    Ok((parent, start_time))
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
fn live_ancestor_start_time(owner_pid: u32) -> Result<u64, BoxError> {
    let mut current = std::process::id();
    let mut visited = HashSet::new();
    while current > 0 && visited.insert(current) {
        let (parent, start_time) = process_parent_and_start_time(current)?;
        if current == owner_pid {
            return Ok(start_time);
        }
        current = parent;
    }
    Err("snapshot owner must be the current process or a live ancestor".into())
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
fn open_live_pidfd(pid: u32) -> Result<File, BoxError> {
    // SAFETY: pidfd_open has no pointer arguments and the result is checked.
    let descriptor = unsafe { libc::syscall(libc::SYS_pidfd_open, pid, 0) };
    if descriptor < 0 {
        return Err(format!(
            "snapshot owner is not live: {pid}: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    // SAFETY: the successful syscall returned one owned descriptor.
    Ok(unsafe { File::from_raw_fd(descriptor as i32) })
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
fn require_live_pidfd(pidfd: &File) -> Result<(), BoxError> {
    let mut pollfd = libc::pollfd {
        fd: pidfd.as_raw_fd(),
        events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
        revents: 0,
    };
    // SAFETY: pollfd points to one initialized entry for the duration of the call.
    let status = unsafe { libc::poll(&mut pollfd, 1, 0) };
    if status < 0 {
        return Err(format!(
            "cannot inspect snapshot owner liveness: {}",
            std::io::Error::last_os_error()
        )
        .into());
    }
    require(
        status == 0,
        "snapshot owner exited while its memfd was in use",
    )
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
#[derive(Debug, Eq, PartialEq)]
struct PinnedFileIdentity {
    device: u64,
    inode: u64,
    mode: u32,
    size: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

#[cfg(feature = "hardware-test-hooks")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpectedProcFdIdentity {
    owner_pid: u32,
    owner_start_time_ticks: u64,
    device: u64,
    inode: u64,
    mode: u32,
    size: u64,
}

#[cfg(feature = "hardware-test-hooks")]
impl ExpectedProcFdIdentity {
    fn from_environment(prefix: &str) -> Result<Self, BoxError> {
        fn decimal(prefix: &str, suffix: &str, allow_zero: bool) -> Result<u64, BoxError> {
            let key = format!("{prefix}_{suffix}");
            let value = std::env::var(&key).map_err(|_| format!("{key} is not set"))?;
            require(
                !value.is_empty()
                    && value.bytes().all(|byte| byte.is_ascii_digit())
                    && (value == "0" || !value.starts_with('0')),
                format!("{key} is not a canonical decimal"),
            )?;
            let decoded = value
                .parse::<u64>()
                .map_err(|_| format!("{key} exceeds its integer bound"))?;
            require(
                allow_zero || decoded != 0,
                format!("{key} must not be zero"),
            )?;
            Ok(decoded)
        }

        Ok(Self {
            owner_pid: u32::try_from(decimal(prefix, "OWNER_PID", false)?)?,
            owner_start_time_ticks: decimal(prefix, "OWNER_START_TIME_TICKS", false)?,
            device: decimal(prefix, "DEVICE", true)?,
            inode: decimal(prefix, "INODE", false)?,
            mode: u32::try_from(decimal(prefix, "MODE", false)?)?,
            size: decimal(prefix, "SIZE", true)?,
        })
    }
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
impl PinnedFileIdentity {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            size: metadata.size(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }

    fn matches_expected(&self, expected: ExpectedProcFdIdentity) -> bool {
        self.device == expected.device
            && self.inode == expected.inode
            && self.mode == expected.mode
            && self.size == expected.size
    }
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
fn validate_snapshot_owner(
    owner_pid: u32,
    expected: ExpectedProcFdIdentity,
    pidfd: &File,
) -> Result<(), BoxError> {
    require_live_pidfd(pidfd)?;
    require(
        owner_pid == expected.owner_pid,
        "snapshot proc path does not name the expected owner PID",
    )?;
    let observed_start_time = live_ancestor_start_time(owner_pid)?;
    let owner_uid = std::fs::symlink_metadata(format!("/proc/{owner_pid}"))?.uid();
    // SAFETY: getuid has no preconditions.
    require(
        owner_uid == unsafe { libc::getuid() },
        "snapshot owner has the wrong UID",
    )?;
    require(
        observed_start_time == expected.owner_start_time_ticks,
        "snapshot owner PID was reused or has the wrong starttime",
    )?;
    require_live_pidfd(pidfd)
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
fn read_sealed_proc_fd(path: &Path, expected: ExpectedProcFdIdentity) -> Result<Vec<u8>, BoxError> {
    let (owner_pid, _) = parse_exact_proc_fd_path(path)
        .ok_or("sealed snapshot path is not an exact numeric proc-fd path")?;
    let pidfd = open_live_pidfd(owner_pid)?;
    validate_snapshot_owner(owner_pid, expected, &pidfd)?;

    let mut file = File::open(path)?;
    validate_snapshot_owner(owner_pid, expected, &pidfd)?;

    let identity = PinnedFileIdentity::from_metadata(&file.metadata()?);
    require(
        identity.matches_expected(expected),
        "opened snapshot object does not match the supervisor-provided identity",
    )?;
    require(
        identity.mode & libc::S_IFMT == libc::S_IFREG,
        "sealed snapshot must be a regular file",
    )?;
    require(
        (1..=MAX_PINNED_FILE_BYTES).contains(&identity.size),
        format!("sealed snapshot size must be within 1..{MAX_PINNED_FILE_BYTES} bytes"),
    )?;
    let opened_path = format!("/proc/self/fd/{}", file.as_raw_fd());
    let opened_target = std::fs::read_link(opened_path)?;
    let opened_target = opened_target.to_string_lossy();
    require(
        opened_target.starts_with("/memfd:fe2o3-s09-")
            || opened_target.starts_with("memfd:fe2o3-s09-"),
        "sealed snapshot is not an S09 memfd",
    )?;
    // SAFETY: F_GET_SEALS only reads flags from the valid owned descriptor.
    let seals = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GET_SEALS) };
    require(
        seals >= 0 && seals & REQUIRED_MEMFD_SEALS == REQUIRED_MEMFD_SEALS,
        "sealed snapshot is missing required write/resize seals",
    )?;

    file.seek(SeekFrom::Start(0))?;
    let mut bytes = Vec::with_capacity(identity.size as usize);
    (&mut file)
        .take(MAX_PINNED_FILE_BYTES + 1)
        .read_to_end(&mut bytes)?;
    require(
        bytes.len() as u64 == identity.size,
        "sealed snapshot size changed while being read",
    )?;
    let final_identity = PinnedFileIdentity::from_metadata(&file.metadata()?);
    require(
        final_identity == identity,
        "sealed snapshot identity changed while being read",
    )?;
    validate_snapshot_owner(owner_pid, expected, &pidfd)?;
    Ok(bytes)
}

#[cfg(feature = "hardware-test-hooks")]
fn read_pinned_path(
    path: &std::path::Path,
    expected_proc_identity: Option<ExpectedProcFdIdentity>,
) -> Result<Vec<u8>, BoxError> {
    if parse_exact_proc_fd_path(path).is_some() {
        #[cfg(target_os = "linux")]
        return read_sealed_proc_fd(
            path,
            expected_proc_identity
                .ok_or("sealed proc-fd input requires a supervisor-provided object identity")?,
        );
        #[cfg(not(target_os = "linux"))]
        return Err("sealed proc-fd snapshots require Linux".into());
    }

    let metadata = std::fs::symlink_metadata(path)?;
    require(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "pinned path must name a regular non-symlink file",
    )?;
    require(
        std::fs::canonicalize(path)? == path,
        "pinned path must already be canonical",
    )?;
    Ok(std::fs::read(path)?)
}

#[cfg(feature = "hardware-test-hooks")]
fn read_pinned_file(
    path_key: &str,
    digest_key: &str,
) -> Result<(Vec<u8>, PayloadDigest), BoxError> {
    let path = std::path::PathBuf::from(
        std::env::var_os(path_key).ok_or_else(|| format!("{path_key} is not set"))?,
    );
    require(path.is_absolute(), format!("{path_key} must be absolute"))?;
    let declared = std::env::var(digest_key).map_err(|_| format!("{digest_key} is not set"))?;
    #[cfg(target_os = "linux")]
    let expected_proc_identity = if parse_exact_proc_fd_path(&path).is_some() {
        Some(ExpectedProcFdIdentity::from_environment(path_key)?)
    } else {
        None
    };
    #[cfg(not(target_os = "linux"))]
    let expected_proc_identity = None;
    let bytes = read_pinned_path(&path, expected_proc_identity)
        .map_err(|error| format!("{path_key}: {error}"))?;
    let digest = DigestAlgorithm::Sha256.calculate(&bytes);
    let actual = *digest.bytes().as_bytes();
    require_declared_digest(actual, &declared)?;
    Ok((bytes, digest))
}

#[cfg(feature = "hardware-test-hooks")]
fn pinned_s09_artifact() -> Result<(Vec<u8>, PayloadDigest), BoxError> {
    require(
        std::env::var("FE2O3_RUN_S09_GFX942_ALPHA").as_deref() == Ok("1"),
        "set FE2O3_RUN_S09_GFX942_ALPHA=1 to opt into the S09 alpha hardware controller",
    )?;
    let (bytes, digest) = read_pinned_file(
        "FE2O3_S09_GFX942_ALPHA_HSACO",
        "FE2O3_S09_GFX942_ALPHA_SHA256",
    )?;
    let (facts, _) = read_pinned_file(
        "FE2O3_S09_GFX942_ALPHA_FACTS",
        "FE2O3_S09_GFX942_ALPHA_FACTS_SHA256",
    )?;
    validate_artifact_facts(&facts)?;
    Ok((bytes, digest))
}

#[cfg(feature = "hardware-test-hooks")]
struct RuntimeKernarg {
    pointer: std::ptr::NonNull<u8>,
    layout: std::alloc::Layout,
}

#[cfg(feature = "hardware-test-hooks")]
impl RuntimeKernarg {
    fn new(size: u64, alignment: u64) -> Result<Self, BoxError> {
        let layout = std::alloc::Layout::from_size_align(
            usize::try_from(size)?,
            usize::try_from(alignment)?,
        )?;
        // SAFETY: `layout` is valid and this owner deallocates the result once.
        let pointer = std::ptr::NonNull::new(unsafe { std::alloc::alloc_zeroed(layout) })
            .ok_or("failed to allocate runtime-aligned kernarg storage")?;
        Ok(Self { pointer, layout })
    }

    fn bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: the allocation is live and exactly `layout.size()` bytes.
        unsafe { std::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.layout.size()) }
    }
}

#[cfg(feature = "hardware-test-hooks")]
impl Drop for RuntimeKernarg {
    fn drop(&mut self) {
        // SAFETY: this owner deallocates the exact live allocation once.
        unsafe { std::alloc::dealloc(self.pointer.as_ptr(), self.layout) };
    }
}

#[cfg(feature = "hardware-test-hooks")]
fn device_region_pointer(buffer: &DeviceBuffer<f32>, body_len: usize) -> Result<u64, BoxError> {
    require(
        buffer.len() == GUARD_PREFIX_ELEMENTS + body_len + GUARD_SUFFIX_ELEMENTS,
        "guarded device allocation has the wrong extent",
    )?;
    // SAFETY: the allocation contains the checked prefix and complete body.
    let pointer = unsafe { buffer.raw_device_ptr().add(GUARD_PREFIX_ELEMENTS) };
    require(!pointer.is_null(), "non-empty guarded allocation is null")?;
    Ok(u64::try_from(pointer.addr())?)
}

#[cfg(feature = "hardware-test-hooks")]
unsafe fn dispatch_alpha_cov6(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    length: usize,
    explicit: &[u8; ALPHA_EXPLICIT_BYTES],
) -> Result<(), BoxError> {
    let expected_total = ALPHA_EXPLICIT_BYTES + COV6_IMPLICIT_BYTES;
    require(
        resolution.export_symbol() == "alpha",
        "runtime resolution did not bind the exact alpha entry",
    )?;
    require(
        resolution.kernarg_segment_size() == expected_total as u64,
        format!(
            "alpha exposes {} kernarg bytes, expected {expected_total}",
            resolution.kernarg_segment_size()
        ),
    )?;
    require(
        resolution.kernarg_segment_alignment() == EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT,
        format!(
            "alpha exposes HSA kernarg alignment {}, expected {}",
            resolution.kernarg_segment_alignment(),
            EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT
        ),
    )?;

    let mut storage = RuntimeKernarg::new(
        resolution.kernarg_segment_size(),
        resolution.kernarg_segment_alignment(),
    )?;
    let kernarg = storage.bytes_mut();
    kernarg[..ALPHA_EXPLICIT_BYTES].copy_from_slice(explicit);
    let geometry = HsaLaunchGeometryV1::new([grid_x(length)?, 1, 1], [256, 1, 1], 0);

    // SAFETY: the exact digest-pinned alpha-only contract fixes the explicit
    // layout and complete COV6 hidden span; dispatch is synchronous.
    unsafe {
        adapter.initialize_implicit_kernarg(
            executable,
            kernel,
            geometry,
            ALPHA_EXPLICIT_BYTES,
            ALPHA_EXPLICIT_BYTES,
            COV6_IMPLICIT_BYTES,
            kernarg,
        )?;
        let completion = adapter.launch_and_wait(executable, kernel, geometry, kernarg)?;
        require(
            completion.completed(),
            "S09 alpha dispatch did not complete",
        )?;
    }
    Ok(())
}

#[cfg(feature = "hardware-test-hooks")]
fn run_length_case(
    adapter: &mut ReviewedHsaRuntimeAdapterV1,
    context: &std::sync::Arc<GpuContext>,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
    resolution: &HsaKernelResolutionObservationV1,
    length: usize,
) -> Result<(), BoxError> {
    const SCALE: f32 = 1.5;

    let stream = context.default_stream();
    let input_body = alpha_input(length);
    let expected_output = alpha_oracle(SCALE, &input_body);
    let input_host = guarded(&input_body, INPUT_PREFIX, INPUT_SUFFIX);
    let output_initial = guarded(&vec![OUTPUT_FILL; length], OUTPUT_PREFIX, OUTPUT_SUFFIX);
    let input = DeviceBuffer::from_host(&stream, &input_host)?;
    let output = DeviceBuffer::from_host(&stream, &output_initial)?;
    let input_pointer = device_region_pointer(&input, length)?;
    let output_pointer = device_region_pointer(&output, length)?;
    let explicit = alpha_explicit_kernarg(SCALE, input_pointer, length, output_pointer, length)?;

    // SAFETY: all allocations and the exact loaded executable outlive this
    // synchronous call, and the explicit layout was checked above.
    unsafe {
        dispatch_alpha_cov6(adapter, executable, kernel, resolution, length, &explicit)?;
    }

    let input_after = input.to_host_vec(&stream)?;
    let output_after = output.to_host_vec(&stream)?;
    require(
        input_after == input_host,
        "S09 alpha input changed during dispatch",
    )?;
    verify_guarded(
        &output_after,
        &expected_output,
        OUTPUT_PREFIX,
        OUTPUT_SUFFIX,
    )
    .map_err(|error| format!("S09 alpha length {length}: {error}"))?;
    Ok(())
}

/// Executes the local capability S09 alpha-only COV6 hardware controller.
///
/// The fixed runner must first derive the exact physical artifact facts from
/// the same digest-pinned HSACO. This test does not authenticate provenance or
/// promote S09 parity; production admission remains outside this process.
#[cfg(feature = "hardware-test-hooks")]
#[test]
#[ignore = "requires the exact S09 alpha-only COV6 HSACO and a gfx942:xnack- GPU"]
fn s09_gfx942_cov6_alpha_only_controller() -> Result<(), BoxError> {
    let (bytes, digest) = pinned_s09_artifact()?;
    let context = GpuContext::new(0)?;
    let mut adapter = ReviewedHsaRuntimeAdapterV1::new(context.clone())?;
    require(
        adapter.environment().physical_device().target().processor() == "gfx942",
        "the S09 alpha controller requires gfx942",
    )?;
    require(
        adapter.environment().physical_device().target().xnack() == Some(FeatureState::Disabled),
        "the S09 alpha controller requires gfx942:xnack-",
    )?;

    // SAFETY: the immutable bytes are pinned and retained until the one unload.
    let (executable, load) = unsafe { adapter.load_executable(&bytes, digest) }?;
    let executable_identity = load.executable_object();
    let execution = (|| -> Result<(), BoxError> {
        require(load.finalized_digest() == digest, "load digest changed")?;
        require(
            load.byte_len() == bytes.len() as u64,
            "load byte length changed",
        )?;
        // SAFETY: the physical binder facts require exactly alpha/alpha.kd.
        let (kernels, resolutions) = unsafe { adapter.resolve_kernel_set(&executable, ["alpha"]) }?;
        require(
            kernels.len() == 1,
            "runtime did not return one alpha kernel",
        )?;
        require(
            resolutions.len() == 1,
            "runtime did not return one alpha resolution",
        )?;
        require(
            resolutions[0].executable_object() == executable_identity,
            "alpha resolved from a substituted executable",
        )?;
        let kernel = kernels.get(0).ok_or("runtime omitted alpha")?;
        for length in HARDWARE_LENGTHS {
            run_length_case(
                &mut adapter,
                &context,
                &executable,
                kernel,
                &resolutions[0],
                length,
            )?;
        }
        Ok(())
    })();

    // SAFETY: retained kernels were dropped by the completed closure.
    let unload = unsafe { adapter.unload_executable(executable) }?;
    require(unload.released(), "the S09 executable was not released")?;
    require(
        unload.executable_object() == executable_identity,
        "unload released a substituted executable",
    )?;
    execution
}

#[test]
fn alpha_only_artifact_facts_are_exact_and_closed() {
    validate_artifact_facts(S09_ARTIFACT_FACTS.as_bytes()).unwrap();

    let extra_kernel = format!("{S09_ARTIFACT_FACTS}kernel=zeta:zeta.kd\n");
    assert!(validate_artifact_facts(extra_kernel.as_bytes()).is_err());

    let wrong_symbol = S09_ARTIFACT_FACTS.replace("alpha:alpha.kd", "alpha:wrong.kd");
    assert!(validate_artifact_facts(wrong_symbol.as_bytes()).is_err());
}

#[test]
fn pinned_digest_rejects_substitution_and_noncanonical_text() {
    let actual = [0xab; 32];
    let declared = "ab".repeat(32);
    require_declared_digest(actual, &declared).unwrap();

    assert!(require_declared_digest([0xcd; 32], &declared).is_err());
    assert!(require_declared_digest(actual, &declared.to_uppercase()).is_err());
    assert!(require_declared_digest(actual, &declared[..63]).is_err());
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
fn test_memfd_with_seals(name: &str, contents: &[u8], seals: libc::c_int) -> File {
    use std::io::Write;

    let name = std::ffi::CString::new(name).unwrap();
    // SAFETY: name is a valid C string and the returned descriptor is checked.
    let descriptor =
        unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING) };
    assert!(descriptor >= 0, "{}", std::io::Error::last_os_error());
    // SAFETY: the successful syscall returned one owned descriptor.
    let mut file = unsafe { File::from_raw_fd(descriptor) };
    file.write_all(contents).unwrap();
    file.flush().unwrap();
    if seals != 0 {
        // SAFETY: F_ADD_SEALS updates flags on the valid owned descriptor.
        let status = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_ADD_SEALS, seals) };
        assert_eq!(status, 0, "{}", std::io::Error::last_os_error());
    }
    file
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
fn test_memfd(name: &str, contents: &[u8], sealed: bool) -> File {
    test_memfd_with_seals(
        name,
        contents,
        if sealed { REQUIRED_MEMFD_SEALS } else { 0 },
    )
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
fn proc_fd_path(owner_pid: u32, file: &File) -> std::path::PathBuf {
    format!("/proc/{owner_pid}/fd/{}", file.as_raw_fd()).into()
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
fn expected_proc_identity(file: &File, owner_pid: u32) -> ExpectedProcFdIdentity {
    let metadata = file.metadata().unwrap();
    ExpectedProcFdIdentity {
        owner_pid,
        owner_start_time_ticks: process_parent_and_start_time(owner_pid).unwrap().1,
        device: metadata.dev(),
        inode: metadata.ino(),
        mode: metadata.mode(),
        size: metadata.size(),
    }
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
fn set_expected_identity_environment(
    command: &mut std::process::Command,
    prefix: &str,
    expected: ExpectedProcFdIdentity,
) {
    command
        .env(
            format!("{prefix}_OWNER_PID"),
            expected.owner_pid.to_string(),
        )
        .env(
            format!("{prefix}_OWNER_START_TIME_TICKS"),
            expected.owner_start_time_ticks.to_string(),
        )
        .env(format!("{prefix}_DEVICE"), expected.device.to_string())
        .env(format!("{prefix}_INODE"), expected.inode.to_string())
        .env(format!("{prefix}_MODE"), expected.mode.to_string())
        .env(format!("{prefix}_SIZE"), expected.size.to_string());
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
#[test]
fn sealed_proc_fd_reader_accepts_one_pinned_descriptor() {
    let file = test_memfd("fe2o3-s09-rust-positive", b"pinned bytes", true);
    let owner_pid = std::process::id();
    assert_eq!(
        read_sealed_proc_fd(
            &proc_fd_path(owner_pid, &file),
            expected_proc_identity(&file, owner_pid),
        )
        .unwrap(),
        b"pinned bytes"
    );
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
#[test]
fn sealed_proc_fd_actual_ancestor_helper() {
    if std::env::var("FE2O3_S09_ANCESTOR_HELPER").as_deref() != Ok("1") {
        return;
    }
    let path = std::path::PathBuf::from(std::env::var_os("FE2O3_S09_TEST_PATH").unwrap());
    let expected = ExpectedProcFdIdentity::from_environment("FE2O3_S09_TEST").unwrap();
    assert_eq!(
        read_sealed_proc_fd(&path, expected).unwrap(),
        b"ancestor-owned bytes"
    );
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
#[test]
fn sealed_proc_fd_reader_accepts_actual_ancestor_owned_memfd() {
    let file = test_memfd("fe2o3-s09-rust-ancestor", b"ancestor-owned bytes", true);
    let owner_pid = std::process::id();
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .arg("sealed_proc_fd_actual_ancestor_helper")
        .arg("--exact")
        .arg("--nocapture")
        .env("FE2O3_S09_ANCESTOR_HELPER", "1")
        .env("FE2O3_S09_TEST_PATH", proc_fd_path(owner_pid, &file));
    set_expected_identity_environment(
        &mut command,
        "FE2O3_S09_TEST",
        expected_proc_identity(&file, owner_pid),
    );
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "ancestor helper failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
#[test]
fn sealed_proc_fd_reader_accepts_snapshot_supervisor_topology() {
    let directory = std::env::temp_dir().join(format!(
        "fe2o3-s09-supervisor-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let source = directory.join("facts");
    std::fs::write(&source, b"ancestor-owned bytes").unwrap();
    let pinner = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/s09_pinned_snapshot.py");
    let output = std::process::Command::new(pinner)
        .arg("--input")
        .arg(format!("facts={}", source.display()))
        .arg("--")
        .arg("/usr/bin/env")
        .arg("FE2O3_S09_ANCESTOR_HELPER=1")
        .arg("FE2O3_S09_TEST_PATH={facts}")
        .arg("FE2O3_S09_TEST_DEVICE={facts_device}")
        .arg("FE2O3_S09_TEST_INODE={facts_inode}")
        .arg("FE2O3_S09_TEST_MODE={facts_mode}")
        .arg("FE2O3_S09_TEST_SIZE={facts_size}")
        .arg("FE2O3_S09_TEST_OWNER_PID={facts_owner_pid}")
        .arg("FE2O3_S09_TEST_OWNER_START_TIME_TICKS={facts_owner_start_time_ticks}")
        .arg(std::env::current_exe().unwrap())
        .arg("sealed_proc_fd_actual_ancestor_helper")
        .arg("--exact")
        .arg("--nocapture")
        .output()
        .unwrap();
    std::fs::remove_dir_all(directory).unwrap();
    assert!(
        output.status.success(),
        "snapshot supervisor helper failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
#[test]
fn sealed_proc_fd_reader_rejects_unsealed_and_foreign_memfds() {
    let unsealed = test_memfd("fe2o3-s09-rust-unsealed", b"unsealed", false);
    let owner_pid = std::process::id();
    let error = read_sealed_proc_fd(
        &proc_fd_path(owner_pid, &unsealed),
        expected_proc_identity(&unsealed, owner_pid),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("missing required"), "{error}");

    let foreign = test_memfd("foreign-rust-sealed", b"foreign", true);
    let error = read_sealed_proc_fd(
        &proc_fd_path(owner_pid, &foreign),
        expected_proc_identity(&foreign, owner_pid),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("not an S09 memfd"), "{error}");
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
#[test]
fn sealed_proc_fd_reader_rejects_same_content_descriptor_reuse() {
    let original = test_memfd("fe2o3-s09-rust-original", b"same bytes", true);
    let owner_pid = std::process::id();
    let expected = expected_proc_identity(&original, owner_pid);
    let path = proc_fd_path(owner_pid, &original);
    let replacement = test_memfd("fe2o3-s09-rust-replacement", b"same bytes", true);
    assert_ne!(original.as_raw_fd(), replacement.as_raw_fd());
    // SAFETY: both descriptors are live; dup3 atomically replaces only the
    // descriptor owned by `original`, preserving its numeric proc-fd path.
    let rebound = unsafe {
        libc::dup3(
            replacement.as_raw_fd(),
            original.as_raw_fd(),
            libc::O_CLOEXEC,
        )
    };
    assert_eq!(rebound, original.as_raw_fd());
    let error = read_sealed_proc_fd(&path, expected)
        .unwrap_err()
        .to_string();
    assert!(error.contains("supervisor-provided identity"), "{error}");
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
#[test]
fn sealed_proc_fd_reader_rejects_wrong_owner_generation() {
    let file = test_memfd("fe2o3-s09-rust-generation", b"generation", true);
    let owner_pid = std::process::id();
    let mut expected = expected_proc_identity(&file, owner_pid);
    expected.owner_start_time_ticks += 1;
    let error = read_sealed_proc_fd(&proc_fd_path(owner_pid, &file), expected)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("reused") || error.contains("starttime"),
        "{error}"
    );
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
#[test]
fn sealed_proc_fd_reader_rejects_empty_oversized_and_partial_seals() {
    let owner_pid = std::process::id();
    let empty = test_memfd("fe2o3-s09-rust-empty", b"", true);
    let error = read_sealed_proc_fd(
        &proc_fd_path(owner_pid, &empty),
        expected_proc_identity(&empty, owner_pid),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("size must be within"), "{error}");

    let oversized = test_memfd("fe2o3-s09-rust-oversized", b"x", false);
    oversized.set_len(MAX_PINNED_FILE_BYTES + 1).unwrap();
    // SAFETY: F_ADD_SEALS updates flags on the valid owned descriptor.
    assert_eq!(
        unsafe {
            libc::fcntl(
                oversized.as_raw_fd(),
                libc::F_ADD_SEALS,
                REQUIRED_MEMFD_SEALS,
            )
        },
        0
    );
    let error = read_sealed_proc_fd(
        &proc_fd_path(owner_pid, &oversized),
        expected_proc_identity(&oversized, owner_pid),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("size must be within"), "{error}");

    let partial = test_memfd_with_seals(
        "fe2o3-s09-rust-partial",
        b"partial seals",
        libc::F_SEAL_SHRINK | libc::F_SEAL_GROW,
    );
    let error = read_sealed_proc_fd(
        &proc_fd_path(owner_pid, &partial),
        expected_proc_identity(&partial, owner_pid),
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("missing required"), "{error}");
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
#[test]
fn sealed_proc_fd_reader_rejects_non_proc_and_dead_owners() {
    let file = test_memfd("fe2o3-s09-rust-parser", b"parser", true);
    let expected = expected_proc_identity(&file, std::process::id());
    let error = read_sealed_proc_fd(Path::new("/tmp/not-a-proc-fd"), expected)
        .unwrap_err()
        .to_string();
    assert!(error.contains("exact numeric proc-fd"), "{error}");

    let mut child = std::process::Command::new("/usr/bin/sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let child_pid = child.id();
    let child_start = process_parent_and_start_time(child_pid).unwrap().1;
    child.kill().unwrap();
    child.wait().unwrap();
    let dead_expected = ExpectedProcFdIdentity {
        owner_pid: child_pid,
        owner_start_time_ticks: child_start,
        ..expected
    };
    let error = read_sealed_proc_fd(Path::new(&format!("/proc/{child_pid}/fd/0")), dead_expected)
        .unwrap_err()
        .to_string();
    assert!(error.contains("not live"), "{error}");
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
#[test]
fn sealed_proc_fd_reader_rejects_live_nonancestor_owner() {
    let mut child = std::process::Command::new("/usr/bin/sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let path = format!("/proc/{}/fd/0", child.id());
    let expected = ExpectedProcFdIdentity {
        owner_pid: child.id(),
        owner_start_time_ticks: process_parent_and_start_time(child.id()).unwrap().1,
        device: 0,
        inode: 1,
        mode: libc::S_IFREG,
        size: 1,
    };
    let result = read_sealed_proc_fd(Path::new(&path), expected);
    let _ = child.kill();
    let _ = child.wait();
    let error = result.unwrap_err().to_string();
    assert!(error.contains("live ancestor"), "{error}");
}

#[cfg(all(feature = "hardware-test-hooks", target_os = "linux"))]
#[test]
fn ordinary_pinned_file_semantics_remain_canonical_and_non_symlink() {
    use std::os::unix::fs::symlink;

    let directory = std::env::temp_dir().join(format!(
        "fe2o3-s09-ordinary-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let file = directory.join("facts");
    std::fs::write(&file, b"ordinary bytes").unwrap();
    let canonical = std::fs::canonicalize(&file).unwrap();
    assert_eq!(
        read_pinned_path(&canonical, None).unwrap(),
        b"ordinary bytes"
    );
    let link = directory.join("facts-link");
    symlink(&canonical, &link).unwrap();
    assert!(read_pinned_path(&link, None).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(feature = "hardware-test-hooks")]
#[test]
fn proc_fd_path_parser_requires_exact_numeric_spelling() {
    let pid = std::process::id();
    assert_eq!(
        parse_exact_proc_fd_path(std::path::Path::new(&format!("/proc/{pid}/fd/0"))),
        Some((pid, 0))
    );
    for path in [
        "/proc/self/fd/0",
        "/proc/0/fd/0",
        "/proc/01/fd/0",
        "/proc/1/fd/00",
        "/proc/1/fd/-1",
        "/proc/1/fd/0/extra",
        "/tmp/1/fd/0",
    ] {
        assert_eq!(parse_exact_proc_fd_path(std::path::Path::new(path)), None);
    }
}

#[test]
fn boundary_lengths_fix_grid_packing_and_cpu_oracle() {
    const SCALE: f32 = 1.5;
    assert_eq!(
        grid_x(0),
        Err("the S09 controller does not dispatch an empty domain")
    );
    for (length, expected_grid) in [(1, 1), (255, 1), (256, 1), (257, 2), (1023, 4)] {
        assert_eq!(grid_x(length), Ok(expected_grid));
        let input = alpha_input(length);
        let output = alpha_oracle(SCALE, &input);
        assert_eq!(output.len(), length);
        let packed = alpha_explicit_kernarg(SCALE, 0x1122, length, 0x3344, length).unwrap();
        assert_eq!(&packed[0..4], &SCALE.to_bits().to_le_bytes());
        assert_eq!(&packed[8..16], &0x1122_u64.to_le_bytes());
        assert_eq!(&packed[16..24], &(length as u64).to_le_bytes());
        assert_eq!(&packed[24..32], &0x3344_u64.to_le_bytes());
        assert_eq!(&packed[32..40], &(length as u64).to_le_bytes());
    }
    assert!(alpha_explicit_kernarg(SCALE, 1, 255, 2, 256).is_err());
}

#[test]
fn output_verification_rejects_body_and_both_canary_mutations() {
    let expected = alpha_oracle(1.5, &alpha_input(3));
    let canonical = guarded(&expected, OUTPUT_PREFIX, OUTPUT_SUFFIX);
    verify_guarded(&canonical, &expected, OUTPUT_PREFIX, OUTPUT_SUFFIX).unwrap();

    for index in [0, GUARD_PREFIX_ELEMENTS, canonical.len() - 1] {
        let mut corrupted = canonical.clone();
        corrupted[index] += 1.0;
        assert!(
            verify_guarded(&corrupted, &expected, OUTPUT_PREFIX, OUTPUT_SUFFIX).is_err(),
            "accepted corruption at {index}"
        );
    }
}

#[test]
fn cov6_runtime_shape_is_frozen() {
    assert_eq!(COV6_IMPLICIT_BYTES, 256);
    assert_eq!(ALPHA_EXPLICIT_BYTES + COV6_IMPLICIT_BYTES, 296);
    assert_eq!(PHYSICAL_COV6_KERNARG_ALIGNMENT, 8);
    assert_eq!(EXPECTED_HSA_RESOLUTION_KERNARG_ALIGNMENT, 16);
}
