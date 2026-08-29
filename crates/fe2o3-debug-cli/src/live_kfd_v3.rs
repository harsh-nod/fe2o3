//! Exact semantic-input binding for a direct-KFD live debugger session.
//!
//! The binding retains authority-free CPU reference inputs. A captured HSACO
//! is only declared, and a host executable is only observed as a pre-exec file.
//! Neither record claims that bytes were loaded, launched, or executed.

use std::collections::BTreeSet;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

use fe2o3_kernel_ir::{MAX_SIMULATION_BUNDLE_BYTES_V2, VerifiedSimulationBundleV2};
use fe2o3_kir_sim_cli::{AdmittedSimulationInputV1, load_debug_simulation_bundle_v2};
use sha2::{Digest, Sha256};

const MAX_REQUEST_BYTES_V3: usize = 16 * 1024 * 1024;
const MAX_HOST_EXECUTABLE_BYTES_V3: usize = 256 * 1024 * 1024;
const SESSION_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/LIVE-KFD-SEMANTIC-SESSION/V3\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveKfdBindingLimitsV3 {
    max_bundle_bytes: usize,
    max_request_bytes: usize,
    max_hsaco_bytes: usize,
    max_host_executable_bytes: usize,
}

impl Default for LiveKfdBindingLimitsV3 {
    fn default() -> Self {
        Self {
            max_bundle_bytes: MAX_SIMULATION_BUNDLE_BYTES_V2,
            max_request_bytes: MAX_REQUEST_BYTES_V3,
            max_hsaco_bytes: fe2o3_hsaco::MAX_HSACO_BYTES,
            max_host_executable_bytes: MAX_HOST_EXECUTABLE_BYTES_V3,
        }
    }
}

impl LiveKfdBindingLimitsV3 {
    pub fn try_new(
        max_bundle_bytes: usize,
        max_request_bytes: usize,
        max_hsaco_bytes: usize,
        max_host_executable_bytes: usize,
    ) -> Result<Self, LiveKfdBindingErrorV3> {
        if max_bundle_bytes == 0
            || max_bundle_bytes > MAX_SIMULATION_BUNDLE_BYTES_V2
            || max_request_bytes == 0
            || max_request_bytes > MAX_REQUEST_BYTES_V3
            || max_hsaco_bytes == 0
            || max_hsaco_bytes > fe2o3_hsaco::MAX_HSACO_BYTES
            || max_host_executable_bytes == 0
            || max_host_executable_bytes > MAX_HOST_EXECUTABLE_BYTES_V3
        {
            return Err(LiveKfdBindingErrorV3::new(
                LiveKfdInputRoleV3::Session,
                LiveKfdBindingErrorCodeV3::InvalidPlan,
                "live KFD semantic binding limits are inconsistent",
            ));
        }
        Ok(Self {
            max_bundle_bytes,
            max_request_bytes,
            max_hsaco_bytes,
            max_host_executable_bytes,
        })
    }

    pub const fn max_bundle_bytes(self) -> usize {
        self.max_bundle_bytes
    }

    pub const fn max_request_bytes(self) -> usize {
        self.max_request_bytes
    }

    pub const fn max_hsaco_bytes(self) -> usize {
        self.max_hsaco_bytes
    }

    pub const fn max_host_executable_bytes(self) -> usize {
        self.max_host_executable_bytes
    }
}

/// Path-bearing capture plan. Its debug view deliberately omits every path.
pub struct LiveKfdSemanticSessionPlanV3 {
    bundle: PathBuf,
    request: PathBuf,
    declared_hsaco: Option<PathBuf>,
    host_executable: PathBuf,
    limits: LiveKfdBindingLimitsV3,
}

