use std::fmt;

use sha2::{Digest, Sha256};

use crate::{PipelineV1, PublicationRightsV1};

/// Distinct Broker V3 frame magic.
pub const BROKER_V3_MAGIC: [u8; 8] = *b"F2AUBR3\0";
/// Broker V3 wire version.
pub const BROKER_V3_VERSION: u16 = 3;
/// Exact Broker V3 frame-header length.
pub const BROKER_V3_HEADER_LEN: usize = 8 + 2 + 2 + 4 + 4 + 4;
/// Exact encoded process-identity length.
pub const PROCESS_IDENTITY_V3_WIRE_LEN: usize = 4 + 4 + 8;
/// Exact encoded descriptor-manifest length.
pub const DESCRIPTOR_MANIFEST_V3_WIRE_LEN: usize = 2 + 2 + (5 * 2) + 2;
/// Exact encoded CapabilityBinding V3 length.
pub const BROKER_V3_BINDING_WIRE_LEN: usize = (9 * 32) + 2 + 2 + 4 + 1 + 7 + 32;
/// Domain for the canonical CapabilityBinding V3 identity.
pub const BROKER_V3_BINDING_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/PROTECTED-AUTHORITY-BROKER-V3-BINDING\0";

/// Exact Hello V3 payload length.
pub const HELLO_V3_PAYLOAD_LEN: usize =
    PROCESS_IDENTITY_V3_WIRE_LEN + BROKER_V3_BINDING_WIRE_LEN + 32;
/// Exact Bootstrap V3 payload length.
pub const BOOTSTRAP_V3_PAYLOAD_LEN: usize =
    PROCESS_IDENTITY_V3_WIRE_LEN + 32 + 32 + DESCRIPTOR_MANIFEST_V3_WIRE_LEN;
/// Exact PostExec V3 payload length.
pub const POST_EXEC_V3_PAYLOAD_LEN: usize = PROCESS_IDENTITY_V3_WIRE_LEN + (3 * 32);
/// Exact Capabilities V3 payload length.
pub const CAPABILITIES_V3_PAYLOAD_LEN: usize =
    PROCESS_IDENTITY_V3_WIRE_LEN + (3 * 32) + DESCRIPTOR_MANIFEST_V3_WIRE_LEN;
/// Exact Prepare V3 payload length.
pub const PREPARE_V3_PAYLOAD_LEN: usize = PROCESS_IDENTITY_V3_WIRE_LEN + (5 * 32);
/// Exact Consume V3 payload length.
pub const CONSUME_V3_PAYLOAD_LEN: usize = PREPARE_V3_PAYLOAD_LEN;

const IDENTITY_LEN: usize = 32;
const MANIFEST_SLOT_COUNT: usize = 5;
const WORKER_PRESENCE_OFFSET: usize = 296;
const WORKER_RESERVED_OFFSET: usize = 297;
const WORKER_IDENTITY_OFFSET: usize = 304;

/// One assigned Broker V3 frame type and sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum BrokerFrameKindV3 {
    /// Static trampoline greeting, sequence zero.
    Hello = 1,
    /// Broker wrapper bootstrap, sequence one.
    Bootstrap = 2,
    /// Post-exec dynamic-wrapper greeting, sequence two.
    PostExec = 3,
    /// Broker compiler capabilities, sequence three.
    Capabilities = 4,
    /// Dynamic-wrapper preparation acknowledgement, sequence four.
    Prepare = 5,
    /// Rust compiler one-shot consumption, sequence five.
    Consume = 6,
}

impl BrokerFrameKindV3 {
    const fn sequence(self) -> u32 {
        match self {
            Self::Hello => 0,
            Self::Bootstrap => 1,
            Self::PostExec => 2,
            Self::Capabilities => 3,
            Self::Prepare => 4,
            Self::Consume => 5,
        }
    }

    const fn payload_len(self) -> usize {
        match self {
            Self::Hello => HELLO_V3_PAYLOAD_LEN,
            Self::Bootstrap => BOOTSTRAP_V3_PAYLOAD_LEN,
            Self::PostExec => POST_EXEC_V3_PAYLOAD_LEN,
            Self::Capabilities => CAPABILITIES_V3_PAYLOAD_LEN,
            Self::Prepare => PREPARE_V3_PAYLOAD_LEN,
            Self::Consume => CONSUME_V3_PAYLOAD_LEN,
        }
    }

    fn from_wire(value: u16) -> Result<Self, BrokerProtocolErrorV3> {
        match value {
            1 => Ok(Self::Hello),
            2 => Ok(Self::Bootstrap),
            3 => Ok(Self::PostExec),
            4 => Ok(Self::Capabilities),
            5 => Ok(Self::Prepare),
            6 => Ok(Self::Consume),
            _ => Err(BrokerProtocolErrorV3::UnknownFrameType { actual: value }),
        }
    }
}

/// The only target admitted by Broker V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum BrokerTargetV3 {
    /// AMD gfx942 with XNACK disabled.
    Gfx942XnackMinus = 1,
}

impl BrokerTargetV3 {
    fn from_wire(value: u16) -> Result<Self, BrokerProtocolErrorV3> {
        match value {
            1 => Ok(Self::Gfx942XnackMinus),
            _ => Err(BrokerProtocolErrorV3::UnknownTarget { actual: value }),
        }
    }
}

/// A required nonzero identity in Broker V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BrokerIdentityFieldV3 {
    /// Canonical Policy V1 identity.
    Policy,
    /// Protected Protocol V1 admission identity.
    ProtectedAdmission,
    /// Build-session identity.
    BuildSession,
    /// Canonical Cargo environment identity.
    CargoEnvironment,
    /// Static rustc-trampoline image identity.
    TrampolineExecutable,
    /// Dynamic `cargo-fe2o3` image identity.
    CargoFe2o3Executable,
    /// Canonical compiler-closure identity.
    CompilerClosure,
    /// Retained runtime-directory object identity.
    RuntimeObject,
    /// Codegen-backend image identity.
    CodegenBackend,
    /// Optional Worker V2 artifact identity.
    WorkerV2,
    /// Canonical CapabilityBinding V3 identity.
    CapabilityBinding,
    /// One-shot bootstrap transfer identity.
    BootstrapTransfer,
    /// One-shot compiler-capability transfer identity.
    CapabilityTransfer,
}

impl fmt::Display for BrokerIdentityFieldV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Policy => "policy",
            Self::ProtectedAdmission => "protected admission",
            Self::BuildSession => "build session",
            Self::CargoEnvironment => "Cargo environment",
            Self::TrampolineExecutable => "rustc trampoline executable",
            Self::CargoFe2o3Executable => "cargo-fe2o3 executable",
            Self::CompilerClosure => "compiler closure",
            Self::RuntimeObject => "runtime object",
            Self::CodegenBackend => "codegen backend",
            Self::WorkerV2 => "Worker V2",
            Self::CapabilityBinding => "capability binding",
            Self::BootstrapTransfer => "bootstrap transfer",
            Self::CapabilityTransfer => "capability transfer",
        };
        formatter.write_str(name)
    }
}

/// One descriptor kind assigned by the inert Broker V3 manifest model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum BrokerDescriptorKindV3 {
    /// Sealed `cargo-fe2o3` wrapper executable image.
    CargoFe2o3WrapperExecutable = 1,
    /// Sealed rustc executable image.
    RustcExecutable = 2,
    /// Retained rustc runtime-directory object.
    RustcRuntimeDirectory = 3,
    /// Sealed codegen-backend image.
    CodegenBackend = 4,
    /// Retained artifact-directory object.
    ArtifactDirectory = 5,
    /// Retained Cargo-observation object.
    CargoObservation = 6,
}

