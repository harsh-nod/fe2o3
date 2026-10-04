//! Private native admission bodies; policy/key types resolve in each family module.

macro_rules! program {
    ($program:ident, $storage:ident, $error:ident) => {
        /// Unreserved additional storage returned by native program admission.
        #[derive(Clone, Copy, Debug)]
        pub struct $storage(usize);
        impl $storage {
            /// Preserve consumed reservations and reserve this delta before retention.
            pub const fn additional_storage(self) -> usize {
                self.0
            }
        }

        impl $program {
            /// Outer logical work, including two fixed runtime-closure derivations,
            /// four credential syscalls and constant ownership bookkeeping.
            /// Native capability/image operations charge separately on the same ledger.
            pub const WORK: usize = ENTRY + 64 * SEALED_STATIC_ISSUER_RUNTIME_CLOSURE_V1.len() + 5 * 1024;
            /// Outer logical ownership/control scratch, not allocator/RSS/stack bounds.
            pub const SCRATCH: usize = 4 * size_of::<(Self, $storage)>() + 4096;

            /// Consumes a prepaid native policy and both source File/image reservations.
            /// Sources may alias; retained launcher and issuer memfds must not.
            pub fn provision(
                launcher_source: File,
                launcher_expected: Provisioned,
                issuer_source: File,
                policy: PolicyCapability,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, $storage)> {
                budget.charge_work(ENTRY)?;
                let launcher_measurement = protected_measurement(launcher_expected, "static launcher")?;
                // The issuer's size validity is checked after launcher admission, as in
                // V1. Resource preflight only accounts the caller's declared File image.
                let issuer_length = usize::try_from(policy.policy().executable().byte_len())
                    .map_err(|_| Resource::Arithmetic)?;
                let issuer_floor = issuer_length
                    .checked_add(size_of::<(File, ImageStorage)>())
                    .ok_or(Resource::Arithmetic)?;
                let floor = policy
                    .retained_storage()
                    .checked_add(Image::file_storage(launcher_measurement)?)
                    .and_then(|n| n.checked_add(issuer_floor))
                    .ok_or(Resource::Arithmetic)?;
                budget.with_prepaid_scope(floor, 0, Self::WORK - ENTRY, Self::SCRATCH, |budget| {
                    policy.revalidate(budget)?;
                    require_runtime(policy.policy())?;
                    let (launcher, delta) = Image::seal_source_for_owner(
                        launcher_source,
                        launcher_measurement,
                        Owner::current(),
                        "static launcher",
                        budget,
                    )?;
                    budget.reserve_storage(delta.additional_storage())?;
                    let issuer_expected = Provisioned::from_issuer_policy(policy.policy().executable())?;
                    let (issuer, delta) = Image::seal_source_for_owner(
                        issuer_source,
                        protected_measurement(issuer_expected, "compiler issuer")?,
                        Owner::current(),
                        "compiler issuer",
                        budget,
                    )?;
                    budget.reserve_storage(delta.additional_storage())?;
                    let admitted = Self {
                        launcher,
                        issuer,
                        policy,
                    };
                    admitted.check(budget)?;
                    let delta = admitted
                        .retained_storage()
                        .checked_sub(floor)
                        .ok_or(Resource::Accounting)?;
                    Ok((admitted, $storage(delta)))
                })
            }

            /// Revalidates native policy/runtime and both exact retained image owners.
            pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
                budget.with_prepaid_scope(
                    self.retained_storage(),
                    ENTRY,
                    Self::WORK,
                    Self::SCRATCH,
                    |budget| self.check(budget),
                )
            }

            fn check(&self, budget: &mut Budget<'_>) -> Result<()> {
                self.policy.revalidate(budget)?;
                require_runtime(self.policy())?;
                if self.policy().executable().sha256() != self.issuer.measurement().sha256()
                    || self.policy().executable().byte_len() != self.issuer.measurement().byte_len()
                {
                    return Err($error::PolicyImageMismatch);
                }
                self.launcher.revalidate(budget)?;
                self.issuer.revalidate(budget)?;
                let l = self.launcher.object_identity();
                let i = self.issuer.object_identity();
                if l.device() == i.device() && l.inode() == i.inode() {
                    return Err($error::AliasedImages);
                }
                Ok(())
            }

            /// Caller-pinned native policy, without exposing its retained descriptor.
            pub const fn policy(&self) -> &Policy {
                self.policy.policy()
            }
            /// Trusted measurement of the admitted launcher.
            pub const fn launcher_measurement(&self) -> Measurement {
                self.launcher.measurement()
            }
            /// Trusted measurement of the admitted issuer.
            pub const fn issuer_measurement(&self) -> Measurement {
                self.issuer.measurement()
            }
            /// Inert launcher object identity for the shared static descriptor manifest.
            pub fn launcher_object_identity(&self) -> Object {
                object(&self.launcher)
            }
            /// Inert issuer object identity for the shared static descriptor manifest.
            pub fn issuer_object_identity(&self) -> Object {
                object(&self.issuer)
            }

            /// Returns a separately charged CLOEXEC executable File for controlled transport.
            /// Does not meter arbitrary subsequent File operations or grant process authority.
            pub fn try_clone_launcher_for_launch(
                &self,
                budget: &mut Budget<'_>,
            ) -> Result<(File, ImageStorage)> {
                self.transport(0, budget, |b| Ok(self.launcher.try_clone_for_exec(b)?))
            }
            /// Returns a separately charged CLOEXEC issuer executable File.
            pub fn try_clone_issuer_for_launch(
                &self,
                budget: &mut Budget<'_>,
            ) -> Result<(File, ImageStorage)> {
                self.transport(0, budget, |b| Ok(self.issuer.try_clone_for_exec(b)?))
            }
            /// Revalidates the launcher File as the retained kernel object, not merely its bytes.
            pub fn revalidate_launcher_clone(&self, image: &File, budget: &mut Budget<'_>) -> Result<()> {
                self.transport(
                    Image::file_storage(self.launcher.measurement())?,
                    budget,
                    |b| Ok(self.launcher.revalidate_exec_clone(image, b)?),
                )
            }
            /// Revalidates the issuer File as the retained kernel object, not merely its bytes.
            pub fn revalidate_issuer_clone(&self, image: &File, budget: &mut Budget<'_>) -> Result<()> {
                self.transport(
                    Image::file_storage(self.issuer.measurement())?,
                    budget,
                    |b| Ok(self.issuer.revalidate_exec_clone(image, b)?),
                )
            }
            pub(super) fn try_clone_policy_for_launch(
                &self,
                budget: &mut Budget<'_>,
            ) -> Result<(
                File,
                fe2o3_compiler_closure_capability::CompilerExecutionCapabilityStorageV2,
            )> {
                self.transport(0, budget, |b| Ok(self.policy.try_clone_for_transfer(b)?))
            }
            pub(super) fn revalidate_policy_clone(
                &self,
                image: &File,
                budget: &mut Budget<'_>,
            ) -> Result<()> {
                self.transport(PolicyCapability::FILE_STORAGE, budget, |b| {
                    Ok(self.policy.validate_transfer(image, b)?)
                })
            }
            fn transport<T>(
                &self,
                extra: usize,
                budget: &mut Budget<'_>,
                action: impl FnOnce(&mut Budget<'_>) -> Result<T>,
            ) -> Result<T> {
                let floor = self
                    .retained_storage()
                    .checked_add(extra)
                    .ok_or(Resource::Arithmetic)?;
                budget.with_prepaid_scope(floor, ENTRY, Self::WORK, Self::SCRATCH, action)
            }
            /// Full retained logical charge; retire only after dropping this program.
            pub fn retained_storage(&self) -> usize {
                self.launcher.retained_storage()
                    + self.issuer.retained_storage()
                    + self.policy.retained_storage()
            }
        }

        fn require_runtime(policy: &Policy) -> Result<()> {
            if policy.runtime() != sealed_static_issuer_runtime_measurement_v1() {
                return Err($error::RuntimePolicyMismatch);
            }
            Ok(())
        }
        fn object(image: &Image) -> Object {
            let o = image.object_identity();
            Object::new(o.device(), o.inode(), o.byte_len(), o.mode())
        }
        impl fmt::Debug for $program {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($program))
                    .field("authority", &"none")
                    .field("launcher", &self.launcher_measurement())
                    .field("issuer", &self.issuer_measurement())
                    .field("policy", &self.policy().identity())
                    .finish_non_exhaustive()
            }
        }

        /// Bounded native program admission failure, without fallback to V1 authority.
        #[derive(Debug)]
        pub enum $error {
            /// Input/outer shared-ledger refusal.
            Resource(Resource),
            /// Native sealed policy refusal.
            Policy(CapabilityError),
            /// Native sealed executable refusal.
            Image(ImageError),
            /// Trusted measurement is invalid under the existing provisioning bound.
            Measurement(LegacyError),
            /// Runtime does not match the sealed-static issuer profile.
            RuntimePolicyMismatch,
            /// Exact policy executable measurement differs from the retained issuer.
            PolicyImageMismatch,
            /// Launcher and issuer unexpectedly refer to the same retained object.
            AliasedImages,
        }
        impl From<Resource> for $error {
            fn from(e: Resource) -> Self {
                Self::Resource(e)
            }
        }
        impl From<CapabilityError> for $error {
            fn from(e: CapabilityError) -> Self {
                Self::Policy(e)
            }
        }
        impl From<ImageError> for $error {
            fn from(e: ImageError) -> Self {
                Self::Image(e)
            }
        }
        impl From<LegacyError> for $error {
            fn from(e: LegacyError) -> Self {
                Self::Measurement(e)
            }
        }
        impl fmt::Display for $error {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Resource(e) => e.fmt(f),
                    Self::Policy(e) => e.fmt(f),
                    Self::Image(e) => e.fmt(f),
                    Self::Measurement(e) => e.fmt(f),
                    Self::RuntimePolicyMismatch => f.write_str("native issuer runtime policy mismatch"),
                    Self::PolicyImageMismatch => f.write_str("native issuer executable policy mismatch"),
                    Self::AliasedImages => f.write_str("native issuer program has aliased retained images"),
                }
            }
        }
        impl Error for $error {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self {
                    Self::Resource(e) => Some(e),
                    Self::Policy(e) => Some(e),
                    Self::Image(e) => Some(e),
                    Self::Measurement(e) => Some(e),
                    _ => None,
                }
            }
        }

        const _: () = {
            use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityStorageV2 as PolicyStorage;
            use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as LedgerIdentity;
            const fn envelope<T>() -> usize {
                size_of::<Result<T>>().saturating_sub(size_of::<T>())
                    + size_of::<std::thread::Result<Result<T>>>().saturating_sub(size_of::<T>())
            }
            assert!(
                size_of::<LedgerIdentity>()
                    + size_of::<std::result::Result<(), Resource>>()
                    + 2 * size_of::<bool>()
                    + 2 * size_of::<sha2::Sha256>()
                    + envelope::<($program, $storage)>()
                    + envelope::<(File, ImageStorage)>()
                    + envelope::<()>()
                    <= 1024
            );
            assert!(
                size_of::<($program, $storage)>()
                    <= 2 * size_of::<(Image, ImageStorage)>()
                        + size_of::<(PolicyCapability, PolicyStorage)>()
            );
            assert!(
                8 * size_of::<$error>() + 64 * size_of::<usize>() + 1024 <= 4096
            );
        };

    };
}
pub(crate) use program;

