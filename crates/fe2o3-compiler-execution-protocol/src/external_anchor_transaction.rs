#![forbid(unsafe_code)]

use std::{error::Error, fmt};

use fe2o3_external_anchor_protocol::{TransactionDigestV1, derive_transaction_digest_v1};
use sha2::{Digest, Sha256};

use crate::{
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1, COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1, CompilerExecutionAttestationErrorV1,
    CompilerExecutionAttestationRequestV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptPublicationErrorV1, CompilerExecutionReceiptPublicationV1,
};

const SHA256_BYTES: usize = 32;
const HEADER_BYTES: usize = 24;
const VERSION_V1: u16 = 1;
const TRANSACTION_MAGIC: [u8; 8] = *b"F2O3CAT1";
const TRANSACTION_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-TRANSACTION/V1\0";

const TRANSACTION_PREIMAGE_BYTES: usize = HEADER_BYTES
    + COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1
    + COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1
    + COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1
    + 8
    + SHA256_BYTES
    + SHA256_BYTES;

/// Exact canonical byte length of one compiler Worker external-anchor transaction.
pub const COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1: usize =
    TRANSACTION_PREIMAGE_BYTES + SHA256_BYTES;

/// Domain-separated identity of one canonical compiler Worker anchor transaction.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionExternalAnchorTransactionIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionExternalAnchorTransactionIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        bytes.len() == COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1
            && bytes[TRANSACTION_PREIMAGE_BYTES..] == self.0
            && transaction_identity(&bytes[..TRANSACTION_PREIMAGE_BYTES]) == self.0
    }
}

impl fmt::Debug for CompilerExecutionExternalAnchorTransactionIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionExternalAnchorTransactionIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Exact authority-free input committed by the external compiler Worker anchor.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionExternalAnchorTransactionV1 {
    policy: CompilerExecutionIssuerPolicyV1,
    request: CompilerExecutionAttestationRequestV1,
    publication: CompilerExecutionReceiptPublicationV1,
    sequence: u64,
    prior_rollback_anchor: [u8; SHA256_BYTES],
    current_rollback_anchor: [u8; SHA256_BYTES],
    identity: CompilerExecutionExternalAnchorTransactionIdentityV1,
    canonical_bytes: [u8; COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1],
}

impl CompilerExecutionExternalAnchorTransactionV1 {
    /// Constructs the sole external-anchor transaction for one signed compiler publication.
    pub fn new(
        policy: CompilerExecutionIssuerPolicyV1,
        request: CompilerExecutionAttestationRequestV1,
        publication: CompilerExecutionReceiptPublicationV1,
    ) -> Result<Self, CompilerExecutionExternalAnchorTransactionErrorV1> {
        if request.challenge().policy_identity() != policy.identity()
            || publication.policy_identity() != policy.identity()
        {
            return Err(CompilerExecutionExternalAnchorTransactionErrorV1::PolicyMismatch);
        }

        let receipt = publication.receipt();
        let sequence = request.challenge().sequence();
        if receipt.sequence() != sequence {
            return Err(CompilerExecutionExternalAnchorTransactionErrorV1::SequenceMismatch);
        }
        let prior_rollback_anchor = request.challenge().prior_rollback_anchor();
        if receipt.prior_rollback_anchor() != prior_rollback_anchor {
            return Err(CompilerExecutionExternalAnchorTransactionErrorV1::RollbackAnchorMismatch);
        }
        receipt
            .clone()
            .verify(&policy, &request, prior_rollback_anchor)?;
        let current_rollback_anchor = receipt.next_rollback_anchor();

        let mut canonical_bytes = [0_u8; COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1];
        let mut offset = encode_header(&mut canonical_bytes);
        put(&mut canonical_bytes, &mut offset, policy.canonical_bytes());
        put(&mut canonical_bytes, &mut offset, request.canonical_bytes());
        put(
            &mut canonical_bytes,
            &mut offset,
            publication.canonical_bytes(),
        );
        put(&mut canonical_bytes, &mut offset, &sequence.to_le_bytes());
        put(&mut canonical_bytes, &mut offset, &prior_rollback_anchor);
        put(&mut canonical_bytes, &mut offset, &current_rollback_anchor);
        debug_assert_eq!(offset, TRANSACTION_PREIMAGE_BYTES);
        let identity = CompilerExecutionExternalAnchorTransactionIdentityV1(transaction_identity(
            &canonical_bytes[..offset],
        ));
        put(&mut canonical_bytes, &mut offset, identity.as_bytes());
        debug_assert_eq!(offset, canonical_bytes.len());

        Ok(Self {
            policy,
            request,
            publication,
            sequence,
            prior_rollback_anchor,
            current_rollback_anchor,
            identity,
            canonical_bytes,
        })
    }