impl BrokerDescriptorKindV3 {
    fn from_wire(value: u16) -> Result<Self, BrokerProtocolErrorV3> {
        match value {
            1 => Ok(Self::CargoFe2o3WrapperExecutable),
            2 => Ok(Self::RustcExecutable),
            3 => Ok(Self::RustcRuntimeDirectory),
            4 => Ok(Self::CodegenBackend),
            5 => Ok(Self::ArtifactDirectory),
            6 => Ok(Self::CargoObservation),
            _ => Err(BrokerProtocolErrorV3::UnknownDescriptorKind { actual: value }),
        }
    }
}

/// One exact Broker V3 descriptor manifest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum BrokerDescriptorManifestV3 {
    /// Exactly one sealed dynamic-wrapper executable.
    Bootstrap = 1,
    /// Exactly the five compiler-chain objects, in canonical order.
    CompilerCapabilities = 2,
}

impl BrokerDescriptorManifestV3 {
    /// Returns the exact descriptor count.
    pub const fn descriptor_count(self) -> u16 {
        match self {
            Self::Bootstrap => 1,
            Self::CompilerCapabilities => 5,
        }
    }

    /// Returns the descriptor kind at one canonical manifest index.
    pub const fn descriptor_kind(self, index: usize) -> Option<BrokerDescriptorKindV3> {
        match (self, index) {
            (Self::Bootstrap, 0) => Some(BrokerDescriptorKindV3::CargoFe2o3WrapperExecutable),
            (Self::CompilerCapabilities, 0) => Some(BrokerDescriptorKindV3::RustcExecutable),
            (Self::CompilerCapabilities, 1) => Some(BrokerDescriptorKindV3::RustcRuntimeDirectory),
            (Self::CompilerCapabilities, 2) => Some(BrokerDescriptorKindV3::CodegenBackend),
            (Self::CompilerCapabilities, 3) => Some(BrokerDescriptorKindV3::ArtifactDirectory),
            (Self::CompilerCapabilities, 4) => Some(BrokerDescriptorKindV3::CargoObservation),
            _ => None,
        }
    }

    fn from_wire(value: u16) -> Result<Self, BrokerProtocolErrorV3> {
        match value {
            1 => Ok(Self::Bootstrap),
            2 => Ok(Self::CompilerCapabilities),
            _ => Err(BrokerProtocolErrorV3::UnknownManifestType { actual: value }),
        }
    }

    fn encode(self) -> [u8; DESCRIPTOR_MANIFEST_V3_WIRE_LEN] {
        let mut encoded = [0_u8; DESCRIPTOR_MANIFEST_V3_WIRE_LEN];
        encoded[0..2].copy_from_slice(&(self as u16).to_le_bytes());
        encoded[2..4].copy_from_slice(&self.descriptor_count().to_le_bytes());
        for index in 0..usize::from(self.descriptor_count()) {
            let kind = self
                .descriptor_kind(index)
                .expect("canonical manifest count has a descriptor kind");
            let offset = 4 + (index * 2);
            encoded[offset..offset + 2].copy_from_slice(&(kind as u16).to_le_bytes());
        }
        encoded
    }

    fn decode(encoded: &[u8]) -> Result<Self, BrokerProtocolErrorV3> {
        let manifest = Self::from_wire(read_u16(encoded, 0))?;
        let count = read_u16(encoded, 2);
        if count != manifest.descriptor_count() {
            return Err(BrokerProtocolErrorV3::InvalidDescriptorCount {
                manifest,
                expected: manifest.descriptor_count(),
                actual: count,
            });
        }
        for index in 0..MANIFEST_SLOT_COUNT {
            let actual = read_u16(encoded, 4 + (index * 2));
            match manifest.descriptor_kind(index) {
                Some(expected) => {
                    let actual_kind = BrokerDescriptorKindV3::from_wire(actual)?;
                    if actual_kind != expected {
                        return Err(BrokerProtocolErrorV3::InvalidDescriptorKind {
                            manifest,
                            index,
                            expected,
                            actual: actual_kind,
                        });
                    }
                }
                None if actual != 0 => {
                    return Err(BrokerProtocolErrorV3::NonzeroUnusedDescriptorSlot { index });
                }
                None => {}
            }
        }
        if read_u16(encoded, 14) != 0 {
            return Err(BrokerProtocolErrorV3::NonzeroManifestReserved);
        }
        Ok(manifest)
    }
}

/// Stable process identity carried across the trampoline, wrapper, and rustc exec chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProcessIdentityV3 {
    pid: u32,
    start_time_ticks: u64,
}

impl ProcessIdentityV3 {
    /// Constructs a nonzero process identity.
    pub fn new(pid: u32, start_time_ticks: u64) -> Result<Self, BrokerProtocolErrorV3> {
        if pid == 0 {
            return Err(BrokerProtocolErrorV3::ZeroProcessId);
        }
        if start_time_ticks == 0 {
            return Err(BrokerProtocolErrorV3::ZeroProcessStartTime);
        }
        Ok(Self {
            pid,
            start_time_ticks,
        })
    }

    /// Returns the numeric process identifier.
    pub const fn pid(self) -> u32 {
        self.pid
    }

    /// Returns the `/proc` start-time tick count.
    pub const fn start_time_ticks(self) -> u64 {
        self.start_time_ticks
    }

    fn encode(self, output: &mut [u8]) {
        output[0..4].copy_from_slice(&self.pid.to_le_bytes());
        output[8..16].copy_from_slice(&self.start_time_ticks.to_le_bytes());
    }

    fn decode(encoded: &[u8]) -> Result<Self, BrokerProtocolErrorV3> {
        if read_u32(encoded, 4) != 0 {
            return Err(BrokerProtocolErrorV3::NonzeroProcessReserved);
        }
        Self::new(read_u32(encoded, 0), read_u64(encoded, 8))
    }
}

/// Canonical, zero-publication-rights binding for one Broker V3 build chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityBindingV3 {
    policy_identity: [u8; 32],
    protected_admission_identity: [u8; 32],
    build_session_identity: [u8; 32],
    pipeline: PipelineV1,
    cargo_environment_identity: [u8; 32],
    trampoline_executable_identity: [u8; 32],
    cargo_fe2o3_executable_identity: [u8; 32],
    compiler_closure_identity: [u8; 32],
    runtime_object_identity: [u8; 32],
    codegen_backend_identity: [u8; 32],
    worker_v2_identity: Option<[u8; 32]>,
}

