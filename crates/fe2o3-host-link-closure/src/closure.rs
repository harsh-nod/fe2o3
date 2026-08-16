use crate::artifact::{
    HostArtifactCatalogV1, HostLinkPlanV1, PublishedHostArtifactV1, checked_retained_bytes,
};
use crate::control::{
    classify_script_input, parse_linker_script, parse_response_file, validate_literal,
    validate_undefined_symbol,
};
use crate::digest::Sha256Digest;
use crate::error::{HostLinkError, HostLinkErrorCodeV1, ResultContext};
use crate::model::{
    ArtifactIdV1, ArtifactIdentityV1, ArtifactProvenanceV1, ElfProfileV1, HostArtifactKindV1,
    LibraryPreferenceV1, LinkerZPolicyV1, OutputTypeV1, PlanArgumentV1, RootInputKindV1,
};
use crate::platform;
use crate::result::{
    HOST_LINK_RESULT_COPY_POLICY_V1, HostLinkResultChannelV1, ResultChannelReadV1, SocketIdentityV1,
};
use crate::root::FixedRootSetV1;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ffi::OsString;
use std::fs::File;
use std::time::{Duration, Instant};

pub const HOST_LLD_RESULT_SOCKET_CHILD_FD_V1: i32 = 91;
pub const HOST_LLD_FIRST_INPUT_CHILD_FD_V1: i32 = 100;
pub const HOST_LLD_PROTOCOL_ARGUMENT_V1: &str = "--fe2o3-host-lld-elf-v2";
pub const HOST_LLD_RESULT_SOCKET_ARGUMENT_PREFIX_V1: &str = "--fe2o3-result-socket-v1=";
pub const HOST_LLD_REQUEST_ARGUMENT_PREFIX_V1: &str = "--fe2o3-request-v1=";
pub const HOST_LLD_INPUT_ARGUMENT_PREFIX_V1: &str = "--fe2o3-input-v1=";
const STATIC_TOOL_MAX_ARGUMENTS_V1: usize = 4096;
const STATIC_TOOL_MAX_ARGUMENT_BYTES_V1: usize = 4096;
const STATIC_TOOL_MAX_TOTAL_ARGUMENT_BYTES_V1: usize = 1024 * 1024;
const STATIC_TOOL_MAX_UNIQUE_INPUTS_V1: usize = crate::MAX_HOST_LINK_UNIQUE_INPUTS_V1;
const MAX_FINAL_ARGUMENTS: usize = STATIC_TOOL_MAX_ARGUMENTS_V1 - 4;
const STATIC_HOST_LLD_WALL_TIMEOUT_V1: Duration = Duration::from_secs(30);
const HOST_LINK_ADMISSION_POLL_WALL_QUANTUM_V1: Duration = Duration::from_millis(10);

/// Maximum output bytes copied or hashed by one admission poll.
pub const HOST_LINK_ADMISSION_MAX_BYTES_PER_POLL_V1: u64 = 256 * 1024;
/// Maximum fixed-size validation operations performed by one admission poll.
pub const HOST_LINK_ADMISSION_MAX_OPERATIONS_PER_POLL_V1: usize = 64;
/// Cooperative wall-time quantum checked between bounded admission operations.
pub const HOST_LINK_ADMISSION_MAX_MILLIS_PER_POLL_V1: u64 = 10;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InheritedDescriptorRoleV1 {
    ResultSocket,
    Input(ArtifactIdV1),
}

pub(crate) struct InheritedDescriptorV1 {
    child_fd: i32,
    role: InheritedDescriptorRoleV1,
    file: File,
}

impl InheritedDescriptorV1 {
    pub(crate) const fn child_fd(&self) -> i32 {
        self.child_fd
    }

    pub(crate) const fn file(&self) -> &File {
        &self.file
    }
}

pub struct LldArgvV1 {
    arguments: Vec<OsString>,
    canonical_arguments: Vec<Vec<u8>>,
    inherited: Vec<InheritedDescriptorV1>,
}

#[derive(Debug, Eq, PartialEq)]
struct StaticHostLldApprovalBindingV1 {
    plan_digest: Sha256Digest,
    tool_id: ArtifactIdV1,
    tool_sha256: Sha256Digest,
    tool_size: u64,
    tool_mode: u32,
    release_nonce: crate::ReleaseNonceV1,
    target: crate::TargetTripleV1,
    llvm_build_identity: String,
}

/// Move-only authority to execute one exact plan-bound static host LLD image.
///
/// This crate validates the captured binding but does not decide whether the tool is approved.
/// The future W1 broker is responsible for checking signed/verified toolchain evidence before it
/// crosses the explicit trusted construction boundary.
///
/// ```compile_fail
/// use fe2o3_host_link_closure::ApprovedStaticHostLldV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ApprovedStaticHostLldV1>();
/// ```
#[derive(Debug)]
pub struct ApprovedStaticHostLldV1 {
    binding: StaticHostLldApprovalBindingV1,
}

impl ApprovedStaticHostLldV1 {
    /// Mints launch authority after an external authority verifies the exact tool evidence.
    ///
    /// # Safety
    ///
    /// The caller must be a trusted tool-approval authority and must have independently verified
    /// that this closure's exact static-host-LLD identity is authorized for the bound release,
    /// target, plan, and LLVM build identity. A false assertion can execute attacker-controlled
    /// native code with the embedding process's privileges.
    #[allow(unsafe_code)]
    pub unsafe fn from_verified_evidence(
        closure: &HostLinkClosureV1,
    ) -> Result<Self, HostLinkError> {
        closure.revalidate_inputs()?;
        Ok(Self {
            binding: closure.static_host_lld_approval_binding()?,
        })
    }
}

impl LldArgvV1 {
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    pub fn canonical_arguments(&self) -> &[Vec<u8>] {
        &self.canonical_arguments
    }
}

/// Immutable bytes admitted by one exact host-link plan and closure.
///
/// Fields and construction are private. The capability is deliberately move-only; descriptor
/// duplication is gated by a complete seal, byte, size, and mode revalidation.
///
/// ```compile_fail
/// use fe2o3_host_link_closure::AdmittedHostOutputV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<AdmittedHostOutputV1>();
/// ```
#[derive(Debug)]
pub struct AdmittedHostOutputV1 {
    file: File,
    sha256: Sha256Digest,
    size: u64,
    mode: u32,
    elf_profile: ElfProfileV1,
    plan_digest: Sha256Digest,
    closure_digest: Sha256Digest,
}

impl AdmittedHostOutputV1 {
    pub const fn sha256(&self) -> Sha256Digest {
        self.sha256
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn mode(&self) -> u32 {
        self.mode
    }

    pub fn elf_profile(&self) -> &ElfProfileV1 {
        &self.elf_profile
    }

    pub const fn plan_digest(&self) -> Sha256Digest {
        self.plan_digest
    }

    pub const fn closure_digest(&self) -> Sha256Digest {
        self.closure_digest
    }

    pub fn try_clone_file(&self) -> Result<File, HostLinkError> {
        platform::verify_sealed_artifact(
            &self.file,
            self.sha256,
            self.size,
            self.mode,
            "admitted host output",
        )?;
        self.file.try_clone().context(HostLinkErrorCodeV1::Io, || {
            "clone sealed admitted host output".to_owned()
        })
    }
}

struct ClosureArtifactV1 {
    identity: ArtifactIdentityV1,
    file: File,
    archive_members: u64,
}

impl ClosureArtifactV1 {
    fn from_published(artifact: &PublishedHostArtifactV1) -> Result<Self, HostLinkError> {
        artifact.revalidate()?;
        Ok(Self {
            identity: artifact.identity().clone(),
            file: artifact.try_clone_file()?,
            archive_members: artifact.archive_members(),
        })
    }

    fn revalidate(&self) -> Result<(), HostLinkError> {
        platform::verify_sealed_artifact(
            &self.file,
            self.identity.sha256,
            self.identity.size,
            self.identity.mode,
            &self.identity.label,
        )
    }

