use fe2o3_artifact_transaction::{
    DurablePublishedHsacoClaimV1, MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES,
};
use fe2o3_artifacts::{
    ArtifactContainerV1, BundleIndexV1, DigestAlgorithm, DigestBytes, DirectLinkBundleEvidenceV1,
    MAX_BUNDLE_INDEX_BYTES, MAX_CONTAINER_BYTES, MAX_DIRECT_LINK_EVIDENCE_BYTES, MAX_KERNELS,
    MAX_PROOF_RECORD_BYTES, PayloadDigest, ProofRecordV1,
};
use fe2o3_kernel_descriptor::{MAX_DESCRIPTOR_TABLE_BYTES, decode_device_descriptor_table_v1};

use crate::{
    DescriptorLineageV1, EnvelopeDecodeError, ExactRawHsacoV1, MAX_WORKER_V2_LOAD_ENVELOPE_BYTES,
    MAX_WORKER_V2_PROOF_EVIDENCE_BYTES, MAX_WORKER_V2_RAW_HSACO_BYTES, WorkerV2LoadEnvelopeV1,
};

pub const WORKER_V2_LOAD_ENVELOPE_MAGIC: [u8; 8] = *b"FE2W2B1\0";
pub const WORKER_V2_LOAD_ENVELOPE_VERSION: u16 = 1;
const FIXED_HEADER_BYTES: usize = 77;

