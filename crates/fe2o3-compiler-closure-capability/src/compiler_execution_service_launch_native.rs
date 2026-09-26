// Native launch custody shares exact-object I/O, while manifests stay nominal.
macro_rules! launch_capability {
    ($Cap:ident, $version:literal) => {
        type Capability = NativeCapability<Manifest, BYTES>;
        impl Record<BYTES> for Manifest {
            const ROLE: CapabilityRole = CapabilityRole {
                name: "native compiler-execution service launch capability",
                memfd_name: concat!("fe2o3-compiler-execution-service-launch-v", $version),
            };
            fn bytes(&self) -> &[u8; BYTES] {
                self.canonical_bytes()
            }
            fn retained_storage(&self) -> usize {
                self.retained_storage()
            }
            fn decode_retained(bytes: &[u8; BYTES], budget: &mut Budget<'_>) -> Result<Self> {
                let (manifest, storage) = Self::decode(bytes, budget)?;
                budget.reserve_storage(storage.additional_storage())?;
                Ok(manifest)
            }
        }

        /// Move-only sealed identity frame, using the unchanged launch wire. Structural
        /// admission is not a native policy match or any process/readiness authority.
        /// Consumers must validate the exact pinned native policy before use.
        ///
        /// Inputs stay prepaid on the same ledger and all operations restore entry
        /// storage. Reserve each returned delta before retention; retire full owner
        /// charges after drop/transfer. On consuming failure retire the dropped input
        /// reservation after return. No external Command inheritance callback is installed.
        ///
        #[doc = concat!("```compile_fail\nuse fe2o3_compiler_closure_capability::", stringify!($Cap), ";\nfn duplicate(value: ", stringify!($Cap), ") { let _ = value.clone(); }\n```\n```compile_fail\nuse fe2o3_compiler_closure_capability::", stringify!($Cap), ";\nfn descriptor<T: std::os::fd::AsFd>() {}\ndescriptor::<", stringify!($Cap), ">();\n```")]
        pub struct $Cap(Capability);
        impl $Cap {
            /// Fixed outer work; admission additionally charges the shared frame decoder.
            pub const IO_WORK: usize = Capability::IO_WORK;
            /// Additional outer logical scratch, not generated stack, allocator or RSS bounds.
            pub const IO_STORAGE: usize = Capability::IO_STORAGE;
            pub const FILE_STORAGE: usize = Capability::FILE_STORAGE;

            pub fn create(manifest: Manifest, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::create(manifest, budget).map(|(value, storage)| (Self(value), storage))
            }
            pub fn from_file(image: File, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::from_file(image, budget).map(|(value, storage)| (Self(value), storage))
            }
            /// Borrows a non-CLOEXEC fd >= 3, prepaid at FILE_STORAGE. Does not own or
            /// close that source slot; its caller must retain it unchanged during admission.
            pub fn from_inherited_at(fd: RawFd, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Capability::from_inherited_at(fd, budget).map(|(value, storage)| (Self(value), storage))
            }
            pub const fn manifest(&self) -> &Manifest {
                &self.0.record
            }
            pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
                self.0.revalidate(budget)
            }
            pub fn try_clone_for_transfer(&self, budget: &mut Budget<'_>) -> Result<(File, Storage)> {
                self.0.try_clone_for_transfer(budget)
            }
            /// Revalidates this owner and a borrowed CLOEXEC transfer against the original
            /// sealed object and bytes. Prepay `retained_storage() + FILE_STORAGE` on
            /// the same ledger. Charges IO_WORK and IO_STORAGE scratch, restoring entry
            /// storage without creating, retaining, closing or moving either descriptor.
            /// This does not establish a native policy match or launch authority.
            pub fn validate_transfer(&self, transfer: &File, budget: &mut Budget<'_>) -> Result<()> {
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
pub(crate) use launch_capability;
