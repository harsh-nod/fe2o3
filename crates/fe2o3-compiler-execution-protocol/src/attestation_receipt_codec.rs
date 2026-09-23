//! Fixed signed-record mechanics shared by nominal protocol adapters.
use crate::{
    attestation::{
        CompilerExecutionAttestationErrorV1 as Error, Reader, decode_header_version,
        derive_identity, encode_header_version, put, require_identity, require_length,
        validate_binding, validate_rollback_position, validate_verifying_key,
    },
    attestation_request_codec::{Binding, Fields as ChallengeFields, REQUEST_BYTES},
};
use ed25519_dalek::{Signature, Signer, SigningKey};
use sha2::{Digest, Sha256};

pub(crate) const SIGNED_BYTES: usize = 304;
pub(crate) const PREIMAGE_BYTES: usize = SIGNED_BYTES + 64;
pub(crate) const BYTES: usize = PREIMAGE_BYTES + 32;
pub(crate) struct Schema {
    magic: [u8; 8],
    version: u16,
    identity_domain: &'static [u8],
    signature_domain: &'static [u8],
    rollback_domain: &'static [u8],
}
pub(crate) const V1: Schema = Schema {
    magic: *b"F2O3CER1",
    version: 1,
    identity_domain: b"FE2O3/COMPILER-EXECUTION-RECEIPT/V1\0",
    signature_domain: b"FE2O3/COMPILER-EXECUTION-RECEIPT-SIGNATURE/V1\0",
    rollback_domain: b"FE2O3/COMPILER-EXECUTION-ROLLBACK-ANCHOR/V1\0",
};
pub(crate) const V2: Schema = Schema {
    magic: *b"F2O3CER2",
    version: 2,
    identity_domain: b"FE2O3/COMPILER-EXECUTION-RECEIPT/V2\0",
    signature_domain: b"FE2O3/COMPILER-EXECUTION-RECEIPT-SIGNATURE/V2\0",
    rollback_domain: b"FE2O3/COMPILER-EXECUTION-ROLLBACK-ANCHOR/V2\0",
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct Fields {
    pub request_sha256: [u8; 32],
    pub request_byte_len: u64,
    pub policy_identity: [u8; 32],
    pub subject: Binding,
    pub challenge_identity: [u8; 32],
    pub nonce: [u8; 32],
    pub sequence: u64,
    pub prior_rollback_anchor: [u8; 32],
    pub next_rollback_anchor: [u8; 32],
    pub verifying_key: [u8; 32],
}
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Record {
    pub fields: Fields,
    pub signature: [u8; 64],
    pub identity: [u8; 32],
    pub bytes: [u8; BYTES],
}
pub(crate) struct ExpectedInput<'a> {
    pub policy_identity: [u8; 32],
    pub verifying_key: [u8; 32],
    pub request_identity: [u8; 32],
    pub subject: Binding,
    pub challenge: &'a ChallengeFields,
    pub challenge_identity: [u8; 32],
}
impl Schema {
    pub fn expected(&self, input: ExpectedInput<'_>) -> Result<Fields, Error> {
        if input.challenge.policy != input.policy_identity {
            return Err(Error::PolicyMismatch);
        }
        let mut fields = Fields {
            request_sha256: input.request_identity,
            request_byte_len: REQUEST_BYTES as u64,
            policy_identity: input.policy_identity,
            subject: input.subject,
            challenge_identity: input.challenge_identity,
            nonce: input.challenge.nonce,
            sequence: input.challenge.sequence,
            prior_rollback_anchor: input.challenge.prior,
            next_rollback_anchor: [0; 32],
            verifying_key: input.verifying_key,
        };
        fields.next_rollback_anchor = self.next_anchor(&fields);
        require_identity(fields.next_rollback_anchor, "next rollback anchor")?;
        Ok(fields)
    }
    pub fn issue(&self, fields: Fields, key: &SigningKey) -> Result<Record, Error> {
        let mut bytes = self.prefix(&fields);
        let message = derive_identity(self.signature_domain, &bytes[..SIGNED_BYTES]);
        let signature = key.sign(&message).to_bytes();
        bytes[SIGNED_BYTES..PREIMAGE_BYTES].copy_from_slice(&signature);
        self.finish(fields, signature, bytes)
    }
    pub fn decode(&self, bytes: &[u8]) -> Result<Record, Error> {
        require_length(bytes, BYTES, "receipt")?;
        let mut reader = Reader::new(bytes);
        decode_header_version(&mut reader, self.magic, self.version, BYTES, "receipt")?;
        let request_sha256 = reader.fixed()?;
        let request_byte_len = reader.u64()?;
        validate_binding(request_sha256, request_byte_len, "attestation request")?;
        if request_byte_len != REQUEST_BYTES as u64 {
            return Err(Error::RequestLengthMismatch);
        }
        let fields = Fields {
            request_sha256,
            request_byte_len,
            policy_identity: reader.fixed()?,
            subject: Binding::new(reader.fixed()?, reader.u64()?)?,
            challenge_identity: reader.fixed()?,
            nonce: reader.fixed()?,
            sequence: reader.u64()?,
            prior_rollback_anchor: reader.fixed()?,
            next_rollback_anchor: reader.fixed()?,
            verifying_key: reader.fixed()?,
        };
        let signature = reader.fixed()?;
        let declared_identity = reader.fixed::<32>()?;
        let mut canonical = self.prefix(&fields);
        canonical[SIGNED_BYTES..PREIMAGE_BYTES].copy_from_slice(&signature);
        let decoded = self.finish(fields, signature, canonical)?;
        if decoded.identity != declared_identity || decoded.bytes.as_slice() != bytes {
            return Err(Error::IdentityMismatch("receipt"));
        }
        Ok(decoded)
    }
    pub fn verify_signature(&self, record: &Record) -> Result<(), Error> {
        let key = validate_verifying_key(record.fields.verifying_key)?;
        let message = derive_identity(self.signature_domain, &record.bytes[..SIGNED_BYTES]);
        key.verify_strict(&message, &Signature::from_bytes(&record.signature))
            .map_err(|_| Error::SignatureRejected)
    }
    pub fn matches(&self, identity: [u8; 32], bytes: &[u8]) -> bool {
        bytes.len() == BYTES
            && bytes[PREIMAGE_BYTES..] == identity
            && derive_identity(self.identity_domain, &bytes[..PREIMAGE_BYTES]) == identity
    }
    fn finish(
        &self,
        fields: Fields,
        signature: [u8; 64],
        mut bytes: [u8; BYTES],
    ) -> Result<Record, Error> {
        validate_verifying_key(fields.verifying_key)?;
        validate_rollback_position(fields.sequence, fields.prior_rollback_anchor)?;
        if fields.next_rollback_anchor != self.next_anchor(&fields) {
            return Err(Error::RollbackTransitionMismatch);
        }
        let identity = derive_identity(self.identity_domain, &bytes[..PREIMAGE_BYTES]);
        bytes[PREIMAGE_BYTES..].copy_from_slice(&identity);
        let record = Record {
            fields,
            signature,
            identity,
            bytes,
        };
        self.verify_signature(&record)?;
        Ok(record)
    }
    fn prefix(&self, fields: &Fields) -> [u8; BYTES] {
        let mut bytes = [0; BYTES];
        let mut offset = encode_header_version(&mut bytes, self.magic, self.version);
        put(&mut bytes, &mut offset, &fields.request_sha256);
        put(
            &mut bytes,
            &mut offset,
            &fields.request_byte_len.to_le_bytes(),
        );
        put(&mut bytes, &mut offset, &fields.policy_identity);
        put(&mut bytes, &mut offset, &fields.subject.sha256);
        put(
            &mut bytes,
            &mut offset,
            &fields.subject.byte_len.to_le_bytes(),
        );
        put(&mut bytes, &mut offset, &fields.challenge_identity);
        put(&mut bytes, &mut offset, &fields.nonce);
        put(&mut bytes, &mut offset, &fields.sequence.to_le_bytes());
        put(&mut bytes, &mut offset, &fields.prior_rollback_anchor);
        put(&mut bytes, &mut offset, &fields.next_rollback_anchor);
        put(&mut bytes, &mut offset, &fields.verifying_key);
        debug_assert_eq!(offset, SIGNED_BYTES);
        bytes
    }
    fn next_anchor(&self, fields: &Fields) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(self.rollback_domain);
        digest.update(fields.sequence.to_le_bytes());
        digest.update(fields.prior_rollback_anchor);
        digest.update(fields.request_sha256);
        digest.update(fields.subject.sha256);
        digest.update(fields.subject.byte_len.to_le_bytes());
        digest.update(fields.nonce);
        digest.update(fields.policy_identity);
        digest.finalize().into()
    }
}

// Signature verification and fallible expected-field construction precede this
// ordered comparison. Keep separate to freeze that diagnostic ordering.
pub(crate) fn compare(actual: &Fields, expected: &Fields, current: [u8; 32]) -> Result<(), Error> {
    if actual.policy_identity != expected.policy_identity
        || actual.verifying_key != expected.verifying_key
    {
        return Err(Error::PolicyMismatch);
    }
    if actual.subject != expected.subject {
        return Err(Error::SubjectMismatch);
    }
    if actual.sequence != expected.sequence {
        return Err(Error::SequenceMismatch);
    }
    if actual.challenge_identity != expected.challenge_identity || actual.nonce != expected.nonce {
        return Err(Error::ChallengeMismatch);
    }
    if actual.prior_rollback_anchor != current
        || actual.prior_rollback_anchor != expected.prior_rollback_anchor
    {
        return Err(Error::RollbackAnchorMismatch);
    }
    if actual.next_rollback_anchor != expected.next_rollback_anchor {
        return Err(Error::RollbackTransitionMismatch);
    }
    if actual.request_sha256 != expected.request_sha256
        || actual.request_byte_len != expected.request_byte_len
    {
        return Err(Error::RequestMismatch);
    }
    Ok(())
}
