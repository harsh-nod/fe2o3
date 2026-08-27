#![no_std]
#![forbid(unsafe_code)]
#![doc = "Fail-closed admission for the isolated upstream LLVM worker boundary."]
//!
//! This crate validates the canonical handoff and the worker's measured
//! LLVM/LLD build identity before any LLVM value may be observed. Successful
//! admission produces inert data only. It does not parse LLVM IR, create a
//! target machine, emit an object, invoke LLD, or grant publication authority.

use core::fmt;

use alloc::string::ToString as _;
use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerModuleHandoffErrorV2, CompilerModuleHandoffIdentityV2,
    CompilerModuleHandoffV2, CompilerModuleKindV1, ProductionGfx950CompilerFfiEnvelopeKindV1,
    inspect_production_gfx950_compiler_ffi_envelope_v1,
};
use fe2o3_llvm_handoff::{
    CodeModelV1, CodeObjectVersionV1, DecodeHandoffErrorV1, DecodeHandoffErrorV2,
    DeviceLibraryKindV1, GFX942_AMDHSA_DATA_LAYOUT_V1, GFX942_AMDHSA_TARGET_TRIPLE_V1,
    Gfx942HandoffV1, Gfx942HandoffV2, Gfx942TargetPolicyV1, HandoffIdentityV1, HandoffIdentityV2,
    OptimizationLevelV1, RelocationModelV1, TargetFeatureStateV1, TargetFeatureV1,
};
use sha2::{Digest as _, Sha256};

extern crate alloc;

/// Maximum canonical handoff bytes accepted by this worker boundary.
pub const MAX_WORKER_ADMISSION_REQUEST_BYTES_V1: usize =
    fe2o3_llvm_handoff::MAX_CANONICAL_HANDOFF_BYTES_V1;

/// Maximum canonical V2 handoff bytes accepted by this worker boundary.
pub const MAX_WORKER_ADMISSION_REQUEST_BYTES_V2: usize =
    fe2o3_llvm_handoff::MAX_CANONICAL_HANDOFF_BYTES_V2;

/// Maximum canonical production compiler-module bytes accepted by V3 admission.
pub const MAX_WORKER_ADMISSION_REQUEST_BYTES_V3: usize =
    fe2o3_compiler_ffi::MAX_COMPILER_MODULE_HANDOFF_BYTES_V2;

/// Maximum bytes in an observed LLVM or LLD version.
pub const MAX_WORKER_BUILD_VERSION_BYTES_V1: usize = 16;

/// Maximum bytes in an observed LLVM or LLD build identity.
pub const MAX_WORKER_BUILD_IDENTITY_BYTES_V1: usize = 160;

/// Maximum bytes in one device-library input admitted by the measured worker.
pub const MAX_WORKER_DEVICE_LIBRARY_BYTES_V1: u64 = 16 * 1024 * 1024;

/// Exact upstream LLVM version admitted by this boundary.
pub const EXACT_LLVM_VERSION_V1: &str = "22.1.8";

/// Exact measured upstream LLVM build identity admitted by this boundary.
pub const EXACT_LLVM_BUILD_IDENTITY_V1: &str =
    "upstream-llvmorg-22.1.8-ca7933e47d3a3451d81e72ac174dcb5aa28b59d1";

/// Exact LLD version admitted by this boundary.
pub const EXACT_LLD_VERSION_V1: &str = "22.1.8";

/// Exact measured LLD build identity admitted by this boundary.
///
/// LLVM and LLD must come from the same measured upstream package tree, so the
/// two build identities are intentionally identical.
pub const EXACT_LLD_BUILD_IDENTITY_V1: &str = EXACT_LLVM_BUILD_IDENTITY_V1;

/// Device-library kinds supported by the measured gfx942 worker closure.
///
/// A request either carries no device libraries or carries this complete set.
pub const SUPPORTED_DEVICE_LIBRARY_CLOSURE_V1: &[DeviceLibraryKindV1] = &[
    DeviceLibraryKindV1::Ocml,
    DeviceLibraryKindV1::OclcIsaVersion942,
    DeviceLibraryKindV1::OclcFiniteOnlyOff,
    DeviceLibraryKindV1::OclcUnsafeMathOff,
];

const ADMISSION_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.llvm-worker.admission.identity.v1";
const ADMISSION_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.llvm-worker.admission.identity.v2";
const ADMISSION_IDENTITY_DOMAIN_V3: &[u8] = b"fe2o3.llvm-worker.admission.identity.v3";

const EXACT_TARGET_FEATURES_V1: [TargetFeatureStateV1; 3] = [
    TargetFeatureStateV1::new(TargetFeatureV1::WavefrontSize32, false),
    TargetFeatureStateV1::new(TargetFeatureV1::WavefrontSize64, true),
    TargetFeatureStateV1::new(TargetFeatureV1::Xnack, false),
];

/// One bounded field in the measured LLVM/LLD build observation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkerBuildFieldV1 {
    /// Upstream LLVM semantic version.
    LlvmVersion,
    /// Upstream LLVM measured build identity.
    LlvmBuildIdentity,
    /// In-process LLD semantic version.
    LldVersion,
    /// In-process LLD measured build identity.
    LldBuildIdentity,
    /// Requirement that LLD is linked and called in-process.
    InProcessLld,
}

