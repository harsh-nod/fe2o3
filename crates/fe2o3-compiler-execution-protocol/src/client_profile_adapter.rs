//! Private native profile body; nominal policy types and schemas stay distinct.
macro_rules! client_profile_adapter {
    ($schema:ident, $version:literal, $previous:literal,
        $bytes:ident, $work:ident, $storage:ident, $Identity:ident, $Profile:ident, $Failure:ident) => {
        pub const $bytes: usize = codec::BYTES;
        const RETAINED: usize = size_of::<$Profile>() + size_of::<Storage>();
        /// Prepaid logical work includes the complete nested native policy decoder.
        pub const $work: usize = POLICY_WORK + 32 * codec::BYTES;
        /// Fixed additional logical peak, including nested policy staging. Not RSS,
        /// heap-allocation or generated stack accounting. Inputs stay separately paid.
        pub const $storage: usize = POLICY_STORAGE
            + 4 * RETAINED
            + 4 * codec::BYTES
            + 2 * size_of::<sha2::Sha256>()
            + 4096;
        const _: () = {
            assert!(RETAINED >= POLICY_RETAINED);
            assert!(size_of::<($Profile, Storage)>() <= RETAINED);
            assert!(8 * size_of::<$Failure>() + 64 * size_of::<usize>() <= 4096);
        };

        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $Identity([u8; 32]);
        impl $Identity {
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
            /// Fixed-wire identity comparison, not provisioning or policy pinning.
            pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
                metered(
                    budget,
                    resources::fixed_input_floor(bytes, codec::BYTES),
                    || Ok(codec::$schema.matches(self.0, bytes)),
                )
            }
        }

        /// Move-only supervisor/anchor credentials and a nominal native issuer policy.
        /// This public configuration grants no signing, compiler, load or launch rights.
        /// All operations restore the caller's entry storage, including on failure.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionClientProfileV", $version, ";")]
        #[doc = concat!("fn duplicate(profile: CompilerExecutionClientProfileV", $version, ") { let _ = profile.clone(); }")]
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionClientProfileV", $version, ", CompilerExecutionIssuerPolicyV", $previous, ", CompilerExecutionExternalAnchorServiceIdentityV1};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
        #[doc = concat!("fn mix(policy: CompilerExecutionIssuerPolicyV", $previous, ", service: CompilerExecutionExternalAnchorServiceIdentityV1,")]
        ///        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
        #[doc = concat!("    let _ = CompilerExecutionClientProfileV", $version, "::new(1000, 1000, service, policy, budget);")]
        /// }
        /// ```
        #[derive(Eq, PartialEq)]
        pub struct $Profile {
            supervisor_uid: u32,
            supervisor_gid: u32,
            external_anchor_service: Service,
            policy: Policy,
            identity: $Identity,
            canonical_bytes: [u8; codec::BYTES],
        }
        impl $Profile {
            /// Consumes a prepaid policy. Retain its reservation and reserve only the
            /// returned delta. On failure the policy drops; retire its reservation then.
            pub fn new(
                supervisor_uid: u32,
                supervisor_gid: u32,
                external_anchor_service: Service,
                policy: Policy,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                metered(budget, policy.retained_storage(), || {
                    codec::validate_credentials(supervisor_uid, supervisor_gid)?;
                    Ok((
                        Self::from_policy(
                            supervisor_uid,
                            supervisor_gid,
                            external_anchor_service,
                            policy,
                        ),
                        Storage(RETAINED - POLICY_RETAINED),
                    ))
                })
            }

            /// Borrows an already prepaid exact wire owner. Unlike `new`, this returns
            /// the FULL retained charge: the caller transferred no nested reservation.
            /// The fixed aggregate quota prepays nested decoding on this same ledger.
            pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                metered(
                    budget,
                    resources::fixed_input_floor(bytes, codec::BYTES),
                    || {
                        let parsed = codec::$schema.parse(bytes)?;
                        let policy = Policy {
                            record: issuer_policy_codec::$schema
                                .decode(parsed.policy)
                                .map_err(PolicyError::from)?,
                        };
                        codec::$schema.check_identity(bytes)?;
                        let decoded = Self::from_policy(parsed.uid, parsed.gid, parsed.service, policy);
                        if decoded.canonical_bytes.as_slice() != bytes {
                            return Err(FramingError::Canonical.into());
                        }
                        Ok((decoded, Storage(RETAINED)))
                    },
                )
            }

            fn from_policy(uid: u32, gid: u32, service: Service, policy: Policy) -> Self {
                let (canonical_bytes, identity) =
                    codec::$schema.encode(uid, gid, service, policy.canonical_bytes());
                Self {
                    supervisor_uid: uid,
                    supervisor_gid: gid,
                    external_anchor_service: service,
                    policy,
                    identity: $Identity(identity),
                    canonical_bytes,
                }
            }
            pub const fn supervisor_uid(&self) -> u32 {
                self.supervisor_uid
            }
            pub const fn supervisor_gid(&self) -> u32 {
                self.supervisor_gid
            }
            pub const fn external_anchor_service(&self) -> Service {
                self.external_anchor_service
            }
            pub const fn policy(&self) -> &Policy {
                &self.policy
            }
            pub const fn identity(&self) -> $Identity {
                self.identity
            }
            pub const fn canonical_bytes(&self) -> &[u8; codec::BYTES] {
                &self.canonical_bytes
            }
            /// Total fixed reservation, including the nested policy and descriptor.
            pub const fn retained_storage(&self) -> usize {
                RETAINED
            }
        }
        impl fmt::Debug for $Profile {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Profile))
                    .field("supervisor_uid", &self.supervisor_uid)
                    .field("supervisor_gid", &self.supervisor_gid)
                    .field("identity", &self.identity)
                    .finish_non_exhaustive()
            }
        }

        /// Shared framing diagnostics do not admit a different profile family.
        #[derive(Debug)]
        pub enum $Failure {
            Framing(FramingError),
            Policy(PolicyError),
            Resource(Resource),
        }
        type Result<T> = std::result::Result<T, $Failure>;
        impl From<FramingError> for $Failure {
            fn from(value: FramingError) -> Self {
                Self::Framing(value)
            }
        }
        impl From<PolicyError> for $Failure {
            fn from(value: PolicyError) -> Self {
                Self::Policy(value)
            }
        }
        impl From<Resource> for $Failure {
            fn from(value: Resource) -> Self {
                Self::Resource(value)
            }
        }
        impl fmt::Display for $Failure {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Framing(e) => e.fmt(f),
                    Self::Policy(e) => e.fmt(f),
                    Self::Resource(e) => e.fmt(f),
                }
            }
        }
        impl Error for $Failure {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                Some(match self {
                    Self::Framing(e) => e,
                    Self::Policy(e) => e,
                    Self::Resource(e) => e,
                })
            }
        }
        fn metered<T>(
            budget: &mut Budget<'_>,
            floor: usize,
            operation: impl FnOnce() -> Result<T>,
        ) -> Result<T> {
            resources::fixed(budget, floor, $work, $storage, operation)
        }
    };
}
pub(crate) use client_profile_adapter;
