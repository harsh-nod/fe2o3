//! Shared fixed mechanics; public adapters retain the nominal subject owner.
use crate::attestation::{
    CompilerExecutionAttestationErrorV1 as Error, Reader, decode_header_version, derive_identity,
    encode_header_version, put, require_identity, require_length, validate_binding,
    validate_rollback_position,
};

pub(crate) const CHALLENGE_BYTES: usize = 200;
pub(crate) const SUBJECT_BYTES: usize = 690;
pub(crate) const REQUEST_BYTES: usize = 24 + CHALLENGE_BYTES + SUBJECT_BYTES + 32;
pub(crate) const V1: Schema = Schema {
    version: 1,
    challenge_magic: *b"F2O3CEC1",
    request_magic: *b"F2O3CEQ1",
    challenge_domain: b"FE2O3/COMPILER-EXECUTION-CHALLENGE/V1\0",
    request_domain: b"FE2O3/COMPILER-EXECUTION-REQUEST/V1\0",
};
pub(crate) const V2: Schema = Schema {
    version: 2,
    challenge_magic: *b"F2O3CEC2",
    request_magic: *b"F2O3CEQ2",
    challenge_domain: b"FE2O3/COMPILER-EXECUTION-CHALLENGE/V2\0",
    request_domain: b"FE2O3/COMPILER-EXECUTION-REQUEST/V2\0",
};
const _: () = {
    assert!(SUBJECT_BYTES == fe2o3_artifact_transaction::INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1);
    assert!(SUBJECT_BYTES == fe2o3_artifact_transaction::INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V2);
};
pub(crate) struct Schema {
    version: u16,
    challenge_magic: [u8; 8],
    request_magic: [u8; 8],
    challenge_domain: &'static [u8],
    request_domain: &'static [u8],
}
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct Binding {
    pub sha256: [u8; 32],
    pub byte_len: u64,
}
impl Binding {
    pub fn new(sha256: [u8; 32], byte_len: u64) -> Result<Self, Error> {
        validate_binding(sha256, byte_len, "compiler-execution subject")?;
        if byte_len != SUBJECT_BYTES as u64 {
            return Err(Error::SubjectLengthMismatch);
        }
        Ok(Self { sha256, byte_len })
    }
}
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Fields {
    pub policy: [u8; 32],
    pub subject: Binding,
    pub nonce: [u8; 32],
    pub sequence: u64,
    pub prior: [u8; 32],
}
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Challenge {
    pub fields: Fields,
    pub identity: [u8; 32],
    pub bytes: [u8; CHALLENGE_BYTES],
}
pub(crate) struct RequestParts<'a> {
    pub challenge: &'a [u8],
    pub subject: &'a [u8],
    pub identity: [u8; 32],
}
impl Schema {
    pub fn challenge(&self, fields: Fields) -> Result<Challenge, Error> {
        require_identity(fields.policy, "issuer policy")?;
        require_identity(fields.nonce, "challenge nonce")?;
        validate_rollback_position(fields.sequence, fields.prior)?;
        let mut bytes = [0; CHALLENGE_BYTES];
        let mut offset = encode_header_version(&mut bytes, self.challenge_magic, self.version);
        put(&mut bytes, &mut offset, &fields.policy);
        put(&mut bytes, &mut offset, &fields.subject.sha256);
        put(
            &mut bytes,
            &mut offset,
            &fields.subject.byte_len.to_le_bytes(),
        );
        put(&mut bytes, &mut offset, &fields.nonce);
        put(&mut bytes, &mut offset, &fields.sequence.to_le_bytes());
        put(&mut bytes, &mut offset, &fields.prior);
        debug_assert_eq!(offset, CHALLENGE_BYTES - 32);
        let identity = derive_identity(self.challenge_domain, &bytes[..offset]);
        put(&mut bytes, &mut offset, &identity);
        Ok(Challenge {
            fields,
            identity,
            bytes,
        })
    }
    pub fn decode_challenge(&self, bytes: &[u8]) -> Result<Challenge, Error> {
        require_length(bytes, CHALLENGE_BYTES, "challenge")?;
        let mut reader = Reader::new(bytes);
        decode_header_version(
            &mut reader,
            self.challenge_magic,
            self.version,
            CHALLENGE_BYTES,
            "challenge",
        )?;
        let fields = Fields {
            policy: reader.fixed()?,
            subject: Binding::new(reader.fixed()?, reader.u64()?)?,
            nonce: reader.fixed()?,
            sequence: reader.u64()?,
            prior: reader.fixed()?,
        };
        let declared = reader.fixed()?;
        require_identity(declared, "challenge")?;
        let decoded = self.challenge(fields)?;
        if decoded.identity != declared || decoded.bytes.as_slice() != bytes {
            return Err(Error::IdentityMismatch("challenge"));
        }
        Ok(decoded)
    }
    pub fn request(
        &self,
        challenge: &[u8; CHALLENGE_BYTES],
        subject: &[u8; SUBJECT_BYTES],
    ) -> ([u8; REQUEST_BYTES], [u8; 32]) {
        let mut bytes = [0; REQUEST_BYTES];
        let mut offset = encode_header_version(&mut bytes, self.request_magic, self.version);
        put(&mut bytes, &mut offset, challenge);
        put(&mut bytes, &mut offset, subject);
        debug_assert_eq!(offset, REQUEST_BYTES - 32);
        let identity = derive_identity(self.request_domain, &bytes[..offset]);
        put(&mut bytes, &mut offset, &identity);
        (bytes, identity)
    }
    // Terminal identity validation deliberately follows both nested decoders.
    pub fn request_parts<'a>(&self, bytes: &'a [u8]) -> Result<RequestParts<'a>, Error> {
        require_length(bytes, REQUEST_BYTES, "request")?;
        let mut reader = Reader::new(bytes);
        decode_header_version(
            &mut reader,
            self.request_magic,
            self.version,
            REQUEST_BYTES,
            "request",
        )?;
        Ok(RequestParts {
            challenge: &bytes[24..24 + CHALLENGE_BYTES],
            subject: &bytes[24 + CHALLENGE_BYTES..REQUEST_BYTES - 32],
            identity: bytes[REQUEST_BYTES - 32..].try_into().expect("fixed slice"),
        })
    }
    pub fn matches_challenge(&self, identity: [u8; 32], bytes: &[u8]) -> bool {
        matches(self.challenge_domain, CHALLENGE_BYTES, identity, bytes)
    }
    pub fn matches_request(&self, identity: [u8; 32], bytes: &[u8]) -> bool {
        matches(self.request_domain, REQUEST_BYTES, identity, bytes)
    }
}
fn matches(domain: &[u8], length: usize, identity: [u8; 32], bytes: &[u8]) -> bool {
    bytes.len() == length
        && bytes[length - 32..] == identity
        && derive_identity(domain, &bytes[..length - 32]) == identity
}