    fn revalidate_identity(&self) -> Result<(), HostLinkError> {
        platform::verify_sealed_artifact_identity(
            &self.file,
            self.identity.size,
            self.identity.mode,
            &self.identity.label,
        )
    }
}

#[derive(Clone)]
enum FinalArgument {
    Literal(Vec<u8>),
    Artifact(ArtifactIdV1),
}

struct StableLldArgvV1 {
    canonical_arguments: Vec<Vec<u8>>,
    inherited_inputs: Vec<InheritedDescriptorV1>,
}

enum OutputAdmissionStateV1 {
    AwaitPacket,
    AwaitWriteClose {
        record: crate::HostLinkResultRecordV1,
        output: File,
    },
    RevalidateArtifacts {
        record: crate::HostLinkResultRecordV1,
        output: File,
        next: usize,
    },
    RevalidateRoots {
        record: crate::HostLinkResultRecordV1,
        output: File,
        next: usize,
    },
    CopyOutput {
        record: crate::HostLinkResultRecordV1,
        copy: platform::IncrementalOutputCopyV1,
    },
    InspectOutput {
        record: crate::HostLinkResultRecordV1,
        inspection: platform::IncrementalStaticOutputInspectionV1,
    },
    Complete,
    Poisoned,
}

pub struct HostLinkClosureV1 {
    plan: HostLinkPlanV1,
    fixed_roots: FixedRootSetV1,
    artifact_catalog: HostArtifactCatalogV1,
    artifacts: BTreeMap<ArtifactIdV1, ClosureArtifactV1>,
    archive_members: u64,
    result_channel: HostLinkResultChannelV1,
    lld_argv: LldArgvV1,
    closure_digest: Sha256Digest,
    nonce_sha256: Sha256Digest,
    prevalidated: bool,
    child_handoff_complete: bool,
    admission_state: OutputAdmissionStateV1,
    admission_artifact_ids: Vec<ArtifactIdV1>,
    admission_root_labels: Vec<String>,
    admitted_output: Option<AdmittedHostOutputV1>,
}

impl HostLinkClosureV1 {
    pub fn prepare(
        plan: HostLinkPlanV1,
        fixed_roots: FixedRootSetV1,
        artifact_catalog: HostArtifactCatalogV1,
    ) -> Result<Self, HostLinkError> {
        plan.revalidate()?;
        artifact_catalog.validate_binding(plan.release_nonce(), plan.target())?;
        fixed_roots.revalidate()?;

        let mut resolver = Resolver {
            plan: &plan,
            roots: &fixed_roots,
            catalog: &artifact_catalog,
            search_roots: Vec::new(),
            artifacts: BTreeMap::new(),
            final_arguments: Vec::new(),
            final_argument_bytes: 0,
            retained_bytes: 0,
            archive_members: 0,
        };
        for argument in &plan.manifest().spec.arguments {
            resolver.process(argument, false)?;
        }
        resolver.validate_runtime_closure()?;
        let archive_members = resolver.archive_members;
        let artifacts = resolver.artifacts;
        let stable_argv = build_stable_lld_argv(&artifacts, &resolver.final_arguments)?;
        let closure_digest = closure_digest(&plan, &fixed_roots, &artifacts, &stable_argv);
        let nonce_sha256 = fresh_request_nonce_sha256()?;
        let (result_channel, result_child) = HostLinkResultChannelV1::new()?;
        let lld_argv = finalize_lld_argv(
            stable_argv,
            result_child,
            result_channel.child_identity(),
            plan.plan_digest(),
            closure_digest,
            nonce_sha256,
        )?;
        let admission_artifact_ids = artifacts.keys().copied().collect();
        let admission_root_labels = fixed_roots
            .iter()
            .map(|(label, _)| label.to_owned())
            .collect();

        Ok(Self {
            plan,
            fixed_roots,
            artifact_catalog,
            artifacts,
            archive_members,
            result_channel,
            lld_argv,
            closure_digest,
            nonce_sha256,
            prevalidated: false,
            child_handoff_complete: false,
            admission_state: OutputAdmissionStateV1::AwaitPacket,
            admission_artifact_ids,
            admission_root_labels,
            admitted_output: None,
        })
    }

    pub fn prevalidate(&mut self) -> Result<(), HostLinkError> {
        self.revalidate_inputs()?;
        self.revalidate_result_channel()?;
        self.prevalidated = true;
        Ok(())
    }

