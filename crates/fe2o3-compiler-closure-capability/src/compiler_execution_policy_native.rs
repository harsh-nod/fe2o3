//! Shared sealed native policy body; only nominal family and memfd label vary.
macro_rules! policy_capability {
    ($Cap:ident, $memfd:literal) => {
        type Capability = NativeCapability<Policy, BYTES>;
        impl Record<BYTES> for Policy {
            const ROLE: CapabilityRole = CapabilityRole {
                name: "native compiler-execution policy capability",
                memfd_name: $memfd,
            };
            fn bytes(&self) -> &[u8; BYTES] {
                self.canonical_bytes()
            }
            fn retained_storage(&self) -> usize {
                self.retained_storage()
            }
            fn decode_retained(bytes: &[u8; BYTES], budget: &mut Budget<'_>) -> Result<Self> {
                let (policy, storage) = Self::decode(bytes, budget)?;
                budget.reserve_storage(storage.additional_storage())?;
                Ok(policy)
            }
        }

        impl $Cap {
            /// Outer logical work. `from_file`/`from_inherited_at` additionally charge
            /// the native policy decoder on this same ledger. Not an instruction bound.
            pub const IO_WORK: usize = Capability::IO_WORK;
            /// Additional outer scratch; decoding separately reserves its own scratch
            /// and retained policy. Not heap, generated stack, wall-time, or RSS bounds.
            pub const IO_STORAGE: usize = Capability::IO_STORAGE;
            /// Full logical charge for an incoming/outgoing File and its sealed image.
            pub const FILE_STORAGE: usize = Capability::FILE_STORAGE;

            pub fn create(policy: Policy, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::create(policy, budget).map(|(value, storage)| (Self(value), storage))
            }
            /// Consumes a prepaid File and returns only the retained-storage delta.
            pub fn from_file(image: File, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::from_file(image, budget).map(|(value, storage)| (Self(value), storage))
            }
            /// Borrows a non-CLOEXEC inherited fd >= 3; never owns/closes it. The
            /// caller must keep that source live and prepaid at FILE_STORAGE throughout
            /// this call. The newly duplicated capability returns its FULL charge.
            /// No descriptor number is installed or reserved by this API.
            pub fn from_inherited_at(
                fd: RawFd,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                Capability::from_inherited_at(fd, budget)
                    .map(|(value, storage)| (Self(value), storage))
            }
            pub const fn policy(&self) -> &Policy {
                &self.0.record
            }
            pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
                self.0.revalidate(budget)
            }
            /// Returns a separately charged CLOEXEC File for one transport owner.
            pub fn try_clone_for_transfer(
                &self,
                budget: &mut Budget<'_>,
            ) -> Result<(File, Storage)> {
                self.0.try_clone_for_transfer(budget)
            }
            /// Revalidates this owner and a borrowed CLOEXEC transfer against the original
            /// sealed object and bytes. Prepay `retained_storage() + FILE_STORAGE` on
            /// the same ledger. Charges IO_WORK and IO_STORAGE scratch, restoring entry
            /// storage without creating, retaining, closing or moving either descriptor.
            pub fn validate_transfer(
                &self,
                transfer: &File,
                budget: &mut Budget<'_>,
            ) -> Result<()> {
                self.0.validate_transfer(transfer, budget)
            }
            pub const fn retained_storage(&self) -> usize {
                Capability::RETAINED
            }
        }

        const _: () = {
            Capability::assert_layout::<$Cap>();
        };
    };
}
pub(crate) use policy_capability;