/// One exact target-policy field checked after canonical decoding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkerTargetPolicyFieldV1 {
    /// AMDHSA target triple.
    TargetTriple,
    /// Exact AMDGPU data layout.
    DataLayout,
    /// Exact gfx942 processor.
    Cpu,
    /// Closed wavefront and XNACK feature states.
    Features,
    /// AMDHSA code-object version.
    CodeObjectVersion,
    /// LLVM optimization level.
    OptimizationLevel,
    /// LLVM relocation model.
    RelocationModel,
    /// LLVM code model.
    CodeModel,
}

/// Typed, bounded worker-boundary admission failure.
///
/// Diagnostics never retain or echo attacker-controlled request text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerAdmissionErrorV1 {
    /// The request was rejected before canonical decoding.
    RequestTooLong {
        /// Received byte count.
        observed: usize,
        /// Maximum admitted byte count.
        maximum: usize,
    },
    /// One measured build field exceeded its text bound.
    BuildFieldTooLong {
        /// Field that exceeded its bound.
        field: WorkerBuildFieldV1,
        /// Received byte count.
        observed: usize,
        /// Maximum admitted byte count.
        maximum: usize,
    },
    /// One measured build field was not bounded printable ASCII.
    InvalidBuildField(WorkerBuildFieldV1),
    /// The measured LLVM/LLD closure differs from the exact admitted build.
    BuildIdentitySubstitution(WorkerBuildFieldV1),
    /// The claimed handoff identity used the reserved all-zero value.
    ZeroHandoffIdentity,
    /// Canonical handoff decoding failed.
    Decode(DecodeHandoffErrorV1),
    /// The recomputed canonical handoff identity differs from the claim.
    HandoffIdentityMismatch,
    /// A decoded target field differs from the exact gfx942 worker policy.
    TargetPolicySubstitution(WorkerTargetPolicyFieldV1),
    /// The current worker does not support this device-library kind.
    UnsupportedDeviceLibrary(DeviceLibraryKindV1),
    /// A supported device-library input exceeds the worker's tighter bound.
    DeviceLibraryTooLong {
        /// Device-library kind.
        kind: DeviceLibraryKindV1,
        /// Declared byte count.
        observed: u64,
        /// Maximum admitted byte count.
        maximum: u64,
    },
    /// A nonempty device-library list omitted part of the closed worker set.
    IncompleteDeviceLibraryClosure {
        /// Number of supported inputs observed.
        observed: usize,
        /// Number of inputs required by the measured closure.
        required: usize,
    },
}

impl fmt::Display for WorkerAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestTooLong { observed, maximum } => write!(
                formatter,
                "LLVM worker admission request has {observed} bytes, maximum is {maximum}"
            ),
            Self::BuildFieldTooLong {
                field,
                observed,
                maximum,
            } => write!(
                formatter,
                "measured {field} has {observed} bytes, maximum is {maximum}"
            ),
            Self::InvalidBuildField(field) => {
                write!(formatter, "measured {field} is not bounded printable ASCII")
            }
            Self::BuildIdentitySubstitution(field) => {
                write!(
                    formatter,
                    "measured {field} differs from the admitted build"
                )
            }
            Self::ZeroHandoffIdentity => {
                formatter.write_str("claimed LLVM handoff identity is zero")
            }
            Self::Decode(error) => write!(formatter, "LLVM handoff decode failed: {error}"),
            Self::HandoffIdentityMismatch => {
                formatter.write_str("recomputed LLVM handoff identity does not match the claim")
            }
            Self::TargetPolicySubstitution(field) => {
                write!(formatter, "decoded LLVM handoff substituted {field}")
            }
            Self::UnsupportedDeviceLibrary(kind) => write!(
                formatter,
                "device-library kind {} is not supported by this worker",
                kind.canonical_name()
            ),
            Self::DeviceLibraryTooLong {
                kind,
                observed,
                maximum,
            } => write!(
                formatter,
                "device-library kind {} declares {observed} bytes, maximum is {maximum}",
                kind.canonical_name()
            ),
            Self::IncompleteDeviceLibraryClosure { observed, required } => write!(
                formatter,
                "device-library closure has {observed} inputs, exactly {required} are required"
            ),
        }
    }
}

impl core::error::Error for WorkerAdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            _ => None,
        }
    }
}

/// Typed, bounded V2 worker-boundary admission failure.
///
/// Build-policy failures wrap the existing V1 policy error because both wire
/// versions are admitted against the same exact LLVM/LLD closure. This value
/// records policy admission only; it is not worker measurement or attestation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerAdmissionErrorV2 {
    /// The canonical V2 request exceeded its hard byte bound.
    RequestTooLong {
        /// Received byte count.
        observed: usize,
        /// Maximum admitted byte count.
        maximum: usize,
    },
    /// The claimed V2 identity used the reserved all-zero value.
    ZeroHandoffIdentity,
    /// Canonical V2 decoding failed.
    Decode(DecodeHandoffErrorV2),
    /// The recomputed V2 identity differs from the claim.
    HandoffIdentityMismatch,
    /// Exact build, target, or device-library policy admission failed.
    Policy(WorkerAdmissionErrorV1),
}