    pub fn lld_argv(&self) -> Result<&LldArgvV1, HostLinkError> {
        if !self.prevalidated {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidState,
                "LLD argv is unavailable before HostLinkClosureV1::prevalidate",
            ));
        }
        Ok(&self.lld_argv)
    }

    pub const fn closure_digest(&self) -> Sha256Digest {
        self.closure_digest
    }

    pub fn plan_digest(&self) -> Sha256Digest {
        self.plan.plan_digest()
    }

    pub const fn nonce_sha256(&self) -> Sha256Digest {
        self.nonce_sha256
    }

    fn admit_output_from_authenticated_worker(
        &mut self,
        worker_pid: u32,
        absolute_deadline: Instant,
    ) -> Result<(), HostLinkError> {
        if !self.prevalidated {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidState,
                "output admission requires a prevalidated closure",
            ));
        }
        if !self.child_handoff_complete {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidState,
                "finish child descriptor handoff before polling the result channel",
            ));
        }
        if matches!(
            self.admission_state,
            OutputAdmissionStateV1::Complete | OutputAdmissionStateV1::Poisoned
        ) {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidState,
                "host-link result admission is single-use",
            ));
        }
        let quantum_deadline = Instant::now()
            .checked_add(HOST_LINK_ADMISSION_POLL_WALL_QUANTUM_V1)
            .unwrap_or(absolute_deadline)
            .min(absolute_deadline);
        let result = self.advance_output_admission(worker_pid, absolute_deadline, quantum_deadline);
        if result
            .as_ref()
            .is_err_and(|error| error.code() != HostLinkErrorCodeV1::ResultPending)
        {
            self.admission_state = OutputAdmissionStateV1::Poisoned;
        }
        result
    }

    fn advance_output_admission(
        &mut self,
        worker_pid: u32,
        absolute_deadline: Instant,
        quantum_deadline: Instant,
    ) -> Result<(), HostLinkError> {
        let mut operations = 0_usize;
        loop {
            check_admission_deadline(absolute_deadline)?;
            if operations >= HOST_LINK_ADMISSION_MAX_OPERATIONS_PER_POLL_V1
                || Instant::now() >= quantum_deadline
            {
                return Err(admission_pending(
                    "host-link admission exhausted its work quantum",
                ));
            }
            let state =
                std::mem::replace(&mut self.admission_state, OutputAdmissionStateV1::Poisoned);
            match state {
                OutputAdmissionStateV1::AwaitPacket => {
                    self.revalidate_result_channel()?;
                    match self.result_channel.try_receive(worker_pid)? {
                        ResultChannelReadV1::Pending => {
                            self.admission_state = OutputAdmissionStateV1::AwaitPacket;
                            return Err(admission_pending("host-link result packet is not ready"));
                        }
                        ResultChannelReadV1::Closed => {
                            return Err(HostLinkError::new(
                                HostLinkErrorCodeV1::OutputEmpty,
                                "host-link worker closed the result channel before sending",
                            ));
                        }
                        ResultChannelReadV1::Packet(record, output) => {
                            self.admission_state =
                                OutputAdmissionStateV1::AwaitWriteClose { record, output };
                        }
                    }
                }
                OutputAdmissionStateV1::AwaitWriteClose { record, output } => {
                    if !self.result_channel.poll_write_closed()? {
                        self.admission_state =
                            OutputAdmissionStateV1::AwaitWriteClose { record, output };
                        return Err(admission_pending(
                            "host-link result arrived but worker write shutdown is not observable",
                        ));
                    }
                    self.validate_result_binding(&record)?;
                    self.admission_state = OutputAdmissionStateV1::RevalidateArtifacts {
                        record,
                        output,
                        next: 0,
                    };
                }
                OutputAdmissionStateV1::RevalidateArtifacts {
                    record,
                    output,
                    next,
                } => {
                    if next == self.admission_artifact_ids.len() {
                        self.admission_state = OutputAdmissionStateV1::RevalidateRoots {
                            record,
                            output,
                            next: 0,
                        };
                        continue;
                    }
                    let id = self.admission_artifact_ids[next];
                    self.artifacts
                        .get(&id)
                        .expect("admission artifact IDs derive from closure map")
                        .revalidate_identity()?;
                    self.admission_state = OutputAdmissionStateV1::RevalidateArtifacts {
                        record,
                        output,
                        next: next + 1,
                    };
                }
                OutputAdmissionStateV1::RevalidateRoots {
                    record,
                    output,
                    next,
                } => {
                    if next == self.admission_root_labels.len() {
                        let copy = platform::IncrementalOutputCopyV1::new(
                            output,
                            "host-link result output",
                            crate::MAX_HOST_LINK_OUTPUT_BYTES_V1,
                            record.output_length(),
                        )?;
                        self.admission_state = OutputAdmissionStateV1::CopyOutput { record, copy };
                        continue;
                    }
                    let label = &self.admission_root_labels[next];
                    self.fixed_roots
                        .get(label)
                        .expect("admission root labels derive from fixed-root set")
                        .revalidate_after_execution()?;
                    self.admission_state = OutputAdmissionStateV1::RevalidateRoots {
                        record,
                        output,
                        next: next + 1,
                    };
                }
                OutputAdmissionStateV1::CopyOutput { record, copy } => {
                    match copy.advance(
                        HOST_LINK_ADMISSION_MAX_BYTES_PER_POLL_V1,
                        absolute_deadline,
                        quantum_deadline,
                    )? {
                        platform::IncrementalOutputCopyProgressV1::Pending(copy) => {
                            self.admission_state = OutputAdmissionStateV1::CopyOutput {
                                record,
                                copy: *copy,
                            };
                            return Err(admission_pending(
                                "receiver-owned output copy or hash remains pending",
                            ));
                        }
                        platform::IncrementalOutputCopyProgressV1::Complete(captured) => {
                            if captured.sha256 != record.output_sha256()
                                || captured.size != record.output_length()
                            {
                                return Err(HostLinkError::new(
                                    HostLinkErrorCodeV1::DigestMismatch,
                                    "sealed host-link output does not match the canonical result record",
                                ));
                            }
                            if captured.mode != self.plan.manifest().spec.expected_output_mode {
                                return Err(HostLinkError::new(
                                    HostLinkErrorCodeV1::ElfPolicy,
                                    "receiver-owned host-link output mode does not match the sealed plan",
                                ));
                            }
                            let inspection =
                                platform::IncrementalStaticOutputInspectionV1::new(captured)?;
                            self.admission_state =
                                OutputAdmissionStateV1::InspectOutput { record, inspection };
                        }
                    }
                }
                OutputAdmissionStateV1::InspectOutput { record, inspection } => {
                    let remaining_operations =
                        HOST_LINK_ADMISSION_MAX_OPERATIONS_PER_POLL_V1.saturating_sub(operations);
                    match inspection.advance(
                        remaining_operations,
                        absolute_deadline,
                        quantum_deadline,
                    )? {
                        platform::IncrementalOutputInspectionProgressV1::Pending(inspection) => {
                            self.admission_state =
                                OutputAdmissionStateV1::InspectOutput { record, inspection };
                            return Err(admission_pending(
                                "incremental static output ELF inspection remains pending",
                            ));
                        }
                        platform::IncrementalOutputInspectionProgressV1::Complete(
                            captured,
                            profile,
                        ) => {
                            validate_output_type(
                                self.plan.manifest().spec.output_type,
                                profile.elf_type,
                            )?;
                            if profile != self.plan.manifest().spec.expected_output_elf {
                                return Err(HostLinkError::new(
                                    HostLinkErrorCodeV1::ElfPolicy,
                                    "sealed host-link output ELF profile does not match the exact plan policy",
                                ));
                            }
                            platform::verify_sealed_artifact_identity(
                                &captured.file,
                                captured.size,
                                captured.mode,
                                "admitted host output",
                            )?;
                            check_admission_deadline(absolute_deadline)?;
                            self.admitted_output = Some(AdmittedHostOutputV1 {
                                file: captured.file,
                                sha256: captured.sha256,
                                size: captured.size,
                                mode: captured.mode,
                                elf_profile: profile,
                                plan_digest: self.plan.plan_digest(),
                                closure_digest: self.closure_digest,
                            });
                            self.admission_state = OutputAdmissionStateV1::Complete;
                            return Ok(());
                        }
                    }
                }
                OutputAdmissionStateV1::Complete | OutputAdmissionStateV1::Poisoned => {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::InvalidState,
                        "host-link admission state is terminal",
                    ));
                }
            }
            operations += 1;
            check_admission_deadline(absolute_deadline)?;
        }
    }

    fn validate_result_binding(
        &self,
        record: &crate::HostLinkResultRecordV1,
    ) -> Result<(), HostLinkError> {
        if record.plan_digest() != self.plan.plan_digest() {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ReplayMismatch,
                "host-link result is bound to a different plan",
            ));
        }
        if record.closure_digest() != self.closure_digest {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ReplayMismatch,
                "host-link result is bound to a different closure",
            ));
        }
        if record.nonce_sha256() != self.nonce_sha256 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::WrongNonce,
                "host-link result is bound to a different fresh request nonce",
            ));
        }
        Ok(())
    }

    fn admitted_output(&self) -> Option<&AdmittedHostOutputV1> {
        self.admitted_output.as_ref()
    }

    /// Consumes this closure and an explicit tool approval into the only capability that can
    /// admit a host-link result.
    ///
    /// The worker image is the exact sealed `static_host_lld` descriptor from the plan. The
    /// launcher installs only the canonical descriptor table and executes it with
    /// `execveat(AT_EMPTY_PATH)`.
    pub fn launch(
        mut self,
        approval: ApprovedStaticHostLldV1,
    ) -> Result<AuthenticatedHostLinkExecutionV1, HostLinkError> {
        if !self.prevalidated {
            self.prevalidate()?;
        }
        self.revalidate_inputs()?;
        self.revalidate_result_channel()?;
        if approval.binding != self.static_host_lld_approval_binding()? {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ToolApproval,
                "static host LLD approval does not bind this exact plan and tool identity",
            ));
        }
        let tool = self
            .plan
            .producer(self.plan.manifest().spec.toolchain.static_host_lld)
            .ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::WorkerLaunch,
                    "sealed plan lost its exact static_host_lld descriptor",
                )
            })?;
        tool.revalidate()?;
        if tool.identity().kind != HostArtifactKindV1::StaticHostLld {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::WorkerLaunch,
                "sealed plan static_host_lld identity has the wrong artifact kind",
            ));
        }
        let tool = tool.try_clone_file()?;
        let arguments = self.lld_argv.canonical_arguments.clone();
        let inherited = std::mem::take(&mut self.lld_argv.inherited);
        let started_at = Instant::now();
        let deadline = started_at
            .checked_add(STATIC_HOST_LLD_WALL_TIMEOUT_V1)
            .ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::WorkerLaunch,
                    "static LLD wall deadline overflowed the monotonic clock",
                )
            })?;
        let process =
            crate::process::AuthenticatedProcessV1::launch(tool, &arguments, inherited, deadline)?;
        self.child_handoff_complete = true;
        Ok(AuthenticatedHostLinkExecutionV1 {
            closure: self,
            process,
            deadline,
            terminal: false,
        })
    }

    fn static_host_lld_approval_binding(
        &self,
    ) -> Result<StaticHostLldApprovalBindingV1, HostLinkError> {
        let tool = self
            .plan
            .producer(self.plan.manifest().spec.toolchain.static_host_lld)
            .ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::ToolApproval,
                    "sealed plan lost its exact static_host_lld descriptor",
                )
            })?;
        if tool.identity().kind != HostArtifactKindV1::StaticHostLld {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ToolApproval,
                "sealed plan static_host_lld identity has the wrong artifact kind",
            ));
        }
        let identity = tool.identity();
        Ok(StaticHostLldApprovalBindingV1 {
            plan_digest: self.plan.plan_digest(),
            tool_id: identity.id,
            tool_sha256: identity.sha256,
            tool_size: identity.size,
            tool_mode: identity.mode,
            release_nonce: self.plan.release_nonce(),
            target: self.plan.target().clone(),
            llvm_build_identity: self
                .plan
                .manifest()
                .spec
                .toolchain
                .llvm_build_identity
                .clone(),
        })
    }

    pub fn revalidate(&self) -> Result<(), HostLinkError> {
        self.revalidate_inputs()?;
        self.revalidate_result_channel()?;
        if let Some(output) = &self.admitted_output {
            platform::verify_sealed_artifact(
                &output.file,
                output.sha256,
                output.size,
                output.mode,
                "admitted host output",
            )?;
        }
        Ok(())
    }

    fn revalidate_result_channel(&self) -> Result<(), HostLinkError> {
        self.result_channel.revalidate_receiver()?;
        if self.child_handoff_complete {
            return Ok(());
        }
        let descriptor = self
            .lld_argv
            .inherited
            .iter()
            .find(|descriptor| descriptor.role == InheritedDescriptorRoleV1::ResultSocket)
            .ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::InvalidState,
                    "canonical LLD handoff lost its result socket",
                )
            })?;
        if descriptor.child_fd != HOST_LLD_RESULT_SOCKET_CHILD_FD_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DescriptorChanged,
                "result socket has the wrong child descriptor assignment",
            ));
        }
        self.result_channel.revalidate_child(&descriptor.file)
    }

    fn revalidate_inputs(&self) -> Result<(), HostLinkError> {
        self.plan.revalidate()?;
        self.fixed_roots.revalidate()?;
        self.artifact_catalog.revalidate()?;
        let mut archive_members = 0_u64;
        for artifact in self.artifacts.values() {
            artifact.revalidate()?;
            archive_members = checked_archive_members(
                archive_members,
                artifact.archive_members,
                "revalidated host-link closure",
            )?;
        }
        if archive_members != self.archive_members {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::DescriptorChanged,
                "resolved closure archive-member accounting changed",
            ));
        }
        Ok(())
    }
}

