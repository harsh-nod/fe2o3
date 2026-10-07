//! Distinct inert record for the root-observation and issuer-readiness join.

use super::*;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionIssuerPolicyV1, CompilerExecutionServiceReadyErrorV1,
    CompilerExecutionServiceReadyV1,
};

const MAGIC: &[u8; 8] = b"F3ARDY1\0";
const DOMAIN: &[u8] = b"FE2O3/WORKER-V3/APPLICATION-SUPERVISOR-READY/V1\0";
const COMPILER_OFFSET: usize = 56;
const IDENTITY_OFFSET: usize = 176;

/// Exact size of application readiness; compiler-only readiness is a different profile.
pub const WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1: usize = 208;

/// Canonical descriptive application readiness. Bytes alone authenticate neither root
/// observation nor issuer liveness, and do not establish application ACK or proof authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationSupervisorReadyV1 {
    registration: [u8; 32],
    compiler: CompilerExecutionServiceReadyV1,
    bytes: [u8; WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1],
}

impl WorkerV3ApplicationSupervisorReadyV1 {
    pub fn new(
        binding: &WorkerV3ApplicationRegistrationBindingV1,
        compiler: CompilerExecutionServiceReadyV1,
    ) -> std::result::Result<Self, WorkerV3ApplicationSupervisorReadyErrorV1> {
        let launch = binding.compiler_handoff().launch_manifest();
        if compiler.launch_manifest_identity() != launch.identity()
            || compiler.policy_identity() != launch.policy_identity()
        {
            return Err(WorkerV3ApplicationSupervisorReadyErrorV1::BindingMismatch);
        }
        Ok(Self::from_parts(*binding.identity().as_bytes(), compiler))
    }

    fn from_parts(registration: [u8; 32], compiler: CompilerExecutionServiceReadyV1) -> Self {
        let mut bytes = [0; WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[12..16].copy_from_slice(
            &(WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1 as u32).to_le_bytes(),
        );
        bytes[24..COMPILER_OFFSET].copy_from_slice(&registration);
        bytes[COMPILER_OFFSET..IDENTITY_OFFSET].copy_from_slice(compiler.canonical_bytes());
        let identity = identity(&bytes[..IDENTITY_OFFSET]);
        bytes[IDENTITY_OFFSET..].copy_from_slice(&identity);
        Self {
            registration,
            compiler,
            bytes,
        }
    }

    pub fn decode(
        bytes: &[u8],
    ) -> std::result::Result<Self, WorkerV3ApplicationSupervisorReadyErrorV1> {
        use WorkerV3ApplicationSupervisorReadyErrorV1 as E;
        if bytes.len() != WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1 {
            return Err(E::Length);
        }
        if &bytes[..8] != MAGIC
            || bytes[8..10] != 1u16.to_le_bytes()
            || bytes[12..16] != (bytes.len() as u32).to_le_bytes()
        {
            return Err(E::Header);
        }
        if bytes[10..12] != [0; 2] || bytes[16..24] != [0; 8] {
            return Err(E::Reserved);
        }
        if bytes[24..COMPILER_OFFSET] == [0; 32]
            || bytes[IDENTITY_OFFSET..] != identity(&bytes[..IDENTITY_OFFSET])
        {
            return Err(E::Identity);
        }
        let compiler =
            CompilerExecutionServiceReadyV1::decode(&bytes[COMPILER_OFFSET..IDENTITY_OFFSET])
                .map_err(E::Compiler)?;
        let value = Self::from_parts(bytes[24..COMPILER_OFFSET].try_into().unwrap(), compiler);
        if value.bytes != bytes {
            return Err(E::Canonical);
        }
        Ok(value)
    }

    pub fn matches_binding(
        &self,
        binding: &WorkerV3ApplicationRegistrationBindingV1,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> bool {
        self.registration == *binding.identity().as_bytes()
            && self.compiler.matches_launch(
                self.compiler.issuer_pid(),
                binding.compiler_handoff().launch_manifest(),
                policy,
            )
    }

    pub const fn compiler_readiness(&self) -> &CompilerExecutionServiceReadyV1 {
        &self.compiler
    }

    pub const fn canonical_bytes(&self) -> &[u8; WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1] {
        &self.bytes
    }
}

fn identity(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3ApplicationSupervisorReadyErrorV1 {
    Length,
    Header,
    Reserved,
    Identity,
    Canonical,
    BindingMismatch,
    Compiler(CompilerExecutionServiceReadyErrorV1),
}

impl fmt::Display for WorkerV3ApplicationSupervisorReadyErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid application supervisor readiness: {self:?}")
    }
}

impl Error for WorkerV3ApplicationSupervisorReadyErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Compiler(error) => Some(error),
            _ => None,
        }
    }
}
