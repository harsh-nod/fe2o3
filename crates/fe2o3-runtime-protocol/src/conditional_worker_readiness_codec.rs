//! Canonical bounded framing for conditional V5 replay custody, not admission.
use fe2o3_artifact_transaction::{
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V5, MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V3,
    MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
    MAX_WORKER_V3_LOAD_ENVELOPE_CUSTODY_BYTES_V2, MAX_WORKER_V3_PUBLICATION_INTENT_RECORD_BYTES_V1,
    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
    MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1,
};
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V3;
use sha2::{Digest, Sha256};
use std::fmt;

pub const CONDITIONAL_WORKER_READINESS_MAGIC_V5: [u8; 8] = *b"F3CENV05";
pub const MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5: usize =
    MAX_WORKER_V3_LOAD_ENVELOPE_CUSTODY_BYTES_V2;
const DOMAIN: &[u8] = b"FE2O3/CONDITIONAL-WORKER-READINESS/V5\0";
const HEADER: usize = 44;
const CHECKSUM: usize = 32;
const LIMITS: [usize; 5] = [
    MAX_WORKER_V3_PUBLICATION_INTENT_RECORD_BYTES_V1,
    MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES_V3,
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V5,
    MAX_WORKER_V3_FINALIZER_REPLAY_TRANSCRIPT_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V3,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionalWorkerReadinessCodecErrorV5 {
    Length,
    Limit,
    Header,
    Checksum,
    Provider,
    Trailing,
    Allocation,
}
type Error = ConditionalWorkerReadinessCodecErrorV5;
type Result<T> = std::result::Result<T, Error>;
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conditional V5 readiness framing: {self:?}")
    }
}
impl std::error::Error for Error {}

/// An inert, zero-allocation view of complete canonical framing. Nested record,
/// claim, handoff, transcript and carriage bytes are NOT authenticated or
/// semantically decoded by this view. Only the separate actual-owner envelope
/// may establish replay custody. No load or launch authority is produced.
pub struct InertConditionalWorkerReadinessWireV5<'a> {
    fields: [&'a [u8]; 5],
    providers: &'a [u8],
    count: usize,
}
impl<'a> InertConditionalWorkerReadinessWireV5<'a> {
    pub fn decode(bytes: &'a [u8], maximum: usize) -> Result<Self> {
        if bytes.len() < HEADER + CHECKSUM + 5
            || bytes.len() > maximum.min(MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5)
        {
            return Err(Error::Limit);
        }
        let (body, tail) = bytes.split_at(bytes.len() - CHECKSUM);
        let mut reader = Reader(body);
        if reader.take(8)? != CONDITIONAL_WORKER_READINESS_MAGIC_V5
            || reader.take(2)? != 5_u16.to_le_bytes()
            || reader.take(2)? != [0, 0]
        {
            return Err(Error::Header);
        }
        if reader.u64()? != bytes.len() as u64 {
            return Err(Error::Length);
        }
        let mut lengths = [0; 5];
        for (length, limit) in lengths.iter_mut().zip(LIMITS) {
            *length = reader.u32()?;
            require_length(*length, limit)?;
        }
        let count = reader.u32()?;
        if count > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1 {
            return Err(Error::Provider);
        }
        let mut fields = [&[][..]; 5];
        for (field, length) in fields.iter_mut().zip(lengths) {
            *field = reader.take(length)?;
        }
        let providers = reader.0;
        let mut payload_bytes = 0;
        for _ in 0..count {
            let length = reader.u32()?;
            require_length(length, MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1)?;
            payload_bytes = checked_add(payload_bytes, length)?;
            if payload_bytes > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1 {
                return Err(Error::Provider);
            }
            reader.take(length)?;
        }
        if !reader.0.is_empty() {
            return Err(Error::Trailing);
        }
        if checksum(body).as_slice() != tail {
            return Err(Error::Checksum);
        }
        Ok(Self {
            fields,
            providers,
            count,
        })
    }
    pub fn record_bytes(&self) -> &'a [u8] {
        self.fields[0]
    }
    pub fn claim_bytes(&self) -> &'a [u8] {
        self.fields[1]
    }
    pub fn outer_handoff_bytes(&self) -> &'a [u8] {
        self.fields[2]
    }
    pub fn transcript_bytes(&self) -> &'a [u8] {
        self.fields[3]
    }
    pub fn compiler_execution_bytes(&self) -> &'a [u8] {
        self.fields[4]
    }
    pub fn providers(&self) -> impl ExactSizeIterator<Item = &'a [u8]> {
        Providers {
            remaining: self.providers,
            count: self.count,
        }
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

pub(super) fn encoded_length(
    lengths: [usize; 5],
    count: usize,
    provider_bytes: usize,
) -> Result<usize> {
    if count > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_PAYLOADS_V1
        || provider_bytes > MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1
        || (count == 0) != (provider_bytes == 0)
    {
        return Err(Error::Provider);
    }
    let mut total = checked_add(HEADER, CHECKSUM)?;
    for (length, limit) in lengths.into_iter().zip(LIMITS) {
        require_length(length, limit)?;
        total = checked_add(total, length)?;
    }
    total = checked_add(total, count.checked_mul(4).ok_or(Error::Length)?)?;
    total = checked_add(total, provider_bytes)?;
    if total > MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 {
        return Err(Error::Limit);
    }
    Ok(total)
}