/// Move-only witness for one exact sealed static-LLD process and result endpoint.
///
/// There is no constructor other than [`HostLinkClosureV1::launch`], which requires an
/// [`ApprovedStaticHostLldV1`], and no child result descriptor is exposed. Output admission remains
/// nonblocking and requires a successful exit observation through the retained kernel pidfd for
/// the unreaped direct child. Repeated polling cannot extend the fixed 30-second wall deadline.
/// Timeout and drop return without a blocking wait; one bounded process-wide event loop retains
/// the pidfd until eventual kernel reap succeeds.
pub struct AuthenticatedHostLinkExecutionV1 {
    closure: HostLinkClosureV1,
    process: crate::process::AuthenticatedProcessV1,
    deadline: Instant,
    terminal: bool,
}

impl AuthenticatedHostLinkExecutionV1 {
    pub fn process_id(&self) -> u32 {
        self.process.pid()
    }

    pub fn plan_digest(&self) -> Sha256Digest {
        self.closure.plan_digest()
    }

    pub const fn closure_digest(&self) -> Sha256Digest {
        self.closure.closure_digest()
    }

    pub const fn nonce_sha256(&self) -> Sha256Digest {
        self.closure.nonce_sha256()
    }

    pub fn try_admit_output(&mut self) -> Result<&AdmittedHostOutputV1, HostLinkError> {
        if self.terminal {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidState,
                "authenticated host-link execution is already terminal",
            ));
        }
        if Instant::now() >= self.deadline {
            self.terminal = true;
            self.process.terminate_for_timeout()?;
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::WorkerTimeout,
                "authenticated static LLD exceeded its fixed 30-second wall deadline",
            ));
        }
        match self.process.poll_successful_exit() {
            Ok(false) => {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ResultPending,
                    "exact static LLD child has not exited successfully yet",
                ));
            }
            Ok(true) => {}
            Err(error) => {
                self.terminal = true;
                self.process.terminate_after_admission_failure()?;
                return Err(error);
            }
        }
        if let Err(error) = self
            .closure
            .admit_output_from_authenticated_worker(self.process.pid(), self.deadline)
        {
            if error.code() != HostLinkErrorCodeV1::ResultPending {
                self.terminal = true;
                self.process.terminate_after_admission_failure()?;
            }
            return Err(error);
        }
        if Instant::now() >= self.deadline {
            self.terminal = true;
            self.process.terminate_for_timeout()?;
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::WorkerTimeout,
                "authenticated static LLD admission reached its fixed 30-second wall deadline",
            ));
        }
        if let Err(error) = self.process.reap_success() {
            self.terminal = true;
            return Err(error);
        }
        self.terminal = true;
        Ok(self
            .closure
            .admitted_output()
            .expect("authenticated admission installed one output capability"))
    }

    pub fn revalidate(&self) -> Result<(), HostLinkError> {
        self.closure.revalidate()
    }
}

fn check_admission_deadline(deadline: Instant) -> Result<(), HostLinkError> {
    if Instant::now() >= deadline {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::WorkerTimeout,
            "authenticated static LLD admission reached its fixed 30-second wall deadline",
        ));
    }
    Ok(())
}

fn admission_pending(detail: &str) -> HostLinkError {
    HostLinkError::new(HostLinkErrorCodeV1::ResultPending, detail)
}

fn fresh_request_nonce_sha256() -> Result<Sha256Digest, HostLinkError> {
    let mut bytes = [0_u8; 32];
    let mut filled = 0;
    while filled < bytes.len() {
        let count =
            rustix::rand::getrandom(&mut bytes[filled..], rustix::rand::GetRandomFlags::empty())
                .context(HostLinkErrorCodeV1::Io, || {
                    "generate fresh host-link request nonce".to_owned()
                })?;
        if count == 0 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::Io,
                "Linux getrandom returned no request nonce bytes",
            ));
        }
        filled += count;
    }
    if bytes == [0; 32] {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidNonce,
            "fresh host-link request nonce is all zero",
        ));
    }
    Ok(Sha256Digest::from_bytes(bytes))
}

struct Resolver<'a> {
    plan: &'a HostLinkPlanV1,
    roots: &'a FixedRootSetV1,
    catalog: &'a HostArtifactCatalogV1,
    search_roots: Vec<String>,
    artifacts: BTreeMap<ArtifactIdV1, ClosureArtifactV1>,
    final_arguments: Vec<FinalArgument>,
    final_argument_bytes: usize,
    retained_bytes: u64,
    archive_members: u64,
}