    /// Strictly decodes and revalidates one complete canonical transaction.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionExternalAnchorTransactionErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1 {
            return Err(
                CompilerExecutionExternalAnchorTransactionErrorV1::InvalidLength {
                    expected: COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1,
                    actual: bytes.len(),
                },
            );
        }
        let mut reader = Reader::new(bytes);
        decode_header(&mut reader)?;
        let policy = CompilerExecutionIssuerPolicyV1::decode(
            reader.take(COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1)?,
        )?;
        let request = CompilerExecutionAttestationRequestV1::decode(
            reader.take(COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V1)?,
        )?;
        let publication = CompilerExecutionReceiptPublicationV1::decode(
            reader.take(COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V1)?,
        )?;
        let sequence = reader.u64()?;
        let prior_rollback_anchor = reader.fixed::<SHA256_BYTES>()?;
        let current_rollback_anchor = reader.fixed::<SHA256_BYTES>()?;
        let declared_identity =
            CompilerExecutionExternalAnchorTransactionIdentityV1(reader.fixed::<SHA256_BYTES>()?);
        if !reader.is_empty() {
            return Err(
                CompilerExecutionExternalAnchorTransactionErrorV1::InvalidEncoding(
                    "transaction has trailing bytes",
                ),
            );
        }

        let decoded = Self::new(policy, request, publication)?;
        if decoded.sequence != sequence
            || decoded.prior_rollback_anchor != prior_rollback_anchor
            || decoded.current_rollback_anchor != current_rollback_anchor
        {
            return Err(
                CompilerExecutionExternalAnchorTransactionErrorV1::RollbackPositionMismatch,
            );
        }
        if decoded.identity != declared_identity
            || decoded.canonical_bytes.as_slice() != bytes
            || !declared_identity.matches_canonical_bytes(bytes)
        {
            return Err(CompilerExecutionExternalAnchorTransactionErrorV1::IdentityMismatch);
        }
        Ok(decoded)
    }

    pub const fn policy(&self) -> &CompilerExecutionIssuerPolicyV1 {
        &self.policy
    }

    pub const fn request(&self) -> &CompilerExecutionAttestationRequestV1 {
        &self.request
    }

    pub const fn publication(&self) -> &CompilerExecutionReceiptPublicationV1 {
        &self.publication
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn prior_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.prior_rollback_anchor
    }

    pub const fn current_rollback_anchor(&self) -> [u8; SHA256_BYTES] {
        self.current_rollback_anchor
    }

    pub const fn identity(&self) -> CompilerExecutionExternalAnchorTransactionIdentityV1 {
        self.identity
    }

    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1] {
        &self.canonical_bytes
    }

    /// Derives the digest carried by the independent external-anchor protocol.
    pub fn external_anchor_digest(&self) -> TransactionDigestV1 {
        derive_transaction_digest_v1(&self.canonical_bytes)
            .expect("the fixed compiler anchor transaction fits the protocol bound")
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for CompilerExecutionExternalAnchorTransactionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionExternalAnchorTransactionV1")
            .field("sequence", &self.sequence)
            .field("prior_rollback_anchor", &self.prior_rollback_anchor)
            .field("current_rollback_anchor", &self.current_rollback_anchor)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionExternalAnchorTransactionErrorV1 {
    InvalidLength { expected: usize, actual: usize },
    InvalidEncoding(&'static str),
    PolicyMismatch,
    SequenceMismatch,
    RollbackAnchorMismatch,
    RollbackPositionMismatch,
    IdentityMismatch,
    Attestation(CompilerExecutionAttestationErrorV1),
    Publication(CompilerExecutionReceiptPublicationErrorV1),
}

impl fmt::Display for CompilerExecutionExternalAnchorTransactionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { expected, actual } => write!(
                formatter,
                "compiler anchor transaction length mismatch: expected {expected}, got {actual}"
            ),
            Self::InvalidEncoding(reason) => {
                write!(formatter, "invalid compiler anchor transaction: {reason}")
            }
            Self::PolicyMismatch => formatter.write_str("compiler anchor policy mismatch"),
            Self::SequenceMismatch => formatter.write_str("compiler anchor sequence mismatch"),
            Self::RollbackAnchorMismatch => {
                formatter.write_str("compiler anchor prior rollback mismatch")
            }
            Self::RollbackPositionMismatch => {
                formatter.write_str("compiler anchor rollback position mismatch")
            }
            Self::IdentityMismatch => formatter.write_str("compiler anchor identity mismatch"),
            Self::Attestation(error) => write!(formatter, "compiler attestation: {error}"),
            Self::Publication(error) => write!(formatter, "compiler publication: {error}"),
        }
    }
}

