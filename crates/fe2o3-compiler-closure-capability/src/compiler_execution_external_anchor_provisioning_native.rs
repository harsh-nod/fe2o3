//! Closed provisioning adapters over the existing contextual sealed transport.
macro_rules! provisioning_capability {
    ($Cap:ident, $version:literal, $other:literal) => {
        use crate::{
            native_capability::{NativeCapability, Record, Result, Storage},
            sealed_image::CapabilityRole,
        };
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        use std::{fs::File, os::fd::RawFd};
        type Capability = NativeCapability<Provisioning, BYTES>;
        impl Record<BYTES> for Provisioning {
            type Context<'a> = &'a Deployment;
            const ROLE: CapabilityRole = CapabilityRole {
                name: "native external-anchor provisioning capability",
                memfd_name: concat!("fe2o3-external-anchor-provisioning-v", $version),
            };
            fn bytes(&self) -> &[u8; BYTES] { self.canonical_bytes() }
            fn retained_storage(&self) -> usize { self.retained_storage() }
            fn context_storage(d: &Deployment) -> Result<usize> { Ok(d.retained_storage()) }
            fn decode_retained(bytes: &[u8; BYTES], d: &Deployment, b: &mut Budget<'_>) -> Result<Self> {
                let (record, charge) = Self::decode(bytes, d, b)?;
                b.reserve_storage(charge.additional_storage())?;
                Ok(record)
            }
        }

        /// Move-only sealed native provisioning configuration. Recovery requires the
        /// actual same-family deployment, never its digest alone or a legacy owner.
        /// The complete borrowed deployment stays prepaid on the original ledger;
        /// its storage is not consumed or included in returned growth. File recovery
        /// consumes FILE_STORAGE; inherited recovery borrows it and owns a private
        /// CLOEXEC duplicate. The caller retains the unchanged inherited source slot.
        /// Reserve returned storage before retention, and retire full retained_storage()
        /// after Drop. Consuming errors close the File but leave its charge to the caller.
        /// Entry storage is restored; work, peak and first-denial history are preserved.
        ///
        /// Exact-object validation establishes neither provisioning provenance nor a
        /// measured helper. The parent must pin both before protected use. This owner
        /// grants no execution, signing, compiler, publication, load or launch authority.
        ///
        /// ```
        #[doc = concat!("use fe2o3_compiler_closure_capability::{", stringify!($Cap), " as Cap, CompilerExecutionCapabilityErrorV2 as Error};")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn recover(f: std::fs::File, d: &Deployment, b: &mut Budget<'_>) -> Result<Cap, Error> {
        ///     let (cap, growth) = Cap::from_file(f, d, b)?;
        ///     b.reserve_storage(growth.additional_storage())?;
        ///     cap.revalidate(b)?;
        ///     Ok(cap)
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        /// fn duplicate(c: Cap) { let _ = c.clone(); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        /// fn descriptor<T: std::os::fd::AsFd>() {} descriptor::<Cap>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::{", stringify!($Cap), " as Cap, CompilerExecutionExternalAnchorProvisioningCapabilityV", $other, " as Other};")]
        /// fn mix(c: Cap) -> Other { c.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        /// use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorProvisioningV1 as Old;
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn upgrade(p: Old, b: &mut Budget<'_>) { let _ = Cap::create(p, b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV", $other, " as Deployment;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(f: std::fs::File, d: &Deployment, b: &mut Budget<'_>) { let _ = Cap::from_file(f, d, b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentIdentityV", $version, " as Identity;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn digest(f: std::fs::File, d: &Identity, b: &mut Budget<'_>) { let _ = Cap::from_file(f, d, b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn context_free(f: std::fs::File, b: &mut Budget<'_>) { let _ = Cap::from_file(f, b); }
        /// ```
        pub struct $Cap(Capability);
        impl $Cap {
            pub const IO_WORK: usize = Capability::IO_WORK;
            pub const IO_STORAGE: usize = Capability::IO_STORAGE;
            pub const FILE_STORAGE: usize = Capability::FILE_STORAGE;
            /// Logical quotas for fixed I/O and nested native decoding, not time/RSS bounds.
            pub const ADMISSION_WORK: usize = Self::IO_WORK + PROVISIONING_WORK;
            pub const ADMISSION_STORAGE: usize = Self::IO_STORAGE + PROVISIONING_STORAGE;
            /// Consumes a prepaid native record; returns growth above that record.
            pub fn create(p: Provisioning, b: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::create(p, b).map(|(c, s)| (Self(c), s))
            }
            /// Consumes FILE_STORAGE; borrows actual deployment; returns only growth.
            pub fn from_file(f: File, d: &Deployment, b: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::from_file(f, d, b).map(|(c, s)| (Self(c), s))
            }
            /// Borrows non-CLOEXEC fd >= 3 and deployment; returns FULL retained storage.
            pub fn from_inherited_at(fd: RawFd, d: &Deployment, b: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::from_inherited_at(fd, d, b).map(|(c, s)| (Self(c), s))
            }
            pub const fn provisioning(&self) -> &Provisioning { &self.0.record }
            pub fn revalidate(&self, b: &mut Budget<'_>) -> Result<()> { self.0.revalidate(b) }
            pub fn try_clone_for_transfer(&self, b: &mut Budget<'_>) -> Result<(File, Storage)> {
                self.0.try_clone_for_transfer(b)
            }
            pub fn validate_transfer(&self, f: &File, b: &mut Budget<'_>) -> Result<()> {
                self.0.validate_transfer(f, b)
            }
            pub const fn retained_storage(&self) -> usize { Capability::RETAINED }
        }
        const _: () = { assert!(BYTES == 128); Capability::assert_layout::<$Cap>(); };
    };
}
pub(crate) use provisioning_capability;
