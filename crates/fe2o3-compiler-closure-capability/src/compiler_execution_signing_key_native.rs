//! Shared native signing custody; adapters choose only nominal protocol operations.
macro_rules! signing_key_capability {
    ($Cap:ident, $issue_current:ident, $map_current_error:path) => {
        impl $Cap {
            const RETAINED: usize = size_of::<(Self, Storage)>() + KEY_BYTES;

            /// Logical charge for one File and its 32-byte sealed image, including when
            /// multiple descriptors share backing. This is not physical kernel memory.
            pub const FILE_STORAGE: usize = size_of::<(File, Storage)>() + KEY_BYTES;
            /// Named fixed crypto allowance for one pinned Dalek Ed25519 key derivation.
            /// It is an admission unit, not a measured instruction or wall-time bound.
            pub const DERIVATION_WORK: usize = 65_536;
            /// Fixed Dalek signing allowance for one journal digest.
            pub const SIGN_WORK: usize = 65_536;
            /// Entry, at most 64 descriptor/credential/cleanup calls at weight 1024,
            /// and fixed byte staging/comparison. No native I/O operation retries.
            /// Borrowed transfer validation uses 38 calls: two 19-call secret checks,
            /// each with pre/post metadata, credentials, access and one positional read.
            pub const IO_WORK: usize = ENTRY_WORK + 64 * 1024 + 32 * KEY_BYTES;
            /// Create, consuming-file and inherited admission each derive one key.
            pub const ADMISSION_WORK: usize = Self::IO_WORK + Self::DERIVATION_WORK;
            /// Complete reissue work: fixed secret I/O, one key derivation, and one
            /// separately charged native deployment-policy match on the same ledger.
            /// The I/O envelope includes pre/post template and current-owner checks,
            /// source closure, a fresh image, and its guarded read-back; at most 72 calls.
            pub const REISSUE_WORK: usize = Self::ADMISSION_WORK + 8 * 1024 + DEPLOYMENT_WORK;
            /// Peak additional scratch while the native deployment match is nested
            /// in the secret I/O frame. Borrowed deployment/policy and consumed File
            /// reservations remain separately prepaid throughout the operation.
            pub const REISSUE_STORAGE: usize = Self::IO_STORAGE + DEPLOYMENT_STORAGE;
            /// Additional logical scratch for result/staging owners, guarded seeds,
            /// metadata, fixed path/control frames and named crypto scratch. This is
            /// not a generated stack, allocator, kernel-page, RSS, or time bound.
            pub const IO_STORAGE: usize = 4 * Self::RETAINED
                + 4 * KEY_BYTES
                + 4 * size_of::<std::fs::Metadata>()
                + 4096
                + CRYPTO_SCRATCH;

            /// Borrows a prepaid 32-byte seed and policy, returning a FULL owner charge.
            /// The caller's seed is guarded before even entry-work admission, and wiped
            /// on success, error and unwind. Copies made before this call remain the
            /// caller's responsibility. Closing a memfd does not prove kernel-page erasure.
            pub fn create_and_zeroize(
                seed: &mut [u8; KEY_BYTES],
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                let seed = SeedGuard(seed);
                Self::scope(
                    budget,
                    KEY_BYTES + policy.retained_storage(),
                    Self::ADMISSION_WORK,
                    |_| {
                        let key = SigningKey::from_bytes(seed.0);
                        require_policy_key(&key, policy)?;
                        let image = SealedCapabilityImage::create_fixed(seed.0, ROLE)?
                            .into_read_only_fixed::<KEY_BYTES>()?;
                        let admitted = Self {
                            key,
                            image,
                            policy: policy.identity(),
                        };
                        admitted.check_image()?;
                        Ok((admitted, Storage(Self::RETAINED)))
                    },
                )
            }

            /// Consumes a prepaid File, borrowing the prepaid native policy. Returns
            /// only growth over FILE_STORAGE, not a second full image charge.
            pub fn from_file(
                image: File,
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                Self::scope(
                    budget,
                    Self::FILE_STORAGE + policy.retained_storage(),
                    Self::ADMISSION_WORK,
                    |_| {
                        let image =
                            SealedCapabilityImage::from_file_fixed::<KEY_BYTES>(image, ROLE)?;
                        Ok((
                            Self::decode_image(image, policy)?,
                            Storage(Self::RETAINED - Self::FILE_STORAGE),
                        ))
                    },
                )
            }

            /// Reissues a root-owned template into current nonroot service key custody.
            ///
            /// The consumed File must be an anonymous, immutable, mode-0400, read-only
            /// CLOEXEC 32-byte image owned by UID/GID 0:0. Current effective UID/GID
            /// must equal the deployment, which must bind the complete native policy.
            /// The fresh image is created under those current credentials. All reads
            /// and writes are single attempts; guarded seed staging is wiped on every
            /// exit and unwind. Closing an image does not prove kernel-page erasure.
            ///
            /// Deployment remains inert configuration. The caller must INDEPENDENTLY
            /// pin trusted deployment provenance; public construction/decoding and this
            /// reissue do not authenticate provisioning or grant process authority.
            /// No legacy admitted owner is constructed or upgraded.
            ///
            /// Prepay FILE_STORAGE + deployment.retained_storage() + policy.retained_storage().
            /// The call charges REISSUE_WORK with REISSUE_STORAGE additional peak scratch,
            /// restores entry storage, and returns only growth over the consumed File.
            /// Preserve that File reservation and add the returned delta on success;
            /// on error the File is closed and its reservation is left for the caller.
            ///
            /// ```compile_fail
            /// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV2 as Cap;
            /// use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV3 as D,
            ///     CompilerExecutionIssuerPolicyV2 as P};
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
            /// fn mix(f: std::fs::File, d: &D, p: &P, b: &mut B<'_>) {
            ///     let _ = Cap::reissue_root_template_for_current_service(f, d, p, b);
            /// }
            /// ```
            /// ```compile_fail
            /// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Cap;
            /// use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV2 as D,
            ///     CompilerExecutionIssuerPolicyV3 as P};
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
            /// fn mix(f: std::fs::File, d: &D, p: &P, b: &mut B<'_>) {
            ///     let _ = Cap::reissue_root_template_for_current_service(f, d, p, b);
            /// }
            /// ```
            /// ```compile_fail
            /// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Cap;
            /// use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV1 as D,
            ///     CompilerExecutionIssuerPolicyV3 as P};
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
            /// fn mix(f: std::fs::File, d: &D, p: &P, b: &mut B<'_>) {
            ///     let _ = Cap::reissue_root_template_for_current_service(f, d, p, b);
            /// }
            /// ```
            /// ```compile_fail
            /// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Cap;
            /// use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV3 as D,
            ///     CompilerExecutionIssuerPolicyV2 as P};
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
            /// fn mix(f: std::fs::File, d: &D, p: &P, b: &mut B<'_>) {
            ///     let _ = Cap::reissue_root_template_for_current_service(f, d, p, b);
            /// }
            /// ```
            /// ```compile_fail
            /// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Cap;
            /// use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV3 as D,
            ///     CompilerExecutionIssuerPolicyV3 as P};
            /// fn unmetered(f: std::fs::File, d: &D, p: &P) {
            ///     let _ = Cap::reissue_root_template_for_current_service(f, d, p);
            /// }
            /// ```
            /// ```compile_fail
            /// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Cap;
            /// use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV3 as D,
            ///     CompilerExecutionIssuerPolicyV3 as P};
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
            /// fn reuse(f: std::fs::File, d: &D, p: &P, b: &mut B<'_>) {
            ///     let _ = Cap::reissue_root_template_for_current_service(f, d, p, b);
            ///     let _ = f.metadata();
            /// }
            /// ```
            /// ```compile_fail
            /// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Cap;
            /// use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV3 as D,
            ///     CompilerExecutionIssuerPolicyV3 as P};
            /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
            /// fn choose_owner(f: std::fs::File, d: &D, p: &P, b: &mut B<'_>) {
            ///     let _ = Cap::reissue_template_for_current_service(f, d, p, (1000, 1000), &mut [0; 32], b);
            /// }
            /// ```
            pub fn reissue_root_template_for_current_service(
                image: File,
                deployment: &Deployment,
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                let mut seed = [0; KEY_BYTES];
                Self::reissue_template_for_current_service(
                    image,
                    deployment,
                    policy,
                    (0, 0),
                    &mut seed,
                    budget,
                )
            }

            // Only the public root-owner wrapper is a production entry point.
            // Private expected-owner tests exercise rootless mechanics, not provenance
            // or protected-root execution. Borrowing staging lets tests inspect wiping.
            fn reissue_template_for_current_service(
                image: File,
                deployment: &Deployment,
                policy: &Policy,
                template_owner: (u32, u32),
                seed: &mut [u8; KEY_BYTES],
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                Self::reissue_template_for_current_service_with(
                    image,
                    deployment,
                    policy,
                    template_owner,
                    seed,
                    budget,
                    |_| Ok(()),
                )
            }

            // A private observation point lets tests fail or unwind after allocation
            // inside the real scope. Production always uses the no-op above.
            fn reissue_template_for_current_service_with(
                image: File,
                deployment: &Deployment,
                policy: &Policy,
                template_owner: (u32, u32),
                seed: &mut [u8; KEY_BYTES],
                budget: &mut Budget<'_>,
                after_image: impl FnOnce(&Self) -> Result<()>,
            ) -> Result<(Self, Storage)> {
                let seed = SeedGuard(seed);
                let floor = Self::FILE_STORAGE
                    .checked_add(deployment.retained_storage())
                    .and_then(|n| n.checked_add(policy.retained_storage()))
                    .ok_or(Resource::Arithmetic)?;
                Self::scope(
                    budget,
                    floor,
                    Self::REISSUE_WORK - DEPLOYMENT_WORK,
                    |budget| {
                        if !deployment.matches_policy(policy, budget)? {
                            return Err(Error::Rejected(
                                "key reissue deployment names another native policy",
                            ));
                        }
                        require_deployment_credentials(deployment)?;
                        let growth = Self::RETAINED
                            .checked_sub(Self::FILE_STORAGE)
                            .ok_or(Resource::Accounting)?;
                        let template =
                            SealedCapabilityImage::from_file_fixed::<KEY_BYTES>(image, ROLE)?;
                        template.validate_secret_owner_fixed(template_owner.0, template_owner.1)?;
                        template.read_fixed_into(seed.0)?;
                        template.validate_secret_owner_fixed(template_owner.0, template_owner.1)?;
                        let key = SigningKey::from_bytes(seed.0);
                        require_policy_key(&key, policy)?;
                        let image = SealedCapabilityImage::create_fixed(seed.0, ROLE)?
                            .into_read_only_fixed::<KEY_BYTES>()?;
                        let admitted = Self {
                            key,
                            image,
                            policy: policy.identity(),
                        };
                        after_image(&admitted)?;
                        admitted.check_image()?;
                        template.validate_secret_owner_fixed(template_owner.0, template_owner.1)?;
                        admitted.image.validate_secret_owner_fixed(
                            deployment.service_uid(),
                            deployment.service_gid(),
                        )?;
                        require_deployment_credentials(deployment)?;
                        Ok((admitted, Storage(growth)))
                    },
                )
            }

            /// Borrows a live, non-CLOEXEC inherited fd >= 3 and the native policy.
            /// Keep FILE_STORAGE and the policy prepaid for the call. The source stays
            /// open; the private CLOEXEC owner returns its FULL retained charge.
            pub fn from_inherited_at(
                fd: RawFd,
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<(Self, Storage)> {
                Self::scope(
                    budget,
                    Self::FILE_STORAGE + policy.retained_storage(),
                    Self::ADMISSION_WORK,
                    |_| {
                        let image =
                            SealedCapabilityImage::from_inherited_fixed::<KEY_BYTES>(fd, ROLE)?;
                        Ok((Self::decode_image(image, policy)?, Storage(Self::RETAINED)))
                    },
                )
            }

            fn decode_image(image: SealedCapabilityImage, policy: &Policy) -> Result<Self> {
                let key = read_secret(&image, |seed| Ok(SigningKey::from_bytes(seed)))?;
                require_policy_key(&key, policy)?;
                Ok(Self {
                    key,
                    image,
                    policy: policy.identity(),
                })
            }

            /// Checks exact native policy identity as well as the seed, retained inode,
            /// seals, length, permissions, descriptor access and current service owner.
            pub fn revalidate(&self, policy: &Policy, budget: &mut Budget<'_>) -> Result<()> {
                Self::scope(
                    budget,
                    Self::RETAINED + policy.retained_storage(),
                    Self::IO_WORK,
                    |_| self.check_policy_image(policy),
                )
            }

            /// Signs one native request without exporting a key or seed. This authenticates
            /// bytes only; live occurrence custody must be established by the issuer.
            /// All inputs stay prepaid and the returned protocol storage is unreserved.
            pub fn issue_receipt(
                &self,
                policy: &Policy,
                request: &Request,
                budget: &mut Budget<'_>,
            ) -> Result<(Receipt, ProtocolStorage)> {
                self.signing_scope(policy, request.retained_storage(), budget, |budget| {
                    Ok(Receipt::issue(policy, request, &self.key, budget)?)
                })
            }

            /// Consumes a prepaid currentness verification and returns its protocol growth.
            /// The native authenticator checks both anchor receipts and the fresh challenge;
            /// protected journal/currentness custody is still the consuming issuer's duty.
            pub fn attest_current(
                &self,
                policy: &Policy,
                carriage: &Carriage,
                verification: CurrentVerification,
                challenge: [u8; 32],
                budget: &mut Budget<'_>,
            ) -> Result<(CurrentAttestation, ProtocolStorage)> {
                let inputs = carriage
                    .retained_storage()
                    .checked_add(size_of::<(CurrentVerification, ProtocolStorage)>())
                    .ok_or(Resource::Arithmetic)?;
                self.signing_scope(policy, inputs, budget, |budget| {
                    CurrentAttestation::$issue_current(
                        policy,
                        carriage,
                        verification,
                        challenge,
                        &self.key,
                        budget,
                    )
                    .map_err($map_current_error)
                })
            }

            /// Signs a fixed, caller-domain-separated journal digest. This is a key-use
            /// primitive, not proof that the journal is durable or its claims are true.
            pub fn sign_journal_digest(
                &self,
                policy: &Policy,
                digest: &[u8; 32],
                budget: &mut Budget<'_>,
            ) -> Result<([u8; 64], Storage)> {
                self.signing_scope(policy, 32, budget, |budget| {
                    budget.charge_work(Self::SIGN_WORK)?;
                    Ok((
                        self.key.sign(digest).to_bytes(),
                        Storage(size_of::<([u8; 64], Storage)>()),
                    ))
                })
            }

            fn signing_scope<T>(
                &self,
                policy: &Policy,
                inputs: usize,
                budget: &mut Budget<'_>,
                operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
            ) -> Result<T> {
                let floor = Self::RETAINED
                    .checked_add(policy.retained_storage())
                    .and_then(|n| n.checked_add(inputs))
                    .ok_or(Resource::Arithmetic)?;
                Self::scope(budget, floor, 2 * Self::IO_WORK, |budget| {
                    self.check_policy_image(policy)?;
                    let output = operation(budget)?;
                    self.check_policy_image(policy)?;
                    Ok(output)
                })
            }

            fn check_policy_image(&self, policy: &Policy) -> Result<()> {
                if policy.identity() != self.policy {
                    return Err(Error::Rejected(
                        "signing key is pinned to another native policy",
                    ));
                }
                self.check_image()?;
                require_policy_key(&self.key, policy)
            }

            fn check_image(&self) -> Result<()> {
                read_secret(&self.image, |seed| self.check_seed(seed))
            }

            fn check_seed(&self, seed: &[u8; KEY_BYTES]) -> Result<()> {
                if !bool::from(seed.ct_eq(self.key.as_bytes())) {
                    return Err(Error::Rejected("signing-key bytes changed"));
                }
                Ok(())
            }

            /// Revalidates and returns a separately charged read-only CLOEXEC File.
            /// This meters the transfer, not arbitrary future operations on that File.
            /// The trusted recipient can read the seed; read-only means immutable, not secret.
            pub fn try_clone_for_transfer(
                &self,
                budget: &mut Budget<'_>,
            ) -> Result<(File, Storage)> {
                Self::scope(budget, Self::RETAINED, Self::IO_WORK, |_| {
                    self.check_image()?;
                    Ok((self.image.clone_fixed()?, Storage(Self::FILE_STORAGE)))
                })
            }

            /// Revalidates the exact native policy, owner and borrowed read-only CLOEXEC
            /// transfer, including original object identity and guarded seed comparison.
            /// Prepay `retained_storage() + FILE_STORAGE + policy.retained_storage()` on
            /// the same ledger. Charges IO_WORK and IO_STORAGE scratch, restoring entry
            /// storage. No key derivation, descriptor duplication or ownership transfer;
            /// both descriptors stay live and secret staging is wiped on every exit.
            pub fn validate_transfer(
                &self,
                transfer: &File,
                policy: &Policy,
                budget: &mut Budget<'_>,
            ) -> Result<()> {
                Self::scope(
                    budget,
                    Self::RETAINED + Self::FILE_STORAGE + policy.retained_storage(),
                    Self::IO_WORK,
                    |_| {
                        self.check_policy_image(policy)?;
                        self.image.validate_secret_transfer_fixed(transfer)?;
                        let mut seed = [0; KEY_BYTES];
                        with_secret(
                            &mut seed,
                            |seed| {
                                self.image.read_transfer_fixed_into(transfer, seed)?;
                                self.image.validate_secret_transfer_fixed(transfer)
                            },
                            |seed| self.check_seed(seed),
                        )
                    },
                )
            }

            pub fn verifying_key(&self) -> [u8; KEY_BYTES] {
                self.key.verifying_key().to_bytes()
            }
            pub const fn policy_identity(&self) -> PolicyIdentity {
                self.policy
            }
            pub const fn retained_storage(&self) -> usize {
                Self::RETAINED
            }

            fn scope<T>(
                budget: &mut Budget<'_>,
                floor: usize,
                work: usize,
                operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
            ) -> Result<T> {
                budget.with_prepaid_scope(floor, ENTRY_WORK, work, Self::IO_STORAGE, operation)
            }
        }

        impl fmt::Debug for $Cap {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_struct(stringify!($Cap))
                    .field("authority", &"signing-key-custody-only")
                    .field("policy", &self.policy)
                    .finish_non_exhaustive()
            }
        }

        fn require_policy_key(key: &SigningKey, policy: &Policy) -> Result<()> {
            if key.verifying_key().as_bytes() != policy.verifying_key() {
                return Err(Error::Rejected(
                    "signing key does not match the pinned native policy",
                ));
            }
            Ok(())
        }

        fn require_deployment_credentials(deployment: &Deployment) -> Result<()> {
            let uid = rustix::process::geteuid().as_raw();
            let gid = rustix::process::getegid().as_raw();
            if uid == 0
                || gid == 0
                || uid != deployment.service_uid()
                || gid != deployment.service_gid()
            {
                return Err(Error::Rejected(
                    "key reissue process does not have the nonroot deployment credentials",
                ));
            }
            Ok(())
        }

        const _: () = {
            use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
            type Cap = $Cap;
            assert!(Cap::RETAINED >= Cap::FILE_STORAGE);
            assert!(ROLE.memfd_name.len() < 128);
            assert!(
                8 * size_of::<Error>()
                    + 64 * size_of::<usize>()
                    + size_of::<Ledger>()
                    + size_of::<std::result::Result<(), Resource>>()
                    + 2 * size_of::<bool>()
                    + 2 * size_of::<SeedGuard<'static, KEY_BYTES>>()
                    + 128
                    + envelope_overhead::<(Cap, Storage), Error>()
                    + envelope_overhead::<(File, Storage), Error>()
                    + envelope_overhead::<(), Error>()
                    <= 4096
            );
            fn zeroizing_key<T: ZeroizeOnDrop>() {}
            let _ = zeroizing_key::<SigningKey>;
        };
    };
}
pub(crate) use signing_key_capability;