impl Resolver<'_> {
    fn process(
        &mut self,
        argument: &PlanArgumentV1,
        nested_response: bool,
    ) -> Result<(), HostLinkError> {
        match argument {
            PlanArgumentV1::Literal(value) => {
                validate_literal(value)?;
                self.push_final(FinalArgument::Literal(value.clone()))?;
            }
            PlanArgumentV1::ZPolicy(policy) => {
                self.push_final(FinalArgument::Literal(b"-z".to_vec()))?;
                self.push_final(FinalArgument::Literal(policy.as_str().as_bytes().to_vec()))?;
            }
            PlanArgumentV1::UndefinedSymbol(symbol) => {
                validate_undefined_symbol(symbol)?;
                self.push_final(FinalArgument::Literal(
                    format!("--undefined={symbol}").into_bytes(),
                ))?;
            }
            PlanArgumentV1::SearchRoot(label) => {
                if self.roots.get(label).is_none() {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::UnresolvedSearch,
                        format!("plan names absent fixed root {label}"),
                    ));
                }
                if self.search_roots.contains(label) {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::DuplicateRecord,
                        format!("duplicate fixed-root search directive {label}"),
                    ));
                }
                self.search_roots.push(label.clone());
            }
            PlanArgumentV1::Library { name, preference } => {
                self.resolve_library(name, *preference)?;
            }
            PlanArgumentV1::FixedRootInput {
                root,
                relative_path,
                kind,
            } => self.resolve_root_input(root, relative_path, *kind)?,
            PlanArgumentV1::ProducerArtifact(id) => {
                let artifact = self.plan.producer(*id).ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ReplayMismatch,
                        "producer artifact disappeared from decoded plan",
                    )
                })?;
                self.add_link_artifact(artifact)?;
            }
            PlanArgumentV1::CatalogArtifact(id) => {
                let artifact = self.catalog.get(*id).ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::ReplayMismatch,
                        "plan references an absent catalog artifact",
                    )
                })?;
                self.add_link_artifact(artifact)?;
            }
            PlanArgumentV1::ResponseFile {
                root,
                relative_path,
            } => {
                if nested_response {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::NestedResponseFile,
                        "nested response files are outside HostLinkClosureV1",
                    ));
                }
                let artifact =
                    self.open_root_artifact(root, relative_path, RootInputKindV1::ResponseFile)?;
                let bytes = artifact.sealed_bytes()?;
                self.retain(&artifact)?;
                for expanded in parse_response_file(&bytes)? {
                    self.process(&expanded, true)?;
                }
            }
        }
        Ok(())
    }

    fn resolve_library(
        &mut self,
        name: &str,
        preference: LibraryPreferenceV1,
    ) -> Result<(), HostLinkError> {
        if self.search_roots.is_empty() {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::UnresolvedLibrary,
                format!("library {name} has no preceding retained search root"),
            ));
        }
        let (file_name, kind) = if let Some(exact) = name.strip_prefix(':') {
            let kind = classify_script_input(exact.as_bytes())?;
            (exact.as_bytes().to_vec(), kind)
        } else {
            match preference {
                LibraryPreferenceV1::StaticOnly => (
                    format!("lib{name}.a").into_bytes(),
                    RootInputKindV1::RegularArchive,
                ),
                LibraryPreferenceV1::DynamicOnly => {
                    (format!("lib{name}.so").into_bytes(), RootInputKindV1::Dso)
                }
            }
        };
        for label in self.search_roots.clone() {
            let root = self.roots.get(&label).expect("search roots were validated");
            if let Some(artifact) = root.try_open_artifact(
                &file_name,
                kind,
                self.plan.release_nonce(),
                self.plan.target().clone(),
            )? {
                self.add_link_artifact(&artifact)?;
                return Ok(());
            }
        }
        Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnresolvedLibrary,
            format!("library {name} did not resolve in any retained fixed root"),
        ))
    }

    fn resolve_root_input(
        &mut self,
        root_label: &str,
        relative_path: &[u8],
        kind: RootInputKindV1,
    ) -> Result<(), HostLinkError> {
        let artifact = self.open_root_artifact(root_label, relative_path, kind)?;
        match kind {
            RootInputKindV1::ResponseFile => {
                let bytes = artifact.sealed_bytes()?;
                self.retain(&artifact)?;
                for expanded in parse_response_file(&bytes)? {
                    self.process(&expanded, true)?;
                }
            }
            RootInputKindV1::LinkerScript => {
                let bytes = artifact.sealed_bytes()?;
                self.retain(&artifact)?;
                for nested_path in parse_linker_script(&bytes)? {
                    let nested_kind = classify_script_input(&nested_path)?;
                    let nested = self.open_root_artifact(root_label, &nested_path, nested_kind)?;
                    self.add_link_artifact(&nested)?;
                }
            }
            _ => self.add_link_artifact(&artifact)?,
        }
        Ok(())
    }

    fn open_root_artifact(
        &self,
        root_label: &str,
        relative_path: &[u8],
        kind: RootInputKindV1,
    ) -> Result<PublishedHostArtifactV1, HostLinkError> {
        let root = self.roots.get(root_label).ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::UnresolvedSearch,
                format!("plan names absent fixed root {root_label}"),
            )
        })?;
        root.try_open_artifact(
            relative_path,
            kind,
            self.plan.release_nonce(),
            self.plan.target().clone(),
        )?
        .ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::UnresolvedSearch,
                format!(
                    "fixed-root input does not exist: {root_label}:{}",
                    String::from_utf8_lossy(relative_path)
                ),
            )
        })
    }

    fn add_link_artifact(
        &mut self,
        artifact: &PublishedHostArtifactV1,
    ) -> Result<(), HostLinkError> {
        match artifact.identity().kind {
            HostArtifactKindV1::StaticWrapper | HostArtifactKindV1::StaticHostLld => {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "host-link tool executable cannot also be a linker input",
                ));
            }
            HostArtifactKindV1::LinkerScript | HostArtifactKindV1::ResponseFile => {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "control files require a typed fixed-root expansion record",
                ));
            }
            HostArtifactKindV1::Plugin => {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::Plugin,
                    "linker plugins are outside HostLinkClosureV1",
                ));
            }
            HostArtifactKindV1::LtoCache => {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::Lto,
                    "LTO inputs are outside HostLinkClosureV1",
                ));
            }
            HostArtifactKindV1::Dso => {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::ArtifactKind,
                    "DSO inputs are outside the static host-link execution profile",
                ));
            }
            HostArtifactKindV1::BuildScriptNative
                if artifact.identity().provenance != ArtifactProvenanceV1::BuildScript =>
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::UnpublishedBuildScript,
                    "build-script native library was not published through the artifact catalog",
                ));
            }
            _ => {}
        }
        let id = artifact.id();
        self.retain(artifact)?;
        self.push_final(FinalArgument::Artifact(id))?;
        Ok(())
    }

    fn push_final(&mut self, argument: FinalArgument) -> Result<(), HostLinkError> {
        if self.final_arguments.len() >= MAX_FINAL_ARGUMENTS {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "expanded LLD argument list exceeds its bound",
            ));
        }
        let argument_bytes = match &argument {
            FinalArgument::Literal(value) => value.len(),
            FinalArgument::Artifact(id) => {
                let artifact = self.artifacts.get(id).ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::InvalidState,
                        "expanded artifact argument is not retained",
                    )
                })?;
                input_argument(HOST_LLD_FIRST_INPUT_CHILD_FD_V1, &artifact.identity)?.len()
            }
        };
        self.final_argument_bytes = self
            .final_argument_bytes
            .checked_add(argument_bytes)
            .filter(|total| *total <= STATIC_TOOL_MAX_TOTAL_ARGUMENT_BYTES_V1)
            .ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::FieldTooLarge,
                    "expanded LLD argument bytes exceed the static-tool bound",
                )
            })?;
        self.final_arguments.push(argument);
        Ok(())
    }

    fn retain(&mut self, artifact: &PublishedHostArtifactV1) -> Result<(), HostLinkError> {
        if let Some(existing) = self.artifacts.get(&artifact.id()) {
            if existing.identity != *artifact.identity() {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::DigestMismatch,
                    "same artifact ID resolved to different identities",
                ));
            }
            return Ok(());
        }
        if self.artifacts.len() >= crate::MAX_HOST_LINK_PRODUCERS_V1 {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "resolved closure artifact count exceeds the canonical argv bound",
            ));
        }
        let retained_bytes = checked_retained_bytes(
            self.retained_bytes,
            artifact.identity().size,
            "resolved host-link closure",
        )?;
        let archive_members = checked_archive_members(
            self.archive_members,
            artifact.archive_members(),
            "resolved host-link closure",
        )?;
        let retained = ClosureArtifactV1::from_published(artifact)?;
        self.artifacts.insert(artifact.id(), retained);
        self.retained_bytes = retained_bytes;
        self.archive_members = archive_members;
        Ok(())
    }

    fn ensure_runtime_artifact(
        &mut self,
        id: ArtifactIdV1,
    ) -> Result<&ArtifactIdentityV1, HostLinkError> {
        if !self.artifacts.contains_key(&id) {
            let published = self
                .plan
                .producer(id)
                .or_else(|| self.catalog.get(id))
                .ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::RuntimeDsoClosure,
                        "runtime DSO binding references no retained producer or catalog artifact",
                    )
                })?;
            self.retain(published)?;
        }
        Ok(&self
            .artifacts
            .get(&id)
            .expect("runtime artifact was retained")
            .identity)
    }

    fn validate_runtime_closure(&mut self) -> Result<(), HostLinkError> {
        let expected = &self.plan.manifest().spec.expected_output_elf;
        let runtime = &self.plan.manifest().spec.runtime_dsos;
        match (expected.interpreter.as_ref(), runtime.interpreter_artifact) {
            (None, None) => {}
            (Some(_), Some(id)) => {
                let identity = self.ensure_runtime_artifact(id)?;
                if identity.kind != HostArtifactKindV1::Dso {
                    return Err(HostLinkError::new(
                        HostLinkErrorCodeV1::RuntimeDsoClosure,
                        "PT_INTERP runtime binding is not a DSO artifact",
                    ));
                }
            }
            _ => {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::RuntimeDsoClosure,
                    "PT_INTERP policy and interpreter artifact presence disagree",
                ));
            }
        }

        let bindings = runtime
            .bindings
            .iter()
            .map(|binding| (binding.soname.as_slice(), binding))
            .collect::<BTreeMap<_, _>>();
        let mut pending = VecDeque::from(expected.needed.clone());
        if let Some(loader) = runtime.interpreter_artifact {
            let loader_profile = self
                .ensure_runtime_artifact(loader)?
                .elf_profile
                .as_ref()
                .ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::RuntimeDsoClosure,
                        "interpreter artifact has no parsed ELF profile",
                    )
                })?;
            pending.extend(loader_profile.needed.iter().cloned());
        }
        let mut visited = BTreeSet::new();
        while let Some(soname) = pending.pop_front() {
            if !visited.insert(soname.clone()) {
                continue;
            }
            let binding = bindings.get(soname.as_slice()).ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::RuntimeDsoClosure,
                    format!(
                        "runtime DSO closure has no binding for {}",
                        String::from_utf8_lossy(&soname)
                    ),
                )
            })?;
            let identity = self.ensure_runtime_artifact(binding.artifact)?;
            if identity.kind != HostArtifactKindV1::Dso {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::RuntimeDsoClosure,
                    "runtime SONAME resolves to a non-DSO artifact",
                ));
            }
            let profile = identity.elf_profile.as_ref().ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::RuntimeDsoClosure,
                    "runtime DSO has no parsed ELF profile",
                )
            })?;
            if profile.soname.as_deref() != Some(binding.soname.as_slice())
                || profile.needed != binding.needed
            {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::RuntimeDsoClosure,
                    "runtime DSO SONAME or transitive dependency list does not match the plan",
                ));
            }
            pending.extend(binding.needed.iter().cloned());
        }
        if visited.len() != runtime.bindings.len() {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::RuntimeDsoClosure,
                "runtime DSO closure contains unreachable extra bindings",
            ));
        }
        Ok(())
    }
}