impl CapabilityBindingV3 {
    /// Constructs one inert, staging-only Broker V3 binding.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        policy_identity: [u8; 32],
        protected_admission_identity: [u8; 32],
        build_session_identity: [u8; 32],
        pipeline: PipelineV1,
        cargo_environment_identity: [u8; 32],
        trampoline_executable_identity: [u8; 32],
        cargo_fe2o3_executable_identity: [u8; 32],
        compiler_closure_identity: [u8; 32],
        runtime_object_identity: [u8; 32],
        codegen_backend_identity: [u8; 32],
        worker_v2_identity: Option<[u8; 32]>,
    ) -> Result<Self, BrokerProtocolErrorV3> {
        for (field, identity) in [
            (BrokerIdentityFieldV3::Policy, policy_identity),
            (
                BrokerIdentityFieldV3::ProtectedAdmission,
                protected_admission_identity,
            ),
            (BrokerIdentityFieldV3::BuildSession, build_session_identity),
            (
                BrokerIdentityFieldV3::CargoEnvironment,
                cargo_environment_identity,
            ),
            (
                BrokerIdentityFieldV3::TrampolineExecutable,
                trampoline_executable_identity,
            ),
            (
                BrokerIdentityFieldV3::CargoFe2o3Executable,
                cargo_fe2o3_executable_identity,
            ),
            (
                BrokerIdentityFieldV3::CompilerClosure,
                compiler_closure_identity,
            ),
            (
                BrokerIdentityFieldV3::RuntimeObject,
                runtime_object_identity,
            ),
            (
                BrokerIdentityFieldV3::CodegenBackend,
                codegen_backend_identity,
            ),
        ] {
            validate_identity(identity, field)?;
        }
        if let Some(worker) = worker_v2_identity {
            validate_identity(worker, BrokerIdentityFieldV3::WorkerV2)?;
        }
        Ok(Self {
            policy_identity,
            protected_admission_identity,
            build_session_identity,
            pipeline,
            cargo_environment_identity,
            trampoline_executable_identity,
            cargo_fe2o3_executable_identity,
            compiler_closure_identity,
            runtime_object_identity,
            codegen_backend_identity,
            worker_v2_identity,
        })
    }

    /// Returns the Policy V1 identity.
    pub const fn policy_identity(self) -> [u8; 32] {
        self.policy_identity
    }

    /// Returns the protected Protocol V1 admission identity.
    pub const fn protected_admission_identity(self) -> [u8; 32] {
        self.protected_admission_identity
    }

    /// Returns the build-session identity.
    pub const fn build_session_identity(self) -> [u8; 32] {
        self.build_session_identity
    }

    /// Returns the fixed target.
    pub const fn target(self) -> BrokerTargetV3 {
        BrokerTargetV3::Gfx942XnackMinus
    }

    /// Returns the selected pipeline.
    pub const fn pipeline(self) -> PipelineV1 {
        self.pipeline
    }

    /// Returns the zero publication-rights value.
    pub const fn publication_rights(self) -> PublicationRightsV1 {
        PublicationRightsV1::NONE
    }

    /// Returns the canonical Cargo-environment identity.
    pub const fn cargo_environment_identity(self) -> [u8; 32] {
        self.cargo_environment_identity
    }

    /// Returns the static trampoline executable identity.
    pub const fn trampoline_executable_identity(self) -> [u8; 32] {
        self.trampoline_executable_identity
    }

    /// Returns the dynamic `cargo-fe2o3` executable identity.
    pub const fn cargo_fe2o3_executable_identity(self) -> [u8; 32] {
        self.cargo_fe2o3_executable_identity
    }

    /// Returns the canonical compiler-closure identity.
    pub const fn compiler_closure_identity(self) -> [u8; 32] {
        self.compiler_closure_identity
    }

    /// Returns the retained runtime-object identity.
    pub const fn runtime_object_identity(self) -> [u8; 32] {
        self.runtime_object_identity
    }

    /// Returns the codegen-backend image identity.
    pub const fn codegen_backend_identity(self) -> [u8; 32] {
        self.codegen_backend_identity
    }

    /// Returns the optional Worker V2 identity.
    pub const fn worker_v2_identity(self) -> Option<[u8; 32]> {
        self.worker_v2_identity
    }

    /// Returns the canonical fixed-width encoding.
    pub fn encode(self) -> [u8; BROKER_V3_BINDING_WIRE_LEN] {
        let mut encoded = [0_u8; BROKER_V3_BINDING_WIRE_LEN];
        let mut cursor = 0;
        for identity in [
            self.policy_identity,
            self.protected_admission_identity,
            self.build_session_identity,
        ] {
            write_bytes(&mut encoded, &mut cursor, &identity);
        }
        write_bytes(
            &mut encoded,
            &mut cursor,
            &(BrokerTargetV3::Gfx942XnackMinus as u16).to_le_bytes(),
        );
        write_bytes(
            &mut encoded,
            &mut cursor,
            &(self.pipeline as u16).to_le_bytes(),
        );
        write_bytes(
            &mut encoded,
            &mut cursor,
            &PublicationRightsV1::NONE.bits().to_le_bytes(),
        );
        for identity in [
            self.cargo_environment_identity,
            self.trampoline_executable_identity,
            self.cargo_fe2o3_executable_identity,
            self.compiler_closure_identity,
            self.runtime_object_identity,
            self.codegen_backend_identity,
        ] {
            write_bytes(&mut encoded, &mut cursor, &identity);
        }
        if let Some(worker) = self.worker_v2_identity {
            encoded[WORKER_PRESENCE_OFFSET] = 1;
            encoded[WORKER_IDENTITY_OFFSET..WORKER_IDENTITY_OFFSET + IDENTITY_LEN]
                .copy_from_slice(&worker);
        }
        debug_assert_eq!(cursor, WORKER_PRESENCE_OFFSET);
        encoded
    }

    /// Returns the domain-separated canonical binding identity.
    pub fn identity_sha256(self) -> [u8; 32] {
        let encoded = self.encode();
        let mut digest = Sha256::new();
        digest.update(BROKER_V3_BINDING_IDENTITY_DOMAIN);
        digest.update((BROKER_V3_BINDING_WIRE_LEN as u64).to_le_bytes());
        digest.update(encoded);
        digest.finalize().into()
    }
}

/// Decodes one exact canonical CapabilityBinding V3.
pub fn decode_capability_binding_v3(
    encoded: &[u8],
) -> Result<CapabilityBindingV3, BrokerProtocolErrorV3> {
    if encoded.len() != BROKER_V3_BINDING_WIRE_LEN {
        return Err(BrokerProtocolErrorV3::InvalidBindingLength {
            actual: encoded.len(),
        });
    }
    BrokerTargetV3::from_wire(read_u16(encoded, 96))?;
    let pipeline = match read_u16(encoded, 98) {
        1 => PipelineV1::CollectedRowSoftmax,
        2 => PipelineV1::CollectedTiledGemm,
        actual => return Err(BrokerProtocolErrorV3::UnknownPipeline { actual }),
    };
    let rights = read_u32(encoded, 100);
    if rights != PublicationRightsV1::NONE.bits() {
        return Err(BrokerProtocolErrorV3::PublicationAuthorityForbidden { actual: rights });
    }
    let presence = encoded[WORKER_PRESENCE_OFFSET];
    if encoded[WORKER_RESERVED_OFFSET..WORKER_IDENTITY_OFFSET] != [0; 7] {
        return Err(BrokerProtocolErrorV3::NonzeroBindingReserved);
    }
    let worker = digest_at(encoded, WORKER_IDENTITY_OFFSET);
    let worker = match presence {
        0 if worker == [0; 32] => None,
        0 => return Err(BrokerProtocolErrorV3::WorkerIdentityWithoutPresence),
        1 => Some(worker),
        actual => return Err(BrokerProtocolErrorV3::InvalidWorkerPresence { actual }),
    };
    CapabilityBindingV3::new(
        digest_at(encoded, 0),
        digest_at(encoded, 32),
        digest_at(encoded, 64),
        pipeline,
        digest_at(encoded, 104),
        digest_at(encoded, 136),
        digest_at(encoded, 168),
        digest_at(encoded, 200),
        digest_at(encoded, 232),
        digest_at(encoded, 264),
        worker,
    )
}

/// Static-trampoline Hello V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelloV3 {
    process: ProcessIdentityV3,
    binding: CapabilityBindingV3,
    observed_trampoline_identity: [u8; 32],
}

impl HelloV3 {
    /// Constructs a typed Hello frame.
    pub fn new(
        process: ProcessIdentityV3,
        binding: CapabilityBindingV3,
        observed_trampoline_identity: [u8; 32],
    ) -> Result<Self, BrokerProtocolErrorV3> {
        validate_identity(
            observed_trampoline_identity,
            BrokerIdentityFieldV3::TrampolineExecutable,
        )?;
        Ok(Self {
            process,
            binding,
            observed_trampoline_identity,
        })
    }

    /// Constructs a Hello whose observation matches the binding.
    pub fn for_binding(process: ProcessIdentityV3, binding: CapabilityBindingV3) -> Self {
        Self {
            process,
            binding,
            observed_trampoline_identity: binding.trampoline_executable_identity(),
        }
    }