impl LiveKfdSemanticSessionPlanV3 {
    pub fn try_new(
        bundle: impl Into<PathBuf>,
        request: impl Into<PathBuf>,
        declared_hsaco: Option<PathBuf>,
        host_executable: impl Into<PathBuf>,
        limits: LiveKfdBindingLimitsV3,
    ) -> Result<Self, LiveKfdBindingErrorV3> {
        let plan = Self {
            bundle: bundle.into(),
            request: request.into(),
            declared_hsaco,
            host_executable: host_executable.into(),
            limits,
        };
        if plan.bundle.as_os_str().is_empty()
            || plan.request.as_os_str().is_empty()
            || plan.host_executable.as_os_str().is_empty()
            || plan
                .declared_hsaco
                .as_ref()
                .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err(LiveKfdBindingErrorV3::new(
                LiveKfdInputRoleV3::Session,
                LiveKfdBindingErrorCodeV3::InvalidPlan,
                "live KFD semantic input paths must be nonempty",
            ));
        }
        Ok(plan)
    }

    pub const fn limits(&self) -> LiveKfdBindingLimitsV3 {
        self.limits
    }
}

impl std::fmt::Debug for LiveKfdSemanticSessionPlanV3 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LiveKfdSemanticSessionPlanV3")
            .field("declared_hsaco_present", &self.declared_hsaco.is_some())
            .field("limits", &self.limits)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LiveKfdContentIdentityV3 {
    sha256: [u8; 32],
    length: u64,
}

impl LiveKfdContentIdentityV3 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn length(self) -> u64 {
        self.length
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredHsacoIdentityV3 {
    content: LiveKfdContentIdentityV3,
    code_object_version: u8,
}

impl DeclaredHsacoIdentityV3 {
    pub const fn content(self) -> LiveKfdContentIdentityV3 {
        self.content
    }

    pub const fn code_object_version(self) -> u8 {
        self.code_object_version
    }

    pub const fn claims_loaded(self) -> bool {
        false
    }