impl fmt::Display for WorkerAdmissionErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestTooLong { observed, maximum } => write!(
                formatter,
                "LLVM worker V2 admission request has {observed} bytes, maximum is {maximum}"
            ),
            Self::ZeroHandoffIdentity => {
                formatter.write_str("claimed LLVM V2 handoff identity is zero")
            }
            Self::Decode(error) => write!(formatter, "LLVM V2 handoff decode failed: {error}"),
            Self::HandoffIdentityMismatch => {
                formatter.write_str("recomputed LLVM V2 handoff identity does not match the claim")
            }
            Self::Policy(error) => write!(formatter, "LLVM V2 handoff policy rejected: {error}"),
        }
    }
}

impl core::error::Error for WorkerAdmissionErrorV2 {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            Self::Policy(error) => Some(error),
            _ => None,
        }
    }
}

/// Typed failure from append-only production compiler-module admission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerAdmissionErrorV3 {
    /// The canonical compiler-module request exceeded its hard byte bound.
    RequestTooLong {
        /// Received byte count.
        observed: usize,
        /// Maximum admitted byte count.
        maximum: usize,
    },
    /// The claimed compiler-module identity used the reserved all-zero value.
    ZeroHandoffIdentity,
    /// Canonical compiler-module decoding failed.
    Decode(CompilerModuleHandoffErrorV2),
    /// The recomputed compiler-module identity differs from the claim.
    HandoffIdentityMismatch,
    /// The handoff is not LLVM text, the only production input admitted here.
    ModuleKindSubstitution,
    /// The exact target is not an admitted production gfx942/gfx950 profile.
    TargetPolicySubstitution,
    /// The compiler-module handoff does not request code-object V6.
    CodeObjectVersionSubstitution,
    /// The gfx950 device-FFI envelope or retained LLVM import closure is not exact.
    Gfx950DeviceFfiPolicySubstitution,
    /// Exact LLVM/LLD build policy admission failed.
    BuildPolicy(WorkerAdmissionErrorV1),
}

impl fmt::Display for WorkerAdmissionErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestTooLong { observed, maximum } => write!(
                formatter,
                "LLVM worker V3 admission request has {observed} bytes, maximum is {maximum}"
            ),
            Self::ZeroHandoffIdentity => {
                formatter.write_str("claimed compiler-module handoff identity is zero")
            }
            Self::Decode(error) => write!(
                formatter,
                "production compiler-module handoff decode failed: {error}"
            ),
            Self::HandoffIdentityMismatch => formatter
                .write_str("recomputed compiler-module handoff identity does not match the claim"),
            Self::ModuleKindSubstitution => {
                formatter.write_str("production worker requires exact LLVM text input")
            }
            Self::TargetPolicySubstitution => formatter.write_str(
                "production worker requires exact gfx942:xnack- or gfx950:xnack- target",
            ),
            Self::CodeObjectVersionSubstitution => {
                formatter.write_str("production worker requires code-object version 6")
            }
            Self::Gfx950DeviceFfiPolicySubstitution => formatter.write_str(
                "production gfx950 worker admits either no device FFI or the exact compiler-owned __ocml_exp_f32 import",
            ),
            Self::BuildPolicy(error) => {
                write!(
                    formatter,
                    "production worker build policy rejected: {error}"
                )
            }
        }
    }
}

impl core::error::Error for WorkerAdmissionErrorV3 {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Decode(error) => Some(error),
            Self::BuildPolicy(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for WorkerBuildFieldV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LlvmVersion => "LLVM version",
            Self::LlvmBuildIdentity => "LLVM build identity",
            Self::LldVersion => "LLD version",
            Self::LldBuildIdentity => "LLD build identity",
            Self::InProcessLld => "in-process LLD mode",
        })
    }
}

impl fmt::Display for WorkerTargetPolicyFieldV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TargetTriple => "target triple",
            Self::DataLayout => "data layout",
            Self::Cpu => "CPU",
            Self::Features => "target features",
            Self::CodeObjectVersion => "code-object version",
            Self::OptimizationLevel => "optimization level",
            Self::RelocationModel => "relocation model",
            Self::CodeModel => "code model",
        })
    }
}

/// Untrusted worker-self observation supplied to admission.
///
/// Constructing this value does not authenticate a build. Admission compares
/// every field with the exact measured LLVM/LLD closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeasuredLlvmLldBuildV1<'a> {
    llvm_version: &'a str,
    llvm_build_identity: &'a str,
    lld_version: &'a str,
    lld_build_identity: &'a str,
    in_process_lld: bool,
}

impl<'a> MeasuredLlvmLldBuildV1<'a> {
    /// Constructs one untrusted worker build observation.
    pub const fn new(
        llvm_version: &'a str,
        llvm_build_identity: &'a str,
        lld_version: &'a str,
        lld_build_identity: &'a str,
        in_process_lld: bool,
    ) -> Self {
        Self {
            llvm_version,
            llvm_build_identity,
            lld_version,
            lld_build_identity,
            in_process_lld,
        }
    }