    /// Returns the stable process identity.
    pub const fn process(self) -> ProcessIdentityV3 {
        self.process
    }

    /// Returns the complete capability binding.
    pub const fn binding(self) -> CapabilityBindingV3 {
        self.binding
    }

    /// Returns the trampoline image observed by the sender.
    pub const fn observed_trampoline_identity(self) -> [u8; 32] {
        self.observed_trampoline_identity
    }
}

/// Broker-to-trampoline Bootstrap V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootstrapV3 {
    process: ProcessIdentityV3,
    binding_identity: [u8; 32],
    bootstrap_identity: [u8; 32],
    descriptor_manifest: BrokerDescriptorManifestV3,
}

impl BootstrapV3 {
    /// Constructs a typed Bootstrap frame.
    pub fn new(
        process: ProcessIdentityV3,
        binding_identity: [u8; 32],
        bootstrap_identity: [u8; 32],
        descriptor_manifest: BrokerDescriptorManifestV3,
    ) -> Result<Self, BrokerProtocolErrorV3> {
        validate_identity(binding_identity, BrokerIdentityFieldV3::CapabilityBinding)?;
        validate_identity(bootstrap_identity, BrokerIdentityFieldV3::BootstrapTransfer)?;
        Ok(Self {
            process,
            binding_identity,
            bootstrap_identity,
            descriptor_manifest,
        })
    }

    /// Returns the stable process identity.
    pub const fn process(self) -> ProcessIdentityV3 {
        self.process
    }

    /// Returns the canonical capability-binding identity.
    pub const fn binding_identity(self) -> [u8; 32] {
        self.binding_identity
    }

    /// Returns the one-shot bootstrap transfer identity.
    pub const fn bootstrap_identity(self) -> [u8; 32] {
        self.bootstrap_identity
    }

    /// Returns the declared descriptor manifest.
    pub const fn descriptor_manifest(self) -> BrokerDescriptorManifestV3 {
        self.descriptor_manifest
    }
}

/// Dynamic-wrapper PostExec V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PostExecV3 {
    process: ProcessIdentityV3,
    binding_identity: [u8; 32],
    bootstrap_identity: [u8; 32],
    observed_cargo_fe2o3_identity: [u8; 32],
}

impl PostExecV3 {
    /// Constructs a typed PostExec frame.
    pub fn new(
        process: ProcessIdentityV3,
        binding_identity: [u8; 32],
        bootstrap_identity: [u8; 32],
        observed_cargo_fe2o3_identity: [u8; 32],
    ) -> Result<Self, BrokerProtocolErrorV3> {
        validate_identity(binding_identity, BrokerIdentityFieldV3::CapabilityBinding)?;
        validate_identity(bootstrap_identity, BrokerIdentityFieldV3::BootstrapTransfer)?;
        validate_identity(
            observed_cargo_fe2o3_identity,
            BrokerIdentityFieldV3::CargoFe2o3Executable,
        )?;
        Ok(Self {
            process,
            binding_identity,
            bootstrap_identity,
            observed_cargo_fe2o3_identity,
        })
    }

    /// Returns the stable process identity.
    pub const fn process(self) -> ProcessIdentityV3 {
        self.process
    }

    /// Returns the canonical capability-binding identity.
    pub const fn binding_identity(self) -> [u8; 32] {
        self.binding_identity
    }

    /// Returns the one-shot bootstrap transfer identity.
    pub const fn bootstrap_identity(self) -> [u8; 32] {
        self.bootstrap_identity
    }

    /// Returns the dynamic wrapper image observed after exec.
    pub const fn observed_cargo_fe2o3_identity(self) -> [u8; 32] {
        self.observed_cargo_fe2o3_identity
    }
}

/// Broker-to-wrapper Capabilities V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilitiesV3 {
    process: ProcessIdentityV3,
    binding_identity: [u8; 32],
    bootstrap_identity: [u8; 32],
    capability_identity: [u8; 32],
    descriptor_manifest: BrokerDescriptorManifestV3,
}

impl CapabilitiesV3 {
    /// Constructs a typed Capabilities frame.
    pub fn new(
        process: ProcessIdentityV3,
        binding_identity: [u8; 32],
        bootstrap_identity: [u8; 32],
        capability_identity: [u8; 32],
        descriptor_manifest: BrokerDescriptorManifestV3,
    ) -> Result<Self, BrokerProtocolErrorV3> {
        validate_identity(binding_identity, BrokerIdentityFieldV3::CapabilityBinding)?;
        validate_identity(bootstrap_identity, BrokerIdentityFieldV3::BootstrapTransfer)?;
        validate_identity(
            capability_identity,
            BrokerIdentityFieldV3::CapabilityTransfer,
        )?;
        Ok(Self {
            process,
            binding_identity,
            bootstrap_identity,
            capability_identity,
            descriptor_manifest,
        })
    }

    /// Returns the stable process identity.
    pub const fn process(self) -> ProcessIdentityV3 {
        self.process
    }

    /// Returns the canonical capability-binding identity.
    pub const fn binding_identity(self) -> [u8; 32] {
        self.binding_identity
    }

    /// Returns the one-shot bootstrap transfer identity.
    pub const fn bootstrap_identity(self) -> [u8; 32] {
        self.bootstrap_identity
    }

    /// Returns the one-shot compiler-capability transfer identity.
    pub const fn capability_identity(self) -> [u8; 32] {
        self.capability_identity
    }

    /// Returns the declared descriptor manifest.
    pub const fn descriptor_manifest(self) -> BrokerDescriptorManifestV3 {
        self.descriptor_manifest
    }
}

/// Dynamic-wrapper Prepare V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PrepareV3 {
    process: ProcessIdentityV3,
    binding_identity: [u8; 32],
    capability_identity: [u8; 32],
    compiler_closure_identity: [u8; 32],
    runtime_object_identity: [u8; 32],
    codegen_backend_identity: [u8; 32],
}

/// Rust compiler Consume V3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConsumeV3 {
    process: ProcessIdentityV3,
    binding_identity: [u8; 32],
    capability_identity: [u8; 32],
    compiler_closure_identity: [u8; 32],
    runtime_object_identity: [u8; 32],
    codegen_backend_identity: [u8; 32],
}

macro_rules! impl_compiler_observation {
    ($type:ident) => {
        impl $type {
            /// Constructs a typed compiler-chain observation.
            pub fn new(
                process: ProcessIdentityV3,
                binding_identity: [u8; 32],
                capability_identity: [u8; 32],
                compiler_closure_identity: [u8; 32],
                runtime_object_identity: [u8; 32],
                codegen_backend_identity: [u8; 32],
            ) -> Result<Self, BrokerProtocolErrorV3> {
                for (field, identity) in [
                    (BrokerIdentityFieldV3::CapabilityBinding, binding_identity),
                    (
                        BrokerIdentityFieldV3::CapabilityTransfer,
                        capability_identity,
                    ),
                    (
                        BrokerIdentityFieldV3::CompilerClosure,
                        compiler_closure_identity,
                    ),
                    (
                        BrokerIdentityFieldV3::RuntimeObject,
                        runtime_object_identity,
                    ),
                    (
                        BrokerIdentityFieldV3::CodegenBackend,
                        codegen_backend_identity,
                    ),
                ] {
                    validate_identity(identity, field)?;
                }
                Ok(Self {
                    process,
                    binding_identity,
                    capability_identity,
                    compiler_closure_identity,
                    runtime_object_identity,
                    codegen_backend_identity,
                })
            }

            /// Returns the stable process identity.
            pub const fn process(self) -> ProcessIdentityV3 {
                self.process
            }

            /// Returns the canonical capability-binding identity.
            pub const fn binding_identity(self) -> [u8; 32] {
                self.binding_identity
            }

            /// Returns the one-shot compiler-capability transfer identity.
            pub const fn capability_identity(self) -> [u8; 32] {
                self.capability_identity
            }

            /// Returns the observed compiler-closure identity.
            pub const fn compiler_closure_identity(self) -> [u8; 32] {
                self.compiler_closure_identity
            }

            /// Returns the observed runtime-object identity.
            pub const fn runtime_object_identity(self) -> [u8; 32] {
                self.runtime_object_identity
            }

            /// Returns the observed codegen-backend identity.
            pub const fn codegen_backend_identity(self) -> [u8; 32] {
                self.codegen_backend_identity
            }
        }
    };
}

