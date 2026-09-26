//! Shared native implementation; only nominal names, subject and schema vary.
// Kept private so downstream crates cannot instantiate another trust family.
macro_rules! issuer_policy_adapter {
    ($schema:ident, $version:literal, $handoff:literal,
        $bytes:ident, $work:ident, $storage:ident, $Identity:ident, $Policy:ident, $Failure:ident) => {
        pub const $bytes: usize = codec::BYTES;
        pub(crate) const RETAINED: usize =
            size_of::<$Policy>() + size_of::<Storage>();
        /// Fixed logical admission work, including two Ed25519 public-key validations,
        /// canonical encoding, hash, comparison and entry. Not an instruction bound.
        pub const $work: usize =
            resources::ENTRY_WORK + 2 * resources::KEY_VALIDATION_WORK + 32 * codec::BYTES;
        /// Additional logical peak: result and staging records, fields, SHA state and
        /// fixed codec/crypto scratch. Caller-owned input remains separately prepaid.
        /// This is not a generated stack, heap-allocation or RSS bound.
        pub const $storage: usize = 4 * RETAINED
            + 4 * size_of::<codec::Fields>()
            + 4 * codec::BYTES
            + 2 * size_of::<sha2::Sha256>()
            + 4096;
        const _: () = {
            assert!(size_of::<($Policy, Storage)>() <= RETAINED);
            assert!(
                8 * size_of::<$Failure>()
                    + 64 * size_of::<usize>()
                    + 2 * size_of::<ed25519_dalek::VerifyingKey>()
                    <= 4096
            );
        };

        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $Identity([u8; 32]);
        impl $Identity {
            pub(crate) const fn from_bytes_for_protocol(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// Checks an exact fixed wire identity, not policy pinning or authority.
            /// Keep the complete borrowed input owner prepaid on this same ledger.
            pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
                metered(
                    budget,
                    resources::fixed_input_floor(bytes, codec::BYTES),
                    || Ok(codec::$schema.matches(self.0, bytes)),
                )
            }
        }

        #[doc = concat!("Move-only, Subject", "V", $version, "-specific public trust configuration. The policy's")]
        /// measurements and two distinct keys must still be pinned by a trusted caller.
        /// Construction/decoding grants no compiler, signing, load or launch authority.
        ///
        /// Inputs stay prepaid; reserve the returned additional storage before keeping
        /// the owner. All calls restore entry storage on success, error and unwind.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicy", "V", $version, ";")]
        #[doc = concat!("fn duplicate(policy: CompilerExecutionIssuerPolicy", "V", $version, ") { let _ = policy.clone(); }")]
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionIssuerPolicyV1, CompilerExecutionIssuerPolicy", "V", $version, "};")]
        /// fn legacy(_: &CompilerExecutionIssuerPolicyV1) {}
        #[doc = concat!("fn mix(policy: &CompilerExecutionIssuerPolicy", "V", $version, ") { legacy(policy); }")]
        /// ```
        #[derive(Eq, PartialEq)]
        pub struct $Policy {
            pub(crate) record: codec::Record,
        }
        impl $Policy {
            pub fn new(
                generation: u64,
                executable: Measurement,
                runtime: Measurement,
                verifying_key: [u8; 32],
                external_anchor_verifying_key: [u8; 32],
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                metered(budget, 0, || {
                    Ok((
                        Self {
                            record: codec::$schema.encode(codec::Fields {
                                generation,
                                executable,
                                runtime,
                                verifying_key,
                                external_anchor_verifying_key,
                            })?,
                        },
                        Storage(RETAINED),
                    ))
                })
            }

            #[doc = concat!("Strict ", "V", $version, " decode. A V1 policy, including one with the same generation,")]
            /// keys and measurements, cannot be upgraded or accepted by this method.
            pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                metered(
                    budget,
                    resources::fixed_input_floor(bytes, codec::BYTES),
                    || {
                        Ok((
                            Self {
                                record: codec::$schema.decode(bytes)?,
                            },
                            Storage(RETAINED),
                        ))
                    },
                )
            }

            pub const fn generation(&self) -> u64 {
                self.record.fields.generation
            }
            pub const fn executable(&self) -> Measurement {
                self.record.fields.executable
            }
            pub const fn runtime(&self) -> Measurement {
                self.record.fields.runtime
            }
            pub const fn verifying_key(&self) -> &[u8; 32] {
                &self.record.fields.verifying_key
            }
            pub const fn external_anchor_verifying_key(&self) -> &[u8; 32] {
                &self.record.fields.external_anchor_verifying_key
            }
            pub const fn identity(&self) -> $Identity {
                $Identity(self.record.identity)
            }
            pub const fn canonical_bytes(&self) -> &[u8; codec::BYTES] {
                &self.record.bytes
            }
            /// Total fixed reservation to retain this owner, including its descriptor.
            pub const fn retained_storage(&self) -> usize {
                RETAINED
            }
        }
        impl fmt::Debug for $Policy {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Policy))
                    .field("generation", &self.generation())
                    .field("identity", &self.identity())
                    .finish_non_exhaustive()
            }
        }

        /// Shared framing diagnostics do not invoke a V1 decoder or grant V1 admission.
        #[derive(Debug)]
        pub enum $Failure {
            Framing(CompilerExecutionAttestationErrorV1),
            Subject(SubjectError),
            Resource(Resource),
        }
        type Result<T> = std::result::Result<T, $Failure>;
        impl From<CompilerExecutionAttestationErrorV1> for $Failure {
            fn from(value: CompilerExecutionAttestationErrorV1) -> Self {
                Self::Framing(value)
            }
        }
        impl From<Resource> for $Failure {
            fn from(value: Resource) -> Self {
                Self::Resource(value)
            }
        }
        impl From<SubjectError>
            for $Failure
        {
            fn from(value: SubjectError) -> Self {
                Self::Subject(value)
            }
        }
        impl fmt::Display for $Failure {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Framing(e) => e.fmt(f),
                    Self::Subject(e) => e.fmt(f),
                    Self::Resource(e) => e.fmt(f),
                }
            }
        }
        impl Error for $Failure {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                Some(match self {
                    Self::Framing(e) => e,
                    Self::Subject(e) => e,
                    Self::Resource(e) => e,
                })
            }
        }

        fn metered<T>(
            budget: &mut Budget<'_>,
            floor: usize,
            operation: impl FnOnce() -> Result<T>,
        ) -> Result<T> {
            resources::fixed(
                budget,
                floor,
                $work,
                $storage,
                operation,
            )
        }
    };
}
pub(crate) use issuer_policy_adapter;
