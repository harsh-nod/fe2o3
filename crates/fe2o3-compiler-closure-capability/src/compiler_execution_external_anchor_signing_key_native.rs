//! Native role/deployment binding over the existing sealed secret-image machinery.
macro_rules! anchor_signing_key {
    ($Cap:ident, $version:literal, $other:literal) => {
        use crate::{
            native_capability::{CompilerExecutionCapabilityErrorV2 as Error, ENTRY_WORK, Result, Storage, envelope_overhead},
            native_secret::{SeedGuard, read_secret, with_secret},
            sealed_image::{CapabilityRole, SealedCapabilityImage},
        };
        use ed25519_dalek::{Signer, SigningKey};
        use fe2o3_external_anchor_protocol::{
            ANCHOR_OBSERVATION_SIGNING_BYTES_V1, ANCHOR_OBSERVATION_WIRE_LEN_V1,
            AnchorChallengeV1, AnchorPositionV1, PinnedAnchorKeyV1, UnsignedAnchorObservationV1,
        };
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        };
        use std::{fmt, fs::File, mem::size_of, os::fd::RawFd};
        use subtle::ConstantTimeEq;

        const KEY_BYTES: usize = 32;
        const WIRE_BYTES: usize = 88;
        const SEED_OFFSET: usize = 56;
        const ROLE: CapabilityRole = CapabilityRole {
            name: "native external-anchor signing-key capability",
            memfd_name: concat!("fe2o3-external-anchor-signing-key-v", $version),
        };
        const HEADER: [u8; 24] = {
            let mut bytes = [0; 24];
            let magic = concat!("F2O3EAK", $version).as_bytes();
            let mut i = 0;
            while i < 8 { bytes[i] = magic[i]; i += 1; }
            bytes[8] = $version.as_bytes()[0] - b'0';
            bytes[12] = WIRE_BYTES as u8;
            bytes
        };

        /// Move-only native anchor key custody, bound to its role and exact deployment.
        /// No V1/issuer-key upgrade, raw key/seed accessor or raw descriptor is exposed.
        /// Transferred Files contain readable secrets and require trusted custody.
        /// A root-owned template and a service-owned image use the same native wire;
        /// ownership is checked separately. Neither establishes provisioning provenance.
        ///
        /// Inputs stay prepaid on the original ledger. Reserve returned growth before
        /// retention and retire FULL retained_storage() only after Drop. Consumed File
        /// failures close the descriptor but leave its reservation for caller cleanup.
        /// Seed and wire staging are guarded before admission and wiped on every exit
        /// and unwind. Prior copies and kernel-page erasure are not covered.
        ///
        /// Signing authenticates an exact caller-supplied observation, NOT durable
        /// state, currentness, compiler occurrence, publication, load or launch.
        /// The protected anchor must establish persistence before signing.
        ///
        /// ```
        #[doc = concat!("use fe2o3_compiler_closure_capability::{", stringify!($Cap), " as Cap, CompilerExecutionCapabilityErrorV2 as Error, CompilerExecutionCapabilityStorageV2 as Storage};")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment;")]
        /// use fe2o3_external_anchor_protocol::{AnchorChallengeV1, AnchorPositionV1, ANCHOR_OBSERVATION_WIRE_LEN_V1};
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn sign(k: &Cap, d: &Deployment, c: &AnchorChallengeV1, b: &mut Budget<'_>)
        ///     -> Result<([u8; ANCHOR_OBSERVATION_WIRE_LEN_V1], Storage), Error> {
        ///     k.sign_observation(d, c, AnchorPositionV1::Proposed, b)
        /// }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        /// fn copy(k: Cap) { let _ = k.clone(); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        /// fn fd<T: std::os::fd::AsFd>() {} fd::<Cap>();
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::{", stringify!($Cap), " as Cap, CompilerExecutionExternalAnchorSigningKeyCapabilityV1 as Old};")]
        /// fn upgrade(k: Old) -> Cap { k.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::{", stringify!($Cap), " as Cap, CompilerExecutionSigningKeyCapabilityV", $version, " as Issuer};")]
        /// fn substitute(k: Issuer) -> Cap { k.into() }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV", $other, " as Deployment;")]
        /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
        /// fn mix(f: std::fs::File, d: &Deployment, b: &mut Budget<'_>) { let _ = Cap::from_file(f, d, b); }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        /// fn expose(k: Cap) { let _: ed25519_dalek::SigningKey = k.key; }
        /// ```
        /// ```compile_fail
        #[doc = concat!("use fe2o3_compiler_closure_capability::", stringify!($Cap), " as Cap;")]
        #[doc = concat!("use fe2o3_compiler_execution_protocol::CompilerExecutionExternalAnchorDeploymentV", $version, " as Deployment;")]
        /// fn unmetered(f: std::fs::File, d: &Deployment) { let _ = Cap::from_file(f, d); }
        /// ```
        pub struct $Cap {
            key: SigningKey,
            image: SealedCapabilityImage,
            deployment: DeploymentIdentity,
        }

        impl $Cap {
            const RETAINED: usize = size_of::<(Self, Storage)>() + WIRE_BYTES;
            pub const FILE_STORAGE: usize = size_of::<(File, Storage)>() + WIRE_BYTES;
            /// Fixed logical crypto quotas, not instruction or elapsed-time bounds.
            pub const DERIVATION_WORK: usize = 65_536;
            pub const SIGN_WORK: usize = 65_536;
            pub const KEY_VALIDATION_WORK: usize = 65_536;
            /// At most 64 descriptor/credential calls, plus fixed staging/comparison.
            pub const IO_WORK: usize = ENTRY_WORK + 64 * 1024 + 32 * WIRE_BYTES;
            pub const ADMISSION_WORK: usize = Self::IO_WORK + Self::DERIVATION_WORK;
            /// Template and fresh-image postchecks add at most eight descriptor calls.
            pub const REISSUE_WORK: usize = Self::ADMISSION_WORK + 8 * 1024;
            pub const OBSERVATION_WORK: usize =
                2 * Self::IO_WORK + Self::KEY_VALIDATION_WORK + Self::SIGN_WORK;
            /// Fixed logical scratch, not a generated-stack, allocator or RSS bound.
            pub const IO_STORAGE: usize = 4 * Self::RETAINED + 4 * WIRE_BYTES
                + 4 * size_of::<std::fs::Metadata>() + 4096 + 4096
                + 4 * ANCHOR_OBSERVATION_WIRE_LEN_V1 + ANCHOR_OBSERVATION_SIGNING_BYTES_V1;

            pub fn create_and_zeroize(seed: &mut [u8; KEY_BYTES], d: &Deployment,
                b: &mut Budget<'_>) -> Result<(Self, Storage)> {
                let seed = SeedGuard(seed);
                Self::scope(b, input_floor(KEY_BYTES, d)?, Self::ADMISSION_WORK, |_| {
                    let key = SigningKey::from_bytes(seed.0);
                    require_key(&key, d)?;
                    let mut wire = [0; WIRE_BYTES];
                    let wire = SeedGuard(&mut wire);
                    wire.0[..24].copy_from_slice(&HEADER);
                    wire.0[24..SEED_OFFSET].copy_from_slice(d.identity().as_bytes());
                    wire.0[SEED_OFFSET..].copy_from_slice(seed.0);
                    let image = SealedCapabilityImage::create_fixed(wire.0, ROLE)?
                        .into_read_only_fixed::<WIRE_BYTES>()?;
                    let cap = Self { key, image, deployment: d.identity() };
                    cap.check_image()?;
                    Ok((cap, Storage(Self::RETAINED)))
                })
            }

            /// Consumes FILE_STORAGE; the actual deployment remains borrowed/prepaid.
            pub fn from_file(f: File, d: &Deployment, b: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Self::scope(b, input_floor(Self::FILE_STORAGE, d)?, Self::ADMISSION_WORK, |_| {
                    let image = SealedCapabilityImage::from_file_fixed::<WIRE_BYTES>(f, ROLE)?;
                    Ok((Self::decode_image(image, d)?, Storage(Self::RETAINED - Self::FILE_STORAGE)))
                })
            }

            /// Borrows an unchanged non-CLOEXEC fd >= 3; owns a private CLOEXEC duplicate.
            /// Source File and deployment charges stay live; the result returns FULL storage.
            pub fn from_inherited_at(fd: RawFd, d: &Deployment, b: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Self::scope(b, input_floor(Self::FILE_STORAGE, d)?, Self::ADMISSION_WORK, |_| {
                    let image = SealedCapabilityImage::from_inherited_fixed::<WIRE_BYTES>(fd, ROLE)?;
                    Ok((Self::decode_image(image, d)?, Storage(Self::RETAINED)))
                })
            }

            fn decode_image(image: SealedCapabilityImage, d: &Deployment) -> Result<Self> {
                let key = read_secret(&image, |wire: &[u8; WIRE_BYTES]| {
                    require_context(wire, d.identity())?;
                    let key = SigningKey::from_bytes(wire[SEED_OFFSET..].try_into().expect("fixed seed"));
                    require_key(&key, d)?;
                    Ok(key)
                })?;
                Ok(Self { key, image, deployment: d.identity() })
            }

            /// Consumes an independently provisioned UID/GID 0:0 template and creates
            /// a fresh image under the deployment's exact nonroot current credentials.
            /// Root ownership alone does not authenticate the deployment's provenance.
            pub fn reissue_root_template_for_current_service(f: File, d: &Deployment,
                b: &mut Budget<'_>) -> Result<(Self, Storage)> {
                Self::reissue_template(f, d, (0, 0), &mut [0; WIRE_BYTES], b, |_| Ok(()))
            }

            // Private observer exercises late error/unwind in the real scope; production
            // fixes template ownership to root and uses a no-op observer.
            fn reissue_template(f: File, d: &Deployment, owner: (u32, u32),
                wire: &mut [u8; WIRE_BYTES], b: &mut Budget<'_>,
                after_image: impl FnOnce(&Self) -> Result<()>) -> Result<(Self, Storage)> {
                let wire = SeedGuard(wire);
                Self::scope(b, input_floor(Self::FILE_STORAGE, d)?, Self::REISSUE_WORK, |_| {
                    require_service(d)?;
                    let template = SealedCapabilityImage::from_file_fixed::<WIRE_BYTES>(f, ROLE)?;
                    template.validate_secret_owner_fixed(owner.0, owner.1)?;
                    template.read_fixed_into(wire.0)?;
                    template.validate_secret_owner_fixed(owner.0, owner.1)?;
                    require_context(wire.0, d.identity())?;
                    let key = SigningKey::from_bytes(wire.0[SEED_OFFSET..].try_into().expect("fixed seed"));
                    require_key(&key, d)?;
                    let image = SealedCapabilityImage::create_fixed(wire.0, ROLE)?
                        .into_read_only_fixed::<WIRE_BYTES>()?;
                    let cap = Self { key, image, deployment: d.identity() };
                    after_image(&cap)?;
                    cap.check_image()?;
                    template.validate_secret_owner_fixed(owner.0, owner.1)?;
                    cap.image.validate_secret_owner_fixed(d.service().uid(), d.service().gid())?;
                    require_service(d)?;
                    Ok((cap, Storage(Self::RETAINED - Self::FILE_STORAGE)))
                })
            }

            pub fn revalidate(&self, d: &Deployment, b: &mut Budget<'_>) -> Result<()> {
                Self::scope(b, input_floor(Self::RETAINED, d)?, Self::IO_WORK, |_| self.check(d))
            }

            /// Signs the existing domain-separated anchor protocol, without exporting
            /// the key or allocating the message. The caller must have durably established
            /// the reported position; successful signing does not establish that fact.
            pub fn sign_observation(&self, d: &Deployment, challenge: &AnchorChallengeV1,
                position: AnchorPositionV1, b: &mut Budget<'_>)
                -> Result<([u8; ANCHOR_OBSERVATION_WIRE_LEN_V1], Storage)> {
                let floor = input_floor(Self::RETAINED, d)?
                    .checked_add(size_of::<AnchorChallengeV1>()).ok_or(Resource::Arithmetic)?;
                Self::scope(b, floor, Self::OBSERVATION_WORK, |_| {
                    self.check(d)?;
                    let pinned = PinnedAnchorKeyV1::from_bytes(self.verifying_key())
                        .map_err(|_| Error::Rejected("invalid anchor verification key"))?;
                    if pinned.identity() != challenge.anchor_key_identity() {
                        return Err(Error::Rejected("anchor challenge names another key"));
                    }
                    let unsigned = UnsignedAnchorObservationV1::from_challenge(challenge, position);
                    let signature = self.key.sign(&unsigned.signing_bytes_fixed()).to_bytes();
                    let wire = unsigned.attach_signature(signature);
                    self.check(d)?;
                    Ok((wire, Storage(size_of::<([u8; ANCHOR_OBSERVATION_WIRE_LEN_V1], Storage)>())))
                })
            }

            pub fn try_clone_for_transfer(&self, b: &mut Budget<'_>) -> Result<(File, Storage)> {
                Self::scope(b, Self::RETAINED, Self::IO_WORK, |_| {
                    self.check_image()?;
                    Ok((self.image.clone_fixed()?, Storage(Self::FILE_STORAGE)))
                })
            }

            pub fn validate_transfer(&self, f: &File, d: &Deployment, b: &mut Budget<'_>) -> Result<()> {
                let floor = input_floor(Self::RETAINED, d)?
                    .checked_add(Self::FILE_STORAGE).ok_or(Resource::Arithmetic)?;
                Self::scope(b, floor, Self::IO_WORK, |_| {
                    self.check(d)?;
                    self.image.validate_secret_transfer_fixed(f)?;
                    with_secret(&mut [0; WIRE_BYTES], |wire| {
                        self.image.read_transfer_fixed_into(f, wire)?;
                        self.image.validate_secret_transfer_fixed(f)
                    }, |wire| self.check_wire(wire))
                })
            }

            fn check(&self, d: &Deployment) -> Result<()> {
                if self.deployment != d.identity() {
                    return Err(Error::Rejected("anchor key names another deployment"));
                }
                require_key(&self.key, d)?;
                self.check_image()
            }
            fn check_image(&self) -> Result<()> { read_secret(&self.image, |wire| self.check_wire(wire)) }
            fn check_wire(&self, wire: &[u8; WIRE_BYTES]) -> Result<()> {
                require_context(wire, self.deployment)?;
                if !bool::from(wire[SEED_OFFSET..].ct_eq(self.key.as_bytes())) {
                    return Err(Error::Rejected("anchor signing-key bytes changed"));
                }
                Ok(())
            }
            pub fn verifying_key(&self) -> [u8; KEY_BYTES] { self.key.verifying_key().to_bytes() }
            pub const fn deployment_identity(&self) -> DeploymentIdentity { self.deployment }
            pub const fn retained_storage(&self) -> usize { Self::RETAINED }
            fn scope<T>(b: &mut Budget<'_>, floor: usize, work: usize,
                f: impl FnOnce(&mut Budget<'_>) -> Result<T>) -> Result<T> {
                b.with_prepaid_scope(floor, ENTRY_WORK, work, Self::IO_STORAGE, f)
            }
        }

        fn input_floor(input: usize, d: &Deployment) -> Result<usize> {
            input.checked_add(d.retained_storage()).ok_or_else(|| Resource::Arithmetic.into())
        }
        fn require_context(wire: &[u8; WIRE_BYTES], identity: DeploymentIdentity) -> Result<()> {
            if wire[..24] != HEADER || &wire[24..SEED_OFFSET] != identity.as_bytes() {
                return Err(Error::Rejected("anchor key role or deployment mismatch"));
            }
            Ok(())
        }
        fn require_key(key: &SigningKey, d: &Deployment) -> Result<()> {
            if key.verifying_key().as_bytes() != d.verifying_key() {
                return Err(Error::Rejected("anchor signing key does not match deployment"));
            }
            Ok(())
        }
        fn require_service(d: &Deployment) -> Result<()> {
            let uid = rustix::process::geteuid().as_raw();
            let gid = rustix::process::getegid().as_raw();
            if uid == 0 || gid == 0 || uid != d.service().uid() || gid != d.service().gid() {
                return Err(Error::Rejected("anchor key reissue requires exact nonroot service credentials"));
            }
            Ok(())
        }
        impl fmt::Debug for $Cap {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Cap)).field("deployment", &self.deployment).finish_non_exhaustive()
            }
        }
        const _: () = {
            use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
            assert!($Cap::RETAINED >= $Cap::FILE_STORAGE);
            assert!(ROLE.memfd_name.len() < 128);
            assert!(8 * size_of::<Error>() + 64 * size_of::<usize>() + size_of::<Ledger>()
                + 4 * size_of::<SeedGuard<'static, WIRE_BYTES>>() + 256
                + envelope_overhead::<($Cap, Storage), Error>()
                + envelope_overhead::<(File, Storage), Error>()
                + envelope_overhead::<([u8; ANCHOR_OBSERVATION_WIRE_LEN_V1], Storage), Error>()
                + envelope_overhead::<(), Error>() <= 4096);
            fn zeroizing_key<T: zeroize::ZeroizeOnDrop>() {}
            let _ = zeroizing_key::<SigningKey>;
        };
    };
}
pub(crate) use anchor_signing_key;
