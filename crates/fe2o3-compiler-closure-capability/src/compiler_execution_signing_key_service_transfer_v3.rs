//! Root preparation of fresh service-owned V3 secret custody, not provisioning authority.
use super::{CompilerExecutionSigningKeyCapabilityV3 as Key, *};
use fe2o3_compiler_execution_protocol::CompilerExecutionSupervisorDeploymentIdentityV3;
use std::os::unix::fs::MetadataExt;

/// Move-only, service-owned image pinned to its root key inode and exact V3 records.
/// This owner cannot sign, expose a private key, or yield a writable descriptor.
/// Its read-only transfer Files contain the seed and must stay in trusted custody.
/// Retained typed deployment configuration is inert, not installed provisioning
/// authority. The trusted root caller must independently supply the actual deployment.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyServiceTransferV3 as Transfer;
/// fn clone<T: Clone>() {}
/// clone::<Transfer>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyServiceTransferV3 as Transfer;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<Transfer>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyServiceTransferV3 as Transfer;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV3 as Policy;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn sign(t: &Transfer, p: &Policy, b: &mut Budget<'_>) {
///     let _ = t.sign_journal_digest(p, &[0; 32], b);
/// }
/// ```
pub struct CompilerExecutionSigningKeyServiceTransferV3 {
    image: SealedCapabilityImage,
    deployment: CompilerExecutionSupervisorDeploymentIdentityV3,
    policy: PolicyIdentity,
    source: (u64, u64),
}
type Transfer = CompilerExecutionSigningKeyServiceTransferV3;

impl Key {
    /// Borrows a genuine retained root key and exact V3 deployment/policy to make
    /// a fresh anonymous 32-byte mode-0400, exactly sealed, read-only CLOEXEC image.
    /// Requires effective UID/GID 0:0 and a source still owned by 0:0. Destination
    /// credentials come only from the validated deployment; source aliases are
    /// never chowned. The caller independently pins actual deployment provenance.
    ///
    /// Prepay this key, deployment and policy on the original Budget. Charges
    /// Transfer::WORK with Transfer::STORAGE peak scratch and restores entry storage.
    /// Returns the FULL unreserved transfer charge; all borrowed inputs stay live.
    /// Guarded seed staging is wiped on every exit/unwind. Closing an image does
    /// not prove kernel-page erasure. No key derivation or signing is performed.
    ///
    /// ```compile_fail
    /// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Key;
    /// use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV2 as D,
    ///     CompilerExecutionIssuerPolicyV3 as P};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
    /// fn mix(k: &Key, d: &D, p: &P, b: &mut B<'_>) {
    ///     let _ = k.reissue_for_deployed_service(d, p, b);
    /// }
    /// ```
    /// ```compile_fail
    /// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Key;
    /// use fe2o3_compiler_execution_protocol::{CompilerExecutionSupervisorDeploymentV3 as D,
    ///     CompilerExecutionIssuerPolicyV2 as P};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as B;
    /// fn mix(k: &Key, d: &D, p: &P, b: &mut B<'_>) {
    ///     let _ = k.reissue_for_deployed_service(d, p, b);
    /// }
    /// ```
    pub fn reissue_for_deployed_service(
        &self,
        deployment: &Deployment,
        policy: &Policy,
        budget: &mut Budget<'_>,
    ) -> Result<(Transfer, Storage)> {
        self.reissue_for_deployed_service_with(
            deployment,
            policy,
            &mut [0; KEY_BYTES],
            budget,
            |_| Ok(()),
        )
    }

    // The observation point exercises late refusal/unwind without weakening root
    // admission. Production always uses the no-op above.
    fn reissue_for_deployed_service_with(
        &self,
        deployment: &Deployment,
        policy: &Policy,
        seed: &mut [u8; KEY_BYTES],
        budget: &mut Budget<'_>,
        after_image: impl FnOnce(&Transfer) -> Result<()>,
    ) -> Result<(Transfer, Storage)> {
        let seed = SeedGuard(seed);
        Transfer::scope(self, deployment, policy, 0, budget, |budget| {
            require_deployment_policy(deployment, policy, budget)?;
            require_root()?;
            self.check_policy_image(policy)?;
            let source = source_identity(self)?;
            self.image.read_fixed_into(seed.0)?;
            self.check_seed(seed.0)?;
            let transfer = Transfer {
                image: SealedCapabilityImage::create_service_key_v3(seed.0, ROLE, deployment)?,
                deployment: deployment.identity(),
                policy: policy.identity(),
                source,
            };
            after_image(&transfer)?;
            transfer.check_binding(self, deployment, policy)?;
            transfer.check_file(transfer.image.as_file(), self, deployment)?;
            Ok((transfer, Storage(Transfer::RETAINED)))
        })
    }
}

impl Transfer {
    const RETAINED: usize = size_of::<(Self, Storage)>() + KEY_BYTES;
    /// Full logical descriptor and 32-byte image charge, including shared aliases.
    pub const FILE_STORAGE: usize = Key::FILE_STORAGE;
    /// Entry, at most 128 native calls at weight 1024, fixed seed comparisons,
    /// and one separately charged deployment-policy match. No I/O retries.
    pub const WORK: usize = ENTRY_WORK + 128 * 1024 + 32 * KEY_BYTES + DEPLOYMENT_WORK;
    /// Peak above all prepaid inputs: secret I/O, result owners and nested
    /// deployment matching. Logical storage, not kernel pages or process RSS.
    pub const STORAGE: usize = Self::IO_STORAGE + DEPLOYMENT_STORAGE;
    const IO_STORAGE: usize = Key::IO_STORAGE + 4 * Self::RETAINED;

