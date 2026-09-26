//! Fixed deployment framing shared only by the closed native family adapters.
use crate::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionSupervisorDeploymentErrorV1 as Error,
};
use sha2::{Digest, Sha256};

pub(crate) const PREIMAGE_BYTES: usize = 152;
pub(crate) const BYTES: usize = PREIMAGE_BYTES + 32;
pub(crate) const MAX_EXECUTABLE_BYTES: u64 = 128 * 1024 * 1024;
pub(crate) const MAX_LAUNCHER_BYTES: u64 = 128 * 1024 * 1024;

pub(crate) struct Schema {
    magic: [u8; 8],
    version: u16,
    domain: &'static [u8],
}

pub(crate) const V2: Schema = Schema {
    magic: *b"F2O3CED2",
    version: 2,
    domain: b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-DEPLOYMENT/V2\0",
};
pub(crate) const V3: Schema = Schema {
    magic: *b"F2O3CED3",
    version: 3,
    domain: b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-DEPLOYMENT/V3\0",
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Fields {
    pub uid: u32,
    pub gid: u32,
    pub service: Service,
    pub executable: Measurement,
    pub launcher: Measurement,
    pub policy: [u8; 32],
}

#[derive(Eq, PartialEq)]
pub(crate) struct Record {
    pub fields: Fields,
    pub identity: [u8; 32],
    pub bytes: [u8; BYTES],
}

impl Schema {
    pub fn encode(&self, fields: Fields) -> Result<Record, Error> {
        validate_fields(&fields)?;
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(&self.magic);
        bytes[8..10].copy_from_slice(&self.version.to_le_bytes());
        bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
        bytes[24..28].copy_from_slice(&fields.uid.to_le_bytes());
        bytes[28..32].copy_from_slice(&fields.gid.to_le_bytes());
        bytes[32..36].copy_from_slice(&fields.service.uid().to_le_bytes());
        bytes[36..40].copy_from_slice(&fields.service.gid().to_le_bytes());
        bytes[40..72].copy_from_slice(&fields.executable.sha256());
        bytes[72..80].copy_from_slice(&fields.executable.byte_len().to_le_bytes());
        bytes[80..112].copy_from_slice(&fields.launcher.sha256());
        bytes[112..120].copy_from_slice(&fields.launcher.byte_len().to_le_bytes());
        bytes[120..152].copy_from_slice(&fields.policy);
        let identity = self.identity(&bytes[..PREIMAGE_BYTES]);
        bytes[PREIMAGE_BYTES..].copy_from_slice(&identity);
        Ok(Record {
            fields,
            identity,
            bytes,
        })
    }

    // Private structural decoding is always prepaid by the native adapter. The
    // public owner decoder also checks the complete supplied native policy ID.
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
            uid: u32_at(bytes, 24),
            gid: u32_at(bytes, 28),
            service: Service::new(u32_at(bytes, 32), u32_at(bytes, 36))
                .map_err(Error::ExternalAnchorServiceIdentity)?,
            executable: Measurement::new(
                bytes[40..72].try_into().expect("fixed slice"),
                u64_at(bytes, 72),
            )
            .map_err(|_| Error::ExecutableMeasurement)?,
            launcher: Measurement::new(
                bytes[80..112].try_into().expect("fixed slice"),
                u64_at(bytes, 112),
            )
            .map_err(|_| Error::LauncherMeasurement)?,
            policy: bytes[120..152].try_into().expect("fixed slice"),
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

    pub fn matches(&self, identity: [u8; 32], bytes: &[u8]) -> bool {
        self.decode(bytes)
            .is_ok_and(|record| record.identity == identity)
    }

    fn identity(&self, preimage: &[u8]) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(self.domain);
        hash.update((PREIMAGE_BYTES as u64).to_le_bytes());
        hash.update(preimage);
        hash.finalize().into()
    }
}

fn validate_fields(fields: &Fields) -> Result<(), Error> {
    if fields.uid == 0 || fields.uid == u32::MAX {
        return Err(Error::ServiceUid);
    }
    if fields.gid == 0 || fields.gid == u32::MAX {
        return Err(Error::ServiceGid);
    }
    if fields.uid == fields.service.uid() {
        return Err(Error::SharedServiceUid);
    }
    if fields.executable.byte_len() > MAX_EXECUTABLE_BYTES {
        return Err(Error::ExecutableMeasurement);
    }
    if fields.launcher.byte_len() > MAX_LAUNCHER_BYTES {
        return Err(Error::LauncherMeasurement);
    }
    if fields.executable == fields.launcher {
        return Err(Error::AliasedExecutableMeasurements);
    }
    if fields.policy == [0; 32] {
        return Err(Error::PolicyIdentity);
    }
    Ok(())
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed slice"))
}
fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("fixed slice"))
}