macro_rules! authority {
    ($supervisor:ident, $storage:ident, $error:ident, $program_storage:ident) => {
        /// Unreserved growth over the consumed program, key, anchor and root charges.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub struct $storage(usize);
        impl Storage {
            /// Preserve input reservations and reserve this delta before retaining the result.
            pub const fn additional_storage(self) -> usize {
                self.0
            }
        }

        impl $supervisor {
            /// Logical input charge for the consumed root File, including receipt padding.
            pub const ROOT_FILE_STORAGE: usize = size_of::<(File, Storage)>();
            /// Fixed ownership growth for the root snapshot, credentials and outer bookkeeping.
            pub const OWNER_GROWTH: usize =
                size_of::<(RootSnapshot, Credentials, usize, Storage)>() + align_of::<Self>();
            /// Outer logical allowance: entry plus 64 weighted credential/root/cleanup calls.
            /// Program, key and anchor operations additionally charge the same ledger.
            pub const WORK: usize = ENTRY + 64 * 1024;
            /// Outer logical staging/control scratch, excluding nested operations' scratch.
            /// This is not a bound on generated stack, RSS, syscall latency or kernel memory.
            pub const SCRATCH: usize = 4 * size_of::<(Self, Storage)>() + 4096;

            /// Consumes prepaid native inputs and root custody, returning only owner growth.
            /// Validation order is credentials, program, exact-policy key, anchor, root,
            /// then full revalidation. Equal public keys do not substitute policy identities.
            pub fn bind(
                program: Program,
                credentials: Credentials,
                root: File,
                signing_key: Key,
                external_anchor: Anchor,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                budget.charge_work(ENTRY)?;
                let floor = program
                    .retained_storage()
                    .checked_add(signing_key.retained_storage())
                    .and_then(|n| n.checked_add(external_anchor.retained_storage()))
                    .and_then(|n| n.checked_add(Self::ROOT_FILE_STORAGE))
                    .ok_or(Resource::Arithmetic)?;
                let retained = floor
                    .checked_add(Self::OWNER_GROWTH)
                    .ok_or(Resource::Arithmetic)?;
                budget.with_prepaid_scope(floor, 0, Self::WORK - ENTRY, Self::SCRATCH, |budget| {
                    require_credentials(credentials)?;
                    program.revalidate(budget)?;
                    signing_key.revalidate(program.policy(), budget)?;
                    external_anchor.validate_continuity(budget)?;
                    let root_snapshot = root_checks::inspect(&root, credentials)?;
                    if root_checks::inspect(&root, credentials)? != root_snapshot {
                        return Err($error::RootChanged);
                    }
                    let supervisor = Self {
                        program,
                        credentials,
                        root,
                        root_snapshot,
                        signing_key,
                        external_anchor,
                        retained,
                    };
                    budget.reserve_storage(Self::OWNER_GROWTH)?;
                    supervisor.check(budget)?;
                    Ok((supervisor, Storage(Self::OWNER_GROWTH)))
                })
            }

            /// Rechecks the complete native pre-session chain, including root object identity.
            pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
                budget.with_prepaid_scope(self.retained, ENTRY, Self::WORK, Self::SCRATCH, |budget| {
                    self.check(budget)
                })
            }

            fn check(&self, budget: &mut Budget<'_>) -> Result<()> {
                require_credentials(self.credentials)?;
                self.program.revalidate(budget)?;
                self.signing_key.revalidate(self.program.policy(), budget)?;
                self.external_anchor.validate_continuity(budget)?;
                if root_checks::inspect(&self.root, self.credentials)? != self.root_snapshot {
                    return Err($error::RootChanged);
                }
                Ok(())
            }

            /// Returns immutable caller-pinned native policy facts, not a descriptor.
            pub const fn policy(&self) -> &Policy {
                self.program.policy()
            }
            /// Returns the configured dedicated service identity, not process confinement evidence.
            pub const fn credentials(&self) -> Credentials {
                self.credentials
            }
            /// Returns the provisioned anchor service identity, without its endpoint.
            pub const fn external_anchor_service(&self) -> AnchorIdentity {
                self.external_anchor.service_identity()
            }
            /// Returns cached anchor process facts; continuity still requires revalidation.
            pub const fn external_anchor_process(
                &self,
            ) -> fe2o3_broker_authority_service::ExpectedClientProcessIdentityV1 {
                self.external_anchor.service_process_identity()
            }
            /// Full retained charge; retire it only after this owner is dropped or transferred.
            pub const fn retained_storage(&self) -> usize {
                self.retained
            }
        }

        fn require_credentials(credentials: Credentials) -> Result<()> {
            if rustix::process::geteuid().as_raw() != credentials.uid()
                || rustix::process::getegid().as_raw() != credentials.gid()
            {
                return Err($error::ServiceIdentityMismatch);
            }
            Ok(())
        }

        impl fmt::Debug for $supervisor {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($supervisor))
                    .field("authority", &"pre-session-custody-only")
                    .field("policy", &self.policy().identity())
                    .field("credentials", &self.credentials)
                    .field("external_anchor_service", &self.external_anchor_service())
                    .finish_non_exhaustive()
            }
        }

        /// Native supervisor binding or revalidation failure. No refusal retries V1.
        #[derive(Debug)]
        #[non_exhaustive]
        pub enum $error {
            /// The caller's logical work or storage ledger refused the operation.
            Resource(Resource),
            /// Current effective UID/GID does not match the configured non-root service.
            ServiceIdentityMismatch,
            /// The independently admitted native program or policy failed revalidation.
            Program(ProgramError),
            /// Native key custody or its complete policy identity failed revalidation.
            SigningKey(KeyError),
            /// The exact provisioned endpoint or live anchor process failed continuity.
            ExternalAnchor(AnchorError),
            /// The root descriptor, access, type, owner, mode, links or xattrs are invalid.
            InvalidRoot(&'static str),
            /// Root identity or security metadata changed between observations.
            RootChanged,
            /// A single-attempt root inspection failed with a kernel errno.
            Io {
                /// Fixed operation label.
                operation: &'static str,
                /// Kernel error; no allocated diagnostic is retained.
                errno: rustix::io::Errno,
            },
        }
        impl From<Resource> for $error {
            fn from(e: Resource) -> Self {
                Self::Resource(e)
            }
        }
        impl From<ProgramError> for $error {
            fn from(e: ProgramError) -> Self {
                Self::Program(e)
            }
        }
        impl From<KeyError> for $error {
            fn from(e: KeyError) -> Self {
                Self::SigningKey(e)
            }
        }
        impl From<AnchorError> for $error {
            fn from(e: AnchorError) -> Self {
                Self::ExternalAnchor(e)
            }
        }
        impl From<RootCheckError> for $error {
            fn from(e: RootCheckError) -> Self {
                match e {
                    RootCheckError::Invalid(reason) => Self::InvalidRoot(reason),
                    RootCheckError::Io { operation, errno } => Self::Io { operation, errno },
                }
            }
        }
        impl fmt::Display for $error {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self {
                    Self::Resource(e) => e.fmt(f),
                    Self::ServiceIdentityMismatch => f.write_str(
                        "supervisor process does not match the protected issuer service UID and GID",
                    ),
                    Self::Program(e) => write!(f, "protected issuer program changed: {e}"),
                    Self::SigningKey(e) => write!(f, "protected issuer signing key changed: {e}"),
                    Self::ExternalAnchor(e) => write!(f, "protected external-anchor endpoint changed: {e}"),
                    Self::InvalidRoot(reason) => write!(f, "invalid protected issuer root: {reason}"),
                    Self::RootChanged => {
                        f.write_str("protected issuer root identity or security metadata changed")
                    }
                    Self::Io { operation, errno } => write!(f, "{operation}: {errno}"),
                }
            }
        }
        impl Error for $error {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                match self {
                    Self::Resource(e) => Some(e),
                    Self::Program(e) => Some(e),
                    Self::SigningKey(e) => Some(e),
                    Self::ExternalAnchor(e) => Some(e),
                    Self::Io { errno, .. } => Some(errno),
                    _ => None,
                }
            }
        }

        const _: () = {
            use crate::$program_storage as ProgramStorage;
            use fe2o3_broker_authority_service::ProtectedExternalAnchorServiceStorageV2 as AnchorStorage;
            use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityStorageV2 as KeyStorage;
            use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
            type Supervisor = $supervisor;
            const fn envelope<T>() -> usize {
                size_of::<Result<T>>().saturating_sub(size_of::<T>())
                    + size_of::<std::thread::Result<Result<T>>>().saturating_sub(size_of::<T>())
            }
            assert!(
                size_of::<(Supervisor, Storage)>()
                    <= size_of::<(Program, ProgramStorage)>()
                        + size_of::<(Key, KeyStorage)>()
                        + size_of::<(Anchor, AnchorStorage)>()
                        + Supervisor::ROOT_FILE_STORAGE
                        + Supervisor::OWNER_GROWTH
            );
            assert!(
                8 * size_of::<$error>()
                    + 64 * size_of::<usize>()
                    + 4 * size_of::<rustix::fs::Stat>()
                    + size_of::<Ledger>()
                    + size_of::<std::result::Result<(), Resource>>()
                    + 2 * size_of::<bool>()
                    + envelope::<(Supervisor, Storage)>()
                    + envelope::<()>()
                    <= 4096
            );
        };

    };
}
pub(crate) use authority;