    pub const fn claims_executed(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedPreExecHostIdentityV3 {
    content: LiveKfdContentIdentityV3,
}

impl ObservedPreExecHostIdentityV3 {
    pub const fn content(self) -> LiveKfdContentIdentityV3 {
        self.content
    }

    pub const fn claims_launched(self) -> bool {
        false
    }

    pub const fn claims_executed(self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum LiveKfdSemanticCapabilityNameV3 {
    CpuReference,
    SourceMapV2,
    DeclaredHsacoBytes,
    HostPreExecFileIdentity,
    ExactLaunchedHostContent,
    HsacoLoadedIdentity,
    GpuExecutionIdentity,
    KfdLiveObservation,
}

impl LiveKfdSemanticCapabilityNameV3 {
    const ALL: [Self; 8] = [
        Self::CpuReference,
        Self::SourceMapV2,
        Self::DeclaredHsacoBytes,
        Self::HostPreExecFileIdentity,
        Self::ExactLaunchedHostContent,
        Self::HsacoLoadedIdentity,
        Self::GpuExecutionIdentity,
        Self::KfdLiveObservation,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveKfdSemanticUnavailableReasonV3 {
    HsacoNotDeclared,
    LoadNotObserved,
    ExecutionNotObserved,
    LiveSessionNotAttached,
    ExecStopNotObserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveKfdSemanticCapabilityAvailabilityV3 {
    Available,
    Unavailable(LiveKfdSemanticUnavailableReasonV3),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveKfdSemanticCapabilityV3 {
    pub name: LiveKfdSemanticCapabilityNameV3,
    pub availability: LiveKfdSemanticCapabilityAvailabilityV3,
}

/// Authority-free inputs for CPU reference correlation and a future live KFD session.
pub struct LiveKfdSemanticSessionBindingV3 {
    session_identity: [u8; 32],
    bundle_identity: [u8; 32],
    bundle_content_identity: LiveKfdContentIdentityV3,
    request_content_identity: LiveKfdContentIdentityV3,
    declared_hsaco: Option<DeclaredHsacoIdentityV3>,
    observed_host: ObservedPreExecHostIdentityV3,
    admitted_input: AdmittedSimulationInputV1,
    bundle: VerifiedSimulationBundleV2,
    request_bytes: Vec<u8>,
    declared_hsaco_bytes: Option<Vec<u8>>,
    host_executable: RetainedHostExecutableV3,
}

impl LiveKfdSemanticSessionBindingV3 {
    pub const fn session_identity(&self) -> [u8; 32] {
        self.session_identity
    }

    pub const fn bundle_identity(&self) -> [u8; 32] {
        self.bundle_identity
    }

    pub const fn bundle_content_identity(&self) -> LiveKfdContentIdentityV3 {
        self.bundle_content_identity
    }

    pub const fn request_content_identity(&self) -> LiveKfdContentIdentityV3 {
        self.request_content_identity
    }

    pub const fn declared_hsaco(&self) -> Option<DeclaredHsacoIdentityV3> {
        self.declared_hsaco
    }

    pub const fn observed_host(&self) -> ObservedPreExecHostIdentityV3 {
        self.observed_host
    }

    pub const fn host_launch_content(&self) -> LiveKfdHostLaunchContentV3 {
        match self.host_executable.launch_state {
            HostLaunchStateV3::ObservedPreExec => {
                LiveKfdHostLaunchContentV3::ObservedPreExec(self.observed_host)
            }
            HostLaunchStateV3::ExecSigtrapObserved => {
                LiveKfdHostLaunchContentV3::ExactLaunchedAfterExecSigtrap {
                    content: self.observed_host.content,
                }
            }
        }
    }

    /// The coordinator may use this descriptor to construct its exact
    /// `/proc/self/fd/N` launch path. It never crosses the crate boundary.
    pub(crate) fn host_executable_fd(&self) -> BorrowedFd<'_> {
        self.host_executable.file.as_fd()
    }

    /// Upgrades the host binding only after the coordinator observes the
    /// launch-owned exec SIGTRAP. This proves image selection, not execution.
    pub(crate) fn record_host_exec_sigtrap_v3(&mut self) -> Result<(), LiveKfdBindingErrorV3> {
        self.host_executable.revalidate_descriptor()?;
        self.host_executable.launch_state = HostLaunchStateV3::ExecSigtrapObserved;
        Ok(())
    }

    pub fn admitted_input(&self) -> &AdmittedSimulationInputV1 {
        &self.admitted_input
    }

    pub fn bundle(&self) -> &VerifiedSimulationBundleV2 {
        &self.bundle
    }

    pub fn bundle_bytes(&self) -> &[u8] {
        self.bundle.canonical_bytes()
    }

    pub fn request_bytes(&self) -> &[u8] {
        &self.request_bytes
    }

    /// Returns the exact inspected, authority-free HSACO declaration bytes.
    pub fn declared_hsaco_bytes(&self) -> Option<&[u8]> {
        self.declared_hsaco_bytes.as_deref()
    }

    pub fn capabilities(&self) -> Vec<LiveKfdSemanticCapabilityV3> {
        use LiveKfdSemanticCapabilityAvailabilityV3::{Available, Unavailable};
        use LiveKfdSemanticCapabilityNameV3 as Name;
        use LiveKfdSemanticUnavailableReasonV3 as Reason;
        LiveKfdSemanticCapabilityNameV3::ALL
            .into_iter()
            .map(|name| {
                let availability = match name {
                    Name::CpuReference | Name::SourceMapV2 | Name::HostPreExecFileIdentity => {
                        Available
                    }
                    Name::ExactLaunchedHostContent
                        if self.host_executable.launch_state
                            == HostLaunchStateV3::ExecSigtrapObserved =>
                    {
                        Available
                    }
                    Name::ExactLaunchedHostContent => Unavailable(Reason::ExecStopNotObserved),
                    Name::DeclaredHsacoBytes if self.declared_hsaco.is_some() => Available,
                    Name::DeclaredHsacoBytes => Unavailable(Reason::HsacoNotDeclared),
                    Name::HsacoLoadedIdentity => Unavailable(Reason::LoadNotObserved),
                    Name::GpuExecutionIdentity => Unavailable(Reason::ExecutionNotObserved),
                    Name::KfdLiveObservation => Unavailable(Reason::LiveSessionNotAttached),
                };
                LiveKfdSemanticCapabilityV3 { name, availability }
            })
            .collect()
    }

    pub fn into_cpu_reference_parts(
        self,
    ) -> (
        AdmittedSimulationInputV1,
        VerifiedSimulationBundleV2,
        Vec<u8>,
    ) {
        (self.admitted_input, self.bundle, self.request_bytes)
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub const fn authenticates_gpu_execution(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveKfdHostLaunchContentV3 {
    ObservedPreExec(ObservedPreExecHostIdentityV3),
    ExactLaunchedAfterExecSigtrap { content: LiveKfdContentIdentityV3 },
}

impl LiveKfdHostLaunchContentV3 {
    pub const fn claims_target_instructions_executed(self) -> bool {
        false
    }
}

impl std::fmt::Debug for LiveKfdSemanticSessionBindingV3 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LiveKfdSemanticSessionBindingV3")
            .field("session_identity", &self.session_identity)
            .field("bundle_identity", &self.bundle_identity)
            .field("bundle_content_identity", &self.bundle_content_identity)
            .field("request_content_identity", &self.request_content_identity)
            .field("declared_hsaco", &self.declared_hsaco)
            .field("observed_host", &self.observed_host)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveKfdInputRoleV3 {
    Session,
    Bundle,
    Request,
    DeclaredHsaco,
    HostExecutable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiveKfdBindingErrorCodeV3 {
    InvalidPlan,
    OpenFailed,
    SymlinkRejected,
    NonRegular,
    HardLinkRejected,
    NotExecutable,
    Empty,
    TooLarge,
    ReadFailed,
    InputChanged,
    InputAlias,
    BundleRejected,
    HsacoRejected,
    AdmissionRejected,
    BindingMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiveKfdBindingErrorV3 {
    role: LiveKfdInputRoleV3,
    code: LiveKfdBindingErrorCodeV3,
    message: &'static str,
}

impl LiveKfdBindingErrorV3 {
    const fn new(
        role: LiveKfdInputRoleV3,
        code: LiveKfdBindingErrorCodeV3,
        message: &'static str,
    ) -> Self {
        Self {
            role,
            code,
            message,
        }
    }

    pub const fn role(&self) -> LiveKfdInputRoleV3 {
        self.role
    }

    pub const fn code(&self) -> LiveKfdBindingErrorCodeV3 {
        self.code
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl std::fmt::Display for LiveKfdBindingErrorV3 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for LiveKfdBindingErrorV3 {}

pub fn admit_live_kfd_semantic_session_v3(
    plan: LiveKfdSemanticSessionPlanV3,
) -> Result<LiveKfdSemanticSessionBindingV3, LiveKfdBindingErrorV3> {
    admit_with_after_initial_capture(plan, || {})
}

#[cfg(test)]
pub fn admit_live_kfd_semantic_session_with_hook_v3(
    plan: LiveKfdSemanticSessionPlanV3,
    after_initial_capture: impl FnOnce(),
) -> Result<LiveKfdSemanticSessionBindingV3, LiveKfdBindingErrorV3> {
    admit_with_after_initial_capture(plan, after_initial_capture)
}

fn admit_with_after_initial_capture(
    plan: LiveKfdSemanticSessionPlanV3,
    after_initial_capture: impl FnOnce(),
) -> Result<LiveKfdSemanticSessionBindingV3, LiveKfdBindingErrorV3> {
    let mut bundle_capture = CapturedFileV3::capture(
        &plan.bundle,
        plan.limits.max_bundle_bytes,
        LiveKfdInputRoleV3::Bundle,
        false,
    )?;
    let mut request_capture = CapturedFileV3::capture(
        &plan.request,
        plan.limits.max_request_bytes,
        LiveKfdInputRoleV3::Request,
        false,
    )?;
    let mut hsaco_capture = plan
        .declared_hsaco
        .as_ref()
        .map(|path| {
            CapturedFileV3::capture(
                path,
                plan.limits.max_hsaco_bytes,
                LiveKfdInputRoleV3::DeclaredHsaco,
                false,
            )
        })
        .transpose()?;
    let mut host_capture = CapturedFileV3::capture(
        &plan.host_executable,
        plan.limits.max_host_executable_bytes,
        LiveKfdInputRoleV3::HostExecutable,
        true,
    )?;

    reject_aliases(
        &bundle_capture,
        &request_capture,
        hsaco_capture.as_ref(),
        &host_capture,
    )?;

    let independently_verified =
        VerifiedSimulationBundleV2::from_canonical_bytes(bundle_capture.bytes.clone())
            .and_then(|bundle| {
                bundle.revalidate()?;
                Ok(bundle)
            })
            .map_err(|_| {
                LiveKfdBindingErrorV3::new(
                    LiveKfdInputRoleV3::Bundle,
                    LiveKfdBindingErrorCodeV3::BundleRejected,
                    "simulation bundle V2 admission rejected the exact captured bytes",
                )
            })?;
    let declared_hsaco_code_object_version = hsaco_capture
        .as_ref()
        .map(|capture| {
            fe2o3_hsaco::inspect(&capture.bytes)
                .map(|inspection| inspection.code_object_version().number())
                .map_err(|_| {
                    LiveKfdBindingErrorV3::new(
                        LiveKfdInputRoleV3::DeclaredHsaco,
                        LiveKfdBindingErrorCodeV3::HsacoRejected,
                        "declared HSACO failed strict bounded inspection",
                    )
                })
        })
        .transpose()?;

    after_initial_capture();
    let admitted = load_debug_simulation_bundle_v2(&plan.bundle, &plan.request);

    bundle_capture.revalidate(&plan.bundle)?;
    request_capture.revalidate(&plan.request)?;
    if let (Some(capture), Some(path)) = (hsaco_capture.as_mut(), plan.declared_hsaco.as_ref()) {
        capture.revalidate(path)?;
    }
    host_capture.revalidate(&plan.host_executable)?;
    let admitted = admitted.map_err(|_| {
        LiveKfdBindingErrorV3::new(
            LiveKfdInputRoleV3::Session,
            LiveKfdBindingErrorCodeV3::AdmissionRejected,
            "simulation bundle V2 or request admission failed",
        )
    })?;

    let (admitted_input, bundle) = admitted.into_parts();
    let bundle_identity = *bundle.identity().as_bytes();
    if bundle.canonical_bytes() != bundle_capture.bytes
        || bundle_identity != *independently_verified.identity().as_bytes()
        || admitted_input.request_sha256 != request_capture.identity.sha256
        || admitted_input.request_bytes() != request_capture.identity.length
    {
        return Err(LiveKfdBindingErrorV3::new(
            LiveKfdInputRoleV3::Session,
            LiveKfdBindingErrorCodeV3::BindingMismatch,
            "live KFD semantic inputs do not match exact admitted bundle and request bytes",
        ));
    }

    let declared_hsaco = hsaco_capture
        .as_ref()
        .map(|capture| DeclaredHsacoIdentityV3 {
            content: capture.identity,
            code_object_version: declared_hsaco_code_object_version
                .expect("inspected HSACO version remains paired with capture"),
        });
    let observed_host = ObservedPreExecHostIdentityV3 {
        content: host_capture.identity,
    };
    let session_identity = derive_session_identity(
        bundle_identity,
        bundle_capture.identity,
        request_capture.identity,
        declared_hsaco,
        observed_host,
    );
    Ok(LiveKfdSemanticSessionBindingV3 {
        session_identity,
        bundle_identity,
        bundle_content_identity: bundle_capture.identity,
        request_content_identity: request_capture.identity,
        declared_hsaco,
        observed_host,
        admitted_input,
        bundle,
        request_bytes: request_capture.bytes,
        declared_hsaco_bytes: hsaco_capture.map(|capture| capture.bytes),
        host_executable: RetainedHostExecutableV3 {
            file: host_capture.file,
            bytes: host_capture.bytes,
            snapshot: host_capture.snapshot,
            role: host_capture.role,
            launch_state: HostLaunchStateV3::ObservedPreExec,
        },
    })
}

fn derive_session_identity(
    bundle_identity: [u8; 32],
    bundle_content: LiveKfdContentIdentityV3,
    request: LiveKfdContentIdentityV3,
    declared_hsaco: Option<DeclaredHsacoIdentityV3>,
    observed_host: ObservedPreExecHostIdentityV3,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(SESSION_IDENTITY_DOMAIN_V3);
    hasher.update(bundle_identity);
    update_content_identity(&mut hasher, bundle_content);
    update_content_identity(&mut hasher, request);
    match declared_hsaco {
        Some(hsaco) => {
            hasher.update([1]);
            update_content_identity(&mut hasher, hsaco.content);
            hasher.update([hsaco.code_object_version]);
        }
        None => hasher.update([0]),
    }
    update_content_identity(&mut hasher, observed_host.content);
    hasher.finalize().into()
}

fn update_content_identity(hasher: &mut Sha256, identity: LiveKfdContentIdentityV3) {
    hasher.update(identity.length.to_le_bytes());
    hasher.update(identity.sha256);
}

fn reject_aliases(
    bundle: &CapturedFileV3,
    request: &CapturedFileV3,
    hsaco: Option<&CapturedFileV3>,
    host: &CapturedFileV3,
) -> Result<(), LiveKfdBindingErrorV3> {
    let mut objects = vec![
        (LiveKfdInputRoleV3::Bundle, bundle.snapshot.object()),
        (LiveKfdInputRoleV3::Request, request.snapshot.object()),
        (LiveKfdInputRoleV3::HostExecutable, host.snapshot.object()),
    ];
    if let Some(hsaco) = hsaco {
        objects.push((LiveKfdInputRoleV3::DeclaredHsaco, hsaco.snapshot.object()));
    }
    let mut identities = BTreeSet::new();
    for (role, object) in objects {
        if !identities.insert(object) {
            return Err(LiveKfdBindingErrorV3::new(
                role,
                LiveKfdBindingErrorCodeV3::InputAlias,
                "live KFD semantic input roles must name distinct file objects",
            ));
        }
    }
    Ok(())
}

struct CapturedFileV3 {
    file: File,
    bytes: Vec<u8>,
    identity: LiveKfdContentIdentityV3,
    snapshot: FileSnapshotV3,
    components: Vec<(PathBuf, PathComponentSnapshotV3)>,
    role: LiveKfdInputRoleV3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostLaunchStateV3 {
    ObservedPreExec,
    ExecSigtrapObserved,
}

struct RetainedHostExecutableV3 {
    file: File,
    bytes: Vec<u8>,
    snapshot: FileSnapshotV3,
    role: LiveKfdInputRoleV3,
    launch_state: HostLaunchStateV3,
}

impl RetainedHostExecutableV3 {
    fn revalidate_descriptor(&mut self) -> Result<(), LiveKfdBindingErrorV3> {
        if FileSnapshotV3::from_metadata(&self.file.metadata().map_err(|_| {
            LiveKfdBindingErrorV3::new(
                self.role,
                LiveKfdBindingErrorCodeV3::ReadFailed,
                "retained host executable descriptor could not be reinspected",
            )
        })?) != self.snapshot
        {
            return Err(changed(self.role));
        }
        self.file.seek(SeekFrom::Start(0)).map_err(|_| {
            LiveKfdBindingErrorV3::new(
                self.role,
                LiveKfdBindingErrorCodeV3::ReadFailed,
                "retained host executable descriptor could not be rewound",
            )
        })?;
        let reread = read_bounded(&mut self.file, self.bytes.len(), self.role)?;
        if reread != self.bytes {
            return Err(changed(self.role));
        }
        Ok(())
    }
}

impl CapturedFileV3 {
    fn capture(
        path: &Path,
        maximum: usize,
        role: LiveKfdInputRoleV3,
        require_executable: bool,
    ) -> Result<Self, LiveKfdBindingErrorV3> {
        let components = inspect_components(path, role)?;
        if components.is_empty() {
            return Err(LiveKfdBindingErrorV3::new(
                role,
                LiveKfdBindingErrorCodeV3::InvalidPlan,
                "live KFD semantic input path has no file component",
            ));
        }
        let named = FileSnapshotV3::from_metadata(&fs::symlink_metadata(path).map_err(|_| {
            LiveKfdBindingErrorV3::new(
                role,
                LiveKfdBindingErrorCodeV3::OpenFailed,
                "live KFD semantic input path could not be inspected",
            )
        })?);
        if components
            .last()
            .is_none_or(|(_, component)| component.object() != named.object())
        {
            return Err(changed(role));
        }
        if !named.regular {
            return Err(LiveKfdBindingErrorV3::new(
                role,
                LiveKfdBindingErrorCodeV3::NonRegular,
                "live KFD semantic input is not a regular file",
            ));
        }
        if named.nlink != 1 {
            return Err(LiveKfdBindingErrorV3::new(
                role,
                LiveKfdBindingErrorCodeV3::HardLinkRejected,
                "live KFD semantic input has a hard-link alias",
            ));
        }
        if require_executable && named.mode & 0o111 == 0 {
            return Err(LiveKfdBindingErrorV3::new(
                role,
                LiveKfdBindingErrorCodeV3::NotExecutable,
                "host executable observation requires an executable regular file",
            ));
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(|_| {
                LiveKfdBindingErrorV3::new(
                    role,
                    LiveKfdBindingErrorCodeV3::OpenFailed,
                    "live KFD semantic input could not be securely opened",
                )
            })?;
        let snapshot = FileSnapshotV3::from_metadata(&file.metadata().map_err(|_| {
            LiveKfdBindingErrorV3::new(
                role,
                LiveKfdBindingErrorCodeV3::ReadFailed,
                "live KFD semantic input descriptor could not be inspected",
            )
        })?);
        if snapshot != named {
            return Err(changed(role));
        }
        let bytes = read_bounded(&mut file, maximum, role)?;
        let after = FileSnapshotV3::from_metadata(&file.metadata().map_err(|_| {
            LiveKfdBindingErrorV3::new(
                role,
                LiveKfdBindingErrorCodeV3::ReadFailed,
                "live KFD semantic input descriptor could not be reinspected",
            )
        })?);
        if after != snapshot || u64::try_from(bytes.len()).ok() != Some(snapshot.size) {
            return Err(changed(role));
        }
        let identity = content_identity(&bytes, role)?;
        Ok(Self {
            file,
            bytes,
            identity,
            snapshot,
            components,
            role,
        })
    }

    fn revalidate(&mut self, path: &Path) -> Result<(), LiveKfdBindingErrorV3> {
        if inspect_components(path, self.role)? != self.components
            || FileSnapshotV3::from_metadata(&self.file.metadata().map_err(|_| {
                LiveKfdBindingErrorV3::new(
                    self.role,
                    LiveKfdBindingErrorCodeV3::ReadFailed,
                    "live KFD semantic input descriptor could not be reinspected",
                )
            })?) != self.snapshot
        {
            return Err(changed(self.role));
        }
        self.file.seek(SeekFrom::Start(0)).map_err(|_| {
            LiveKfdBindingErrorV3::new(
                self.role,
                LiveKfdBindingErrorCodeV3::ReadFailed,
                "live KFD semantic input descriptor could not be rewound",
            )
        })?;
        let reread = read_bounded(&mut self.file, self.bytes.len(), self.role)?;
        if reread != self.bytes {
            return Err(changed(self.role));
        }
        Ok(())
    }
}

fn read_bounded(
    file: &mut File,
    maximum: usize,
    role: LiveKfdInputRoleV3,
) -> Result<Vec<u8>, LiveKfdBindingErrorV3> {
    let limit = u64::try_from(maximum)
        .ok()
        .and_then(|maximum| maximum.checked_add(1))
        .ok_or_else(|| {
            LiveKfdBindingErrorV3::new(
                role,
                LiveKfdBindingErrorCodeV3::TooLarge,
                "live KFD semantic input exceeds the platform size range",
            )
        })?;
    let mut bytes = Vec::new();
    file.take(limit).read_to_end(&mut bytes).map_err(|_| {
        LiveKfdBindingErrorV3::new(
            role,
            LiveKfdBindingErrorCodeV3::ReadFailed,
            "live KFD semantic input could not be read",
        )
    })?;
    if bytes.is_empty() {
        return Err(LiveKfdBindingErrorV3::new(
            role,
            LiveKfdBindingErrorCodeV3::Empty,
            "live KFD semantic input must be nonempty",
        ));
    }
    if bytes.len() > maximum {
        return Err(LiveKfdBindingErrorV3::new(
            role,
            LiveKfdBindingErrorCodeV3::TooLarge,
            "live KFD semantic input exceeds its byte bound",
        ));
    }
    Ok(bytes)
}

fn content_identity(
    bytes: &[u8],
    role: LiveKfdInputRoleV3,
) -> Result<LiveKfdContentIdentityV3, LiveKfdBindingErrorV3> {
    Ok(LiveKfdContentIdentityV3 {
        sha256: Sha256::digest(bytes).into(),
        length: u64::try_from(bytes.len()).map_err(|_| {
            LiveKfdBindingErrorV3::new(
                role,
                LiveKfdBindingErrorCodeV3::TooLarge,
                "live KFD semantic input exceeds the identity length range",
            )
        })?,
    })
}

fn inspect_components(
    path: &Path,
    role: LiveKfdInputRoleV3,
) -> Result<Vec<(PathBuf, PathComponentSnapshotV3)>, LiveKfdBindingErrorV3> {
    let mut current = PathBuf::new();
    let mut snapshots = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(_) | Component::ParentDir => {
                return Err(LiveKfdBindingErrorV3::new(
                    role,
                    LiveKfdBindingErrorCodeV3::InvalidPlan,
                    "live KFD semantic input paths cannot contain parent traversal",
                ));
            }
            Component::RootDir => current.push(Path::new("/")),
            Component::CurDir => {}
            Component::Normal(name) => {
                current.push(name);
                let metadata = fs::symlink_metadata(&current).map_err(|_| {
                    LiveKfdBindingErrorV3::new(
                        role,
                        LiveKfdBindingErrorCodeV3::OpenFailed,
                        "live KFD semantic input path could not be inspected",
                    )
                })?;
                if metadata.file_type().is_symlink() {
                    return Err(LiveKfdBindingErrorV3::new(
                        role,
                        LiveKfdBindingErrorCodeV3::SymlinkRejected,
                        "live KFD semantic input paths cannot contain symlinks",
                    ));
                }
                snapshots.push((
                    current.clone(),
                    PathComponentSnapshotV3::from_metadata(&metadata),
                ));
            }
        }
    }
    Ok(snapshots)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PathComponentSnapshotV3 {
    device: u64,
    inode: u64,
    mode: u32,
}

impl PathComponentSnapshotV3 {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
        }
    }

    const fn object(self) -> (u64, u64) {
        (self.device, self.inode)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileSnapshotV3 {
    device: u64,
    inode: u64,
    mode: u32,
    nlink: u64,
    size: u64,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
    regular: bool,
}

impl FileSnapshotV3 {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            nlink: metadata.nlink(),
            size: metadata.size(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
            regular: metadata.file_type().is_file(),
        }
    }

    const fn object(self) -> (u64, u64) {
        (self.device, self.inode)
    }
}

fn changed(role: LiveKfdInputRoleV3) -> LiveKfdBindingErrorV3 {
    LiveKfdBindingErrorV3::new(
        role,
        LiveKfdBindingErrorCodeV3::InputChanged,
        "live KFD semantic input changed during admission",
    )
}
