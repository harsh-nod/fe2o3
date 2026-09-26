//! Shared native implementation; only nominal names, subject and schema vary.
// Kept private so downstream crates cannot instantiate another trust family.
macro_rules! attestation_challenge_adapter {
    ($schema:ident, $version:literal, $handoff:literal,
        $bytes:ident, $work:ident, $storage:ident, $Binding:ident, $Identity:ident, $Challenge:ident) => {
        pub const $bytes: usize = codec::CHALLENGE_BYTES;
        pub const $work: usize =
            resources::ENTRY_WORK + 32 * codec::CHALLENGE_BYTES;
        pub(crate) const SUBJECT_RETAINED: usize = size_of::<Subject>() + size_of::<SubjectStorage>();
        pub(crate) const RETAINED: usize =
            size_of::<$Challenge>() + size_of::<Storage>();
        /// Fixed additional logical peak including result, staging and hash/control
        /// scratch. Caller inputs stay prepaid; this is not allocator/RSS/stack usage.
        pub const $storage: usize = 4 * RETAINED
            + 4 * size_of::<codec::Fields>()
            + 4 * codec::CHALLENGE_BYTES
            + 2 * size_of::<sha2::Sha256>()
            + 4096;
        const _: () = {
            assert!(size_of::<($Challenge, Storage)>() <= RETAINED);
            assert!(8 * size_of::<Error>() + 64 * size_of::<usize>() <= 4096);
        };
        type Result<T> = std::result::Result<T, Error>;

        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $Binding(pub(crate) codec::Binding);
        impl $Binding {
            pub(crate) fn from_subject(subject: &Subject) -> Self {
                Self(codec::Binding {
                    sha256: *subject.identity().sha256(),
                    byte_len: subject.identity().byte_len(),
                })
            }
            pub const fn sha256(self) -> [u8; 32] {
                self.0.sha256
            }
            pub const fn byte_len(self) -> u64 {
                self.0.byte_len
            }
            /// Compares the immutable, already-admitted subject identity. This does not
            /// reconstruct a handoff or authenticate a protected compiler occurrence.
            pub fn matches_subject(self, subject: &Subject, budget: &mut Budget<'_>) -> Result<bool> {
                metered(budget, SUBJECT_RETAINED, || {
                    Ok(self == Self::from_subject(subject))
                })
            }
        }

        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $Identity([u8; 32]);
        impl $Identity {
            pub(crate) const fn from_bytes_for_protocol(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
            pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
                metered(
                    budget,
                    resources::fixed_input_floor(bytes, codec::CHALLENGE_BYTES),
                    || Ok(codec::$schema.matches_challenge(self.0, bytes)),
                )
            }
        }

        /// Caller-supplied nonce/rollback position for one exact native policy/subject.
        /// Canonical decoding establishes neither freshness nor trusted issuer origin.
        /// Inputs stay prepaid; reserve the returned full retained charge before use.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionAttestationChallenge", "V", $version, ";")]
        #[doc = concat!("fn duplicate(value: CompilerExecutionAttestationChallenge", "V", $version, ") { let _ = value.clone(); }")]
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV1, CompilerExecutionAttestationChallenge", "V", $version, "};")]
        #[doc = concat!("use fe2o3_artifact_transaction::InertCompilerExecutionSubject", "V", $version, ";")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
        #[doc = concat!("fn mix(p: &CompilerExecutionIssuerPolicyV1, s: &InertCompilerExecutionSubject", "V", $version, ",")]
        ///        b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
        #[doc = concat!("    let _ = CompilerExecutionAttestationChallenge", "V", $version, "::new(p, s, [1; 32], 1, [0; 32], b);")]
        /// }
        /// ```
        #[derive(Eq, PartialEq)]
        pub struct $Challenge {
            pub(crate) record: codec::Challenge,
        }
        impl $Challenge {
            pub fn new(
                policy: &Policy,
                subject: &Subject,
                nonce: [u8; 32],
                sequence: u64,
                prior_rollback_anchor: [u8; 32],
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                metered(budget, policy.retained_storage() + SUBJECT_RETAINED, || {
                    Ok((
                        Self {
                            record: codec::$schema.challenge(codec::Fields {
                                policy: *policy.identity().as_bytes(),
                                subject: $Binding::from_subject(subject).0,
                                nonce,
                                sequence,
                                prior: prior_rollback_anchor,
                            })?,
                        },
                        Storage(RETAINED),
                    ))
                })
            }
            pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                metered(
                    budget,
                    resources::fixed_input_floor(bytes, codec::CHALLENGE_BYTES),
                    || {
                        Ok((
                            Self {
                                record: codec::$schema.decode_challenge(bytes)?,
                            },
                            Storage(RETAINED),
                        ))
                    },
                )
            }
            pub const fn policy_identity(&self) -> PolicyIdentity {
                PolicyIdentity::from_bytes_for_protocol(self.record.fields.policy)
            }
            pub const fn subject(&self) -> $Binding {
                $Binding(self.record.fields.subject)
            }
            pub const fn nonce(&self) -> [u8; 32] {
                self.record.fields.nonce
            }
            pub const fn sequence(&self) -> u64 {
                self.record.fields.sequence
            }
            pub const fn prior_rollback_anchor(&self) -> [u8; 32] {
                self.record.fields.prior
            }
            pub const fn identity(&self) -> $Identity {
                $Identity(self.record.identity)
            }
            pub const fn canonical_bytes(&self) -> &[u8; codec::CHALLENGE_BYTES] {
                &self.record.bytes
            }
            pub const fn retained_storage(&self) -> usize {
                RETAINED
            }
        }
        impl fmt::Debug for $Challenge {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Challenge))
                    .field("policy_identity", &self.policy_identity())
                    .field("subject", &self.subject())
                    .field("sequence", &self.sequence())
                    .field("identity", &self.identity())
                    .finish_non_exhaustive()
            }
        }
        fn metered<T>(budget: &mut Budget<'_>, floor: usize, f: impl FnOnce() -> Result<T>) -> Result<T> {
            resources::fixed(
                budget,
                floor,
                $work,
                $storage,
                f,
            )
        }
    };
}
pub(crate) use attestation_challenge_adapter;
