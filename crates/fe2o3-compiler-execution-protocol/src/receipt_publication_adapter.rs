//! Private native-family body. Public adapters retain nominal owners and schemas.
macro_rules! receipt_publication_adapter {
    ($schema:ident, $version:literal,
        $pub_bytes:ident, $ack_bytes:ident, $pub_work:ident, $pub_decode_work:ident, $ack_work:ident, $pub_storage:ident, $pub_decode_storage:ident, $ack_storage:ident, $PublicationIdentity:ident, $AckIdentity:ident, $Publication:ident, $Ack:ident, $Failure:ident
    ) => {
        pub const $pub_bytes: usize = codec::PUBLICATION_BYTES;
        pub const $ack_bytes: usize = codec::ACK_BYTES;
        pub const $pub_work: usize =
            resources::ENTRY_WORK + 32 * codec::PUBLICATION_BYTES;
        pub const $pub_decode_work: usize =
            $pub_work + RECEIPT_WORK;
        pub const $ack_work: usize =
            resources::ENTRY_WORK + 32 * codec::ACK_BYTES;
        pub(crate) const PUBLICATION_RETAINED: usize =
            size_of::<$Publication>() + size_of::<Storage>();
        pub(crate) const ACK_RETAINED: usize =
            size_of::<$Ack>() + size_of::<Storage>();
        /// Additional fixed logical peak; not generated stack, heap or RSS usage.
        pub const $pub_storage: usize =
            4 * PUBLICATION_RETAINED + 4 * codec::PUBLICATION_BYTES + 2 * size_of::<sha2::Sha256>() + 4096;
        /// Outer frame stays reserved while the actual nested receipt decoder runs.
        pub const $pub_decode_storage: usize =
            $pub_storage + RECEIPT_STORAGE;
        pub const $ack_storage: usize = 4 * ACK_RETAINED
            + 4 * size_of::<codec::AckFields>()
            + 4 * codec::ACK_BYTES
            + 2 * size_of::<sha2::Sha256>()
            + 4096;
        const _: () = {
            assert!(PUBLICATION_RETAINED >= RECEIPT_RETAINED);
            assert!(RECEIPT_STORAGE >= RECEIPT_RETAINED);
            assert!(size_of::<($Publication, Storage)>() <= PUBLICATION_RETAINED);
            assert!(size_of::<($Ack, Storage)>() <= ACK_RETAINED);
            assert!(
                8 * size_of::<$Failure>()
                    + 64 * size_of::<usize>()
                    + 4 * size_of::<codec::PublicationParts<'static>>()
                    + 4 * size_of::<codec::Bindings>()
                    + 4 * (size_of::<
                        std::thread::Result<Result<($Publication, Storage)>>,
                    >() - size_of::<($Publication, Storage)>())
                    <= 4096
            );
            assert!(size_of::<std::thread::Result<Result<bool>>>() <= 4096);
            assert!(size_of::<std::thread::Result<Result<()>>>() <= 4096);
            assert!(
                size_of::<std::thread::Result<Result<($Publication, Storage)>>>()
                    <= PUBLICATION_RETAINED + 4096
            );
            assert!(
                size_of::<std::thread::Result<Result<($Ack, Storage)>>>(
                ) <= ACK_RETAINED + 4096
            );
        };
        pub(crate) type Result<T> = std::result::Result<T, $Failure>;

        #[derive(Debug)]
        #[non_exhaustive]
        pub enum $Failure {
            Framing(Framing),
            Attestation(AttestationError),
            Resource(Resource),
        }
        impl From<Framing> for $Failure {
            fn from(e: Framing) -> Self {
                Self::Framing(e)
            }
        }
        impl From<AttestationError> for $Failure {
            fn from(e: AttestationError) -> Self {
                Self::Attestation(e)
            }
        }
        impl From<Resource> for $Failure {
            fn from(e: Resource) -> Self {
                Self::Resource(e)
            }
        }
        impl fmt::Display for $Failure {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Framing(e) => e.fmt(f),
                    Self::Attestation(e) => e.fmt(f),
                    Self::Resource(e) => e.fmt(f),
                }
            }
        }
        impl StdError for $Failure {
            fn source(&self) -> Option<&(dyn StdError + 'static)> {
                Some(match self {
                    Self::Framing(e) => e,
                    Self::Attestation(e) => e,
                    Self::Resource(e) => e,
                })
            }
        }

        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $PublicationIdentity([u8; 32]);
        impl $PublicationIdentity {
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
            pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
                publication_fixed(
                    budget,
                    resources::fixed_input_floor(bytes, codec::PUBLICATION_BYTES),
                    || Ok(codec::$schema.matches_publication(self.0, bytes)),
                )
            }
        }
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $AckIdentity([u8; 32]);
        impl $AckIdentity {
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
            pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
                ack_fixed(
                    budget,
                    resources::fixed_input_floor(bytes, codec::ACK_BYTES),
                    || Ok(codec::$schema.matches_ack(self.0, bytes)),
                )
            }
        }

        /// Move-only sidecar of a signed native receipt. No durable publication authority.
        ///
        /// ```compile_fail
        #[doc = concat!(" use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptPublication", "V", $version, ";")]
        #[doc = concat!(" fn duplicate(v: CompilerExecutionReceiptPublication", "V", $version, ") { let _ = v.clone(); }")]
        /// ```
        /// ```compile_fail
        #[doc = concat!(" use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationReceiptV1, CompilerExecutionReceiptPublication", "V", $version, "};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
        /// fn mix(r: CompilerExecutionAttestationReceiptV1,b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
        #[doc = concat!("     let _ = CompilerExecutionReceiptPublication", "V", $version, "::new([1;32],[2;32],r,b);")]
        /// }
        /// ```
        #[derive(Eq, PartialEq)]
        pub struct $Publication {
            pub(crate) record: codec::Publication,
            receipt: Receipt,
        }
        impl $Publication {
            /// Transfer the consumed receipt reservation; reserve only the returned delta.
            /// On refusal its owner drops but the inherited reservation remains.
            pub fn new(
                journal: [u8; 32],
                occurrence: [u8; 32],
                receipt: Receipt,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                publication_fixed(budget, RECEIPT_RETAINED, || {
                    Ok((
                        Self::from_receipt(journal, occurrence, receipt)?,
                        Storage(PUBLICATION_RETAINED - RECEIPT_RETAINED),
                    ))
                })
            }
            fn from_receipt(journal: [u8; 32], occurrence: [u8; 32], receipt: Receipt) -> Result<Self> {
                let record = codec::$schema.publication(
                    codec::Bindings {
                        policy: *receipt.policy_identity().as_bytes(),
                        journal,
                        occurrence,
                        receipt: *receipt.identity().as_bytes(),
                    },
                    receipt.canonical_bytes(),
                )?;
                Ok(Self { record, receipt })
            }
            /// Borrow prepaid bytes; returns the FULL owner charge, not an inherited delta.
            pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                resources::nested_fixed(
                    budget,
                    resources::fixed_input_floor(bytes, codec::PUBLICATION_BYTES),
                    $pub_work,
                    $pub_storage,
                    |budget| {
                        let parts = codec::$schema.publication_parts(bytes)?;
                        let (receipt, storage) = Receipt::decode(parts.receipt, budget)?;
                        budget.reserve_storage(storage.additional_storage())?;
                        let record = codec::$schema.finish_publication(
                            parts,
                            *receipt.policy_identity().as_bytes(),
                            *receipt.identity().as_bytes(),
                            receipt.canonical_bytes(),
                            bytes,
                        )?;
                        Ok((Self { record, receipt }, Storage(PUBLICATION_RETAINED)))
                    },
                )
            }
            pub const fn policy_identity(&self) -> PolicyIdentity {
                PolicyIdentity::from_bytes_for_protocol(self.record.bindings.policy)
            }
            pub const fn issuer_journal_identity(&self) -> [u8; 32] {
                self.record.bindings.journal
            }
            pub const fn compiler_occurrence_identity(&self) -> [u8; 32] {
                self.record.bindings.occurrence
            }
            pub const fn receipt_identity(&self) -> ReceiptIdentity {
                ReceiptIdentity::from_bytes_for_protocol(self.record.bindings.receipt)
            }
            pub const fn receipt(&self) -> &Receipt {
                &self.receipt
            }
            pub const fn identity(&self) -> $PublicationIdentity {
                $PublicationIdentity(self.record.wire.identity)
            }
            pub const fn canonical_bytes(&self) -> &[u8; codec::PUBLICATION_BYTES] {
                &self.record.wire.bytes
            }
            pub const fn retained_storage(&self) -> usize {
                PUBLICATION_RETAINED
            }
            pub fn matches_issued_record(
                &self,
                policy: PolicyIdentity,
                journal: [u8; 32],
                occurrence: [u8; 32],
                receipt: ReceiptIdentity,
                budget: &mut Budget<'_>,
            ) -> Result<()> {
                publication_fixed(budget, PUBLICATION_RETAINED, || {
                    Ok(self.record.bindings.matches(codec::Bindings {
                        policy: *policy.as_bytes(),
                        journal,
                        occurrence,
                        receipt: *receipt.as_bytes(),
                    })?)
                })
            }
            pub const fn proves_durable_publication(&self) -> bool {
                false
            }
            pub const fn grants_compiler_authority(&self) -> bool {
                false
            }
        }
        impl fmt::Debug for $Publication {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Publication))
                    .field("policy_identity", &self.policy_identity())
                    .field("issuer_journal_identity", &self.issuer_journal_identity())
                    .field(
                        "compiler_occurrence_identity",
                        &self.compiler_occurrence_identity(),
                    )
                    .field("receipt_identity", &self.receipt_identity())
                    .field("identity", &self.identity())
                    .finish_non_exhaustive()
            }
        }

        /// Move-only inert ACK claim. Reacquire and verify durable Worker state separately.
        ///
        /// ```compile_fail
        #[doc = concat!(" use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptPublicationAck", "V", $version, ";")]
        #[doc = concat!(" fn duplicate(v: CompilerExecutionReceiptPublicationAck", "V", $version, ") { let _ = v.clone(); }")]
        /// ```
        #[derive(Eq, PartialEq)]
        pub struct $Ack {
            record: codec::Ack,
        }
        impl $Ack {
            /// Borrows the publication; returns a full additional ACK charge.
            pub fn new(
                publication: &$Publication,
                worker: [u8; 32],
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                ack_fixed(budget, PUBLICATION_RETAINED, || {
                    Ok((
                        Self {
                            record: codec::$schema.ack(codec::AckFields {
                                bindings: publication.record.bindings,
                                publication: publication.record.wire.identity,
                                worker,
                                sequence: publication.receipt.sequence(),
                                current: publication.receipt.next_rollback_anchor(),
                            })?,
                        },
                        Storage(ACK_RETAINED),
                    ))
                })
            }
            pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                ack_fixed(
                    budget,
                    resources::fixed_input_floor(bytes, codec::ACK_BYTES),
                    || {
                        Ok((
                            Self {
                                record: codec::$schema.decode_ack(bytes)?,
                            },
                            Storage(ACK_RETAINED),
                        ))
                    },
                )
            }
            pub fn matches_publication(
                &self,
                publication: &$Publication,
                budget: &mut Budget<'_>,
            ) -> Result<()> {
                ack_fixed(budget, ACK_RETAINED + PUBLICATION_RETAINED, || {
                    Ok(self.record.matches(
                        &publication.record,
                        publication.receipt.sequence(),
                        publication.receipt.next_rollback_anchor(),
                    )?)
                })
            }
            pub fn matches_worker_ledger_record(
                &self,
                expected: [u8; 32],
                budget: &mut Budget<'_>,
            ) -> Result<()> {
                ack_fixed(budget, ACK_RETAINED, || {
                    Ok(self.record.matches_worker(expected)?)
                })
            }
            pub const fn policy_identity(&self) -> PolicyIdentity {
                PolicyIdentity::from_bytes_for_protocol(self.record.fields.bindings.policy)
            }
            pub const fn issuer_journal_identity(&self) -> [u8; 32] {
                self.record.fields.bindings.journal
            }
            pub const fn compiler_occurrence_identity(&self) -> [u8; 32] {
                self.record.fields.bindings.occurrence
            }
            pub const fn receipt_identity(&self) -> ReceiptIdentity {
                ReceiptIdentity::from_bytes_for_protocol(self.record.fields.bindings.receipt)
            }
            pub const fn publication_identity(&self) -> $PublicationIdentity {
                $PublicationIdentity(self.record.fields.publication)
            }
            pub const fn worker_ledger_record_identity(&self) -> [u8; 32] {
                self.record.fields.worker
            }
            pub const fn sequence(&self) -> u64 {
                self.record.fields.sequence
            }
            pub const fn current_rollback_anchor(&self) -> [u8; 32] {
                self.record.fields.current
            }
            pub const fn identity(&self) -> $AckIdentity {
                $AckIdentity(self.record.wire.identity)
            }
            pub const fn canonical_bytes(&self) -> &[u8; codec::ACK_BYTES] {
                &self.record.wire.bytes
            }
            pub const fn retained_storage(&self) -> usize {
                ACK_RETAINED
            }
            pub const fn proves_durable_publication(&self) -> bool {
                false
            }
            pub const fn grants_compiler_authority(&self) -> bool {
                false
            }
        }
        impl fmt::Debug for $Ack {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Ack))
                    .field("policy_identity", &self.policy_identity())
                    .field("issuer_journal_identity", &self.issuer_journal_identity())
                    .field(
                        "compiler_occurrence_identity",
                        &self.compiler_occurrence_identity(),
                    )
                    .field("receipt_identity", &self.receipt_identity())
                    .field("publication_identity", &self.publication_identity())
                    .field(
                        "worker_ledger_record_identity",
                        &self.worker_ledger_record_identity(),
                    )
                    .field("sequence", &self.sequence())
                    .field("identity", &self.identity())
                    .finish_non_exhaustive()
            }
        }
        fn publication_fixed<T>(
            budget: &mut Budget<'_>,
            floor: usize,
            f: impl FnOnce() -> Result<T>,
        ) -> Result<T> {
            resources::fixed(
                budget,
                floor,
                $pub_work,
                $pub_storage,
                f,
            )
        }
        fn ack_fixed<T>(budget: &mut Budget<'_>, floor: usize, f: impl FnOnce() -> Result<T>) -> Result<T> {
            resources::fixed(
                budget,
                floor,
                $ack_work,
                $ack_storage,
                f,
            )
        }
    };
}
pub(crate) use receipt_publication_adapter;