fn checked_archive_members(
    current: u64,
    additional: u64,
    context: &str,
) -> Result<u64, HostLinkError> {
    current
        .checked_add(additional)
        .filter(|total| *total <= crate::MAX_HOST_LINK_ARCHIVE_MEMBERS_V1)
        .ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                format!("{context} exceeds the cumulative archive-member bound"),
            )
        })
}

fn build_stable_lld_argv(
    artifacts: &BTreeMap<ArtifactIdV1, ClosureArtifactV1>,
    final_arguments: &[FinalArgument],
) -> Result<StableLldArgvV1, HostLinkError> {
    if !final_arguments
        .iter()
        .any(|argument| matches!(argument, FinalArgument::Artifact(_)))
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnsupportedArgument,
            "fe2o3-host-lld V1 requires at least one descriptor-backed linker input",
        ));
    }

    let mut canonical_arguments = Vec::new();
    let mut total_bytes = 0_usize;
    append_canonical_argument(
        &mut canonical_arguments,
        &mut total_bytes,
        b"fe2o3-host-lld".to_vec(),
    )?;
    append_canonical_argument(
        &mut canonical_arguments,
        &mut total_bytes,
        HOST_LLD_PROTOCOL_ARGUMENT_V1.as_bytes().to_vec(),
    )?;
    let mut child_fds = BTreeMap::new();
    let mut inherited_inputs = Vec::new();
    for argument in final_arguments {
        match argument {
            FinalArgument::Literal(value) => append_canonical_argument(
                &mut canonical_arguments,
                &mut total_bytes,
                value.clone(),
            )?,
            FinalArgument::Artifact(id) => {
                let artifact = artifacts.get(id).ok_or_else(|| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::InvalidState,
                        "final argv references an unretained artifact",
                    )
                })?;
                let child_fd = if let Some(existing) = child_fds.get(id) {
                    *existing
                } else {
                    if child_fds.len() >= STATIC_TOOL_MAX_UNIQUE_INPUTS_V1 {
                        return Err(HostLinkError::new(
                            HostLinkErrorCodeV1::FieldTooLarge,
                            "unique inherited inputs exceed the static-tool 2048-input bound",
                        ));
                    }
                    let offset = i32::try_from(child_fds.len()).map_err(|_| {
                        HostLinkError::new(
                            HostLinkErrorCodeV1::FieldTooLarge,
                            "too many inherited linker descriptors",
                        )
                    })?;
                    let child_fd = HOST_LLD_FIRST_INPUT_CHILD_FD_V1
                        .checked_add(offset)
                        .ok_or_else(|| {
                            HostLinkError::new(
                                HostLinkErrorCodeV1::FieldTooLarge,
                                "child descriptor assignment overflow",
                            )
                        })?;
                    inherited_inputs.push(InheritedDescriptorV1 {
                        child_fd,
                        role: InheritedDescriptorRoleV1::Input(*id),
                        file: artifact
                            .file
                            .try_clone()
                            .context(HostLinkErrorCodeV1::Io, || {
                                format!("clone final linker input {}", artifact.identity.label)
                            })?,
                    });
                    child_fds.insert(*id, child_fd);
                    child_fd
                };
                append_canonical_argument(
                    &mut canonical_arguments,
                    &mut total_bytes,
                    input_argument(child_fd, &artifact.identity)?,
                )?;
            }
        }
    }

    Ok(StableLldArgvV1 {
        canonical_arguments,
        inherited_inputs,
    })
}

fn append_canonical_argument(
    arguments: &mut Vec<Vec<u8>>,
    total_bytes: &mut usize,
    argument: Vec<u8>,
) -> Result<(), HostLinkError> {
    if arguments.len() >= MAX_FINAL_ARGUMENTS {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::FieldTooLarge,
            "canonical LLD argument count exceeds its bound",
        ));
    }
    *total_bytes = total_bytes
        .checked_add(argument.len())
        .filter(|total| *total <= STATIC_TOOL_MAX_TOTAL_ARGUMENT_BYTES_V1)
        .ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "canonical LLD argument bytes exceed the static-tool bound",
            )
        })?;
    arguments.push(argument);
    Ok(())
}