    /// Returns the exact observation expected from the measured worker build.
    pub const fn exact() -> Self {
        Self::new(
            EXACT_LLVM_VERSION_V1,
            EXACT_LLVM_BUILD_IDENTITY_V1,
            EXACT_LLD_VERSION_V1,
            EXACT_LLD_BUILD_IDENTITY_V1,
            true,
        )
    }

    /// Returns the observed LLVM version.
    pub const fn llvm_version(self) -> &'a str {
        self.llvm_version
    }

    /// Returns the observed LLVM build identity.
    pub const fn llvm_build_identity(self) -> &'a str {
        self.llvm_build_identity
    }

    /// Returns the observed LLD version.
    pub const fn lld_version(self) -> &'a str {
        self.lld_version
    }

    /// Returns the observed LLD build identity.
    pub const fn lld_build_identity(self) -> &'a str {
        self.lld_build_identity
    }

    /// Returns whether LLD was linked and invoked in-process.
    pub const fn in_process_lld(self) -> bool {
        self.in_process_lld
    }
}

/// Borrowed bytes and identity claims presented to the worker boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerAdmissionRequestV1<'a> {
    handoff_bytes: &'a [u8],
    claimed_handoff_identity: [u8; 32],
    measured_build: MeasuredLlvmLldBuildV1<'a>,
}

/// Borrowed canonical V2 bytes and identity claims presented to admission.
///
/// The build observation is caller-supplied and remains untrusted. Successful
/// admission proves only agreement with the exact configured policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerAdmissionRequestV2<'a> {
    handoff_bytes: &'a [u8],
    claimed_handoff_identity: [u8; 32],
    measured_build: MeasuredLlvmLldBuildV1<'a>,
}

/// Borrowed production compiler-module bytes and identity claim.
///
/// This append-only API uses the target-carrying compiler-module V2 wire and
/// does not reinterpret the legacy gfx942-only worker V1/V2 encodings.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerAdmissionRequestV3<'a> {
    handoff_bytes: &'a [u8],
    claimed_handoff_identity: [u8; 32],
    measured_build: MeasuredLlvmLldBuildV1<'a>,
}

impl<'a> WorkerAdmissionRequestV3<'a> {
    /// Constructs one untrusted production compiler-module request.
    pub const fn new(
        handoff_bytes: &'a [u8],
        claimed_handoff_identity: [u8; 32],
        measured_build: MeasuredLlvmLldBuildV1<'a>,
    ) -> Self {
        Self {
            handoff_bytes,
            claimed_handoff_identity,
            measured_build,
        }
    }

    /// Admits this request as inert exact-target compiler-module data.
    pub fn admit(self) -> Result<AdmittedWorkerRequestV3, WorkerAdmissionErrorV3> {
        admit_worker_request_v3(self)
    }
}

impl<'a> WorkerAdmissionRequestV2<'a> {
    /// Constructs one untrusted V2 admission request.
    pub const fn new(
        handoff_bytes: &'a [u8],
        claimed_handoff_identity: [u8; 32],
        measured_build: MeasuredLlvmLldBuildV1<'a>,
    ) -> Self {
        Self {
            handoff_bytes,
            claimed_handoff_identity,
            measured_build,
        }
    }

    /// Admits this request into inert typed V2 worker-boundary data.
    pub fn admit(self) -> Result<AdmittedWorkerRequestV2, WorkerAdmissionErrorV2> {
        admit_worker_request_v2(self)
    }
}

impl<'a> WorkerAdmissionRequestV1<'a> {
    /// Constructs one untrusted admission request.
    pub const fn new(
        handoff_bytes: &'a [u8],
        claimed_handoff_identity: [u8; 32],
        measured_build: MeasuredLlvmLldBuildV1<'a>,
    ) -> Self {
        Self {
            handoff_bytes,
            claimed_handoff_identity,
            measured_build,
        }
    }

    /// Admits this request into inert worker-boundary data.
    pub fn admit(self) -> Result<AdmittedWorkerRequestV1, WorkerAdmissionErrorV1> {
        admit_worker_request_v1(self)
    }
}

/// Exact LLVM/LLD build identity retained after successful admission.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactLlvmLldBuildIdentityV1;

impl ExactLlvmLldBuildIdentityV1 {
    /// Returns the exact upstream LLVM version.
    pub const fn llvm_version(self) -> &'static str {
        EXACT_LLVM_VERSION_V1
    }

    /// Returns the exact measured LLVM build identity.
    pub const fn llvm_build_identity(self) -> &'static str {
        EXACT_LLVM_BUILD_IDENTITY_V1
    }

    /// Returns the exact in-process LLD version.
    pub const fn lld_version(self) -> &'static str {
        EXACT_LLD_VERSION_V1
    }

    /// Returns the exact measured LLD build identity.
    pub const fn lld_build_identity(self) -> &'static str {
        EXACT_LLD_BUILD_IDENTITY_V1
    }

    /// Reports that the exact LLD closure is linked and invoked in-process.
    pub const fn in_process_lld(self) -> bool {
        true
    }
}