pub(super) fn encode<'a>(
    fields: [&[u8]; 5],
    providers: impl ExactSizeIterator<Item = &'a [u8]> + Clone,
) -> Result<Vec<u8>> {
    let mut provider_bytes = 0;
    for payload in providers.clone() {
        require_length(
            payload.len(),
            MAX_WORKER_V3_REPLAY_EXTERNAL_PROVIDER_BYTES_V1,
        )?;
        provider_bytes = checked_add(provider_bytes, payload.len())?;
    }
    let total = encoded_length(fields.map(<[u8]>::len), providers.len(), provider_bytes)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| Error::Allocation)?;
    if bytes.capacity() > MAX_CONDITIONAL_WORKER_READINESS_BYTES_V5 {
        return Err(Error::Limit);
    }
    bytes.extend_from_slice(&CONDITIONAL_WORKER_READINESS_MAGIC_V5);
    bytes.extend_from_slice(&5_u16.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(total as u64).to_le_bytes());
    for field in fields {
        bytes.extend_from_slice(&(field.len() as u32).to_le_bytes());
    }
    bytes.extend_from_slice(&(providers.len() as u32).to_le_bytes());
    for field in fields {
        bytes.extend_from_slice(field);
    }
    for payload in providers {
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
    }
    let hash = checksum(&bytes);
    bytes.extend_from_slice(&hash);
    if bytes.len() != total {
        return Err(Error::Length);
    }
    Ok(bytes)
}

fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(DOMAIN);
    h.update(bytes);
    h.finalize().into()
}
fn require_length(length: usize, limit: usize) -> Result<()> {
    if length == 0 || length > limit || u32::try_from(length).is_err() {
        return Err(Error::Limit);
    }
    Ok(())
}
fn checked_add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b).ok_or(Error::Length)
}
struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let bytes = self.0.get(..n).ok_or(Error::Length)?;
        self.0 = &self.0[n..];
        Ok(bytes)
    }
    fn u32(&mut self) -> Result<usize> {
        usize::try_from(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| Error::Length)?,
        ))
        .map_err(|_| Error::Length)
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| Error::Length)?,
        ))
    }
}
struct Providers<'a> {
    remaining: &'a [u8],
    count: usize,
}
impl<'a> Iterator for Providers<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        let mut reader = Reader(self.remaining);
        let length = reader.u32().ok()?;
        let value = reader.take(length).ok()?;
        self.remaining = reader.0;
        self.count -= 1;
        Some(value)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.count, Some(self.count))
    }
}
impl ExactSizeIterator for Providers<'_> {}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        encode(
            [b"record", b"claim", b"outer", b"transcript", b"carriage"],
            [b"one".as_slice(), b"two"].into_iter(),
        )
        .unwrap()
    }
    fn reseal(bytes: &mut [u8]) {
        let end = bytes.len() - CHECKSUM;
        let hash = checksum(&bytes[..end]);
        bytes[end..].copy_from_slice(&hash);
    }
    #[test]
    fn canonical_framing_preserves_all_preimages_without_allocating_decode() {
        let bytes = fixture();
        let wire = InertConditionalWorkerReadinessWireV5::decode(&bytes, bytes.len()).unwrap();
        assert_eq!(wire.record_bytes(), b"record");
        assert_eq!(wire.claim_bytes(), b"claim");
        assert_eq!(wire.outer_handoff_bytes(), b"outer");
        assert_eq!(wire.transcript_bytes(), b"transcript");
        assert_eq!(wire.compiler_execution_bytes(), b"carriage");
        assert_eq!(wire.providers().collect::<Vec<_>>(), [b"one", b"two"]);
        assert!(!wire.grants_load_authority() && !wire.grants_launch_authority());
        assert!(InertConditionalWorkerReadinessWireV5::decode(&bytes, bytes.len() - 1).is_err());
    }
    #[test]
    fn every_truncation_and_noncanonical_header_is_rejected() {
        let bytes = fixture();
        for length in 0..bytes.len() {
            assert!(
                InertConditionalWorkerReadinessWireV5::decode(&bytes[..length], usize::MAX)
                    .is_err()
            );
        }
        for offset in [0, 8, 10, 12, 20, 24, 28, 32, 36, 40] {
            let mut altered = bytes.clone();
            altered[offset] ^= 128;
            reseal(&mut altered);
            assert!(InertConditionalWorkerReadinessWireV5::decode(&altered, usize::MAX).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.insert(HEADER, 0);
        let length = trailing.len() as u64;
        trailing[12..20].copy_from_slice(&length.to_le_bytes());
        reseal(&mut trailing);
        assert!(InertConditionalWorkerReadinessWireV5::decode(&trailing, usize::MAX).is_err());
    }
    #[test]
    fn payload_tampering_empty_providers_and_overflows_are_rejected() {
        let mut bytes = fixture();
        bytes[HEADER] ^= 1;
        assert!(matches!(
            InertConditionalWorkerReadinessWireV5::decode(&bytes, usize::MAX),
            Err(Error::Checksum)
        ));
        let fields = [b"x".as_slice(); 5];
        assert!(encode(fields, [b"".as_slice()].into_iter()).is_err());
        assert!(encoded_length([usize::MAX; 5], 0, 0).is_err());
        assert!(encoded_length([1; 5], 128, 128).is_err());
        assert!(encoded_length([1; 5], 0, 1).is_err());
        let empty = encode(fields, [].into_iter()).unwrap();
        assert_eq!(
            InertConditionalWorkerReadinessWireV5::decode(&empty, usize::MAX)
                .unwrap()
                .providers()
                .len(),
            0
        );
    }
}