fn finalize_lld_argv(
    stable: StableLldArgvV1,
    result_child: File,
    result_identity: SocketIdentityV1,
    plan_digest: Sha256Digest,
    closure_digest: Sha256Digest,
    nonce_sha256: Sha256Digest,
) -> Result<LldArgvV1, HostLinkError> {
    let mut canonical_arguments = Vec::with_capacity(stable.canonical_arguments.len() + 2);
    canonical_arguments.extend_from_slice(&stable.canonical_arguments[..2]);
    let result_argument = result_socket_argument(result_identity);
    let request_argument = request_argument(plan_digest, closure_digest, nonce_sha256);
    canonical_arguments.push(result_argument.clone());
    canonical_arguments.push(request_argument.clone());
    canonical_arguments.extend_from_slice(&stable.canonical_arguments[2..]);

    let mut inherited = Vec::with_capacity(stable.inherited_inputs.len() + 1);
    inherited.push(InheritedDescriptorV1 {
        child_fd: HOST_LLD_RESULT_SOCKET_CHILD_FD_V1,
        role: InheritedDescriptorRoleV1::ResultSocket,
        file: result_child,
    });
    inherited.extend(stable.inherited_inputs);
    validate_static_tool_argv(
        &canonical_arguments,
        &inherited,
        &result_argument,
        &request_argument,
    )?;

    let arguments = canonical_arguments
        .iter()
        .map(|argument| {
            String::from_utf8(argument.clone())
                .map(OsString::from)
                .map_err(|_| {
                    HostLinkError::new(
                        HostLinkErrorCodeV1::InvalidText,
                        "canonical LLD argument is not UTF-8",
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LldArgvV1 {
        arguments,
        canonical_arguments,
        inherited,
    })
}

fn input_argument(child_fd: i32, identity: &ArtifactIdentityV1) -> Result<Vec<u8>, HostLinkError> {
    let kind = match identity.kind {
        HostArtifactKindV1::Crt | HostArtifactKindV1::Object
            if identity
                .elf_profile
                .as_ref()
                .is_some_and(|profile| profile.elf_type == object::elf::ET_REL) =>
        {
            "elf-rel"
        }
        HostArtifactKindV1::RegularArchive => "archive",
        HostArtifactKindV1::Rlib => "rlib",
        HostArtifactKindV1::BuildScriptNative
            if identity
                .elf_profile
                .as_ref()
                .is_some_and(|profile| profile.elf_type == object::elf::ET_REL) =>
        {
            "elf-rel"
        }
        _ => {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::ArtifactKind,
                format!(
                    "artifact {} has no fe2o3-host-lld V1 input kind",
                    identity.label
                ),
            ));
        }
    };
    Ok(format!(
        "{HOST_LLD_INPUT_ARGUMENT_PREFIX_V1}{child_fd}:{kind}:{}:{}:{:04o}",
        identity.sha256, identity.size, identity.mode
    )
    .into_bytes())
}

fn result_socket_argument(identity: SocketIdentityV1) -> Vec<u8> {
    format!(
        "{HOST_LLD_RESULT_SOCKET_ARGUMENT_PREFIX_V1}{HOST_LLD_RESULT_SOCKET_CHILD_FD_V1}:{}:{}",
        identity.device, identity.inode
    )
    .into_bytes()
}

fn request_argument(
    plan_digest: Sha256Digest,
    closure_digest: Sha256Digest,
    nonce_sha256: Sha256Digest,
) -> Vec<u8> {
    format!("{HOST_LLD_REQUEST_ARGUMENT_PREFIX_V1}{plan_digest}:{closure_digest}:{nonce_sha256}")
        .into_bytes()
}

fn validate_static_tool_argv(
    arguments: &[Vec<u8>],
    inherited: &[InheritedDescriptorV1],
    expected_result: &[u8],
    expected_request: &[u8],
) -> Result<(), HostLinkError> {
    if arguments.first().map(Vec::as_slice) != Some(b"fe2o3-host-lld")
        || arguments.get(1).map(Vec::as_slice) != Some(HOST_LLD_PROTOCOL_ARGUMENT_V1.as_bytes())
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnsupportedArgument,
            "canonical argv has the wrong static-tool V1 prefix",
        ));
    }
    if arguments.get(2).map(Vec::as_slice) != Some(expected_result)
        || arguments.get(3).map(Vec::as_slice) != Some(expected_request)
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::ReplayMismatch,
            "canonical argv has a missing or substituted result/request binding",
        ));
    }
    if arguments.len() > STATIC_TOOL_MAX_ARGUMENTS_V1 {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::FieldTooLarge,
            "canonical argv exceeds the static tool argument-count bound",
        ));
    }

    let mut total_bytes = 0usize;
    for argument in arguments {
        if argument.is_empty()
            || argument.len() > STATIC_TOOL_MAX_ARGUMENT_BYTES_V1
            || !argument.iter().all(|byte| (0x20..=0x7e).contains(byte))
        {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidText,
                "canonical static-tool arguments must be bounded printable ASCII",
            ));
        }
        total_bytes = total_bytes.checked_add(argument.len()).ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "canonical static-tool argument bytes overflowed",
            )
        })?;
    }
    if total_bytes > STATIC_TOOL_MAX_TOTAL_ARGUMENT_BYTES_V1 {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::FieldTooLarge,
            "canonical argv exceeds the static tool total-byte bound",
        ));
    }

    let result_count = arguments
        .iter()
        .filter(|argument| {
            argument.starts_with(HOST_LLD_RESULT_SOCKET_ARGUMENT_PREFIX_V1.as_bytes())
        })
        .count();
    let request_count = arguments
        .iter()
        .filter(|argument| argument.starts_with(HOST_LLD_REQUEST_ARGUMENT_PREFIX_V1.as_bytes()))
        .count();
    if result_count != 1 || request_count != 1 {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::DuplicateRecord,
            "canonical argv requires exactly one result and one request control",
        ));
    }

    let input_descriptors = inherited
        .iter()
        .filter_map(|descriptor| match descriptor.role {
            InheritedDescriptorRoleV1::Input(id) => Some((descriptor.child_fd, id)),
            InheritedDescriptorRoleV1::ResultSocket => None,
        })
        .collect::<BTreeMap<_, _>>();
    if inherited
        .first()
        .map(|descriptor| (descriptor.child_fd, descriptor.role))
        != Some((
            HOST_LLD_RESULT_SOCKET_CHILD_FD_V1,
            InheritedDescriptorRoleV1::ResultSocket,
        ))
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::DescriptorChanged,
            "result socket is not inherited first at child fd 91",
        ));
    }

    let mut saw_input = false;
    let mut index = 4;
    while index < arguments.len() {
        let argument = &arguments[index];
        if argument.starts_with(HOST_LLD_INPUT_ARGUMENT_PREFIX_V1.as_bytes()) {
            let child_fd = parse_input_child_fd(argument)?;
            if !input_descriptors.contains_key(&child_fd) {
                return Err(HostLinkError::new(
                    HostLinkErrorCodeV1::DescriptorChanged,
                    "typed input record has no matching inherited descriptor",
                ));
            }
            saw_input = true;
            index += 1;
            continue;
        }
        if argument.starts_with(b"/proc/self/fd/") {
            return Err(HostLinkError::new(
                HostLinkErrorCodeV1::InvalidPath,
                "bare /proc/self/fd linker inputs are outside the V1 grammar",
            ));
        }
        if argument == b"-z" {
            let value = arguments.get(index + 1).ok_or_else(|| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::UnsupportedArgument,
                    "canonical -z option has no policy value",
                )
            })?;
            let value = std::str::from_utf8(value).map_err(|_| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::InvalidText,
                    "canonical -z policy is not UTF-8",
                )
            })?;
            LinkerZPolicyV1::from_str(value)?;
            index += 2;
            continue;
        }
        if let Some(symbol) = argument.strip_prefix(b"--undefined=") {
            let symbol = std::str::from_utf8(symbol).map_err(|_| {
                HostLinkError::new(
                    HostLinkErrorCodeV1::InvalidText,
                    "canonical undefined symbol is not UTF-8",
                )
            })?;
            validate_undefined_symbol(symbol)?;
            index += 1;
            continue;
        }
        validate_literal(argument)?;
        index += 1;
    }
    if !saw_input {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::UnsupportedArgument,
            "canonical argv has no typed descriptor-backed linker input",
        ));
    }
    Ok(())
}

fn parse_input_child_fd(argument: &[u8]) -> Result<i32, HostLinkError> {
    let suffix = argument
        .strip_prefix(HOST_LLD_INPUT_ARGUMENT_PREFIX_V1.as_bytes())
        .ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::InvalidWire,
                "typed input has no V1 prefix",
            )
        })?;
    let fields = suffix.split(|byte| *byte == b':').collect::<Vec<_>>();
    if fields.len() != 5 {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::InvalidWire,
            "typed input must contain exactly five colon-separated fields",
        ));
    }
    let fd = fields[0];
    if fd.is_empty() || (fd.len() > 1 && fd[0] == b'0') || !fd.iter().all(u8::is_ascii_digit) {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::NonCanonicalWire,
            "typed input child fd is not canonical decimal",
        ));
    }
    let fd = std::str::from_utf8(fd)
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                "typed input child fd does not fit i32",
            )
        })?;
    if fd < HOST_LLD_FIRST_INPUT_CHILD_FD_V1 {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::DescriptorChanged,
            "typed input child fd is below the V1 input range",
        ));
    }
    if !matches!(fields[1], b"elf-rel" | b"archive" | b"rlib") {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::ArtifactKind,
            "typed input kind is outside the exact V1 set",
        ));
    }
    if fields[2].len() != 64
        || !fields[2]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::NonCanonicalWire,
            "typed input digest is not lowercase SHA-256 hex",
        ));
    }
    parse_canonical_decimal(fields[3], "typed input size")?;
    if fields[4].len() != 4
        || fields[4][0] != b'0'
        || !fields[4][1..]
            .iter()
            .all(|byte| (b'0'..=b'7').contains(byte))
        || fields[4] == b"0000"
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::NonCanonicalWire,
            "typed input mode is not four-digit nonzero octal",
        ));
    }
    Ok(fd)
}

