//! Native deployment custody uses the shared sealed-record transport and actual policy context.
macro_rules! supervisor_deployment_capability {
    ($Cap:ident, $version:literal, $other:literal) => {
        type Capability = NativeCapability<Deployment, BYTES>;
        impl Record<BYTES> for Deployment {
            type Context<'a> = &'a Policy;
            const ROLE: CapabilityRole = CapabilityRole {
                name: "native compiler-execution supervisor deployment capability",
                memfd_name: concat!("fe2o3-compiler-execution-supervisor-deployment-v", $version),
            };
            fn bytes(&self) -> &[u8; BYTES] {
                self.canonical_bytes()
            }
            fn retained_storage(&self) -> usize {
                self.retained_storage()
            }
            fn context_storage(policy: &Policy) -> Result<usize> {
                Ok(policy.retained_storage())
            }
            fn decode_retained(
                bytes: &[u8; BYTES],
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<Self> {
                let (deployment, storage) = Self::decode(bytes, policy, budget)?;
                budget.reserve_storage(storage.additional_storage())?;
                Ok(deployment)
            }
        }

        /// Move-only sealed native deployment configuration. Recovery requires the
        /// actual same-family policy, not just a caller-supplied identity. Creation
        /// consumes an already constructed native deployment; it does not rebind it.
        /// Exact-object revalidation checks the admitted immutable record and image.
        /// Neither operation authenticates provisioning origin or grants process,
        /// compiler, publication, load or launch authority. The trusted parent must
        /// independently pin deployment and policy provenance.
        ///
        /// Keep all inputs prepaid on the original ledger. Operations restore entry
        /// storage, preserve cumulative work/peak/denial history, and return unreserved
        /// growth. After a consuming failure the File is closed; its reservation is
        /// left for the caller to retire. Borrowed policy reservations never move.
        ///
        /// ```
        #[doc = concat!("use fe2o3_compiler_closure_capability::{", stringify!($Cap), " as Cap, CompilerExecutionCapabilityErrorV2 as Error};")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV", $version, " as Policy;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// // The consumed File and borrowed policy are already prepaid.
        /// fn recover(file: std::fs::File, policy: &Policy, budget: &mut Budget<'_>)
        ///     -> Result<Cap, Error>
        /// {
        ///     let (cap, growth) = Cap::from_file(file, policy, budget)?;
        ///     budget.reserve_storage(growth.additional_storage())?;
        ///     cap.revalidate(budget)?;
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
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV", $other, " as Policy;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(file: std::fs::File, policy: &Policy, budget: &mut Budget<'_>) {
        ///     let _ = Cap::from_file(file, policy, budget);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        /// use fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorDeploymentV1 as Old;
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn upgrade(old: Old, budget: &mut Budget<'_>) { let _ = Cap::create(old, budget); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn context_free(file: std::fs::File, budget: &mut Budget<'_>) {
        ///     let _ = Cap::from_file(file, budget);
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyIdentityV", $version, " as Identity;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn identity_only(file: std::fs::File, identity: &Identity, budget: &mut Budget<'_>) {
        ///     let _ = Cap::from_file(file, identity, budget);
        /// }
        /// ```
        pub struct $Cap(Capability);
        impl $Cap {
            /// Fixed outer I/O quota. Logical work, not instruction or elapsed-time bounds.
            pub const IO_WORK: usize = Capability::IO_WORK;
            /// Fixed additional outer scratch. Not allocator, stack, or process RSS bounds.
            pub const IO_STORAGE: usize = Capability::IO_STORAGE;
            /// Full logical charge for one File and its complete sealed image.
            pub const FILE_STORAGE: usize = Capability::FILE_STORAGE;
            /// Complete file/inherited admission, including native contextual decoding.
            pub const ADMISSION_WORK: usize = Self::IO_WORK + DEPLOYMENT_WORK;
            pub const ADMISSION_STORAGE: usize = Self::IO_STORAGE + DEPLOYMENT_STORAGE;

            /// Consumes a prepaid native record and returns only growth over that record.
            pub fn create(deployment: Deployment, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::create(deployment, budget).map(|(value, storage)| (Self(value), storage))
            }
            /// Consumes FILE_STORAGE, borrows the prepaid policy, and returns owner growth.
            pub fn from_file(image: File, policy: &Policy, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::from_file(image, policy, budget).map(|(value, storage)| (Self(value), storage))
            }
            /// Borrows a non-CLOEXEC fd >= 3 and the actual policy. Keep FILE_STORAGE
            /// and the complete policy prepaid. The caller retains the source slot;
            /// only a private CLOEXEC duplicate is owned. Returns a FULL owner charge.
            /// This API neither installs a slot nor authenticates the supplying parent.
            pub fn from_inherited_at(fd: RawFd, policy: &Policy, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::from_inherited_at(fd, policy, budget).map(|(value, storage)| (Self(value), storage))
            }
            pub const fn deployment(&self) -> &Deployment {
                &self.0.record
            }
            pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
                self.0.revalidate(budget)
            }
            /// Revalidates and returns a separately charged CLOEXEC File of the same object.
            pub fn try_clone_for_transfer(&self, budget: &mut Budget<'_>) -> Result<(File, Storage)> {
                self.0.try_clone_for_transfer(budget)
            }
            /// Requires the complete owner and FILE_STORAGE prepaid. Checks exact inode,
            /// seals, metadata, access and bytes without owning or closing the transfer.
            pub fn validate_transfer(&self, transfer: &File, budget: &mut Budget<'_>) -> Result<()> {
                self.0.validate_transfer(transfer, budget)
            }
            pub const fn retained_storage(&self) -> usize {
                Capability::RETAINED
            }
        }

        const _: () = {
            Capability::assert_layout::<$Cap>();
            assert!(DEPLOYMENT_STORAGE >= std::mem::size_of::<(Deployment, ProtocolStorage)>());
        };
    };
}
pub(crate) use supervisor_deployment_capability;
