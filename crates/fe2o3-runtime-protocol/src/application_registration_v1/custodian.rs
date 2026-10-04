//! Distinct supervisor wire profiles; these bytes alone grant no proof custody.

use super::*;
use std::result::Result;

const HEADER: usize = 16;
const HANDOFF_MAGIC: &[u8; 8] = b"F3ACHF1\0";
const READY_MAGIC: &[u8; 8] = b"F3ACRD1\0";

/// Exact four-right custodian handoff size, distinct from legacy registration.
pub const WORKER_V3_APPLICATION_CUSTODIAN_HANDOFF_BYTES_V1: usize =
    HEADER + WORKER_V3_APPLICATION_REGISTRATION_BYTES_V1;
/// Exact custodian supervisor readiness size, distinct from legacy readiness.
pub const WORKER_V3_APPLICATION_CUSTODIAN_READY_BYTES_V1: usize =
    HEADER + WORKER_V3_APPLICATION_SUPERVISOR_READY_BYTES_V1;

/// An explicit request for the mandatory proof-custodian route on the authenticated
/// supervisor connection. The enclosed original binding is not changed or reacquired.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationCustodianHandoffV1 {
    binding: WorkerV3ApplicationRegistrationBindingV1,
    bytes: [u8; WORKER_V3_APPLICATION_CUSTODIAN_HANDOFF_BYTES_V1],
}

impl WorkerV3ApplicationCustodianHandoffV1 {
    pub fn new(binding: WorkerV3ApplicationRegistrationBindingV1) -> Self {
        let bytes = encode(HANDOFF_MAGIC, binding.canonical_bytes());
        Self { binding, bytes }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WorkerV3ApplicationCustodianRouteErrorV1> {
        let body = decode_header(
            bytes,
            HANDOFF_MAGIC,
            WORKER_V3_APPLICATION_CUSTODIAN_HANDOFF_BYTES_V1,
        )?;
        WorkerV3ApplicationRegistrationBindingV1::decode(body)
            .map(Self::new)
            .map_err(WorkerV3ApplicationCustodianRouteErrorV1::Binding)
    }

    pub const fn binding(&self) -> &WorkerV3ApplicationRegistrationBindingV1 {
        &self.binding
    }

    pub const fn canonical_bytes(&self) -> &[u8; WORKER_V3_APPLICATION_CUSTODIAN_HANDOFF_BYTES_V1] {
        &self.bytes
    }
}

/// Inert evidence of custodian-route registration and issuer readiness, not controller
/// Ready, completed proof, application ACK or GPU authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3ApplicationCustodianSupervisorReadyV1 {
    application: WorkerV3ApplicationSupervisorReadyV1,
    bytes: [u8; WORKER_V3_APPLICATION_CUSTODIAN_READY_BYTES_V1],
}

impl WorkerV3ApplicationCustodianSupervisorReadyV1 {
    pub fn new(application: WorkerV3ApplicationSupervisorReadyV1) -> Self {
        let bytes = encode(READY_MAGIC, application.canonical_bytes());
        Self { application, bytes }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, WorkerV3ApplicationCustodianRouteErrorV1> {
        let body = decode_header(
            bytes,
            READY_MAGIC,
            WORKER_V3_APPLICATION_CUSTODIAN_READY_BYTES_V1,
        )?;
        WorkerV3ApplicationSupervisorReadyV1::decode(body)
            .map(Self::new)
            .map_err(WorkerV3ApplicationCustodianRouteErrorV1::Readiness)
    }

    pub const fn application_readiness(&self) -> &WorkerV3ApplicationSupervisorReadyV1 {
        &self.application
    }

    pub const fn canonical_bytes(&self) -> &[u8; WORKER_V3_APPLICATION_CUSTODIAN_READY_BYTES_V1] {
        &self.bytes
    }
}

fn encode<const N: usize>(magic: &[u8; 8], body: &[u8]) -> [u8; N] {
    let mut bytes = [0; N];
    bytes[..8].copy_from_slice(magic);
    bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
    bytes[12..16].copy_from_slice(&(N as u32).to_le_bytes());
    bytes[HEADER..].copy_from_slice(body);
    bytes
}

fn decode_header<'a>(
    bytes: &'a [u8],
    magic: &[u8; 8],
    size: usize,
) -> Result<&'a [u8], WorkerV3ApplicationCustodianRouteErrorV1> {
    if bytes.len() != size {
        return Err(WorkerV3ApplicationCustodianRouteErrorV1::Length);
    }
    if &bytes[..8] != magic
        || bytes[8..10] != 1u16.to_le_bytes()
        || bytes[10..12] != [0; 2]
        || bytes[12..16] != (size as u32).to_le_bytes()
    {
        return Err(WorkerV3ApplicationCustodianRouteErrorV1::Header);
    }
    Ok(&bytes[HEADER..])
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerV3ApplicationCustodianRouteErrorV1 {
    Length,
    Header,
    Binding(WorkerV3ApplicationRegistrationErrorV1),
    Readiness(WorkerV3ApplicationSupervisorReadyErrorV1),
}

impl fmt::Display for WorkerV3ApplicationCustodianRouteErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid application custodian route: {self:?}")
    }
}

impl Error for WorkerV3ApplicationCustodianRouteErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Binding(error) => Some(error),
            Self::Readiness(error) => Some(error),
            _ => None,
        }
    }
}
