//! Native source-bearing input to the existing external monotonic-anchor protocol.
use crate::{
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V2 as Q,
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V2 as P,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_BYTES_V2 as U,
    CompilerExecutionAttestationErrorV2 as AttestationError,
    CompilerExecutionAttestationRequestV2 as Request, CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionReceiptPublicationErrorV2 as PublicationError,
    CompilerExecutionReceiptPublicationV2 as Publication,
    CompilerExecutionServiceProtocolErrorV1 as Framing,
    attestation_resources::CompilerExecutionAttestationStorageV2 as Storage,
    external_anchor_transaction::encode_transaction,
    service::{Reader, decode_versioned_header},
};
use fe2o3_external_anchor_protocol::{
    AnchorProtocolErrorV1 as AnchorError, TransactionDigestV1, derive_transaction_digest_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

pub const COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V2: usize = 24 + P + Q + U + 8 + 96;
const N: usize = COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V2;
const MAGIC: [u8; 8] = *b"F2O3CAT2";
const DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-TRANSACTION/V2\0";

/// Fixed diagnostic family for the native transaction/currentness joins.
#[derive(Debug)]
pub enum CompilerExecutionNativeJournalErrorV2 {
    Resource(Resource),
    Attestation(AttestationError),
    Publication(PublicationError),
    Framing(Framing),
    Anchor(AnchorError),
    Current(crate::CompilerExecutionCurrentRecordVerificationErrorV3),
    Mismatch(&'static str),
}
pub(crate) type Result<T> = std::result::Result<T, CompilerExecutionNativeJournalErrorV2>;
macro_rules! from_error {
    ($ty:ty, $variant:ident) => {
        impl From<$ty> for CompilerExecutionNativeJournalErrorV2 {
            fn from(error: $ty) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(Resource, Resource);
from_error!(AttestationError, Attestation);
from_error!(PublicationError, Publication);
from_error!(Framing, Framing);
from_error!(AnchorError, Anchor);
from_error!(
    crate::CompilerExecutionCurrentRecordVerificationErrorV3,
    Current
);
impl fmt::Display for CompilerExecutionNativeJournalErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mismatch(reason) => f.write_str(reason),
            Self::Resource(e) => fmt::Display::fmt(e, f),
            Self::Attestation(e) => fmt::Display::fmt(e, f),
            Self::Publication(e) => fmt::Display::fmt(e, f),
            Self::Framing(e) => fmt::Display::fmt(e, f),
            Self::Anchor(e) => fmt::Display::fmt(e, f),
            Self::Current(e) => fmt::Display::fmt(e, f),
        }
    }
}
impl Error for CompilerExecutionNativeJournalErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Attestation(e) => Some(e),
            Self::Publication(e) => Some(e),
            Self::Framing(e) => Some(e),
            Self::Anchor(e) => Some(e),
            Self::Current(e) => Some(e),
            Self::Mismatch(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionExternalAnchorTransactionIdentityV2([u8; 32]);
impl CompilerExecutionExternalAnchorTransactionIdentityV2 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Move-only, source-bearing canonical anchor input, not durable authority.
/// It never constructs a legacy issuer policy, request or publication owner.
#[derive(Debug)]
pub struct CompilerExecutionExternalAnchorTransactionV2 {
    policy: Policy,
    request: Request,
    publication: Publication,
    canonical: [u8; N],
    identity: CompilerExecutionExternalAnchorTransactionIdentityV2,
}

impl CompilerExecutionExternalAnchorTransactionV2 {
    pub const RETAINED_STORAGE: usize = size_of::<(Self, Storage)>();
    pub const FRAME_STORAGE: usize = 4 * Self::RETAINED_STORAGE + 4 * N + 4096;
    pub const WORK: usize = 8 + 64 * N;

    /// Reconstructs the exact transaction digest without cloning native owners.
    pub(crate) fn digest_for_parts(
        policy: &Policy,
        request: &Request,
        publication: &Publication,
        b: &mut Budget<'_>,
    ) -> Result<TransactionDigestV1> {
        let floor =
            policy.retained_storage() + request.retained_storage() + publication.retained_storage();
        b.with_prepaid_scope(floor, 8, Self::WORK, Self::FRAME_STORAGE, |b| {
            publication.receipt().verify_matches(
                policy,
                request,
                request.challenge().prior_rollback_anchor(),
                b,
            )?;
            let (bytes, _) = encode_transaction::<N>(
                MAGIC,
                2,
                DOMAIN,
                [
                    policy.canonical_bytes(),
                    request.canonical_bytes(),
                    publication.canonical_bytes(),
                ],
                (
                    request.challenge().sequence(),
                    request.challenge().prior_rollback_anchor(),
                    publication.receipt().next_rollback_anchor(),
                ),
            );
            Ok(derive_transaction_digest_v1(&bytes)?)
        })
    }

    /// Consumes prepaid native children; returns only additional owner storage.
    pub fn new(
        policy: Policy,
        request: Request,
        publication: Publication,
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        let floor = policy
            .retained_storage()
            .checked_add(request.retained_storage())
            .and_then(|v| v.checked_add(publication.retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, Self::WORK, Self::FRAME_STORAGE, |b| {
            publication.receipt().verify_matches(
                &policy,
                &request,
                request.challenge().prior_rollback_anchor(),
                b,
            )?;
            let (canonical, identity) = encode_transaction(
                MAGIC,
                2,
                DOMAIN,
                [
                    policy.canonical_bytes(),
                    request.canonical_bytes(),
                    publication.canonical_bytes(),
                ],
                (
                    request.challenge().sequence(),
                    request.challenge().prior_rollback_anchor(),
                    publication.receipt().next_rollback_anchor(),
                ),
            );
            let growth = Self::RETAINED_STORAGE
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?;
            Ok((
                Self {
                    policy,
                    request,
                    publication,
                    canonical,
                    identity: CompilerExecutionExternalAnchorTransactionIdentityV2(identity),
                },
                Storage(growth),
            ))
        })
    }

    /// Reconstructs typed native children under the same original caller ledger.
    pub fn decode(bytes: &[u8], b: &mut Budget<'_>) -> Result<(Self, Storage)> {
        b.with_prepaid_scope(
            bytes.len().min(N),
            8,
            Self::WORK,
            Self::FRAME_STORAGE,
            |b| {
                if bytes.len() != N {
                    return Err(CompilerExecutionNativeJournalErrorV2::Mismatch(
                        "native anchor transaction length",
                    ));
                }
                let mut reader = Reader::new(bytes);
                if decode_versioned_header(&mut reader, MAGIC, 2, N)? != 0 {
                    return Err(CompilerExecutionNativeJournalErrorV2::Mismatch(
                        "native anchor transaction kind",
                    ));
                }
                let (policy, charge) = Policy::decode(reader.take(P)?, b)?;
                b.reserve_storage(charge.additional_storage())?;
                let (request, charge) = Request::decode(reader.take(Q)?, b)?;
                b.reserve_storage(charge.additional_storage())?;
                let (publication, charge) = Publication::decode(reader.take(U)?, b)?;
                b.reserve_storage(charge.additional_storage())?;
                let (record, _) = Self::new(policy, request, publication, b)?;
                if record.canonical.as_slice() != bytes {
                    return Err(CompilerExecutionNativeJournalErrorV2::Mismatch(
                        "native anchor transaction canonical join",
                    ));
                }
                Ok((record, Storage(Self::RETAINED_STORAGE)))
            },
        )
    }

    pub const fn policy(&self) -> &Policy {
        &self.policy
    }
    pub const fn request(&self) -> &Request {
        &self.request
    }
    pub const fn publication(&self) -> &Publication {
        &self.publication
    }
    pub const fn sequence(&self) -> u64 {
        self.request.challenge().sequence()
    }
    pub const fn prior_rollback_anchor(&self) -> [u8; 32] {
        self.request.challenge().prior_rollback_anchor()
    }
    pub const fn current_rollback_anchor(&self) -> [u8; 32] {
        self.publication.receipt().next_rollback_anchor()
    }
    pub const fn canonical_bytes(&self) -> &[u8; N] {
        &self.canonical
    }
    pub const fn identity(&self) -> CompilerExecutionExternalAnchorTransactionIdentityV2 {
        self.identity
    }
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED_STORAGE
    }
    pub fn external_anchor_digest(&self, b: &mut Budget<'_>) -> Result<TransactionDigestV1> {
        b.with_prepaid_scope(Self::RETAINED_STORAGE, 8, Self::WORK, 4096, |_| {
            Ok(derive_transaction_digest_v1(&self.canonical)?)
        })
    }
}
