//! Only nominal key/deployment types vary; persistence is never instantiated here.
macro_rules! native_anchor {
    ($Owner:ident, $version:literal, $other:literal) => {
        use crate::{
            durable_core::DurableAnchorCoreV1 as Core,
            native::{NativeExternalAnchorErrorV2 as Error, NativeExternalAnchorStorageV2 as Storage, OpenMode},
            DurableExternalAnchorOpenDispositionV1 as Disposition, ExternalAnchorServiceErrorV1 as StateError,
            EXTERNAL_ANCHOR_STATE_BYTES_V1 as STATE_BYTES, NoopPersistenceHooksV1, PersistenceHooksV1,
        };
        use fe2o3_external_anchor_protocol::{
            AnchorChallengeV1, HashChainHeadV1, PinnedAnchorKeyV1,
            ANCHOR_CHALLENGE_WIRE_LEN_V1 as CHALLENGE_BYTES,
            ANCHOR_OBSERVATION_WIRE_LEN_V1 as OBSERVATION_BYTES,
        };
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        };
        use std::{fmt, mem::size_of, os::fd::OwnedFd};
        type Result<T> = std::result::Result<T, Error>;

        /// Native key-owning durable anchor over the shared V1 persistence protocol.
        /// The existing state file format and observation wire are deliberately unchanged.
        /// No raw key, descriptor, clone or V1-owner upgrade is exposed. The caller must
        /// independently establish protected deployment, lifecycle and process custody.
        ///
        /// Constructors consume a prepaid ROOT_STORAGE descriptor and key; the actual
        /// same-family deployment remains borrowed/prepaid. Reserve returned GROWTH
        /// before retaining the owner, and retire FULL retained_storage() after Drop.
        /// Consuming errors close inputs but leave their reservations for caller cleanup.
        /// Exchange borrows the full owner, deployment and exact-sized challenge wire;
        /// it returns the FULL unreserved observation charge. Malformed lengths are not
        /// scanned and need no wire floor. All scopes use the original ledger and preserve
        /// work, peak and first-denial history on error and unwind.
        ///
        /// An advance is durably persisted BEFORE signing. A later resource/key refusal
        /// returns no response, but may leave a committed advance: retry/recovery observes
        /// that exact state. An uncertain persistence error/unwind poisons this instance
        /// and requires reopening. This is not protected startup or compiler authority.
        ///
        /// ```
        #[doc = concat!("use fe2o3_external_anchor_service::{", stringify!($Owner), " as Anchor, NativeExternalAnchorErrorV2 as Error, NativeExternalAnchorStorageV2 as Storage};")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn exchange(a: &mut Anchor, d: &Deployment, bytes: &[u8], b: &mut Budget<'_>)
        ///     -> Result<([u8; 288], Storage), Error> { a.exchange(bytes, d, b) }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_service::", stringify!($Owner), " as Anchor;")]
        /// fn copy(a: Anchor) { let _ = a.clone(); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_service::", stringify!($Owner), " as Anchor;")]
        /// fn descriptor<T: std::os::fd::AsFd>() {} descriptor::<Anchor>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_service::{", stringify!($Owner), " as Anchor, DurableExternalAnchorV1 as Old};")]
        /// fn upgrade(a: Old) -> Anchor { a.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_service::", stringify!($Owner), " as Anchor;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV", $other, " as Deployment;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(a: &mut Anchor, d: &Deployment, b: &mut Budget<'_>) { let _ = a.exchange(&[], d, b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_external_anchor_service::", stringify!($Owner), " as Anchor;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn raw(root: std::os::fd::OwnedFd, key: ed25519_dalek::SigningKey, d: &Deployment, b: &mut Budget<'_>) {
        ///     let _ = Anchor::open(root, key, d, b);
        /// }
        /// ```
        pub struct $Owner {
            core: Core,
            key: Key,
        }
        impl $Owner {
            pub const ROOT_STORAGE: usize = size_of::<(OwnedFd, Storage)>();
            // Includes the whole wrapper and a conservative sealed-image File charge.
            const RETAINED: usize = size_of::<(Self, Storage)>() + Key::FILE_STORAGE;
            pub const STATE_WORK: usize = 8 + 32 * 1024 + 32 * (STATE_BYTES + CHALLENGE_BYTES);
            pub const OPEN_WORK: usize = Self::STATE_WORK + Key::KEY_VALIDATION_WORK;
            pub const ADMISSION_WORK: usize = Self::OPEN_WORK + 2 * Key::IO_WORK;
            pub const EXCHANGE_WORK: usize = Self::STATE_WORK + Key::IO_WORK + Key::OBSERVATION_WORK;
            /// Fixed logical scratch, not a generated-stack, syscall-time or RSS bound.
            pub const STATE_STORAGE: usize = 4 * Self::RETAINED + 4 * STATE_BYTES
                + 4 * size_of::<AnchorChallengeV1>() + 4 * OBSERVATION_BYTES
                + 4 * size_of::<std::fs::Metadata>() + 8192;
            pub const ADMISSION_STORAGE: usize = Self::STATE_STORAGE + Key::IO_STORAGE;
            pub const EXCHANGE_STORAGE: usize = Self::ADMISSION_STORAGE;

            pub fn initialize(root: OwnedFd, key: Key, d: &Deployment, b: &mut Budget<'_>)
                -> Result<(Self, Storage)> {
                Self::admit(root, key, d, b, OpenMode::Initialize).map(|((a, _), s)| (a, s))
            }
            pub fn open(root: OwnedFd, key: Key, d: &Deployment, b: &mut Budget<'_>)
                -> Result<(Self, Storage)> {
                Self::admit(root, key, d, b, OpenMode::Existing).map(|((a, _), s)| (a, s))
            }
            pub fn open_or_initialize(root: OwnedFd, key: Key, d: &Deployment, b: &mut Budget<'_>)
                -> Result<((Self, Disposition), Storage)> {
                Self::admit(root, key, d, b, OpenMode::OpenOrInitialize)
            }
            fn admit(root: OwnedFd, key: Key, d: &Deployment, b: &mut Budget<'_>, mode: OpenMode)
                -> Result<((Self, Disposition), Storage)> {
                let consumed = Self::ROOT_STORAGE.checked_add(key.retained_storage()).ok_or(Resource::Arithmetic)?;
                let floor = consumed.checked_add(d.retained_storage()).ok_or(Resource::Arithmetic)?;
                b.with_prepaid_scope(floor, 8, Self::OPEN_WORK, Self::STATE_STORAGE, |b| {
                    require_service(d)?;
                    key.revalidate(d, b)?;
                    let pinned = PinnedAnchorKeyV1::from_bytes(key.verifying_key()).map_err(StateError::from)?;
                    let (core, disposition) = match mode {
                        OpenMode::Initialize => (Core::initialize(root, pinned)?, Disposition::Initialized),
                        OpenMode::Existing => (Core::open(root, pinned)?, Disposition::Existing),
                        OpenMode::OpenOrInitialize => Core::open_or_initialize(root, pinned)?,
                    };
                    key.revalidate(d, b)?;
                    require_service(d)?;
                    let growth = Self::RETAINED.checked_sub(consumed).ok_or(Resource::Accounting)?;
                    Ok(((Self { core, key }, disposition), Storage(growth)))
                })
            }

            pub fn exchange(&mut self, bytes: &[u8], d: &Deployment, b: &mut Budget<'_>)
                -> Result<([u8; OBSERVATION_BYTES], Storage)> {
                self.exchange_with_hooks(bytes, d, b, &mut NoopPersistenceHooksV1, || {})
            }
            // The peer loop prepays the full owner/context and credential-check frame.
            pub(crate) fn validate_service(&self, d: &Deployment, b: &mut Budget<'_>) -> Result<()> {
                require_service(d)?;
                self.key.revalidate(d, b)?;
                require_service(d)
            }
            fn exchange_with_hooks(&mut self, bytes: &[u8], d: &Deployment, b: &mut Budget<'_>,
                hooks: &mut impl PersistenceHooksV1, after_persistence: impl FnOnce())
                -> Result<([u8; OBSERVATION_BYTES], Storage)> {
                let wire_floor = if bytes.len() == CHALLENGE_BYTES { CHALLENGE_BYTES } else { 0 };
                let floor = Self::RETAINED.checked_add(d.retained_storage())
                    .and_then(|n| n.checked_add(wire_floor)).ok_or(Resource::Arithmetic)?;
                b.with_prepaid_scope(floor, 8, Self::STATE_WORK, Self::STATE_STORAGE, |b| {
                    require_service(d)?;
                    self.key.revalidate(d, b)?;
                    let (challenge, position) = self.core.observe_with_hooks(bytes, hooks)?;
                    after_persistence();
                    require_service(d)?;
                    let (wire, charge) = self.key.sign_observation(d, &challenge, position, b)?;
                    require_service(d)?;
                    Ok((wire, Storage(charge.additional_storage())))
                })
            }
            pub const fn sequence(&self) -> u64 { self.core.sequence() }
            pub const fn head(&self) -> HashChainHeadV1 { self.core.head() }
            pub fn verifying_key_bytes(&self) -> [u8; 32] { self.key.verifying_key() }
            pub const fn retained_storage(&self) -> usize { Self::RETAINED }
        }
        fn require_service(d: &Deployment) -> Result<()> {
            let uid = rustix::process::geteuid().as_raw();
            let gid = rustix::process::getegid().as_raw();
            if uid == 0 || gid == 0 || uid != d.service().uid() || gid != d.service().gid() {
                return Err(Error::ServiceCredentials);
            }
            Ok(())
        }
        impl fmt::Debug for $Owner {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Owner))
                    .field("sequence", &self.sequence()).field("head", &self.head())
                    .field("deployment", &self.key.deployment_identity()).finish_non_exhaustive()
            }
        }
        const _: () = {
            use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityStorageV2 as KeyStorage;
            assert!(size_of::<Storage>() == size_of::<KeyStorage>());
        };
    };
}
pub(crate) use native_anchor;
