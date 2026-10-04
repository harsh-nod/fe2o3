//! Private native launch-input body; each caller supplies actual nominal types.
macro_rules! launch_inputs {
    ($Inputs:ident, $Storage:ident, $Failure:ident) => {
        const ENTRY_WORK: usize = 8;
        const RETAINED: usize = size_of::<(PolicyCapability, CapabilityStorage)>()
            + POLICY_BYTES
            + size_of::<(LaunchCapability, CapabilityStorage)>()
            + MANIFEST_BYTES;

        /// Unreserved full retained charge for the two newly duplicated input owners.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $Storage(usize);
        impl $Storage {
            /// Reserve this charge on the same ledger before retaining the result.
            pub const fn additional_storage(self) -> usize {
                self.0
            }
        }

        impl $Inputs {
            /// Prepaid borrowed File/image charge for fixed slots 6 and 8.
            pub const INPUT_STORAGE: usize =
                PolicyCapability::FILE_STORAGE + LaunchCapability::FILE_STORAGE;
            /// Additional outer logical frame. Nested capability and codec operations
            /// charge separately on the same ledger. Not allocator, RSS or stack bounds.
            pub const FRAME_STORAGE: usize = 4 * RETAINED + 4096;

            /// Borrows the fixed non-CLOEXEC input slots and retains private CLOEXEC
            /// duplicates. Never closes the source slots, including on refusal.
            /// The caller must keep them live, stable and prepaid at INPUT_STORAGE.
            /// All operations restore entry storage; accepted work is never refunded.
            pub fn from_inherited(budget: &mut Budget<'_>) -> Result<(Self, $Storage)> {
                Self::read_at(POLICY_FD, LAUNCH_FD, budget)
            }

            fn read_at(
                policy_fd: RawFd,
                launch_fd: RawFd,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, $Storage)> {
                budget.with_prepaid_scope(
                    Self::INPUT_STORAGE,
                    ENTRY_WORK,
                    READ_WORK,
                    Self::FRAME_STORAGE,
                    |budget| {
                        preflight_inputs(policy_fd, launch_fd)?;
                        let (policy, charge) = read_policy(policy_fd, budget)?;
                        budget.reserve_storage(charge)?;
                        let (launch, charge) = read_launch(launch_fd, budget)?;
                        budget.reserve_storage(charge)?;
                        let inputs = Self { policy, launch };
                        inputs.check(budget)?;
                        Ok((inputs, $Storage(RETAINED)))
                    },
                )
            }

            /// Rechecks both retained kernel objects, exact bytes and their native join.
            pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
                budget.with_prepaid_scope(
                    RETAINED,
                    ENTRY_WORK,
                    ENTRY_WORK,
                    Self::FRAME_STORAGE,
                    |budget| self.check(budget),
                )
            }

            fn check(&self, budget: &mut Budget<'_>) -> Result<()> {
                self.policy.revalidate(budget)?;
                self.launch.revalidate(budget)?;
                if !self
                    .launch
                    .manifest()
                    .matches_policy(self.policy.policy(), budget)?
                {
                    return Err($Failure::PolicyMismatch);
                }
                Ok(())
            }

            /// Returns the separately admitted native policy, not signing authority.
            pub const fn policy(&self) -> &Policy {
                self.policy.policy()
            }
            /// Returns the policy-matched inert launch binding.
            pub const fn manifest(&self) -> &Manifest {
                self.launch.manifest()
            }
            /// Full retained logical charge; retire it only after dropping this owner.
            pub const fn retained_storage(&self) -> usize {
                RETAINED
            }
        }

        impl fmt::Debug for $Inputs {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Inputs))
                    .field("authority", &"none")
                    .field("policy", &self.policy().identity())
                    .field("manifest", &self.manifest().identity())
                    .finish_non_exhaustive()
            }
        }

        /// Bounded native launch-input diagnostic, without supplied paths or strings.
        #[derive(Debug)]
        pub enum $Failure {
            /// Shared ledger refused this operation.
            Resource(Resource),
            /// A sealed capability could not be independently admitted or revalidated.
            Capability(CapabilityError),
            /// Native manifest comparison was refused.
            Manifest(ManifestError),
            /// The manifest does not name the separately admitted native policy.
            PolicyMismatch,
        }
        type Result<T> = std::result::Result<T, $Failure>;
        impl From<Resource> for $Failure {
            fn from(e: Resource) -> Self {
                Self::Resource(e)
            }
        }
        impl From<CapabilityError> for $Failure {
            fn from(e: CapabilityError) -> Self {
                Self::Capability(e)
            }
        }
        impl From<ManifestError> for $Failure {
            fn from(e: ManifestError) -> Self {
                Self::Manifest(e)
            }
        }
        impl fmt::Display for $Failure {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Resource(e) => e.fmt(f),
                    Self::Capability(e) => e.fmt(f),
                    Self::Manifest(e) => e.fmt(f),
                    Self::PolicyMismatch => f.write_str("native issuer launch policy mismatch"),
                }
            }
        }
        impl Error for $Failure {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self {
                    Self::Resource(e) => Some(e),
                    Self::Capability(e) => Some(e),
                    Self::Manifest(e) => Some(e),
                    Self::PolicyMismatch => None,
                }
            }
        }

        const _: () = {
            use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as LedgerIdentity;
            const fn envelopes<T>() -> usize {
                size_of::<Result<T>>().saturating_sub(size_of::<T>())
                    + size_of::<std::thread::Result<Result<T>>>().saturating_sub(size_of::<T>())
            }
            assert!(
                envelopes::<($Inputs, $Storage)>()
                    + envelopes::<()>()
                    + size_of::<LedgerIdentity>()
                    + size_of::<std::result::Result<(), Resource>>()
                    + 2 * size_of::<bool>()
                    <= 512
            );
            assert!(
                size_of::<($Inputs, $Storage)>()
                    <= size_of::<(PolicyCapability, CapabilityStorage)>()
                        + size_of::<(LaunchCapability, CapabilityStorage)>()
            );
            assert!(8 * size_of::<$Failure>() + 64 * size_of::<usize>() + 512 <= 4096);
        };
    };
}
pub(super) use launch_inputs;
