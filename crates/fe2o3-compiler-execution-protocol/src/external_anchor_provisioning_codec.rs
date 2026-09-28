//! Closed native provisioning wire schemas; public adapters additionally pin context.
use crate::{
    CompilerExecutionExternalAnchorProvisioningErrorV1 as Error,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
};
use sha2::{Digest, Sha256};

pub(crate) const PREIMAGE_BYTES: usize = 96;
pub(crate) const BYTES: usize = PREIMAGE_BYTES + 32;
pub(crate) const MAX_HELPER_BYTES: u64 = 128 * 1024 * 1024;

pub(crate) struct Schema {
    magic: [u8; 8],
    version: u16,
    domain: &'static [u8],
}
pub(crate) const V2: Schema = Schema {
    magic: *b"F2O3CEP2",
    version: 2,
    domain: b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-PROVISIONING/V2\0",
};
pub(crate) const V3: Schema = Schema {
    magic: *b"F2O3CEP3",
    version: 3,
    domain: b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-PROVISIONING/V3\0",
};

pub(crate) struct Fields {
    pub deployment: [u8; 32],
    pub helper: Measurement,
}
pub(crate) struct Record {
    pub fields: Fields,
    pub identity: [u8; 32],
    pub bytes: [u8; BYTES],
}

impl Schema {
    pub fn encode(&self, fields: Fields) -> Result<Record, Error> {
        if fields.deployment == [0; 32] {
            return Err(Error::DeploymentIdentity);
        }
        if fields.helper.byte_len() > MAX_HELPER_BYTES {
            return Err(Error::HelperMeasurement);
        }
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(&self.magic);
        bytes[8..10].copy_from_slice(&self.version.to_le_bytes());
        bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
        bytes[24..56].copy_from_slice(&fields.deployment);
        bytes[56..88].copy_from_slice(&fields.helper.sha256());
        bytes[88..96].copy_from_slice(&fields.helper.byte_len().to_le_bytes());
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

    // Structural decoding is private and prepaid. A digest alone never pins
    // the actual deployment; that comparison belongs to each public adapter.
    pub fn decode(&self, bytes: &[u8]) -> Result<Record, Error> {
        if bytes.len() != BYTES {
            return Err(Error::Length);
        }
        if bytes[..8] != self.magic
            || u16::from_le_bytes(bytes[8..10].try_into().expect("fixed slice")) != self.version
            || u32::from_le_bytes(bytes[12..16].try_into().expect("fixed slice")) != BYTES as u32
            || bytes[10..12] != [0; 2]
            || bytes[16..24] != [0; 8]
        {
            return Err(Error::Header);
        }
        let fields = Fields {
            deployment: bytes[24..56].try_into().expect("fixed slice"),
            helper: Measurement::new(
                bytes[56..88].try_into().expect("fixed slice"),
                u64::from_le_bytes(bytes[88..96].try_into().expect("fixed slice")),
            )
            .map_err(|_| Error::HelperMeasurement)?,
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