    /// Clones a private read-only CLOEXEC descriptor for the trusted service handoff.
    /// Prepay this owner, root key, deployment and policy. Returns a FULL unreserved
    /// FILE_STORAGE charge. Uses WORK/STORAGE on the same ledger and restores storage.
    /// Keep both owners and this File prepaid until final validate_transfer; the
    /// receiver must still use KeyV3::from_file under its own deployment credentials.
    pub fn try_clone_for_transfer(
        &self,
        root_key: &Key,
        deployment: &Deployment,
        policy: &Policy,
        budget: &mut Budget<'_>,
    ) -> Result<(File, Storage)> {
        Self::scope(
            root_key,
            deployment,
            policy,
            Self::RETAINED,
            budget,
            |budget| {
                require_deployment_policy(deployment, policy, budget)?;
                self.check_binding(root_key, deployment, policy)?;
                self.check_file(self.image.as_file(), root_key, deployment)?;
                let file = self.image.clone_fixed()?;
                self.check_file(&file, root_key, deployment)?;
                self.check_binding(root_key, deployment, policy)?;
                Ok((file, Storage(Self::FILE_STORAGE)))
            },
        )
    }

    /// Final check of the actual staged descriptor against this original service
    /// image, source root inode/key, exact deployment and complete native policy.
    /// Equal bytes in another inode refuse. Prepay this owner + FILE_STORAGE +
    /// root key + deployment + policy on the original ledger throughout staging.
    /// Charges WORK/STORAGE and restores entry storage on success/error/unwind.
    /// No duplication, ownership transfer, signing, or receiver-owner relaxation.
    pub fn validate_transfer(
        &self,
        file: &File,
        root_key: &Key,
        deployment: &Deployment,
        policy: &Policy,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        Self::scope(
            root_key,
            deployment,
            policy,
            Self::RETAINED + Self::FILE_STORAGE,
            budget,
            |budget| {
                require_deployment_policy(deployment, policy, budget)?;
                self.check_binding(root_key, deployment, policy)?;
                self.check_file(self.image.as_file(), root_key, deployment)?;
                self.check_file(file, root_key, deployment)?;
                self.check_binding(root_key, deployment, policy)
            },
        )
    }

    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }

    fn check_binding(
        &self,
        root_key: &Key,
        deployment: &Deployment,
        policy: &Policy,
    ) -> Result<()> {
        require_root()?;
        if self.deployment != deployment.identity() || self.policy != policy.identity() {
            return Err(Error::Rejected(
                "service key transfer names another deployment or policy",
            ));
        }
        root_key.check_policy_image(policy)?;
        if source_identity(root_key)? != self.source {
            return Err(Error::Rejected(
                "service key transfer names another root key inode",
            ));
        }
        let fresh = self.image.revalidate_fixed()?;
        if (fresh.dev(), fresh.ino()) == self.source {
            return Err(Error::Rejected(
                "service key transfer aliases its root template",
            ));
        }
        Ok(())
    }

    fn check_file(&self, file: &File, root_key: &Key, deployment: &Deployment) -> Result<()> {
        let owner_check = || {
            self.image.validate_secret_transfer_owner_fixed(
                file,
                deployment.service_uid(),
                deployment.service_gid(),
                "service key transfer is not an anonymous deployed-owner read-only image",
            )
        };
        owner_check()?;
        with_secret(
            &mut [0; KEY_BYTES],
            |seed| {
                self.image.read_transfer_fixed_into(file, seed)?;
                owner_check()
            },
            |seed| root_key.check_seed(seed),
        )
    }

    fn scope<T>(
        root_key: &Key,
        deployment: &Deployment,
        policy: &Policy,
        extra: usize,
        budget: &mut Budget<'_>,
        operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
    ) -> Result<T> {
        let floor = root_key
            .retained_storage()
            .checked_add(deployment.retained_storage())
            .and_then(|n| n.checked_add(policy.retained_storage()))
            .and_then(|n| n.checked_add(extra))
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(
            floor,
            ENTRY_WORK,
            Self::WORK - DEPLOYMENT_WORK,
            Self::IO_STORAGE,
            operation,
        )
    }
}

fn require_deployment_policy(
    deployment: &Deployment,
    policy: &Policy,
    budget: &mut Budget<'_>,
) -> Result<()> {
    if !deployment.matches_policy(policy, budget)? {
        return Err(Error::Rejected(
            "service key transfer deployment names another native policy",
        ));
    }
    Ok(())
}

fn require_root() -> Result<()> {
    if rustix::process::geteuid().as_raw() != 0 || rustix::process::getegid().as_raw() != 0 {
        return Err(Error::Rejected(
            "service key transfer requires root UID/GID 0:0",
        ));
    }
    Ok(())
}

fn source_identity(key: &Key) -> Result<(u64, u64)> {
    key.image.validate_secret_owner_fixed(0, 0)?;
    let metadata = key.image.revalidate_fixed()?;
    Ok((metadata.dev(), metadata.ino()))
}

impl fmt::Debug for Transfer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionSigningKeyServiceTransferV3")
            .field("authority", &"service-key-transfer-only")
            .field("deployment", &self.deployment)
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

const _: () = {
    assert!(Transfer::RETAINED >= Transfer::FILE_STORAGE);
    assert!(
        4 * Transfer::RETAINED
            >= size_of::<Transfer>() + envelope_overhead::<(Transfer, Storage), Error>()
    );
};

#[cfg(test)]
#[path = "compiler_execution_signing_key_service_transfer_v3_tests.rs"]
mod tests;