/// Domain-separated identity of an admitted handoff and exact worker build.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerAdmissionIdentityV1([u8; 32]);

impl WorkerAdmissionIdentityV1 {
    /// Returns the SHA-256 identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Domain-separated identity of an admitted V2 handoff and exact build policy.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerAdmissionIdentityV2([u8; 32]);

impl WorkerAdmissionIdentityV2 {
    /// Returns the SHA-256 identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Domain-separated identity of a production handoff, target, and exact build.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerAdmissionIdentityV3([u8; 32]);

impl WorkerAdmissionIdentityV3 {
    /// Returns the SHA-256 identity bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for WorkerAdmissionIdentityV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Display for WorkerAdmissionIdentityV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Display for WorkerAdmissionIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Inert request admitted at the pre-LLVM worker boundary.
///
/// This value proves only that bounded canonical data and an exact build
/// observation passed admission. It grants no object, link, or publication
/// authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedWorkerRequestV1 {
    handoff: Gfx942HandoffV1,
    handoff_identity: HandoffIdentityV1,
    build_identity: ExactLlvmLldBuildIdentityV1,
    admission_identity: WorkerAdmissionIdentityV1,
}

impl AdmittedWorkerRequestV1 {
    /// Returns the validated canonical handoff.
    pub const fn handoff(&self) -> &Gfx942HandoffV1 {
        &self.handoff
    }

    /// Returns the recomputed canonical handoff identity.
    pub const fn handoff_identity(&self) -> HandoffIdentityV1 {
        self.handoff_identity
    }

    /// Returns the exact LLVM/LLD build retained by admission.
    pub const fn build_identity(&self) -> ExactLlvmLldBuildIdentityV1 {
        self.build_identity
    }

    /// Returns the identity binding the handoff and exact worker build.
    pub const fn admission_identity(&self) -> WorkerAdmissionIdentityV1 {
        self.admission_identity
    }

    /// Reports that admission grants no object-emission authority.
    pub const fn grants_object_authority(&self) -> bool {
        false
    }

    /// Reports that admission grants no link authority.
    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    /// Reports that admission grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }
}

/// Inert typed V2 data admitted before the LLVM worker boundary.
///
/// This retains the complete validated executable graph and exact build
/// policy. It does not measure or execute a worker and grants no machine-code,
/// publication, loading, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedWorkerRequestV2 {
    handoff: Gfx942HandoffV2,
    handoff_identity: HandoffIdentityV2,
    build_identity: ExactLlvmLldBuildIdentityV1,
    admission_identity: WorkerAdmissionIdentityV2,
}

impl AdmittedWorkerRequestV2 {
    /// Returns the validated typed V2 handoff.
    pub const fn handoff(&self) -> &Gfx942HandoffV2 {
        &self.handoff
    }

    /// Returns the recomputed canonical V2 identity.
    pub const fn handoff_identity(&self) -> HandoffIdentityV2 {
        self.handoff_identity
    }

    /// Returns the exact LLVM/LLD policy retained by admission.
    pub const fn build_identity(&self) -> ExactLlvmLldBuildIdentityV1 {
        self.build_identity
    }

    /// Returns the identity binding the V2 handoff and exact build policy.
    pub const fn admission_identity(&self) -> WorkerAdmissionIdentityV2 {
        self.admission_identity
    }

    /// Reports that policy admission is not worker measurement.
    pub const fn authenticates_worker_measurement(&self) -> bool {
        false
    }

    /// Reports that admission grants no object-emission authority.
    pub const fn grants_object_authority(&self) -> bool {
        false
    }

    /// Reports that admission grants no link authority.
    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    /// Reports that admission grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Reports that admission grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Reports that admission grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Inert production compiler-module data admitted for one exact target.
///
/// The retained typed profile is derived from the decoded handoff target. It
/// cannot relabel the canonical handoff and grants no execution authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedWorkerRequestV3 {
    handoff: CompilerModuleHandoffV2,
    handoff_identity: CompilerModuleHandoffIdentityV2,
    target_profile: ProductionAmdTargetProfileV1,
    build_identity: ExactLlvmLldBuildIdentityV1,
    admission_identity: WorkerAdmissionIdentityV3,
    gfx950_compiler_ffi_kind: Option<ProductionGfx950CompilerFfiEnvelopeKindV1>,
}

impl AdmittedWorkerRequestV3 {
    /// Returns the validated canonical production handoff.
    pub const fn handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.handoff
    }

    /// Returns the recomputed complete handoff identity.
    pub const fn handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.handoff_identity
    }

    /// Returns the exact profile derived from the decoded handoff target.
    pub const fn target_profile(&self) -> ProductionAmdTargetProfileV1 {
        self.target_profile
    }

    /// Returns the exact LLVM/LLD policy retained by admission.
    pub const fn build_identity(&self) -> ExactLlvmLldBuildIdentityV1 {
        self.build_identity
    }

    /// Returns the identity binding the handoff, target, and build policy.
    pub const fn admission_identity(&self) -> WorkerAdmissionIdentityV3 {
        self.admission_identity
    }

    /// Returns the independently admitted exact gfx950 compiler-FFI shape.
    pub const fn gfx950_compiler_ffi_kind(
        &self,
    ) -> Option<ProductionGfx950CompilerFfiEnvelopeKindV1> {
        self.gfx950_compiler_ffi_kind
    }

    /// Reports that policy admission is not worker measurement.
    pub const fn authenticates_worker_measurement(&self) -> bool {
        false
    }

    /// Reports that admission grants no object-emission authority.
    pub const fn grants_object_authority(&self) -> bool {
        false
    }

    /// Reports that admission grants no link authority.
    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    /// Reports that admission grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Reports that admission grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Reports that admission grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Validates one canonical handoff before the worker may dereference LLVM data.
