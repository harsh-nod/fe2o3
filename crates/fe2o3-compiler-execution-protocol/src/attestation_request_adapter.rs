//! Shared native implementation; only nominal names, subject and schema vary.
// Kept private so downstream crates cannot instantiate another trust family.
macro_rules! attestation_request_adapter {
    ($schema:ident, $version:literal, $handoff:literal,
        $bytes:ident, $work:ident, $decode_work:ident, $storage:ident, $decode_storage:ident, $Identity:ident, $Request:ident) => {
        pub const $bytes: usize = codec::REQUEST_BYTES;
        pub const $work: usize =
            resources::ENTRY_WORK + 32 * codec::REQUEST_BYTES;
        #[doc = concat!("Decode work includes the actual nested Subject", "V", $version, " decoder, not a new meter.")]
        pub const $decode_work: usize =
            $work + SUBJECT_WORK;
        pub(crate) const RETAINED: usize =
            size_of::<$Request>() + size_of::<Storage>();
        const INHERITED: usize = challenge::RETAINED + challenge::SUBJECT_RETAINED;
        /// Additional fixed logical construction peak, excluding prepaid input owners.
        pub const $storage: usize =
            4 * RETAINED + 4 * codec::REQUEST_BYTES + 2 * size_of::<sha2::Sha256>() + 4096;
        /// Exact valid-decode peak quota: fixed outer frame and nested subject scratch.
        pub const $decode_storage: usize =
            $storage + SUBJECT_STORAGE;
        const _: () = {
            assert!(RETAINED >= INHERITED);
            assert!(SUBJECT_STORAGE >= challenge::SUBJECT_RETAINED);
            assert!(size_of::<($Request, Storage)>() <= RETAINED);
            assert!(8 * size_of::<Error>() + 64 * size_of::<usize>() <= 4096);
        };
        type Result<T> = std::result::Result<T, Error>;

        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $Identity([u8; 32]);
        impl $Identity {
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
            pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
                resources::fixed(
                    budget,
                    resources::fixed_input_floor(bytes, codec::REQUEST_BYTES),
                    $work,
                    $storage,
                    || Ok(codec::$schema.matches_request(self.0, bytes)),
                )
            }
        }

        /// Move-only complete native subject plus its exact challenge. This is inert
        /// framing/content agreement, not protected execution or signing authority.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionAttestationRequest", "V", $version, ";")]
        #[doc = concat!("fn duplicate(value: CompilerExecutionAttestationRequest", "V", $version, ") { let _ = value.clone(); }")]
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationChallenge", "V", $version, ", CompilerExecutionAttestationRequest", "V", $version, "};")]
        /// use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1;
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
        #[doc = concat!("fn mix(c: CompilerExecutionAttestationChallenge", "V", $version, ", s: InertCompilerExecutionSubjectV1,")]
        ///        b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
        #[doc = concat!("    let _ = CompilerExecutionAttestationRequest", "V", $version, "::new(c, s, b);")]
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationRequest", "V", $version, "};")]
        #[doc = concat!("use fe2o3_artifact_transaction::InertCompilerExecutionSubject", "V", $version, ";")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
        #[doc = concat!("fn mix(c: CompilerExecutionAttestationChallengeV1, s: InertCompilerExecutionSubject", "V", $version, ",")]
        ///        b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
        #[doc = concat!("    let _ = CompilerExecutionAttestationRequest", "V", $version, "::new(c, s, b);")]
        /// }
        /// ```
        #[derive(Eq, PartialEq)]
        pub struct $Request {
            challenge: Challenge,
            subject: Subject,
            identity: $Identity,
            canonical_bytes: [u8; codec::REQUEST_BYTES],
        }
        impl $Request {
            /// Transfer both prepaid input reservations; reserve only the returned delta.
            /// On error both inputs drop but their reservations remain for caller cleanup.
            pub fn new(
                challenge: Challenge,
                subject: Subject,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                resources::fixed(
                    budget,
                    INHERITED,
                    $work,
                    $storage,
                    || {
                        Ok((
                            Self::from_owned(challenge, subject)?,
                            Storage(RETAINED - INHERITED),
                        ))
                    },
                )
            }
            /// Borrow a prepaid wire owner; returns the FULL result charge, not a delta
            #[doc = concat!("inherited from its internally decoded children. Subject", "V", $version, " enforces its")]
            #[doc = concat!("existing ", "V", $handoff, " storage ceiling on this same ledger. Entry storage is restored")]
            /// on success, refusal and unwind, without resetting work or denial history.
            pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                resources::nested_fixed(
                    budget,
                    resources::fixed_input_floor(bytes, codec::REQUEST_BYTES),
                    $work,
                    $storage,
                    |budget| {
                        let parts = codec::$schema.request_parts(bytes)?;
                        let challenge = Challenge {
                            record: codec::$schema.decode_challenge(parts.challenge)?,
                        };
                        let (subject, storage) = Subject::decode(parts.subject, budget)?;
                        budget.reserve_storage(storage.retained_storage())?;
                        crate::attestation::require_identity(parts.identity, "request")?;
                        let decoded = Self::from_owned(challenge, subject)?;
                        if *decoded.identity.as_bytes() != parts.identity
                            || decoded.canonical_bytes.as_slice() != bytes
                        {
                            return Err(Framing::IdentityMismatch("request").into());
                        }
                        Ok((decoded, Storage(RETAINED)))
                    },
                )
            }
            fn from_owned(challenge: Challenge, subject: Subject) -> Result<Self> {
                if challenge.subject() != Binding::from_subject(&subject) {
                    return Err(Framing::SubjectMismatch.into());
                }
                let (canonical_bytes, identity) =
                    codec::$schema.request(challenge.canonical_bytes(), subject.canonical_bytes());
                Ok(Self {
                    challenge,
                    subject,
                    identity: $Identity(identity),
                    canonical_bytes,
                })
            }
            pub const fn challenge(&self) -> &Challenge {
                &self.challenge
            }
            pub const fn subject(&self) -> &Subject {
                &self.subject
            }
            pub const fn identity(&self) -> $Identity {
                self.identity
            }
            pub const fn canonical_bytes(&self) -> &[u8; codec::REQUEST_BYTES] {
                &self.canonical_bytes
            }
            pub const fn retained_storage(&self) -> usize {
                RETAINED
            }
        }
        impl fmt::Debug for $Request {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Request))
                    .field("challenge", &self.challenge)
                    .field("subject", &self.subject)
                    .field("identity", &self.identity)
                    .finish_non_exhaustive()
            }
        }
    };
}
pub(crate) use attestation_request_adapter;
