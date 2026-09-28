//! Private native-family body. Public adapters retain nominal owners and schemas.
macro_rules! receipt_carriage_adapter {
    ($schema:ident, $version:literal,
        $bytes:ident, $work:ident, $construct_work:ident, $decode_work:ident, $storage:ident, $construct_storage:ident, $decode_storage:ident, $Identity:ident, $Carriage:ident
    ) => {
        pub const $bytes: usize = codec::CARRIAGE_BYTES;
        /// Fixed outer framing work, also used for identity matching.
        pub const $work: usize =
            resources::ENTRY_WORK + 32 * codec::CARRIAGE_BYTES;
        pub const $construct_work: usize =
            $work + VW + AW;
        pub const $decode_work: usize =
            $work + PW + QW + UW + AW + VW + AW;
        const INHERITED: usize = POLICY_RETAINED + REQUEST_RETAINED + PUBLICATION_RETAINED + ACK_RETAINED;
        const RETAINED: usize = size_of::<$Carriage>() + size_of::<Storage>();
        /// Fixed additional logical outer frame; excludes nested operations' scratch.
        pub const $storage: usize =
            4 * RETAINED + 4 * codec::CARRIAGE_BYTES + 2 * size_of::<sha2::Sha256>() + 4096;
        pub const $construct_storage: usize =
            $storage + maximum(&[VS, AS]);
        /// Peak over the actual sequential child-retention schedule, above entry storage.
        pub const $decode_storage: usize =
            $storage
                + maximum(&[
                    PS,
                    POLICY_RETAINED + QS,
                    POLICY_RETAINED + REQUEST_RETAINED + US,
                    POLICY_RETAINED + REQUEST_RETAINED + PUBLICATION_RETAINED + AS,
                    INHERITED + VS,
                    INHERITED + AS,
                ]);
        const fn maximum(values: &[usize]) -> usize {
            let mut result = 0;
            let mut i = 0;
            while i < values.len() {
                if values[i] > result {
                    result = values[i];
                }
                i += 1;
            }
            result
        }
        const _: () = {
            assert!(RETAINED >= INHERITED);
            assert!(PS >= POLICY_RETAINED);
            assert!(QS >= REQUEST_RETAINED);
            assert!(US >= PUBLICATION_RETAINED);
            assert!(AS >= ACK_RETAINED);
            assert!(size_of::<($Carriage, Storage)>() <= RETAINED);
            assert!(
                size_of::<std::thread::Result<Result<($Carriage, Storage)>>>()
                    <= RETAINED + 4096
            );
            assert!(
                8 * size_of::<Error>()
                    + 64 * size_of::<usize>()
                    + 4 * size_of::<codec::CarriageParts<'static>>()
                    + 4 * (size_of::<
                        std::thread::Result<Result<($Carriage, Storage)>>,
                    >() - size_of::<($Carriage, Storage)>())
                    <= 4096
            );
            assert!(size_of::<std::thread::Result<Result<bool>>>() <= 4096);
        };

        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $Identity([u8; 32]);
        impl $Identity {
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
            pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
                resources::fixed(
                    budget,
                    resources::fixed_input_floor(bytes, codec::CARRIAGE_BYTES),
                    $work,
                    $storage,
                    || Ok(codec::$schema.matches_carriage(self.0, bytes)),
                )
            }
        }

        /// Move-only complete native policy, request, publication and ACK. Relationships
        /// are verified, but protected policy pinning and durable currentness are not.
        /// No clone, legacy projection, alternate graph or authority-bearing result.
        ///
        /// ```compile_fail
        #[doc = concat!(" use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptCarriage", "V", $version, ";")]
        #[doc = concat!(" fn duplicate(v: CompilerExecutionReceiptCarriage", "V", $version, ") { let _ = v.clone(); }")]
        /// ```
        /// ```compile_fail
        #[doc = concat!(" use fe2o3_compiler_execution_protocol::{CompilerExecutionReceiptCarriage", "V", $version, ", CompilerExecutionIssuerPolicyV1, CompilerExecutionAttestationRequest", "V", $version, ", CompilerExecutionReceiptPublication", "V", $version, ", CompilerExecutionReceiptPublicationAck", "V", $version, "};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
        #[doc = concat!(" fn mix(p: CompilerExecutionIssuerPolicyV1,q: CompilerExecutionAttestationRequest", "V", $version, ",")]
        #[doc = concat!("        u: CompilerExecutionReceiptPublication", "V", $version, ",a: CompilerExecutionReceiptPublicationAck", "V", $version, ",")]
        ///        b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
        #[doc = concat!("     let _ = CompilerExecutionReceiptCarriage", "V", $version, "::new(p,q,u,a,b);")]
        /// }
        /// ```
        #[derive(Eq, PartialEq)]
        pub struct $Carriage {
            policy: Policy,
            request: Request,
            publication: Publication,
            acknowledgment: Ack,
            record: codec::Record<{ codec::CARRIAGE_BYTES }>,
        }
        impl $Carriage {
            /// Transfer all four prepaid reservations; reserve only the returned delta.
            /// On refusal owners drop but inherited reservations remain for retirement.
            pub fn new(
                policy: Policy,
                request: Request,
                publication: Publication,
                acknowledgment: Ack,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                resources::nested_fixed(
                    budget,
                    INHERITED,
                    $work,
                    $storage,
                    |budget| {
                        Ok((
                            Self::from_owned(policy, request, publication, acknowledgment, budget)?,
                            Storage(RETAINED - INHERITED),
                        ))
                    },
                )
            }
            /// Decode each real native child on this ledger. Returns the FULL carriage
            /// charge; internally reserved children do not transfer from borrowed bytes.
            pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                resources::nested_fixed(
                    budget,
                    resources::fixed_input_floor(bytes, codec::CARRIAGE_BYTES),
                    $work,
                    $storage,
                    |budget| {
                        let parts = codec::$schema.carriage_parts(bytes)?;
                        let (policy, s) = Policy::decode(parts.policy, budget)?;
                        budget.reserve_storage(s.additional_storage())?;
                        let (request, s) = Request::decode(parts.request, budget)?;
                        budget.reserve_storage(s.additional_storage())?;
                        let (publication, s) = Publication::decode(parts.publication, budget)?;
                        budget.reserve_storage(s.additional_storage())?;
                        let (acknowledgment, s) = Ack::decode(parts.ack, budget)?;
                        budget.reserve_storage(s.additional_storage())?;
                        let decoded =
                            Self::from_owned(policy, request, publication, acknowledgment, budget)?;
                        decoded
                            .record
                            .check(parts.identity, bytes, "compiler receipt carriage")?;
                        Ok((decoded, Storage(RETAINED)))
                    },
                )
            }
            fn from_owned(
                policy: Policy,
                request: Request,
                publication: Publication,
                acknowledgment: Ack,
                budget: &mut Budget<'_>,
            ) -> Result<Self> {
                publication.receipt().verify_matches(
                    &policy,
                    &request,
                    request.challenge().prior_rollback_anchor(),
                    budget,
                )?;
                acknowledgment.matches_publication(&publication, budget)?;
                let record = codec::$schema.carriage(
                    policy.canonical_bytes(),
                    request.canonical_bytes(),
                    publication.canonical_bytes(),
                    acknowledgment.canonical_bytes(),
                );
                Ok(Self {
                    policy,
                    request,
                    publication,
                    acknowledgment,
                    record,
                })
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
            pub const fn acknowledgment(&self) -> &Ack {
                &self.acknowledgment
            }
            pub const fn identity(&self) -> $Identity {
                $Identity(self.record.identity)
            }
            pub const fn canonical_bytes(&self) -> &[u8; codec::CARRIAGE_BYTES] {
                &self.record.bytes
            }
            pub const fn retained_storage(&self) -> usize {
                RETAINED
            }
            pub const fn requires_protected_policy_verification(&self) -> bool {
                true
            }
            pub const fn grants_compiler_authority(&self) -> bool {
                false
            }
            pub const fn grants_load_authority(&self) -> bool {
                false
            }
            pub const fn grants_launch_authority(&self) -> bool {
                false
            }
        }
        impl fmt::Debug for $Carriage {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Carriage))
                    .field("policy_identity", &self.policy.identity())
                    .field("request_identity", &self.request.identity())
                    .field("publication_identity", &self.publication.identity())
                    .field("acknowledgment_identity", &self.acknowledgment.identity())
                    .field("identity", &self.identity())
                    .finish_non_exhaustive()
            }
        }
    };
}
pub(crate) use receipt_carriage_adapter;
