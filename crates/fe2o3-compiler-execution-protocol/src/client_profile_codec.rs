//! Profile framing is shared; nested policy admission stays family-specific.
use crate::{
    CompilerExecutionClientProfileErrorV1 as Error,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service, issuer_policy_codec,
};
use sha2::{Digest, Sha256};

pub(crate) const POLICY_OFFSET: usize = 32;
pub(crate) const PREIMAGE_BYTES: usize = POLICY_OFFSET + issuer_policy_codec::BYTES;
pub(crate) const BYTES: usize = PREIMAGE_BYTES + 32;
pub(crate) struct Schema {
    magic: [u8; 8],
    version: u16,
    domain: &'static [u8],
}
pub(crate) const V1: Schema = Schema {
    magic: *b"F2O3CEP1",
    version: 1,
    domain: b"FE2O3/COMPILER-EXECUTION-CLIENT-PROFILE/V1\0",
};
pub(crate) const V2: Schema = Schema {
    magic: *b"F2O3CEP2",
    version: 2,
    domain: b"FE2O3/COMPILER-EXECUTION-CLIENT-PROFILE/V2\0",
};
pub(crate) struct Parsed<'a> {
    pub uid: u32,
    pub gid: u32,
    pub service: Service,
    pub policy: &'a [u8],
}
impl Schema {
    pub fn encode(
        &self,
        uid: u32,
        gid: u32,
        service: Service,
        policy: &[u8; issuer_policy_codec::BYTES],
    ) -> ([u8; BYTES], [u8; 32]) {
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(&self.magic);
        bytes[8..10].copy_from_slice(&self.version.to_le_bytes());
        bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
        bytes[16..20].copy_from_slice(&uid.to_le_bytes());
        bytes[20..24].copy_from_slice(&gid.to_le_bytes());
        bytes[24..28].copy_from_slice(&service.uid().to_le_bytes());
        bytes[28..32].copy_from_slice(&service.gid().to_le_bytes());
        bytes[POLICY_OFFSET..PREIMAGE_BYTES].copy_from_slice(policy);
        let identity = self.identity(&bytes[..PREIMAGE_BYTES]);
        bytes[PREIMAGE_BYTES..].copy_from_slice(&identity);
        (bytes, identity)
    }

    // Nested policy errors must precede the terminal profile hash check.
    pub fn parse<'a>(&self, bytes: &'a [u8]) -> Result<Parsed<'a>, Error> {
        if bytes.len() != BYTES {
            return Err(Error::Length);
        }
        if bytes[..8] != self.magic {
            return Err(Error::Magic);
        }
        let version = u16::from_le_bytes(bytes[8..10].try_into().expect("fixed slice"));
        if version != self.version {
            return Err(Error::Version(version));
        }
        let flags = u16::from_le_bytes(bytes[10..12].try_into().expect("fixed slice"));
        if flags != 0 {
            return Err(Error::UnsupportedFlags(flags));
        }
        if u32_at(bytes, 12) != BYTES as u32 {
            return Err(Error::DeclaredLength);
        }
        let uid = u32_at(bytes, 16);
        let gid = u32_at(bytes, 20);
        validate_credentials(uid, gid)?;
        let service = Service::new(u32_at(bytes, 24), u32_at(bytes, 28))
            .map_err(Error::ExternalAnchorServiceIdentity)?;
        Ok(Parsed {
            uid,
            gid,
            service,
            policy: &bytes[POLICY_OFFSET..PREIMAGE_BYTES],
        })
    }

    pub fn check_identity(&self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() != BYTES {
            return Err(Error::Length);
        }
        let declared: [u8; 32] = bytes[PREIMAGE_BYTES..].try_into().expect("fixed slice");
        if declared == [0; 32] || !self.matches(declared, bytes) {
            return Err(Error::Identity);
        }
        Ok(())
    }
    pub fn matches(&self, identity: [u8; 32], bytes: &[u8]) -> bool {
        bytes.len() == BYTES
            && bytes[PREIMAGE_BYTES..] == identity
            && self.identity(&bytes[..PREIMAGE_BYTES]) == identity
    }
    pub fn identity(&self, preimage: &[u8]) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update((self.domain.len() as u64).to_le_bytes());
        digest.update(self.domain);
        digest.update((preimage.len() as u64).to_le_bytes());
        digest.update(preimage);
        digest.finalize().into()
    }
}
pub(crate) fn validate_credentials(uid: u32, gid: u32) -> Result<(), Error> {
    if uid == 0 || uid == u32::MAX {
        return Err(Error::InvalidSupervisorUid);
    }
    if gid == 0 || gid == u32::MAX {
        return Err(Error::InvalidSupervisorGid);
    }
    Ok(())
}
fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed slice"))
}
