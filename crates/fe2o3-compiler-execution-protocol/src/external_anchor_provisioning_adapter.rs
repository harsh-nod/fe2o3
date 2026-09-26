//! Shared native provisioning API, instantiated only for the two closed families.
macro_rules! external_anchor_provisioning_adapter {
    ($schema:ident, $version:literal, $other:literal,
        $bytes:ident, $work:ident, $storage:ident, $max_helper:ident,
        $Identity:ident, $Owner:ident, $Failure:ident) => {
        /// Exact native wire length, including its terminal SHA-256 identity.
        pub const $bytes: usize = codec::BYTES;
        /// Largest accepted provisioning-helper executable measurement.
        pub const $max_helper: u64 = codec::MAX_HELPER_BYTES;
        const RETAINED: usize = size_of::<($Owner, Storage)>();
        /// Fixed logical entry, framing, hashing and comparison quota. The actual
        /// deployment is already validated and is not reconstructed or decoded again.
        /// This is not an instruction or elapsed-time bound.
        pub const $work: usize = resources::ENTRY_WORK + 32 * codec::BYTES;
        /// Additional logical peak above prepaid inputs, including owner/record,
        /// fields, fixed wires, SHA state and result controls. Not stack or RSS bounds.
        pub const $storage: usize = 4 * RETAINED
            + 4 * size_of::<codec::Fields>()
            + 4 * codec::BYTES
            + 2 * size_of::<sha2::Sha256>()
            + 4096;

        /// Copyable nominal content identity, not authenticated provisioning origin.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorProvisioningIdentityV", $version, " as Identity, CompilerExecutionExternalAnchorProvisioningIdentityV", $other, " as Other};")]
        /// fn mix(value: Identity) -> Other { value }
        /// ```
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $Identity([u8; 32]);
        impl $Identity {
            pub const fn as_bytes(&self) -> &[u8; 32] { &self.0 }

            /// Matches canonical framing, terminal identity AND actual same-family
            /// deployment context. Malformed bytes or wrong context return false;
            /// resource refusals remain errors. Keep the complete wire owner prepaid.
            pub fn matches_canonical_bytes(
                self,
                bytes: &[u8],
                deployment: &Deployment,
                budget: &mut Budget<'_>,
            ) -> Result<bool> {
                let floor = deployment.retained_storage()
                    .checked_add(resources::fixed_input_floor(bytes, codec::BYTES))
                    .ok_or(Resource::Arithmetic)?;
                metered(budget, floor, || {
                    Ok(codec::$schema.decode(bytes).is_ok_and(|record| {
                        record.identity == self.0
                            && record.fields.deployment == *deployment.identity().as_bytes()
                    }))
                })
            }
        }

        /// Move-only inert provisioning configuration for one actual native anchor deployment.
        ///
        /// Binds the complete deployment identity and exact helper SHA-256/nonzero byte
        /// length, at most 128 MiB. The supplied native deployment already binds its
        /// supervisor, policy, anchor credentials/key and executable; none is rebuilt.
        /// Public construction and decoding establish no provisioning origin, protected
        /// process, signing, compiler, publication, load, launch or GPU authority.
        /// A trusted caller must independently pin and measure the intended helper.
        ///
        /// Every working operation uses the original Budget: charge 8 entry units,
        /// check the complete input floor, then prepay remaining WORK and all STORAGE
        /// before inspecting record data. Keep borrowed deployment and wire owners
        /// fully prepaid; owner matching additionally requires this complete owner.
        /// Wrong-sized slices are not scanned and require no wire floor. Calls restore
        /// entry storage on success, error and unwind, preserving accepted work, peak
        /// and first-denial history. Returned Storage is the FULL unreserved
        /// size_of::<(Self, Storage)>() charge; reserve it before retention and release
        /// retained_storage() after owner Drop. Accessors are stored-value reads.
        ///
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorProvisioningV", $version, " as Provisioning;")]
        /// fn duplicate(value: Provisioning) { let _ = value.clone(); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorProvisioningV1 as Old, CompilerExecutionExternalAnchorProvisioningV", $version, " as Provisioning};")]
        /// fn upgrade(value: Old) -> Provisioning { value.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorProvisioningV", $version, " as Provisioning, CompilerExecutionExternalAnchorProvisioningV", $other, " as Other};")]
        /// fn mix(value: Provisioning) -> Other { value.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorProvisioningV", $version, " as Provisioning, CompilerExecutionExternalAnchorDeploymentV", $other, " as Deployment, CompilerExecutionIssuerMeasurementV1 as Measurement};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(d: &Deployment, helper: Measurement, b: &mut Budget<'_>) {
        ///     let _ = Provisioning::new(d, helper, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorProvisioningV", $version, " as Provisioning, CompilerExecutionExternalAnchorDeploymentV1 as Old};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn upgrade(bytes: &[u8], old: &Old, b: &mut Budget<'_>) {
        ///     let _ = Provisioning::decode(bytes, old, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorProvisioningV", $version, " as Provisioning, CompilerExecutionExternalAnchorDeploymentIdentityV", $version, " as Identity};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn identity_only(bytes: &[u8], id: &Identity, b: &mut Budget<'_>) {
        ///     let _ = Provisioning::decode(bytes, id, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorProvisioningV", $version, " as Provisioning;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn context_free(bytes: &[u8], b: &mut Budget<'_>) { let _ = Provisioning::decode(bytes, b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionExternalAnchorProvisioningV", $version, " as Provisioning, CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment};")]
        /// fn unmetered(bytes: &[u8], d: &Deployment) { let _ = Provisioning::decode(bytes, d); }
        /// ```
        #[derive(Eq, PartialEq)]
        pub struct $Owner {
            deployment: DeploymentIdentity,
            helper: Measurement,
            identity: $Identity,
            bytes: [u8; codec::BYTES],
        }
        impl $Owner {
            /// Borrows the actual prepaid deployment and returns the FULL owner charge.
            pub fn new(deployment: &Deployment, helper: Measurement, budget: &mut Budget<'_>)
                -> Result<(Self, Storage)> {
                metered(budget, deployment.retained_storage(), || {
                    let record = codec::$schema.encode(codec::Fields {
                        deployment: *deployment.identity().as_bytes(), helper,
                    })?;
                    Ok((Self::from_record(record, deployment), Storage(RETAINED)))
                })
            }

            /// Requires the complete actual deployment, not a caller-supplied digest.
            /// Validly resealed helper changes describe different inert configuration;
            /// check matches_deployment_and_helper against the independently pinned helper.
            pub fn decode(bytes: &[u8], deployment: &Deployment, budget: &mut Budget<'_>)
                -> Result<(Self, Storage)> {
                let floor = deployment.retained_storage()
                    .checked_add(resources::fixed_input_floor(bytes, codec::BYTES))
                    .ok_or(Resource::Arithmetic)?;
                metered(budget, floor, || {
                    let record = codec::$schema.decode(bytes)?;
                    if record.fields.deployment != *deployment.identity().as_bytes() {
                        return Err($Failure::ContextMismatch);
                    }
                    Ok((Self::from_record(record, deployment), Storage(RETAINED)))
                })
            }

            pub fn matches_deployment(&self, deployment: &Deployment, budget: &mut Budget<'_>)
                -> Result<bool> {
                let floor = RETAINED.checked_add(deployment.retained_storage())
                    .ok_or(Resource::Arithmetic)?;
                metered(budget, floor, || Ok(self.deployment == deployment.identity()))
            }

            /// Compares both helper digest and length, without reading executable bytes.
            pub fn matches_deployment_and_helper(&self, deployment: &Deployment,
                helper: Measurement, budget: &mut Budget<'_>) -> Result<bool> {
                let floor = RETAINED.checked_add(deployment.retained_storage())
                    .ok_or(Resource::Arithmetic)?;
                metered(budget, floor, || {
                    Ok(self.deployment == deployment.identity() && self.helper == helper)
                })
            }

            fn from_record(record: codec::Record, deployment: &Deployment) -> Self {
                Self {
                    deployment: deployment.identity(), helper: record.fields.helper,
                    identity: $Identity(record.identity), bytes: record.bytes,
                }
            }
            pub const fn deployment_identity(&self) -> DeploymentIdentity { self.deployment }
            pub const fn helper(&self) -> Measurement { self.helper }
            pub const fn identity(&self) -> $Identity { self.identity }
            pub const fn canonical_bytes(&self) -> &[u8; codec::BYTES] { &self.bytes }
            pub const fn retained_storage(&self) -> usize { RETAINED }
        }
        impl fmt::Debug for $Owner {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Owner))
                    .field("authority", &"none")
                    .field("deployment_identity", &self.deployment)
                    .field("identity", &self.identity)
                    .finish_non_exhaustive()
            }
        }

        /// Bounded typed diagnostics; shared framing errors never invoke a V1 decoder.
        #[derive(Debug)]
        pub enum $Failure {
            Framing(Framing),
            ContextMismatch,
            Resource(Resource),
        }
        type Result<T> = std::result::Result<T, $Failure>;
        impl From<Framing> for $Failure {
            fn from(e: Framing) -> Self { Self::Framing(e) }
        }
        impl From<Resource> for $Failure {
            fn from(e: Resource) -> Self { Self::Resource(e) }
        }
        impl fmt::Display for $Failure {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Framing(e) => e.fmt(f), Self::Resource(e) => e.fmt(f),
                    Self::ContextMismatch => f.write_str("external-anchor provisioning deployment mismatch"),
                }
            }
        }
        impl Error for $Failure {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self {
                    Self::Framing(e) => Some(e), Self::Resource(e) => Some(e),
                    Self::ContextMismatch => None,
                }
            }
        }
        fn metered<T>(budget: &mut Budget<'_>, floor: usize, operation: impl FnOnce() -> Result<T>)
            -> Result<T> {
            resources::fixed(budget, floor, $work, $storage, operation)
        }
        const _: () = {
            type Output = ($Owner, Storage);
            assert!(codec::BYTES == 128 && codec::BYTES <= u32::MAX as usize);
            assert!(size_of::<codec::Record>() <= RETAINED);
            assert!(size_of::<Result<Output>>() >= RETAINED);
            assert!(size_of::<std::thread::Result<Result<Output>>>() >= RETAINED);
            assert!(size_of::<std::result::Result<codec::Record, Framing>>() >= size_of::<codec::Record>());
            assert!(8 * size_of::<$Failure>() + 64 * size_of::<usize>()
                + size_of::<Budget<'static>>() + 4 * size_of::<Storage>()
                + (size_of::<Result<Output>>() - RETAINED)
                + (size_of::<std::thread::Result<Result<Output>>>() - RETAINED)
                + (size_of::<std::result::Result<codec::Record, Framing>>() - size_of::<codec::Record>())
                <= 4096);
        };
    };
}
pub(crate) use external_anchor_provisioning_adapter;