impl Error for CompilerExecutionExternalAnchorTransactionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Attestation(error) => Some(error),
            Self::Publication(error) => Some(error),
            _ => None,
        }
    }
}

impl From<CompilerExecutionAttestationErrorV1>
    for CompilerExecutionExternalAnchorTransactionErrorV1
{
    fn from(error: CompilerExecutionAttestationErrorV1) -> Self {
        Self::Attestation(error)
    }
}

impl From<CompilerExecutionReceiptPublicationErrorV1>
    for CompilerExecutionExternalAnchorTransactionErrorV1
{
    fn from(error: CompilerExecutionReceiptPublicationErrorV1) -> Self {
        Self::Publication(error)
    }
}

fn encode_header(output: &mut [u8]) -> usize {
    let mut offset = 0;
    put(output, &mut offset, &TRANSACTION_MAGIC);
    put(output, &mut offset, &VERSION_V1.to_le_bytes());
    put(output, &mut offset, &0_u16.to_le_bytes());
    put(
        output,
        &mut offset,
        &(COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1 as u64).to_le_bytes(),
    );
    put(output, &mut offset, &0_u32.to_le_bytes());
    offset
}

fn decode_header(
    reader: &mut Reader<'_>,
) -> Result<(), CompilerExecutionExternalAnchorTransactionErrorV1> {
    if reader.fixed::<8>()? != TRANSACTION_MAGIC {
        return Err(
            CompilerExecutionExternalAnchorTransactionErrorV1::InvalidEncoding(
                "transaction magic mismatch",
            ),
        );
    }
    if reader.u16()? != VERSION_V1 {
        return Err(
            CompilerExecutionExternalAnchorTransactionErrorV1::InvalidEncoding(
                "transaction version mismatch",
            ),
        );
    }
    if reader.u16()? != 0 {
        return Err(
            CompilerExecutionExternalAnchorTransactionErrorV1::InvalidEncoding(
                "transaction reserved field is nonzero",
            ),
        );
    }
    if reader.u64()? != COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V1 as u64 {
        return Err(
            CompilerExecutionExternalAnchorTransactionErrorV1::InvalidEncoding(
                "transaction declared length mismatch",
            ),
        );
    }
    if reader.u32()? != 0 {
        return Err(
            CompilerExecutionExternalAnchorTransactionErrorV1::InvalidEncoding(
                "transaction reserved tail is nonzero",
            ),
        );
    }
    Ok(())
}

fn transaction_identity(preimage: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(TRANSACTION_IDENTITY_DOMAIN);
    digest.update(preimage);
    digest.finalize().into()
}

fn put(output: &mut [u8], offset: &mut usize, bytes: &[u8]) {
    let end = *offset + bytes.len();
    output[*offset..end].copy_from_slice(bytes);
    *offset = end;
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], CompilerExecutionExternalAnchorTransactionErrorV1> {
        let end = self.offset.checked_add(length).ok_or(
            CompilerExecutionExternalAnchorTransactionErrorV1::InvalidEncoding(
                "transaction offset overflow",
            ),
        )?;
        let value = self.bytes.get(self.offset..end).ok_or(
            CompilerExecutionExternalAnchorTransactionErrorV1::InvalidEncoding(
                "transaction is truncated",
            ),
        )?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], CompilerExecutionExternalAnchorTransactionErrorV1> {
        self.take(N)?.try_into().map_err(|_| {
            CompilerExecutionExternalAnchorTransactionErrorV1::InvalidEncoding(
                "transaction fixed field has the wrong length",
            )
        })
    }

    fn u16(&mut self) -> Result<u16, CompilerExecutionExternalAnchorTransactionErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, CompilerExecutionExternalAnchorTransactionErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, CompilerExecutionExternalAnchorTransactionErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
