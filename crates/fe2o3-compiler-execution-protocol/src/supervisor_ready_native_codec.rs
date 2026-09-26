//! Native bootstrap framing; private-channel provenance is never inferred here.
use crate::CompilerExecutionSupervisorReadyErrorV1 as Error;
use sha2::{Digest, Sha256};

pub(crate) const BYTES: usize = 88;
const PREIMAGE: usize = 56;

pub(crate) struct Schema {
    magic: [u8; 8],
    version: u16,
    domain: &'static [u8],
}
pub(crate) const V2: Schema = Schema {
    magic: *b"F2O3CSR2",
    version: 2,
    domain: b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-READY/V2\0",
};
pub(crate) const V3: Schema = Schema {
    magic: *b"F2O3CSR3",
    version: 3,
    domain: b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-READY/V3\0",
};

impl Schema {
    pub(crate) fn encode(&self, pid: u32, deployment: &[u8; 32]) -> Result<[u8; BYTES], Error> {
        if pid == 0 {
            return Err(Error::SupervisorPid);
        }
        if *deployment == [0; 32] {
            return Err(Error::DeploymentIdentity);
        }
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(&self.magic);
        bytes[8..10].copy_from_slice(&self.version.to_le_bytes());
        bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
        bytes[16..20].copy_from_slice(&pid.to_le_bytes());
        bytes[24..56].copy_from_slice(deployment);
        let identity = self.identity(&bytes[..PREIMAGE]);
        bytes[PREIMAGE..].copy_from_slice(&identity);
        Ok(bytes)
    }

    pub(crate) fn decode(&self, bytes: &[u8]) -> Result<(u32, [u8; 32]), Error> {
        if bytes.len() != BYTES {
            return Err(Error::Length);
        }
        if bytes[..8] != self.magic {
            return Err(Error::Magic);
        }
        if u16::from_le_bytes(bytes[8..10].try_into().unwrap()) != self.version {
            return Err(Error::Version);
        }
        if bytes[10..12] != [0; 2] || bytes[20..24] != [0; 4] {
            return Err(Error::Reserved);
        }
        if u32::from_le_bytes(bytes[12..16].try_into().unwrap()) != BYTES as u32 {
            return Err(Error::Length);
        }
        let pid = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let deployment = bytes[24..56].try_into().unwrap();
        let canonical = self.encode(pid, &deployment)?;
        if bytes[PREIMAGE..] != canonical[PREIMAGE..] {
            return Err(Error::Identity);
        }
        if bytes != canonical {
            return Err(Error::Canonical);
        }
        Ok((pid, deployment))
    }

    fn identity(&self, bytes: &[u8]) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(self.domain);
        hash.update((PREIMAGE as u64).to_le_bytes());
        hash.update(bytes);
        hash.finalize().into()
    }
}