impl_compiler_observation!(PrepareV3);
impl_compiler_observation!(ConsumeV3);

/// One typed Broker V3 frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerFrameV3 {
    /// Static-trampoline greeting.
    Hello(HelloV3),
    /// Dynamic-wrapper bootstrap.
    Bootstrap(BootstrapV3),
    /// Dynamic-wrapper post-exec greeting.
    PostExec(PostExecV3),
    /// Compiler capability transfer.
    Capabilities(CapabilitiesV3),
    /// Wrapper preparation acknowledgement.
    Prepare(PrepareV3),
    /// Rust compiler one-shot consumption.
    Consume(ConsumeV3),
}

/// A phase in the inert Broker V3 transcript validator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BrokerPhaseV3 {
    /// No trampoline greeting has been accepted.
    AwaitHello,
    /// A matching trampoline greeting was accepted.
    AwaitBootstrap,
    /// The exact wrapper bootstrap was accepted.
    AwaitPostExec,
    /// The matching post-exec wrapper greeting was accepted.
    AwaitCapabilities,
    /// The exact compiler-capability transfer was accepted.
    AwaitPrepare,
    /// The matching wrapper preparation was accepted.
    AwaitConsume,
    /// The matching rustc consumption completed the inert transcript.
    Complete,
}

/// A field that did not remain continuous across a Broker V3 transcript.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BrokerTranscriptFieldV3 {
    /// Complete canonical CapabilityBinding V3 value.
    CapabilityBinding,
    /// Stable PID and process start time.
    ProcessIdentity,
    /// Static trampoline executable identity.
    TrampolineExecutableIdentity,
    /// Canonical CapabilityBinding V3 identity.
    CapabilityBindingIdentity,
    /// One-shot bootstrap transfer identity.
    BootstrapIdentity,
    /// Dynamic `cargo-fe2o3` executable identity.
    CargoFe2o3ExecutableIdentity,
    /// Bootstrap descriptor manifest.
    BootstrapDescriptorManifest,
    /// One-shot compiler-capability transfer identity.
    CapabilityIdentity,
    /// Compiler-capabilities descriptor manifest.
    CapabilitiesDescriptorManifest,
    /// Canonical compiler-closure identity.
    CompilerClosureIdentity,
    /// Retained runtime-object identity.
    RuntimeObjectIdentity,
    /// Codegen-backend image identity.
    CodegenBackendIdentity,
}

impl fmt::Display for BrokerTranscriptFieldV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::CapabilityBinding => "capability binding",
            Self::ProcessIdentity => "process identity",
            Self::TrampolineExecutableIdentity => "trampoline executable identity",
            Self::CapabilityBindingIdentity => "capability-binding identity",
            Self::BootstrapIdentity => "bootstrap identity",
            Self::CargoFe2o3ExecutableIdentity => "cargo-fe2o3 executable identity",
            Self::BootstrapDescriptorManifest => "bootstrap descriptor manifest",
            Self::CapabilityIdentity => "capability identity",
            Self::CapabilitiesDescriptorManifest => "capabilities descriptor manifest",
            Self::CompilerClosureIdentity => "compiler-closure identity",
            Self::RuntimeObjectIdentity => "runtime-object identity",
            Self::CodegenBackendIdentity => "codegen-backend identity",
        };
        formatter.write_str(name)
    }
}

/// Why the pure Broker V3 state machine rejected a typed frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BrokerStateErrorV3 {
    /// The frame type was not valid in the current phase.
    UnexpectedFrame {
        /// Phase before the rejected transition.
        phase: BrokerPhaseV3,
        /// Rejected frame type.
        actual: BrokerFrameKindV3,
    },
    /// A frame was supplied after the transcript completed.
    TerminalState,
    /// A field did not match the expected binding or preceding frame.
    TranscriptMismatch {
        /// Field whose continuity check failed.
        field: BrokerTranscriptFieldV3,
    },
}

impl fmt::Display for BrokerStateErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedFrame { phase, actual } => {
                write!(formatter, "unexpected {actual:?} frame in {phase:?}")
            }
            Self::TerminalState => formatter.write_str("Broker V3 transcript is already complete"),
            Self::TranscriptMismatch { field } => {
                write!(formatter, "Broker V3 transcript {field} mismatch")
            }
        }
    }
}

impl std::error::Error for BrokerStateErrorV3 {}

/// Pure validation state for one two-stage protected rustc transcript.
///
/// This type checks canonical order and continuity only. It performs no socket,
/// descriptor, peer, PID, executable, or freshness observation. Completion is
/// inert and grants no publication authority. Broker V2 is intentionally not
/// modeled or changed by this crate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BrokerStateV3 {
    expected_binding: CapabilityBindingV3,
    phase: BrokerPhaseV3,
    process: Option<ProcessIdentityV3>,
    bootstrap_identity: Option<[u8; 32]>,
    capability_identity: Option<[u8; 32]>,
}

impl BrokerStateV3 {
    /// Creates an inert state machine bound to one zero-rights binding.
    pub const fn new(expected_binding: CapabilityBindingV3) -> Self {
        Self {
            expected_binding,
            phase: BrokerPhaseV3::AwaitHello,
            process: None,
            bootstrap_identity: None,
            capability_identity: None,
        }
    }

    /// Returns the current transcript phase.
    pub const fn phase(self) -> BrokerPhaseV3 {
        self.phase
    }

    /// Returns the retained process identity after Hello is accepted.
    pub const fn process(self) -> Option<ProcessIdentityV3> {
        self.process
    }

    /// Returns the completed binding identity only after Consume succeeds.
    pub fn completed_binding_identity(self) -> Option<[u8; 32]> {
        matches!(self.phase, BrokerPhaseV3::Complete)
            .then(|| self.expected_binding.identity_sha256())
    }

    /// Validates and applies one frame without changing state on failure.
    pub fn advance(&mut self, frame: BrokerFrameV3) -> Result<(), BrokerStateErrorV3> {
        match self.phase {
            BrokerPhaseV3::AwaitHello => {
                let BrokerFrameV3::Hello(value) = frame else {
                    return Err(self.unexpected(frame));
                };
                self.validate_hello(value)?;
                self.process = Some(value.process());
                self.phase = BrokerPhaseV3::AwaitBootstrap;
            }
            BrokerPhaseV3::AwaitBootstrap => {
                let BrokerFrameV3::Bootstrap(value) = frame else {
                    return Err(self.unexpected(frame));
                };
                self.validate_bootstrap(value)?;
                self.bootstrap_identity = Some(value.bootstrap_identity());
                self.phase = BrokerPhaseV3::AwaitPostExec;
            }
            BrokerPhaseV3::AwaitPostExec => {
                let BrokerFrameV3::PostExec(value) = frame else {
                    return Err(self.unexpected(frame));
                };
                self.validate_post_exec(value)?;
                self.phase = BrokerPhaseV3::AwaitCapabilities;
            }
            BrokerPhaseV3::AwaitCapabilities => {
                let BrokerFrameV3::Capabilities(value) = frame else {
                    return Err(self.unexpected(frame));
                };
                self.validate_capabilities(value)?;
                self.capability_identity = Some(value.capability_identity());
                self.phase = BrokerPhaseV3::AwaitPrepare;
            }
            BrokerPhaseV3::AwaitPrepare => {
                let BrokerFrameV3::Prepare(value) = frame else {
                    return Err(self.unexpected(frame));
                };
                self.validate_compiler_observation(value)?;
                self.phase = BrokerPhaseV3::AwaitConsume;
            }
            BrokerPhaseV3::AwaitConsume => {
                let BrokerFrameV3::Consume(value) = frame else {
                    return Err(self.unexpected(frame));
                };
                self.validate_compiler_observation(value)?;
                self.phase = BrokerPhaseV3::Complete;
            }
            BrokerPhaseV3::Complete => return Err(BrokerStateErrorV3::TerminalState),
        }
        Ok(())
    }

