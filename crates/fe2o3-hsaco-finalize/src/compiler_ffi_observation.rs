//! Opaque finalizer-side retention of a complete compiler FFI envelope.

use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerFfiEnvelopeIdentityV1, CompilerFfiEnvelopeV1, DeviceTargetV1,
};
use sha2::{Digest, Sha256};
use std::fmt;

const STAGED_COMPILER_FFI_ENVELOPE_DOMAIN_V1: &[u8] = b"FE2O3/STAGED-COMPILER-FFI-ENVELOPE/V1\0";

/// Identity of a complete neutral compiler envelope retained by finalization.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StagedCompilerFfiEnvelopeIdentityV1([u8; 32]);

impl StagedCompilerFfiEnvelopeIdentityV1 {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }

    pub fn to_hex(self) -> String {
        lower_hex(&self.0)
    }
}

/// Exact reason this observation cannot enter an executable worker path.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StagedCompilerFfiEnvelopeBlockerV1 {
    MissingExactCompilerModuleArtifactAndWorkerProtocolV2,
}

/// Non-authoritative summary without contract or linker closures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StagedCompilerFfiEnvelopeInspectionV1 {
    envelope_identity: CompilerFfiEnvelopeIdentityV1,
    target: DeviceTargetV1,
    code_object_version: CodeObjectVersion,
    import_count: usize,
    export_count: usize,
    blocker: StagedCompilerFfiEnvelopeBlockerV1,
}

impl StagedCompilerFfiEnvelopeInspectionV1 {
    pub const fn envelope_identity(self) -> CompilerFfiEnvelopeIdentityV1 {
        self.envelope_identity
    }

    pub const fn target(self) -> DeviceTargetV1 {
        self.target
    }

    pub const fn code_object_version(self) -> CodeObjectVersion {
        self.code_object_version
    }

    pub const fn import_count(self) -> usize {
        self.import_count
    }

    pub const fn export_count(self) -> usize {
        self.export_count
    }

    pub const fn blocker(self) -> StagedCompilerFfiEnvelopeBlockerV1 {
        self.blocker
    }
}

/// Complete caller-supplied compiler observation retained without exposing reducible closures.
#[derive(Clone, Eq, PartialEq)]
pub struct StagedCompilerFfiEnvelopeV1 {
    identity: StagedCompilerFfiEnvelopeIdentityV1,
    inspection: StagedCompilerFfiEnvelopeInspectionV1,
    #[allow(dead_code)]
    envelope: CompilerFfiEnvelopeV1,
}

impl fmt::Debug for StagedCompilerFfiEnvelopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StagedCompilerFfiEnvelopeV1")
            .field("identity", &self.identity)
            .field("inspection", &self.inspection)
            .finish_non_exhaustive()
    }
}

impl StagedCompilerFfiEnvelopeV1 {
    pub const fn identity(&self) -> StagedCompilerFfiEnvelopeIdentityV1 {
        self.identity
    }

    pub const fn inspection(&self) -> StagedCompilerFfiEnvelopeInspectionV1 {
        self.inspection
    }

    pub const fn grants_worker_authority(&self) -> bool {
        false
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
}

/// Consumes and retains one already-bounded compiler envelope as inert state.
pub fn stage_compiler_ffi_envelope_v1(
    envelope: CompilerFfiEnvelopeV1,
) -> StagedCompilerFfiEnvelopeV1 {
    let envelope_inspection = envelope.inspection();
    let inspection = StagedCompilerFfiEnvelopeInspectionV1 {
        envelope_identity: envelope.identity(),
        target: envelope.target(),
        code_object_version: envelope.code_object_version(),
        import_count: envelope_inspection.import_count(),
        export_count: envelope_inspection.export_count(),
        blocker:
            StagedCompilerFfiEnvelopeBlockerV1::MissingExactCompilerModuleArtifactAndWorkerProtocolV2,
    };
    let mut digest = Sha256::new();
    digest.update(STAGED_COMPILER_FFI_ENVELOPE_DOMAIN_V1);
    digest.update((envelope.canonical_bytes().len() as u64).to_le_bytes());
    digest.update(envelope.canonical_bytes());
    let identity = StagedCompilerFfiEnvelopeIdentityV1(digest.finalize().into());
    StagedCompilerFfiEnvelopeV1 {
        identity,
        inspection,
        envelope,
    }
}

fn lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