pub fn admit_worker_request_v1(
    request: WorkerAdmissionRequestV1<'_>,
) -> Result<AdmittedWorkerRequestV1, WorkerAdmissionErrorV1> {
    if request.handoff_bytes.len() > MAX_WORKER_ADMISSION_REQUEST_BYTES_V1 {
        return Err(WorkerAdmissionErrorV1::RequestTooLong {
            observed: request.handoff_bytes.len(),
            maximum: MAX_WORKER_ADMISSION_REQUEST_BYTES_V1,
        });
    }
    if request.claimed_handoff_identity == [0; 32] {
        return Err(WorkerAdmissionErrorV1::ZeroHandoffIdentity);
    }
    validate_measured_build(request.measured_build)?;

    let handoff = Gfx942HandoffV1::decode_canonical(request.handoff_bytes)
        .map_err(WorkerAdmissionErrorV1::Decode)?;
    let handoff_identity = handoff.identity();
    if handoff_identity.as_bytes() != &request.claimed_handoff_identity {
        return Err(WorkerAdmissionErrorV1::HandoffIdentityMismatch);
    }

    validate_target_policy(handoff.target())?;
    validate_device_libraries(&handoff)?;

    let build_identity = ExactLlvmLldBuildIdentityV1;
    let admission_identity = calculate_admission_identity(handoff_identity, build_identity);
    Ok(AdmittedWorkerRequestV1 {
        handoff,
        handoff_identity,
        build_identity,
        admission_identity,
    })
}

/// Validates one canonical typed V2 handoff before LLVM bytes are produced.
pub fn admit_worker_request_v2(
    request: WorkerAdmissionRequestV2<'_>,
) -> Result<AdmittedWorkerRequestV2, WorkerAdmissionErrorV2> {
    if request.handoff_bytes.len() > MAX_WORKER_ADMISSION_REQUEST_BYTES_V2 {
        return Err(WorkerAdmissionErrorV2::RequestTooLong {
            observed: request.handoff_bytes.len(),
            maximum: MAX_WORKER_ADMISSION_REQUEST_BYTES_V2,
        });
    }
    if request.claimed_handoff_identity == [0; 32] {
        return Err(WorkerAdmissionErrorV2::ZeroHandoffIdentity);
    }
    validate_measured_build(request.measured_build).map_err(WorkerAdmissionErrorV2::Policy)?;

    let handoff = Gfx942HandoffV2::decode_canonical(request.handoff_bytes)
        .map_err(WorkerAdmissionErrorV2::Decode)?;
    let handoff_identity = handoff.identity();
    if handoff_identity.as_bytes() != &request.claimed_handoff_identity {
        return Err(WorkerAdmissionErrorV2::HandoffIdentityMismatch);
    }
    validate_target_policy(handoff.base().target()).map_err(WorkerAdmissionErrorV2::Policy)?;
    validate_device_libraries(handoff.base()).map_err(WorkerAdmissionErrorV2::Policy)?;

    let build_identity = ExactLlvmLldBuildIdentityV1;
    let admission_identity = calculate_admission_identity_v2(handoff_identity, build_identity);
    Ok(AdmittedWorkerRequestV2 {
        handoff,
        handoff_identity,
        build_identity,
        admission_identity,
    })
}

/// Validates one canonical production compiler-module handoff.
///
/// Target selection is derived only from the decoded canonical handoff. The
/// append-only V3 API leaves legacy gfx942 V1/V2 wire interpretation intact.
pub fn admit_worker_request_v3(
    request: WorkerAdmissionRequestV3<'_>,
) -> Result<AdmittedWorkerRequestV3, WorkerAdmissionErrorV3> {
    if request.handoff_bytes.len() > MAX_WORKER_ADMISSION_REQUEST_BYTES_V3 {
        return Err(WorkerAdmissionErrorV3::RequestTooLong {
            observed: request.handoff_bytes.len(),
            maximum: MAX_WORKER_ADMISSION_REQUEST_BYTES_V3,
        });
    }
    if request.claimed_handoff_identity == [0; 32] {
        return Err(WorkerAdmissionErrorV3::ZeroHandoffIdentity);
    }
    validate_measured_build(request.measured_build).map_err(WorkerAdmissionErrorV3::BuildPolicy)?;

    let handoff = CompilerModuleHandoffV2::decode(request.handoff_bytes)
        .map_err(WorkerAdmissionErrorV3::Decode)?;
    let handoff_identity = handoff.identity();
    if handoff_identity.sha256() != &request.claimed_handoff_identity {
        return Err(WorkerAdmissionErrorV3::HandoffIdentityMismatch);
    }
    if handoff.kind() != CompilerModuleKindV1::LlvmTextIr {
        return Err(WorkerAdmissionErrorV3::ModuleKindSubstitution);
    }
    let target = handoff.target().to_string();
    let target_profile = ProductionAmdTargetProfileV1::from_device_target(&target)
        .ok_or(WorkerAdmissionErrorV3::TargetPolicySubstitution)?;
    if handoff.code_object_version() != CodeObjectVersion::V6 {
        return Err(WorkerAdmissionErrorV3::CodeObjectVersionSubstitution);
    }
    let gfx950_compiler_ffi_kind = if target_profile == ProductionAmdTargetProfileV1::Gfx950 {
        Some(validate_gfx950_device_ffi_policy(&handoff)?)
    } else {
        None
    };

    let build_identity = ExactLlvmLldBuildIdentityV1;
    let admission_identity =
        calculate_admission_identity_v3(handoff_identity, target_profile, build_identity);
    Ok(AdmittedWorkerRequestV3 {
        handoff,
        handoff_identity,
        target_profile,
        build_identity,
        admission_identity,
        gfx950_compiler_ffi_kind,
    })
}