    fn unexpected(self, frame: BrokerFrameV3) -> BrokerStateErrorV3 {
        BrokerStateErrorV3::UnexpectedFrame {
            phase: self.phase,
            actual: frame.kind(),
        }
    }

    fn validate_hello(self, value: HelloV3) -> Result<(), BrokerStateErrorV3> {
        ensure_broker_transcript(
            value.binding() == self.expected_binding,
            BrokerTranscriptFieldV3::CapabilityBinding,
        )?;
        ensure_broker_transcript(
            value.observed_trampoline_identity()
                == self.expected_binding.trampoline_executable_identity(),
            BrokerTranscriptFieldV3::TrampolineExecutableIdentity,
        )
    }

    fn validate_bootstrap(self, value: BootstrapV3) -> Result<(), BrokerStateErrorV3> {
        self.validate_process_and_binding(value.process(), value.binding_identity())?;
        ensure_broker_transcript(
            value.descriptor_manifest() == BrokerDescriptorManifestV3::Bootstrap,
            BrokerTranscriptFieldV3::BootstrapDescriptorManifest,
        )
    }

    fn validate_post_exec(self, value: PostExecV3) -> Result<(), BrokerStateErrorV3> {
        self.validate_process_and_binding(value.process(), value.binding_identity())?;
        ensure_broker_transcript(
            Some(value.bootstrap_identity()) == self.bootstrap_identity,
            BrokerTranscriptFieldV3::BootstrapIdentity,
        )?;
        ensure_broker_transcript(
            value.observed_cargo_fe2o3_identity()
                == self.expected_binding.cargo_fe2o3_executable_identity(),
            BrokerTranscriptFieldV3::CargoFe2o3ExecutableIdentity,
        )
    }

    fn validate_capabilities(self, value: CapabilitiesV3) -> Result<(), BrokerStateErrorV3> {
        self.validate_process_and_binding(value.process(), value.binding_identity())?;
        ensure_broker_transcript(
            Some(value.bootstrap_identity()) == self.bootstrap_identity,
            BrokerTranscriptFieldV3::BootstrapIdentity,
        )?;
        ensure_broker_transcript(
            value.descriptor_manifest() == BrokerDescriptorManifestV3::CompilerCapabilities,
            BrokerTranscriptFieldV3::CapabilitiesDescriptorManifest,
        )
    }

    fn validate_compiler_observation<T: CompilerObservationV3>(
        self,
        value: T,
    ) -> Result<(), BrokerStateErrorV3> {
        self.validate_process_and_binding(value.process(), value.binding_identity())?;
        ensure_broker_transcript(
            Some(value.capability_identity()) == self.capability_identity,
            BrokerTranscriptFieldV3::CapabilityIdentity,
        )?;
        ensure_broker_transcript(
            value.compiler_closure_identity() == self.expected_binding.compiler_closure_identity(),
            BrokerTranscriptFieldV3::CompilerClosureIdentity,
        )?;
        ensure_broker_transcript(
            value.runtime_object_identity() == self.expected_binding.runtime_object_identity(),
            BrokerTranscriptFieldV3::RuntimeObjectIdentity,
        )?;
        ensure_broker_transcript(
            value.codegen_backend_identity() == self.expected_binding.codegen_backend_identity(),
            BrokerTranscriptFieldV3::CodegenBackendIdentity,
        )
    }

    fn validate_process_and_binding(
        self,
        process: ProcessIdentityV3,
        binding_identity: [u8; 32],
    ) -> Result<(), BrokerStateErrorV3> {
        ensure_broker_transcript(
            Some(process) == self.process,
            BrokerTranscriptFieldV3::ProcessIdentity,
        )?;
        ensure_broker_transcript(
            binding_identity == self.expected_binding.identity_sha256(),
            BrokerTranscriptFieldV3::CapabilityBindingIdentity,
        )
    }
}

fn ensure_broker_transcript(
    condition: bool,
    field: BrokerTranscriptFieldV3,
) -> Result<(), BrokerStateErrorV3> {
    if condition {
        Ok(())
    } else {
        Err(BrokerStateErrorV3::TranscriptMismatch { field })
    }
}

impl BrokerFrameV3 {
    /// Returns the assigned message type.
    pub const fn kind(self) -> BrokerFrameKindV3 {
        match self {
            Self::Hello(_) => BrokerFrameKindV3::Hello,
            Self::Bootstrap(_) => BrokerFrameKindV3::Bootstrap,
            Self::PostExec(_) => BrokerFrameKindV3::PostExec,
            Self::Capabilities(_) => BrokerFrameKindV3::Capabilities,
            Self::Prepare(_) => BrokerFrameKindV3::Prepare,
            Self::Consume(_) => BrokerFrameKindV3::Consume,
        }
    }

    /// Returns the exact encoded frame length.
    pub const fn encoded_len(self) -> usize {
        BROKER_V3_HEADER_LEN + self.kind().payload_len()
    }

    /// Encodes this frame canonically.
    pub fn encode(self) -> Vec<u8> {
        encode_broker_frame_v3(&self)
    }
}

/// Encodes one typed Broker V3 frame with its exact 24-byte header.
pub fn encode_broker_frame_v3(frame: &BrokerFrameV3) -> Vec<u8> {
    let kind = frame.kind();
    let mut encoded = vec![0_u8; BROKER_V3_HEADER_LEN + kind.payload_len()];
    encoded[0..8].copy_from_slice(&BROKER_V3_MAGIC);
    encoded[8..10].copy_from_slice(&BROKER_V3_VERSION.to_le_bytes());
    encoded[10..12].copy_from_slice(&(kind as u16).to_le_bytes());
    encoded[12..16].copy_from_slice(&(kind.payload_len() as u32).to_le_bytes());
    encoded[16..20].copy_from_slice(&kind.sequence().to_le_bytes());
    let payload = &mut encoded[BROKER_V3_HEADER_LEN..];
    match frame {
        BrokerFrameV3::Hello(value) => encode_hello(*value, payload),
        BrokerFrameV3::Bootstrap(value) => encode_bootstrap(*value, payload),
        BrokerFrameV3::PostExec(value) => encode_post_exec(*value, payload),
        BrokerFrameV3::Capabilities(value) => encode_capabilities(*value, payload),
        BrokerFrameV3::Prepare(value) => encode_compiler_observation(*value, payload),
        BrokerFrameV3::Consume(value) => encode_compiler_observation(*value, payload),
    }
    encoded
}