fn parse_canonical_decimal(value: &[u8], name: &str) -> Result<u64, HostLinkError> {
    if value.is_empty()
        || (value.len() > 1 && value[0] == b'0')
        || !value.iter().all(u8::is_ascii_digit)
    {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::NonCanonicalWire,
            format!("{name} is not canonical decimal"),
        ));
    }
    let value = std::str::from_utf8(value)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .ok_or_else(|| {
            HostLinkError::new(
                HostLinkErrorCodeV1::FieldTooLarge,
                format!("{name} does not fit u64"),
            )
        })?;
    if value == 0 || value > crate::MAX_HOST_LINK_INPUT_BYTES_V1 {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::ArtifactTooLarge,
            format!("{name} is outside its admitted bound"),
        ));
    }
    Ok(value)
}

fn closure_digest(
    plan: &HostLinkPlanV1,
    roots: &FixedRootSetV1,
    artifacts: &BTreeMap<ArtifactIdV1, ClosureArtifactV1>,
    stable_argv: &StableLldArgvV1,
) -> Sha256Digest {
    let mut digest = Sha256::new();
    digest.update(b"fe2o3-host-link-closure-v1\0");
    hash_field(&mut digest, HOST_LINK_RESULT_COPY_POLICY_V1.as_bytes());
    hash_field(&mut digest, plan.plan_digest().as_bytes());
    digest.update(HOST_LLD_RESULT_SOCKET_CHILD_FD_V1.to_le_bytes());
    for (label, root) in roots.iter() {
        hash_field(&mut digest, label.as_bytes());
        hash_field(&mut digest, &root.identity_bytes());
        hash_field(&mut digest, root.tree_digest().as_bytes());
    }
    for (id, artifact) in artifacts {
        hash_field(&mut digest, id.sha256().as_bytes());
        hash_field(&mut digest, artifact.identity.sha256.as_bytes());
        digest.update(artifact.identity.size.to_le_bytes());
        digest.update(artifact.identity.mode.to_le_bytes());
    }
    for argument in &stable_argv.canonical_arguments {
        hash_field(&mut digest, argument);
    }
    for descriptor in &stable_argv.inherited_inputs {
        digest.update(descriptor.child_fd.to_le_bytes());
        let InheritedDescriptorRoleV1::Input(id) = descriptor.role else {
            unreachable!("stable argv contains only input descriptors");
        };
        hash_field(&mut digest, id.sha256().as_bytes());
    }
    Sha256Digest::from_bytes(digest.finalize().into())
}

fn hash_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value);
}

fn validate_output_type(output_type: OutputTypeV1, elf_type: u16) -> Result<(), HostLinkError> {
    let expected = match output_type {
        OutputTypeV1::Executable => object::elf::ET_EXEC,
        OutputTypeV1::SharedObject => object::elf::ET_DYN,
        OutputTypeV1::Relocatable => object::elf::ET_REL,
    };
    if elf_type != expected {
        return Err(HostLinkError::new(
            HostLinkErrorCodeV1::ElfPolicy,
            "sealed ELF type does not match the bound output type",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: u8) -> Sha256Digest {
        Sha256Digest::from_bytes([byte; 32])
    }

    #[test]
    fn cumulative_archive_member_budget_has_exact_boundary_and_overflow_checks() {
        assert_eq!(
            checked_archive_members(
                crate::MAX_HOST_LINK_ARCHIVE_MEMBERS_V1 - 1,
                1,
                "test closure",
            )
            .unwrap(),
            crate::MAX_HOST_LINK_ARCHIVE_MEMBERS_V1
        );
        for (current, additional) in [(crate::MAX_HOST_LINK_ARCHIVE_MEMBERS_V1, 1), (u64::MAX, 1)] {
            assert_eq!(
                checked_archive_members(current, additional, "test closure")
                    .unwrap_err()
                    .code(),
                HostLinkErrorCodeV1::FieldTooLarge
            );
        }
    }

    #[test]
    fn result_and_request_controls_are_exactly_once_and_ordered() {
        assert_eq!(
            crate::MAX_HOST_LINK_PRODUCERS_V1,
            STATIC_TOOL_MAX_UNIQUE_INPUTS_V1
        );
        assert_eq!(HOST_LLD_PROTOCOL_ARGUMENT_V1, "--fe2o3-host-lld-elf-v2");
        let result = b"--fe2o3-result-socket-v1=91:7:11".to_vec();
        let request = request_argument(digest(1), digest(2), digest(3));
        let input = b"--fe2o3-input-v1=100:elf-rel:1111111111111111111111111111111111111111111111111111111111111111:64:0644".to_vec();
        let inherited = vec![
            InheritedDescriptorV1 {
                child_fd: 91,
                role: InheritedDescriptorRoleV1::ResultSocket,
                file: tempfile::tempfile().unwrap(),
            },
            InheritedDescriptorV1 {
                child_fd: 100,
                role: InheritedDescriptorRoleV1::Input(ArtifactIdV1::from_sha256(digest(4))),
                file: tempfile::tempfile().unwrap(),
            },
        ];
        let valid = vec![
            b"fe2o3-host-lld".to_vec(),
            HOST_LLD_PROTOCOL_ARGUMENT_V1.as_bytes().to_vec(),
            result.clone(),
            request.clone(),
            input,
        ];
        validate_static_tool_argv(&valid, &inherited, &result, &request).unwrap();

        let mut missing = valid.clone();
        missing.remove(2);
        assert!(validate_static_tool_argv(&missing, &inherited, &result, &request).is_err());

        let mut duplicate = valid.clone();
        duplicate.insert(4, result.clone());
        assert_eq!(
            validate_static_tool_argv(&duplicate, &inherited, &result, &request)
                .unwrap_err()
                .code(),
            HostLinkErrorCodeV1::DuplicateRecord
        );

        let mut substituted = valid.clone();
        substituted[2] = b"--fe2o3-result-socket-v1=91:7:12".to_vec();
        assert_eq!(
            validate_static_tool_argv(&substituted, &inherited, &result, &request)
                .unwrap_err()
                .code(),
            HostLinkErrorCodeV1::ReplayMismatch
        );

        for malformed in [
            b"/proc/self/fd/100".to_vec(),
            b"--fe2o3-input-v1=100:object:1111111111111111111111111111111111111111111111111111111111111111:64:0644".to_vec(),
            b"--fe2o3-input-v1=100:elf-dso:1111111111111111111111111111111111111111111111111111111111111111:64:0644".to_vec(),
            b"--fe2o3-input-v1=100:elf-rel:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA:64:0644".to_vec(),
            b"--fe2o3-input-v1=100:elf-rel:1111111111111111111111111111111111111111111111111111111111111111:064:0644".to_vec(),
            b"--fe2o3-input-v1=100:elf-rel:1111111111111111111111111111111111111111111111111111111111111111:64:644".to_vec(),
            b"--fe2o3-input-v1=100:elf-rel:1111111111111111111111111111111111111111111111111111111111111111:64:0644:extra".to_vec(),
        ] {
            let mut hostile = valid.clone();
            hostile[4] = malformed;
            assert!(validate_static_tool_argv(
                &hostile,
                &inherited,
                &result,
                &request
            )
            .is_err());
        }
    }
}
