//! Closed family adapters for native anchor records on the shared sealed transport.
macro_rules! external_anchor_deployment_capability {
    ($Cap:ident, $version:literal, $other:literal) => {
        type Capability = NativeCapability<Deployment, BYTES>;
        impl Record<BYTES> for Deployment {
            type Context<'a> = (&'a Supervisor, &'a Policy);
            const ROLE: CapabilityRole = CapabilityRole {
                name: "native compiler-execution external-anchor deployment capability",
                memfd_name: concat!("fe2o3-compiler-execution-external-anchor-deployment-v", $version),
            };
            fn bytes(&self) -> &[u8; BYTES] {
                self.canonical_bytes()
            }
            fn retained_storage(&self) -> usize {
                self.retained_storage()
            }
            fn context_storage((supervisor, policy): Self::Context<'_>) -> Result<usize> {
                supervisor.retained_storage().checked_add(policy.retained_storage())
                    .ok_or_else(|| Resource::Arithmetic.into())
            }
            fn decode_retained(
                bytes: &[u8; BYTES],
                (supervisor, policy): Self::Context<'_>,
                budget: &mut Budget<'_>,
            ) -> Result<Self> {
                let (deployment, storage) = Self::decode(bytes, supervisor, policy, budget)?;
                budget.reserve_storage(storage.additional_storage())?;
                Ok(deployment)
            }
        }

        /// Move-only sealed native external-anchor configuration. Recovery requires
        /// BOTH actual same-family supervisor and policy owners, including their
        /// complete relationship, exact anchor credentials and policy anchor key.
        /// Creation consumes an already constructed record without rebinding it.
        /// Revalidation checks the admitted record and exact immutable sealed object.
        /// These checks establish no trusted provisioning origin, process custody,
        /// signing, compiler, publication, load or launch authority. A trusted parent
        /// must independently pin configuration, executable and provenance.
        ///
        /// Keep all borrowed owners and consumed inputs prepaid on the original ledger.
        /// Operations restore entry storage and preserve cumulative work, peak and
        /// first-denial history. Reserve returned growth before retaining the result;
        /// retire the FULL retained_storage() only after owner Drop. Consuming failures
        /// close the File but leave its reservation for caller cleanup. Context
        /// reservations never transfer. No V1 upgrade or fallback is available.
        ///
        /// ```
        #[doc = concat!("use fe2o3_compiler_closure_capability::{", stringify!($Cap), " as Cap, CompilerExecutionCapabilityErrorV2 as Error};")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV", $version, " as Supervisor, CompilerExecutionIssuerPolicyV", $version, " as Policy};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// // File/source slot and both context owners are already prepaid.
        /// fn recover(file: std::fs::File, s: &Supervisor, p: &Policy, b: &mut Budget<'_>)
        ///     -> Result<Cap, Error>
        /// {
        ///     let (cap, growth) = Cap::from_file(file, s, p, b)?;
        ///     b.reserve_storage(growth.additional_storage())?;
        ///     cap.revalidate(b)?;
        ///     Ok(cap)
        /// }
        /// fn inherit(fd: std::os::fd::RawFd, s: &Supervisor, p: &Policy, b: &mut Budget<'_>)
        ///     -> Result<Cap, Error>
        /// {
        ///     let (cap, full) = Cap::from_inherited_at(fd, s, p, b)?;
        ///     b.reserve_storage(full.additional_storage())?;
        ///     Ok(cap)
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        /// fn duplicate(value: Cap) { let _ = value.clone(); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        /// fn descriptor<T: std::os::fd::AsFd>() {}
        /// descriptor::<Cap>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::{", stringify!($Cap), " as Cap, CompilerExecutionExternalAnchorDeploymentCapabilityV", $other, " as Other};")]
        /// fn mix(value: Cap) -> Other { value.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        /// use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV1 as Old;
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn upgrade(old: Old, b: &mut Budget<'_>) { let _ = Cap::create(old, b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV", $other, " as Other;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(other: Other, b: &mut Budget<'_>) { let _ = Cap::create(other, b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV", $other, " as Supervisor, CompilerExecutionIssuerPolicyV", $version, " as Policy};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(file: std::fs::File, s: &Supervisor, p: &Policy, b: &mut Budget<'_>) {
        ///     let _ = Cap::from_file(file, s, p, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV", $version, " as Supervisor, CompilerExecutionIssuerPolicyV", $other, " as Policy};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(fd: std::os::fd::RawFd, s: &Supervisor, p: &Policy, b: &mut Budget<'_>) {
        ///     let _ = Cap::from_inherited_at(fd, s, p, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn context_free(file: std::fs::File, b: &mut Budget<'_>) { let _ = Cap::from_file(file, b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorDeploymentV", $version, " as Supervisor;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn missing_policy(file: std::fs::File, s: &Supervisor, b: &mut Budget<'_>) {
        ///     let _ = Cap::from_file(file, s, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV", $version, " as Policy;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn missing_supervisor(fd: std::os::fd::RawFd, p: &Policy, b: &mut Budget<'_>) {
        ///     let _ = Cap::from_inherited_at(fd, p, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentIdentityV", $version, " as Identity, CompilerExecutionIssuerPolicyV", $version, " as Policy};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn identity_only(file: std::fs::File, s: &Identity, p: &Policy, b: &mut Budget<'_>) {
        ///     let _ = Cap::from_file(file, s, p, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV", $version, " as Supervisor, CompilerExecutionIssuerPolicyIdentityV", $version, " as Identity};")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn identity_only(fd: std::os::fd::RawFd, s: &Supervisor, p: &Identity, b: &mut Budget<'_>) {
        ///     let _ = Cap::from_inherited_at(fd, s, p, b);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV", $version, " as Supervisor, CompilerExecutionIssuerPolicyV", $version, " as Policy};")]
        /// fn unmetered(file: std::fs::File, s: &Supervisor, p: &Policy) {
        ///     let _ = Cap::from_file(file, s, p);
        /// }
        /// ```
        pub struct $Cap(Capability);
        impl $Cap {
            /// Fixed outer I/O work; logical quota, not instruction or elapsed time.
            pub const IO_WORK: usize = Capability::IO_WORK;
            /// Additional outer scratch, not allocator, generated stack or RSS bounds.
            pub const IO_STORAGE: usize = Capability::IO_STORAGE;
            /// Full charge for one File and its complete sealed 168-byte image.
            pub const FILE_STORAGE: usize = Capability::FILE_STORAGE;
            /// Complete file/inherited admission, including contextual anchor decoding
            /// and its nested actual supervisor-policy check on the same ledger.
            pub const ADMISSION_WORK: usize = Self::IO_WORK + DEPLOYMENT_WORK;
            pub const ADMISSION_STORAGE: usize = Self::IO_STORAGE + DEPLOYMENT_STORAGE;

            /// Consumes a prepaid native record; returns growth above its full charge.
            pub fn create(deployment: Deployment, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::create(deployment, budget).map(|(value, storage)| (Self(value), storage))
            }
            /// Consumes FILE_STORAGE and borrows BOTH prepaid context owners. Returns
            /// only capability growth above FILE_STORAGE, never either context charge.
            pub fn from_file(image: File, supervisor: &Supervisor, policy: &Policy,
                budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::from_file(image, (supervisor, policy), budget)
                    .map(|(value, storage)| (Self(value), storage))
            }
            /// Borrows a non-CLOEXEC fd >= 3 and BOTH actual native context owners.
            /// Keep FILE_STORAGE plus both full context owners prepaid. Owns only a
            /// private CLOEXEC duplicate and returns its FULL capability charge.
            /// The source slot stays caller-owned, including on every refusal.
            pub fn from_inherited_at(fd: RawFd, supervisor: &Supervisor, policy: &Policy,
                budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::from_inherited_at(fd, (supervisor, policy), budget)
                    .map(|(value, storage)| (Self(value), storage))
            }
            pub const fn deployment(&self) -> &Deployment {
                &self.0.record
            }
            pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
                self.0.revalidate(budget)
            }
            /// Revalidates and returns a separately charged CLOEXEC File of this object.
            pub fn try_clone_for_transfer(&self, budget: &mut Budget<'_>) -> Result<(File, Storage)> {
                self.0.try_clone_for_transfer(budget)
            }
            /// Requires full capability and FILE_STORAGE reservations; checks exact
            /// object, metadata, seals, access and bytes without consuming the transfer.
            pub fn validate_transfer(&self, transfer: &File, budget: &mut Budget<'_>) -> Result<()> {
                self.0.validate_transfer(transfer, budget)
            }
            pub const fn retained_storage(&self) -> usize {
                Capability::RETAINED
            }
        }
        const _: () = {
            assert!(BYTES == 168);
            Capability::assert_layout::<$Cap>();
            assert!(DEPLOYMENT_STORAGE >= std::mem::size_of::<(Deployment, ProtocolStorage)>());
        };
    };
}
pub(crate) use external_anchor_deployment_capability;