fn validate_gfx950_device_ffi_policy(
    handoff: &CompilerModuleHandoffV2,
) -> Result<ProductionGfx950CompilerFfiEnvelopeKindV1, WorkerAdmissionErrorV3> {
    let kind = inspect_production_gfx950_compiler_ffi_envelope_v1(handoff.envelope())
        .ok_or(WorkerAdmissionErrorV3::Gfx950DeviceFfiPolicySubstitution)?;
    let llvm = core::str::from_utf8(handoff.module_bytes())
        .map_err(|_| WorkerAdmissionErrorV3::Gfx950DeviceFfiPolicySubstitution)?;
    let valid = match kind {
        ProductionGfx950CompilerFfiEnvelopeKindV1::NoDeviceFfi => !llvm.contains("@__ocml_"),
        ProductionGfx950CompilerFfiEnvelopeKindV1::OcmlExpF32 { .. } => {
            llvm.matches("declare float @__ocml_exp_f32(float)").count() == 1
                && llvm.matches("call float @__ocml_exp_f32(float ").count() >= 1
                && llvm
                    .split("@__ocml_")
                    .skip(1)
                    .all(|suffix| suffix.starts_with("exp_f32"))
        }
    };
    valid
        .then_some(kind)
        .ok_or(WorkerAdmissionErrorV3::Gfx950DeviceFfiPolicySubstitution)
}

fn validate_measured_build(
    measured: MeasuredLlvmLldBuildV1<'_>,
) -> Result<(), WorkerAdmissionErrorV1> {
    validate_build_text(
        WorkerBuildFieldV1::LlvmVersion,
        measured.llvm_version,
        MAX_WORKER_BUILD_VERSION_BYTES_V1,
        EXACT_LLVM_VERSION_V1,
    )?;
    validate_build_text(
        WorkerBuildFieldV1::LlvmBuildIdentity,
        measured.llvm_build_identity,
        MAX_WORKER_BUILD_IDENTITY_BYTES_V1,
        EXACT_LLVM_BUILD_IDENTITY_V1,
    )?;
    validate_build_text(
        WorkerBuildFieldV1::LldVersion,
        measured.lld_version,
        MAX_WORKER_BUILD_VERSION_BYTES_V1,
        EXACT_LLD_VERSION_V1,
    )?;
    validate_build_text(
        WorkerBuildFieldV1::LldBuildIdentity,
        measured.lld_build_identity,
        MAX_WORKER_BUILD_IDENTITY_BYTES_V1,
        EXACT_LLD_BUILD_IDENTITY_V1,
    )?;
    if !measured.in_process_lld {
        return Err(WorkerAdmissionErrorV1::BuildIdentitySubstitution(
            WorkerBuildFieldV1::InProcessLld,
        ));
    }
    Ok(())
}

fn validate_build_text(
    field: WorkerBuildFieldV1,
    observed: &str,
    maximum: usize,
    expected: &str,
) -> Result<(), WorkerAdmissionErrorV1> {
    if observed.len() > maximum {
        return Err(WorkerAdmissionErrorV1::BuildFieldTooLong {
            field,
            observed: observed.len(),
            maximum,
        });
    }
    if observed.is_empty()
        || !observed.is_ascii()
        || observed.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(WorkerAdmissionErrorV1::InvalidBuildField(field));
    }
    if observed != expected {
        return Err(WorkerAdmissionErrorV1::BuildIdentitySubstitution(field));
    }
    Ok(())
}

