//! Closed native wire schemas. Public adapters also require the actual context.
use crate::{
    CompilerExecutionExternalAnchorDeploymentErrorV1 as Error,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use sha2::{Digest, Sha256};

pub(crate) const PREIMAGE_BYTES: usize = 136;
pub(crate) const BYTES: usize = PREIMAGE_BYTES + 32;
pub(crate) const MAX_EXECUTABLE_BYTES: u64 = 128 * 1024 * 1024;

pub(crate) struct Schema {
    magic: [u8; 8],
    version: u16,
    domain: &'static [u8],
}
pub(crate) const V2: Schema = Schema {
    magic: *b"F2O3CEA2",
    version: 2,
    domain: b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-DEPLOYMENT/V2\0",
};
pub(crate) const V3: Schema = Schema {
    magic: *b"F2O3CEA3",
    version: 3,
    domain: b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-DEPLOYMENT/V3\0",
};

pub(crate) struct Fields {
    pub service: Service,
    pub verifying_key: [u8; 32],
    pub supervisor: [u8; 32],
    pub executable: Measurement,
}
pub(crate) struct Record {
    pub fields: Fields,
    pub identity: [u8; 32],
    pub bytes: [u8; BYTES],
}

impl Schema {
    pub fn encode(&self, fields: Fields) -> Result<Record, Error> {
        // Curve validation belongs to the native policy. The adapter requires
        // exact key equality before exposing any decoded owner or identity match.
        if fields.verifying_key == [0; 32] {
            return Err(Error::VerifyingKey);
        }
        if fields.supervisor == [0; 32] {
            return Err(Error::SupervisorIdentity);
        }
        if fields.executable.byte_len() > MAX_EXECUTABLE_BYTES {
            return Err(Error::ExecutableMeasurement);
        }
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(&self.magic);
        bytes[8..10].copy_from_slice(&self.version.to_le_bytes());
        bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
        bytes[24..28].copy_from_slice(&fields.service.uid().to_le_bytes());
        bytes[28..32].copy_from_slice(&fields.service.gid().to_le_bytes());
        bytes[32..64].copy_from_slice(&fields.verifying_key);
        bytes[64..96].copy_from_slice(&fields.supervisor);
        bytes[96..128].copy_from_slice(&fields.executable.sha256());
        bytes[128..136].copy_from_slice(&fields.executable.byte_len().to_le_bytes());
        let mut hash = Sha256::new();
        hash.update(self.domain);
        hash.update((PREIMAGE_BYTES as u64).to_le_bytes());
        hash.update(&bytes[..PREIMAGE_BYTES]);
        let identity: [u8; 32] = hash.finalize().into();
        bytes[PREIMAGE_BYTES..].copy_from_slice(&identity);
        Ok(Record {
            fields,
            identity,
            bytes,
        })
    }

    // Structural only: a valid digest does not validate a key or pin context.
    pub fn decode(&self, bytes: &[u8]) -> Result<Record, Error> {
        if bytes.len() != BYTES {
            return Err(Error::Length);
        }
        if bytes[..8] != self.magic {
            return Err(Error::Magic);
        }
        if u16::from_le_bytes(bytes[8..10].try_into().expect("fixed slice")) != self.version {
            return Err(Error::Version);
        }
        if bytes[10..12] != [0; 2] || bytes[16..24] != [0; 8] {
            return Err(Error::Reserved);
        }
        if u32_at(bytes, 12) != BYTES as u32 {
            return Err(Error::Length);
        }
        let fields = Fields {
            service: Service::new(u32_at(bytes, 24), u32_at(bytes, 28))
                .map_err(Error::ServiceIdentity)?,
            verifying_key: bytes[32..64].try_into().expect("fixed slice"),
            supervisor: bytes[64..96].try_into().expect("fixed slice"),
            executable: Measurement::new(
                bytes[96..128].try_into().expect("fixed slice"),
                u64::from_le_bytes(bytes[128..136].try_into().expect("fixed slice")),
            )
            .map_err(|_| Error::ExecutableMeasurement)?,
        };
        let canonical = self.encode(fields)?;
        if bytes[PREIMAGE_BYTES..] != canonical.identity {
            return Err(Error::Identity);
        }
        if canonical.bytes.as_slice() != bytes {
            return Err(Error::Canonical);
        }
        Ok(canonical)
    }
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed slice"))
}