impl WorkerV2LoadEnvelopeV1 {
    pub fn to_bytes(&self) -> Vec<u8> {
        let container = self.container().to_bytes();
        let bundle = self.bundle_index().to_bytes();
        let direct_link = self.direct_link_evidence().to_bytes();
        let descriptor = self.descriptor_lineage().canonical_bytes();
        let published_claim = self
            .published_claim()
            .encode_canonical()
            .expect("validated publication claim must encode canonically");
        let proofs = self
            .proof_records()
            .iter()
            .map(ProofRecordV1::to_bytes)
            .collect::<Vec<_>>();
        let total_len = FIXED_HEADER_BYTES
            + container.len()
            + bundle.len()
            + direct_link.len()
            + descriptor.len()
            + published_claim.len()
            + proofs.iter().map(|proof| 4 + proof.len()).sum::<usize>()
            + self.raw_hsaco().bytes().len();
        let mut writer = Writer::with_capacity(total_len);
        writer.bytes(&WORKER_V2_LOAD_ENVELOPE_MAGIC);
        writer.u16(WORKER_V2_LOAD_ENVELOPE_VERSION);
        writer.u16(0);
        writer.u32(total_len as u32);
        writer.u32(container.len() as u32);
        writer.u32(bundle.len() as u32);
        writer.u32(direct_link.len() as u32);
        writer.u32(descriptor.len() as u32);
        writer.u32(self.raw_hsaco().bytes().len() as u32);
        writer.u16(proofs.len() as u16);
        writer.u16(0);
        writer.u16(published_claim.len() as u16);
        writer.u16(0);
        writer.payload_digest(self.raw_hsaco().identity());
        debug_assert_eq!(writer.bytes.len(), FIXED_HEADER_BYTES);
        writer.bytes(&container);
        writer.bytes(&bundle);
        writer.bytes(&direct_link);
        writer.bytes(&descriptor);
        writer.bytes(&published_claim);
        for proof in proofs {
            writer.u32(proof.len() as u32);
            writer.bytes(&proof);
        }
        writer.bytes(self.raw_hsaco().bytes());
        debug_assert_eq!(writer.bytes.len(), total_len);
        debug_assert!(writer.bytes.len() <= MAX_WORKER_V2_LOAD_ENVELOPE_BYTES);
        writer.bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, EnvelopeDecodeError> {
        if bytes.len() > MAX_WORKER_V2_LOAD_ENVELOPE_BYTES {
            return Err(EnvelopeDecodeError::TooLarge {
                max: MAX_WORKER_V2_LOAD_ENVELOPE_BYTES,
            });
        }
        let mut reader = Reader::new(bytes);
        if reader.array::<8>()? != WORKER_V2_LOAD_ENVELOPE_MAGIC {
            return Err(EnvelopeDecodeError::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != WORKER_V2_LOAD_ENVELOPE_VERSION {
            return Err(EnvelopeDecodeError::UnknownVersion(version));
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(EnvelopeDecodeError::UnsupportedFlags(flags));
        }
        let total_len = reader.length_u32("envelope", MAX_WORKER_V2_LOAD_ENVELOPE_BYTES)?;
        if total_len > bytes.len() {
            return Err(EnvelopeDecodeError::Truncated);
        }
        if total_len < bytes.len() {
            return Err(EnvelopeDecodeError::TrailingBytes);
        }
        let container_len = reader.length_u32("artifact container", MAX_CONTAINER_BYTES)?;
        let bundle_len = reader.length_u32("bundle index", MAX_BUNDLE_INDEX_BYTES)?;
        let direct_link_len =
            reader.length_u32("direct-link evidence", MAX_DIRECT_LINK_EVIDENCE_BYTES)?;
        let descriptor_len = reader.length_u32("descriptor lineage", MAX_DESCRIPTOR_TABLE_BYTES)?;
        let raw_len = reader.length_u32("raw HSACO", MAX_WORKER_V2_RAW_HSACO_BYTES)?;
        let proof_count = usize::from(reader.u16()?);
        if proof_count > MAX_KERNELS {
            return Err(EnvelopeDecodeError::CountOutOfRange {
                field: "proof records",
                value: proof_count as u64,
                max: MAX_KERNELS,
            });
        }
        require_zero(reader.u16()?)?;
        let published_claim_len = usize::from(reader.u16()?);
        if published_claim_len > MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES {
            return Err(EnvelopeDecodeError::LengthOutOfRange {
                field: "published claim",
                value: published_claim_len as u64,
                max: MAX_DURABLE_PUBLISHED_HSACO_CLAIM_BYTES,
            });
        }
        require_zero(reader.u16()?)?;
        let raw_identity = reader.payload_digest()?;

        let fixed_components = container_len
            .checked_add(bundle_len)
            .and_then(|value| value.checked_add(direct_link_len))
            .and_then(|value| value.checked_add(descriptor_len))
            .and_then(|value| value.checked_add(published_claim_len))
            .and_then(|value| value.checked_add(raw_len))
            .ok_or(EnvelopeDecodeError::LengthOverflow)?;
        if reader.remaining_len() < fixed_components {
            return Err(EnvelopeDecodeError::Truncated);
        }

        let container = ArtifactContainerV1::from_bytes(reader.take(container_len)?)
            .map_err(EnvelopeDecodeError::Container)?;
        let bundle_index = BundleIndexV1::from_bytes(reader.take(bundle_len)?)
            .map_err(EnvelopeDecodeError::Bundle)?;
        let direct_link_evidence =
            DirectLinkBundleEvidenceV1::from_bytes(reader.take(direct_link_len)?)
                .map_err(EnvelopeDecodeError::DirectLink)?;
        let descriptor_lineage = DescriptorLineageV1::new(
            decode_device_descriptor_table_v1(reader.take(descriptor_len)?)
                .map_err(EnvelopeDecodeError::Descriptor)?,
        );
        let published_claim =
            DurablePublishedHsacoClaimV1::decode_canonical(reader.take(published_claim_len)?)
                .map_err(EnvelopeDecodeError::PublishedClaim)?;

        let mut proof_records = Vec::with_capacity(proof_count);
        let mut proof_bytes = 0usize;
        for _ in 0..proof_count {
            let proof_len = reader.length_u32("proof record", MAX_PROOF_RECORD_BYTES)?;
            proof_bytes = proof_bytes
                .checked_add(proof_len)
                .ok_or(EnvelopeDecodeError::LengthOverflow)?;
            if proof_bytes > MAX_WORKER_V2_PROOF_EVIDENCE_BYTES {
                return Err(EnvelopeDecodeError::LengthOutOfRange {
                    field: "proof evidence",
                    value: proof_bytes as u64,
                    max: MAX_WORKER_V2_PROOF_EVIDENCE_BYTES,
                });
            }
            proof_records.push(
                ProofRecordV1::from_bytes(reader.take(proof_len)?)
                    .map_err(EnvelopeDecodeError::Proof)?,
            );
        }
        let raw_hsaco = ExactRawHsacoV1::new(raw_identity, reader.take(raw_len)?.to_vec())
            .map_err(EnvelopeDecodeError::Validation)?;
        if !reader.is_empty() {
            return Err(EnvelopeDecodeError::TrailingBytes);
        }
        let envelope = Self::new(
            container,
            bundle_index,
            direct_link_evidence,
            descriptor_lineage,
            proof_records,
            raw_hsaco,
            published_claim,
        )
        .map_err(EnvelopeDecodeError::Validation)?;
        if envelope.to_bytes() != bytes {
            return Err(EnvelopeDecodeError::NonCanonical);
        }
        Ok(envelope)
    }
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            bytes: Vec::with_capacity(capacity),
        }
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn payload_digest(&mut self, value: PayloadDigest) {
        debug_assert_eq!(value.algorithm(), DigestAlgorithm::Sha256);
        self.u8(0);
        self.bytes(value.bytes().as_bytes());
    }
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    const fn remaining_len(&self) -> usize {
        self.remaining.len()
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], EnvelopeDecodeError> {
        if self.remaining.len() < count {
            return Err(EnvelopeDecodeError::Truncated);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], EnvelopeDecodeError> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, EnvelopeDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, EnvelopeDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, EnvelopeDecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn length_u32(
        &mut self,
        field: &'static str,
        max: usize,
    ) -> Result<usize, EnvelopeDecodeError> {
        let value = u64::from(self.u32()?);
        if value > max as u64 {
            Err(EnvelopeDecodeError::LengthOutOfRange { field, value, max })
        } else {
            Ok(value as usize)
        }
    }

    fn payload_digest(&mut self) -> Result<PayloadDigest, EnvelopeDecodeError> {
        let algorithm = match self.u8()? {
            0 => DigestAlgorithm::Sha256,
            tag => return Err(EnvelopeDecodeError::UnknownDigestAlgorithm(tag)),
        };
        Ok(PayloadDigest::new(
            algorithm,
            DigestBytes::from_bytes(self.array()?),
        ))
    }
}

fn require_zero(value: u16) -> Result<(), EnvelopeDecodeError> {
    if value == 0 {
        Ok(())
    } else {
        Err(EnvelopeDecodeError::NonZeroReserved(value))
    }
}