/// Decodes one exact canonical Broker V3 frame.
pub fn decode_broker_frame_v3(encoded: &[u8]) -> Result<BrokerFrameV3, BrokerProtocolErrorV3> {
    if encoded.len() < BROKER_V3_HEADER_LEN {
        return Err(BrokerProtocolErrorV3::TruncatedHeader {
            actual: encoded.len(),
        });
    }
    if encoded[0..8] != BROKER_V3_MAGIC {
        return Err(BrokerProtocolErrorV3::InvalidMagic);
    }
    let version = read_u16(encoded, 8);
    if version != BROKER_V3_VERSION {
        return Err(BrokerProtocolErrorV3::UnsupportedVersion { actual: version });
    }
    let kind = BrokerFrameKindV3::from_wire(read_u16(encoded, 10))?;
    let payload_len = read_u32(encoded, 12);
    if payload_len != kind.payload_len() as u32 {
        return Err(BrokerProtocolErrorV3::InvalidPayloadLength {
            kind,
            expected: kind.payload_len(),
            actual: payload_len,
        });
    }
    let sequence = read_u32(encoded, 16);
    if sequence != kind.sequence() {
        return Err(BrokerProtocolErrorV3::InvalidSequence {
            kind,
            expected: kind.sequence(),
            actual: sequence,
        });
    }
    let flags = read_u32(encoded, 20);
    if flags != 0 {
        return Err(BrokerProtocolErrorV3::UnsupportedFlags { actual: flags });
    }
    let expected_len = BROKER_V3_HEADER_LEN + kind.payload_len();
    if encoded.len() != expected_len {
        return Err(BrokerProtocolErrorV3::InvalidEncodedLength {
            expected: expected_len,
            actual: encoded.len(),
        });
    }
    let payload = &encoded[BROKER_V3_HEADER_LEN..];
    match kind {
        BrokerFrameKindV3::Hello => decode_hello(payload).map(BrokerFrameV3::Hello),
        BrokerFrameKindV3::Bootstrap => decode_bootstrap(payload).map(BrokerFrameV3::Bootstrap),
        BrokerFrameKindV3::PostExec => decode_post_exec(payload).map(BrokerFrameV3::PostExec),
        BrokerFrameKindV3::Capabilities => {
            decode_capabilities(payload).map(BrokerFrameV3::Capabilities)
        }
        BrokerFrameKindV3::Prepare => decode_prepare(payload).map(BrokerFrameV3::Prepare),
        BrokerFrameKindV3::Consume => decode_consume(payload).map(BrokerFrameV3::Consume),
    }
}

fn encode_hello(value: HelloV3, payload: &mut [u8]) {
    value.process.encode(&mut payload[0..16]);
    payload[16..352].copy_from_slice(&value.binding.encode());
    payload[352..384].copy_from_slice(&value.observed_trampoline_identity);
}

fn decode_hello(payload: &[u8]) -> Result<HelloV3, BrokerProtocolErrorV3> {
    HelloV3::new(
        ProcessIdentityV3::decode(&payload[0..16])?,
        decode_capability_binding_v3(&payload[16..352])?,
        digest_at(payload, 352),
    )
}

fn encode_bootstrap(value: BootstrapV3, payload: &mut [u8]) {
    value.process.encode(&mut payload[0..16]);
    payload[16..48].copy_from_slice(&value.binding_identity);
    payload[48..80].copy_from_slice(&value.bootstrap_identity);
    payload[80..96].copy_from_slice(&value.descriptor_manifest.encode());
}

fn decode_bootstrap(payload: &[u8]) -> Result<BootstrapV3, BrokerProtocolErrorV3> {
    BootstrapV3::new(
        ProcessIdentityV3::decode(&payload[0..16])?,
        digest_at(payload, 16),
        digest_at(payload, 48),
        BrokerDescriptorManifestV3::decode(&payload[80..96])?,
    )
}

fn encode_post_exec(value: PostExecV3, payload: &mut [u8]) {
    value.process.encode(&mut payload[0..16]);
    payload[16..48].copy_from_slice(&value.binding_identity);
    payload[48..80].copy_from_slice(&value.bootstrap_identity);
    payload[80..112].copy_from_slice(&value.observed_cargo_fe2o3_identity);
}

fn decode_post_exec(payload: &[u8]) -> Result<PostExecV3, BrokerProtocolErrorV3> {
    PostExecV3::new(
        ProcessIdentityV3::decode(&payload[0..16])?,
        digest_at(payload, 16),
        digest_at(payload, 48),
        digest_at(payload, 80),
    )
}

fn encode_capabilities(value: CapabilitiesV3, payload: &mut [u8]) {
    value.process.encode(&mut payload[0..16]);
    payload[16..48].copy_from_slice(&value.binding_identity);
    payload[48..80].copy_from_slice(&value.bootstrap_identity);
    payload[80..112].copy_from_slice(&value.capability_identity);
    payload[112..128].copy_from_slice(&value.descriptor_manifest.encode());
}

fn decode_capabilities(payload: &[u8]) -> Result<CapabilitiesV3, BrokerProtocolErrorV3> {
    CapabilitiesV3::new(
        ProcessIdentityV3::decode(&payload[0..16])?,
        digest_at(payload, 16),
        digest_at(payload, 48),
        digest_at(payload, 80),
        BrokerDescriptorManifestV3::decode(&payload[112..128])?,
    )
}

trait CompilerObservationV3: Copy {
    fn process(self) -> ProcessIdentityV3;
    fn binding_identity(self) -> [u8; 32];
    fn capability_identity(self) -> [u8; 32];
    fn compiler_closure_identity(self) -> [u8; 32];
    fn runtime_object_identity(self) -> [u8; 32];
    fn codegen_backend_identity(self) -> [u8; 32];
}

macro_rules! impl_observation_trait {
    ($type:ident) => {
        impl CompilerObservationV3 for $type {
            fn process(self) -> ProcessIdentityV3 {
                self.process
            }
            fn binding_identity(self) -> [u8; 32] {
                self.binding_identity
            }
            fn capability_identity(self) -> [u8; 32] {
                self.capability_identity
            }
            fn compiler_closure_identity(self) -> [u8; 32] {
                self.compiler_closure_identity
            }
            fn runtime_object_identity(self) -> [u8; 32] {
                self.runtime_object_identity
            }
            fn codegen_backend_identity(self) -> [u8; 32] {
                self.codegen_backend_identity
            }
        }
    };
}

impl_observation_trait!(PrepareV3);
impl_observation_trait!(ConsumeV3);

fn encode_compiler_observation<T: CompilerObservationV3>(value: T, payload: &mut [u8]) {
    value.process().encode(&mut payload[0..16]);
    payload[16..48].copy_from_slice(&value.binding_identity());
    payload[48..80].copy_from_slice(&value.capability_identity());
    payload[80..112].copy_from_slice(&value.compiler_closure_identity());
    payload[112..144].copy_from_slice(&value.runtime_object_identity());
    payload[144..176].copy_from_slice(&value.codegen_backend_identity());
}

fn decode_prepare(payload: &[u8]) -> Result<PrepareV3, BrokerProtocolErrorV3> {
    PrepareV3::new(
        ProcessIdentityV3::decode(&payload[0..16])?,
        digest_at(payload, 16),
        digest_at(payload, 48),
        digest_at(payload, 80),
        digest_at(payload, 112),
        digest_at(payload, 144),
    )
}

fn decode_consume(payload: &[u8]) -> Result<ConsumeV3, BrokerProtocolErrorV3> {
    ConsumeV3::new(
        ProcessIdentityV3::decode(&payload[0..16])?,
        digest_at(payload, 16),
        digest_at(payload, 48),
        digest_at(payload, 80),
        digest_at(payload, 112),
        digest_at(payload, 144),
    )
}