fn validate_target_policy(target: &Gfx942TargetPolicyV1) -> Result<(), WorkerAdmissionErrorV1> {
    let checks = [
        (
            target.target_triple() == GFX942_AMDHSA_TARGET_TRIPLE_V1,
            WorkerTargetPolicyFieldV1::TargetTriple,
        ),
        (
            target.data_layout() == GFX942_AMDHSA_DATA_LAYOUT_V1,
            WorkerTargetPolicyFieldV1::DataLayout,
        ),
        (target.cpu() == "gfx942", WorkerTargetPolicyFieldV1::Cpu),
        (
            target.features() == EXACT_TARGET_FEATURES_V1,
            WorkerTargetPolicyFieldV1::Features,
        ),
        (
            target.code_object_version() == CodeObjectVersionV1::V6,
            WorkerTargetPolicyFieldV1::CodeObjectVersion,
        ),
        (
            target.optimization_level() == OptimizationLevelV1::O2,
            WorkerTargetPolicyFieldV1::OptimizationLevel,
        ),
        (
            target.relocation_model() == RelocationModelV1::Pic,
            WorkerTargetPolicyFieldV1::RelocationModel,
        ),
        (
            target.code_model() == CodeModelV1::Small,
            WorkerTargetPolicyFieldV1::CodeModel,
        ),
    ];
    for (accepted, field) in checks {
        if !accepted {
            return Err(WorkerAdmissionErrorV1::TargetPolicySubstitution(field));
        }
    }
    Ok(())
}

fn validate_device_libraries(handoff: &Gfx942HandoffV1) -> Result<(), WorkerAdmissionErrorV1> {
    let libraries = handoff.module().device_libraries();
    if libraries.is_empty() {
        return Ok(());
    }
    for library in libraries {
        if !SUPPORTED_DEVICE_LIBRARY_CLOSURE_V1.contains(&library.kind()) {
            return Err(WorkerAdmissionErrorV1::UnsupportedDeviceLibrary(
                library.kind(),
            ));
        }
        if library.byte_len() > MAX_WORKER_DEVICE_LIBRARY_BYTES_V1 {
            return Err(WorkerAdmissionErrorV1::DeviceLibraryTooLong {
                kind: library.kind(),
                observed: library.byte_len(),
                maximum: MAX_WORKER_DEVICE_LIBRARY_BYTES_V1,
            });
        }
    }
    if libraries.len() != SUPPORTED_DEVICE_LIBRARY_CLOSURE_V1.len()
        || SUPPORTED_DEVICE_LIBRARY_CLOSURE_V1
            .iter()
            .any(|kind| !libraries.iter().any(|library| library.kind() == *kind))
    {
        return Err(WorkerAdmissionErrorV1::IncompleteDeviceLibraryClosure {
            observed: libraries.len(),
            required: SUPPORTED_DEVICE_LIBRARY_CLOSURE_V1.len(),
        });
    }
    Ok(())
}

fn calculate_admission_identity(
    handoff_identity: HandoffIdentityV1,
    build_identity: ExactLlvmLldBuildIdentityV1,
) -> WorkerAdmissionIdentityV1 {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, ADMISSION_IDENTITY_DOMAIN_V1);
    hash_field(&mut hasher, handoff_identity.as_bytes());
    hash_field(&mut hasher, build_identity.llvm_version().as_bytes());
    hash_field(&mut hasher, build_identity.llvm_build_identity().as_bytes());
    hash_field(&mut hasher, build_identity.lld_version().as_bytes());
    hash_field(&mut hasher, build_identity.lld_build_identity().as_bytes());
    hasher.update([u8::from(build_identity.in_process_lld())]);
    WorkerAdmissionIdentityV1(hasher.finalize().into())
}

fn calculate_admission_identity_v2(
    handoff_identity: HandoffIdentityV2,
    build_identity: ExactLlvmLldBuildIdentityV1,
) -> WorkerAdmissionIdentityV2 {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, ADMISSION_IDENTITY_DOMAIN_V2);
    hash_field(&mut hasher, handoff_identity.as_bytes());
    hash_field(&mut hasher, build_identity.llvm_version().as_bytes());
    hash_field(&mut hasher, build_identity.llvm_build_identity().as_bytes());
    hash_field(&mut hasher, build_identity.lld_version().as_bytes());
    hash_field(&mut hasher, build_identity.lld_build_identity().as_bytes());
    hasher.update([u8::from(build_identity.in_process_lld())]);
    WorkerAdmissionIdentityV2(hasher.finalize().into())
}

fn calculate_admission_identity_v3(
    handoff_identity: CompilerModuleHandoffIdentityV2,
    target_profile: ProductionAmdTargetProfileV1,
    build_identity: ExactLlvmLldBuildIdentityV1,
) -> WorkerAdmissionIdentityV3 {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, ADMISSION_IDENTITY_DOMAIN_V3);
    hash_field(&mut hasher, handoff_identity.sha256());
    hasher.update(handoff_identity.byte_len().to_le_bytes());
    hash_field(&mut hasher, target_profile.device_target().as_bytes());
    hash_field(&mut hasher, build_identity.llvm_version().as_bytes());
    hash_field(&mut hasher, build_identity.llvm_build_identity().as_bytes());
    hash_field(&mut hasher, build_identity.lld_version().as_bytes());
    hash_field(&mut hasher, build_identity.lld_build_identity().as_bytes());
    hasher.update([u8::from(build_identity.in_process_lld())]);
    WorkerAdmissionIdentityV3(hasher.finalize().into())
}

fn hash_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u32).to_le_bytes());
    hasher.update(bytes);
}
