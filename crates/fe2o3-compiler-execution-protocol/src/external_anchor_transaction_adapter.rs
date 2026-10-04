//! Closed V2/V3 anchor transaction implementation; callers cannot supply record families.
macro_rules! external_anchor_transaction_adapter {
    ($BYTES:ident, $Identity:ident, $Transaction:ident, $Error:ident) => {
        pub const $BYTES: usize = 24 + P + Q + U + 8 + 96;
        const N: usize = $BYTES;

        /// Fixed diagnostic family for the native transaction/currentness joins.
        #[derive(Debug)]
        pub enum $Error {
            Resource(Resource),
            Attestation(AttestationError),
            Publication(PublicationError),
            Framing(Framing),
            Anchor(AnchorError),
            Current(crate::CompilerExecutionCurrentRecordVerificationErrorV3),
            Mismatch(&'static str),
        }
        pub(crate) type Result<T> = std::result::Result<T, $Error>;
        impl From<Resource> for $Error {
            fn from(error: Resource) -> Self {
                Self::Resource(error)
            }
        }
        impl From<AttestationError> for $Error {
            fn from(error: AttestationError) -> Self {
                Self::Attestation(error)
            }
        }
        impl From<PublicationError> for $Error {
            fn from(error: PublicationError) -> Self {
                Self::Publication(error)
            }
        }
        impl From<Framing> for $Error {
            fn from(error: Framing) -> Self {
                Self::Framing(error)
            }
        }
        impl From<AnchorError> for $Error {
            fn from(error: AnchorError) -> Self {
                Self::Anchor(error)
            }
        }
        impl From<crate::CompilerExecutionCurrentRecordVerificationErrorV3> for $Error {
            fn from(error: crate::CompilerExecutionCurrentRecordVerificationErrorV3) -> Self {
                Self::Current(error)
            }
        }
        impl fmt::Display for $Error {
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
        impl Error for $Error {
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
        pub struct $Identity([u8; 32]);
        impl $Identity {
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }

        /// Move-only, source-bearing canonical anchor input, not durable authority.
        /// It never constructs a legacy issuer policy, request or publication owner.
        #[derive(Debug)]
        pub struct $Transaction {
            policy: Policy,
            request: Request,
            publication: Publication,
            canonical: [u8; N],
            identity: $Identity,
        }

        impl $Transaction {
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
                let floor = policy.retained_storage()
                    + request.retained_storage()
                    + publication.retained_storage();
                b.with_prepaid_scope(floor, 8, Self::WORK, Self::FRAME_STORAGE, |b| {
                    publication.receipt().verify_matches(
                        policy,
                        request,
                        request.challenge().prior_rollback_anchor(),
                        b,
                    )?;
                    let (bytes, _) = encode_transaction::<N>(
                        MAGIC,
                        VERSION,
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
                        VERSION,
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
                            identity: $Identity(identity),
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
                            return Err($Error::Mismatch("native anchor transaction length"));
                        }
                        let mut reader = Reader::new(bytes);
                        if decode_versioned_header(&mut reader, MAGIC, VERSION, N)? != 0 {
                            return Err($Error::Mismatch("native anchor transaction kind"));
                        }
                        let (policy, charge) = Policy::decode(reader.take(P)?, b)?;
                        b.reserve_storage(charge.additional_storage())?;
                        let (request, charge) = Request::decode(reader.take(Q)?, b)?;
                        b.reserve_storage(charge.additional_storage())?;
                        let (publication, charge) = Publication::decode(reader.take(U)?, b)?;
                        b.reserve_storage(charge.additional_storage())?;
                        let (record, _) = Self::new(policy, request, publication, b)?;
                        if record.canonical.as_slice() != bytes {
                            return Err($Error::Mismatch(
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
            pub const fn identity(&self) -> $Identity {
                self.identity
            }
            pub const fn retained_storage(&self) -> usize {
                Self::RETAINED_STORAGE
            }
            pub fn external_anchor_digest(
                &self,
                b: &mut Budget<'_>,
            ) -> Result<TransactionDigestV1> {
                b.with_prepaid_scope(Self::RETAINED_STORAGE, 8, Self::WORK, 4096, |_| {
                    Ok(derive_transaction_digest_v1(&self.canonical)?)
                })
            }
        }
    };
}
pub(crate) use external_anchor_transaction_adapter;