/// Why a Broker V3 value or canonical frame was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BrokerProtocolErrorV3 {
    /// A frame ended before its complete header.
    TruncatedHeader {
        /// Observed byte length.
        actual: usize,
    },
    /// The frame magic was not Broker V3.
    InvalidMagic,
    /// The frame version was not Broker V3.
    UnsupportedVersion {
        /// Observed version.
        actual: u16,
    },
    /// The frame type was not assigned.
    UnknownFrameType {
        /// Observed type number.
        actual: u16,
    },
    /// The declared payload length was not canonical for the frame type.
    InvalidPayloadLength {
        /// Parsed frame type.
        kind: BrokerFrameKindV3,
        /// Required payload length.
        expected: usize,
        /// Declared payload length.
        actual: u32,
    },
    /// The sequence was not canonical for the frame type.
    InvalidSequence {
        /// Parsed frame type.
        kind: BrokerFrameKindV3,
        /// Required sequence.
        expected: u32,
        /// Observed sequence.
        actual: u32,
    },
    /// Header flags were nonzero.
    UnsupportedFlags {
        /// Observed flags.
        actual: u32,
    },
    /// The frame had missing or trailing bytes.
    InvalidEncodedLength {
        /// Required complete frame length.
        expected: usize,
        /// Observed complete frame length.
        actual: usize,
    },
    /// A standalone binding had missing or trailing bytes.
    InvalidBindingLength {
        /// Observed binding length.
        actual: usize,
    },
    /// A required identity was all zero.
    ZeroIdentity {
        /// Zero-valued field.
        field: BrokerIdentityFieldV3,
    },
    /// A process identifier was zero.
    ZeroProcessId,
    /// A process start time was zero.
    ZeroProcessStartTime,
    /// Process-identity reserved bytes were nonzero.
    NonzeroProcessReserved,
    /// The target was not assigned to Broker V3.
    UnknownTarget {
        /// Observed target number.
        actual: u16,
    },
    /// The pipeline was not assigned to Policy V1.
    UnknownPipeline {
        /// Observed pipeline number.
        actual: u16,
    },
    /// Any nonzero publication right is forbidden in Broker V3.
    PublicationAuthorityForbidden {
        /// Observed rights bits.
        actual: u32,
    },
    /// Optional Worker V2 presence was neither zero nor one.
    InvalidWorkerPresence {
        /// Observed presence value.
        actual: u8,
    },
    /// An absent Worker V2 field carried nonzero identity bytes.
    WorkerIdentityWithoutPresence,
    /// Capability-binding reserved bytes were nonzero.
    NonzeroBindingReserved,
    /// The manifest type was not assigned.
    UnknownManifestType {
        /// Observed manifest number.
        actual: u16,
    },
    /// A descriptor kind was not assigned.
    UnknownDescriptorKind {
        /// Observed descriptor number.
        actual: u16,
    },
    /// A manifest declared the wrong descriptor count.
    InvalidDescriptorCount {
        /// Parsed manifest.
        manifest: BrokerDescriptorManifestV3,
        /// Required descriptor count.
        expected: u16,
        /// Observed descriptor count.
        actual: u16,
    },
    /// A manifest descriptor kind or order was wrong.
    InvalidDescriptorKind {
        /// Parsed manifest.
        manifest: BrokerDescriptorManifestV3,
        /// Zero-based descriptor index.
        index: usize,
        /// Required descriptor kind.
        expected: BrokerDescriptorKindV3,
        /// Observed descriptor kind.
        actual: BrokerDescriptorKindV3,
    },
    /// An unused descriptor slot was nonzero.
    NonzeroUnusedDescriptorSlot {
        /// Zero-based descriptor index.
        index: usize,
    },
    /// Manifest reserved bytes were nonzero.
    NonzeroManifestReserved,
}

impl fmt::Display for BrokerProtocolErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedHeader { actual } => {
                write!(formatter, "truncated Broker V3 header ({actual} bytes)")
            }
            Self::InvalidMagic => formatter.write_str("invalid Broker V3 magic"),
            Self::UnsupportedVersion { actual } => {
                write!(formatter, "unsupported Broker V3 version {actual}")
            }
            Self::UnknownFrameType { actual } => {
                write!(formatter, "unknown Broker V3 frame type {actual}")
            }
            Self::InvalidPayloadLength {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "invalid {kind:?} payload length {actual}; expected {expected}"
            ),
            Self::InvalidSequence {
                kind,
                expected,
                actual,
            } => write!(
                formatter,
                "invalid {kind:?} sequence {actual}; expected {expected}"
            ),
            Self::UnsupportedFlags { actual } => {
                write!(formatter, "unsupported Broker V3 flags {actual:#x}")
            }
            Self::InvalidEncodedLength { expected, actual } => write!(
                formatter,
                "invalid Broker V3 frame length {actual}; expected {expected}"
            ),
            Self::InvalidBindingLength { actual } => write!(
                formatter,
                "invalid CapabilityBinding V3 length {actual}; expected {BROKER_V3_BINDING_WIRE_LEN}"
            ),
            Self::ZeroIdentity { field } => write!(formatter, "zero {field} identity"),
            Self::ZeroProcessId => formatter.write_str("zero Broker V3 process identifier"),
            Self::ZeroProcessStartTime => formatter.write_str("zero Broker V3 process start time"),
            Self::NonzeroProcessReserved => {
                formatter.write_str("nonzero Broker V3 process reserved bytes")
            }
            Self::UnknownTarget { actual } => write!(formatter, "unknown target {actual}"),
            Self::UnknownPipeline { actual } => write!(formatter, "unknown pipeline {actual}"),
            Self::PublicationAuthorityForbidden { actual } => write!(
                formatter,
                "publication authority {actual:#x} is forbidden in Broker V3"
            ),
            Self::InvalidWorkerPresence { actual } => {
                write!(formatter, "invalid Worker V2 presence value {actual}")
            }
            Self::WorkerIdentityWithoutPresence => {
                formatter.write_str("Worker V2 identity present without presence marker")
            }
            Self::NonzeroBindingReserved => {
                formatter.write_str("nonzero CapabilityBinding V3 reserved bytes")
            }
            Self::UnknownManifestType { actual } => {
                write!(formatter, "unknown descriptor manifest {actual}")
            }
            Self::UnknownDescriptorKind { actual } => {
                write!(formatter, "unknown descriptor kind {actual}")
            }
            Self::InvalidDescriptorCount {
                manifest,
                expected,
                actual,
            } => write!(
                formatter,
                "invalid {manifest:?} descriptor count {actual}; expected {expected}"
            ),
            Self::InvalidDescriptorKind {
                manifest,
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "invalid {manifest:?} descriptor {index}: {actual:?}; expected {expected:?}"
            ),
            Self::NonzeroUnusedDescriptorSlot { index } => {
                write!(formatter, "nonzero unused descriptor slot {index}")
            }
            Self::NonzeroManifestReserved => {
                formatter.write_str("nonzero descriptor-manifest reserved bytes")
            }
        }
    }
}

impl std::error::Error for BrokerProtocolErrorV3 {}

fn validate_identity(
    value: [u8; 32],
    field: BrokerIdentityFieldV3,
) -> Result<(), BrokerProtocolErrorV3> {
    if value == [0; 32] {
        Err(BrokerProtocolErrorV3::ZeroIdentity { field })
    } else {
        Ok(())
    }
}

fn write_bytes(output: &mut [u8], cursor: &mut usize, value: &[u8]) {
    let end = *cursor + value.len();
    output[*cursor..end].copy_from_slice(value);
    *cursor = end;
}

fn digest_at(input: &[u8], offset: usize) -> [u8; 32] {
    input[offset..offset + 32]
        .try_into()
        .expect("validated Broker V3 wire bounds")
}

fn read_u16(input: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(
        input[offset..offset + 2]
            .try_into()
            .expect("validated Broker V3 wire bounds"),
    )
}

fn read_u32(input: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(
        input[offset..offset + 4]
            .try_into()
            .expect("validated Broker V3 wire bounds"),
    )
}

fn read_u64(input: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        input[offset..offset + 8]
            .try_into()
            .expect("validated Broker V3 wire bounds"),
    )
}
