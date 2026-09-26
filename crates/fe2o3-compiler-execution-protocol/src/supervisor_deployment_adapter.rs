//! Closed native deployment API; only nominal names and wire schema vary.
macro_rules! supervisor_deployment_adapter {
    ($schema:ident, $version:literal, $other:literal,
        $bytes:ident, $work:ident, $storage:ident, $max_executable:ident, $max_launcher:ident,
        $Identity:ident, $Owner:ident, $Failure:ident) => {
        /// Exact native deployment wire size, including the terminal identity.
        pub const $bytes: usize = codec::BYTES;
        /// Largest supervisor executable measurement accepted by this family.
        pub const $max_executable: u64 = codec::MAX_EXECUTABLE_BYTES;
        /// Largest static pre-exec launcher measurement accepted by this family.
        pub const $max_launcher: u64 = codec::MAX_LAUNCHER_BYTES;
        const RETAINED: usize = size_of::<($Owner, Storage)>();
        /// Fixed logical quota for entry, validation, encoding, hashing and comparison.
        /// The already constructed native policy is borrowed, not decoded again.
        /// This is not an instruction or elapsed-time bound.
        pub const $work: usize = resources::ENTRY_WORK + 32 * codec::BYTES;
        /// Additional fixed logical peak above all prepaid live inputs. Covers owner
        /// and record staging, fields, fixed wires, SHA state and result controls.
        /// This is not an allocator, generated-stack or process-RSS bound.
        pub const $storage: usize = 4 * RETAINED
            + 4 * size_of::<codec::Fields>()
            + 4 * codec::BYTES
            + 2 * size_of::<sha2::Sha256>()
            + 4096;
        const _: () = {
            type Output = ($Owner, Storage);
            assert!(codec::BYTES == 184 && codec::BYTES <= u32::MAX as usize);
            assert!(size_of::<codec::Record>() <= RETAINED);
            assert!(size_of::<Result<Output>>() >= RETAINED);
            assert!(size_of::<std::thread::Result<Result<Output>>>() >= RETAINED);
            assert!(
                size_of::<std::result::Result<codec::Record, Framing>>()
                    >= size_of::<codec::Record>()
            );
            assert!(
                8 * size_of::<$Failure>()
                    + 64 * size_of::<usize>()
                    + size_of::<Budget<'static>>()
                    + 4 * size_of::<Storage>()
                    + (size_of::<Result<Output>>() - RETAINED)
                    + (size_of::<std::thread::Result<Result<Output>>>() - RETAINED)
                    + (size_of::<std::result::Result<codec::Record, Framing>>()
                        - size_of::<codec::Record>())
                    <= 4096
            );
        };

        /// Family-specific deployment identity, not authenticated provisioning evidence.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentIdentityV", $version, " as Identity, CompilerExecutionSupervisorDeploymentIdentityV", $other, " as Other};")]
        /// fn mix(identity: Identity) -> Other { identity }
        /// ```
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $Identity([u8; 32]);
        impl $Identity {
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// Checks canonical framing and identity on the original ledger. This
            /// neither pins a policy nor authenticates the source of the bytes.
            /// Keep the complete borrowed input owner prepaid on this same ledger.
            pub fn matches_canonical_bytes(
                self,
                bytes: &[u8],
                budget: &mut Budget<'_>,
            ) -> Result<bool> {
                metered(
                    budget,
                    resources::fixed_input_floor(bytes, codec::BYTES),
                    || Ok(codec::$schema.matches(self.0, bytes)),
                )
            }
        }

        /// Move-only, inert trusted-configuration record for one native policy family.
        ///
        /// Binds non-root supervisor and anchor credentials with distinct UIDs, exact
        /// supervisor/launcher SHA-256 and byte lengths, and the complete actual native
        /// policy identity (including generation, both measurements and both keys).
        /// Public construction and decoding NEVER authenticate provisioning or produce
        /// admitted provenance, signing, compiler, publication, load or launch authority.
        /// A trusted caller must separately pin this record and its native policy.
        ///
        /// All working operations use the original Budget and restore entry storage
        /// on success, error and unwind, preserving work, peak and denial history.
        /// Borrowed inputs stay prepaid; reserve returned additional Storage before
        /// retaining the owner, then release `retained_storage()` after dropping it.
        /// Each working operation charges 8 entry units, checks its input floor, then
        /// prepays the remaining WORK and all STORAGE before examining record data.
        /// WORK and STORAGE are the exported constants for this deployment family.
        /// Accessors only return already stored values and do not charge the ledger.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorDeploymentV", $version, " as Deployment;")]
        /// fn duplicate(value: Deployment) { let _ = value.clone(); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV1, CompilerExecutionSupervisorDeploymentV", $version, " as Deployment};")]
        /// fn upgrade(value: CompilerExecutionSupervisorDeploymentV1) -> Deployment { value.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV", $version, " as Deployment, CompilerExecutionSupervisorDeploymentV", $other, " as Other};")]
        /// fn mix(value: Deployment) -> Other { value }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV", $version, " as Deployment, CompilerExecutionIssuerPolicyV1 as Policy};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(bytes: &[u8], policy: &Policy, budget: &mut Budget<'_>) {
        ///     let _ = Deployment::decode(bytes, policy, budget);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV", $version, " as Deployment, CompilerExecutionIssuerPolicyV", $other, " as Policy};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(value: &Deployment, policy: &Policy, budget: &mut Budget<'_>) {
        ///     let _ = value.matches_policy(policy, budget);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV", $version, " as Deployment, CompilerExecutionIssuerPolicyV", $version, " as Policy};")]
        /// fn unmetered(bytes: &[u8], policy: &Policy) { let _ = Deployment::decode(bytes, policy); }
        /// ```
        #[derive(Eq, PartialEq)]
        pub struct $Owner {
            record: codec::Record,
        }
        impl $Owner {
            /// Constructs inert configuration against an already prepaid native policy.
            /// Returns the FULL owner charge; the borrowed policy remains separately live.
            pub fn new(
                service_uid: u32,
                service_gid: u32,
                external_anchor_service: Service,
                executable: Measurement,
                launcher: Measurement,
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                metered(budget, policy.retained_storage(), || {
                    Ok((
                        Self {
                            record: codec::$schema.encode(codec::Fields {
                                uid: service_uid,
                                gid: service_gid,
                                service: external_anchor_service,
                                executable,
                                launcher,
                                policy: *policy.identity().as_bytes(),
                            })?,
                        },
                        Storage(RETAINED),
                    ))
                })
            }

            /// Strict canonical decode against the exact supplied native policy. Both
            /// the complete input owner and policy stay prepaid. Wrong-sized slices
            /// are rejected without scanning them or requiring a wire input floor.
            /// Relabeling/resealing a foreign policy identity cannot pass this check.
            /// The supplied policy itself is public configuration, not provisioning proof.
            pub fn decode(
                bytes: &[u8],
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                let floor = policy
                    .retained_storage()
                    .checked_add(resources::fixed_input_floor(bytes, codec::BYTES))
                    .ok_or(Resource::Arithmetic)?;
                metered(budget, floor, || {
                    let record = codec::$schema.decode(bytes)?;
                    if &record.fields.policy != policy.identity().as_bytes() {
                        return Err($Failure::PolicyMismatch);
                    }
                    Ok((Self { record }, Storage(RETAINED)))
                })
            }

            /// Metered complete native identity comparison, never provisioning evidence.
            pub fn matches_policy(
                &self,
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<bool> {
                let floor = RETAINED
                    .checked_add(policy.retained_storage())
                    .ok_or(Resource::Arithmetic)?;
                metered(budget, floor, || {
                    Ok(&self.record.fields.policy == policy.identity().as_bytes())
                })
            }

            pub const fn service_uid(&self) -> u32 {
                self.record.fields.uid
            }
            pub const fn service_gid(&self) -> u32 {
                self.record.fields.gid
            }
            pub const fn external_anchor_service(&self) -> Service {
                self.record.fields.service
            }
            pub const fn executable(&self) -> Measurement {
                self.record.fields.executable
            }
            pub const fn launcher(&self) -> Measurement {
                self.record.fields.launcher
            }
            pub const fn policy_identity(&self) -> PolicyIdentity {
                PolicyIdentity::from_bytes_for_protocol(self.record.fields.policy)
            }
            pub const fn identity(&self) -> $Identity {
                $Identity(self.record.identity)
            }
            pub const fn canonical_bytes(&self) -> &[u8; codec::BYTES] {
                &self.record.bytes
            }
            /// Full owner and padded native storage-receipt header.
            pub const fn retained_storage(&self) -> usize {
                RETAINED
            }
        }

        impl fmt::Debug for $Owner {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Owner))
                    .field("authority", &"none")
                    .field("service_uid", &self.service_uid())
                    .field("service_gid", &self.service_gid())
                    .field("policy_identity", &self.policy_identity())
                    .field("identity", &self.identity())
                    .finish_non_exhaustive()
            }
        }

        /// Shared diagnostics do not invoke a V1 decoder or upgrade a V1 owner.
        #[derive(Debug)]
        pub enum $Failure {
            Framing(Framing),
            /// The record does not bind the complete supplied native policy identity.
            PolicyMismatch,
            Resource(Resource),
        }
        type Result<T> = std::result::Result<T, $Failure>;
        impl From<Framing> for $Failure {
            fn from(value: Framing) -> Self {
                Self::Framing(value)
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
                    Self::PolicyMismatch => {
                        f.write_str("compiler-execution supervisor deployment policy mismatch")
                    }
                    Self::Resource(e) => e.fmt(f),
                }
            }
        }
        impl Error for $Failure {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self {
                    Self::Framing(e) => Some(e),
                    Self::PolicyMismatch => None,
                    Self::Resource(e) => Some(e),
                }
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
pub(crate) use supervisor_deployment_adapter;
