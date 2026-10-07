//! Application-side installed proof policy, never a root controller-launch owner.
use super::{
    InstalledFile, InstalledTree, NamespaceCustody,
    ProductionCompilerExecutionDeploymentV3 as Compiler,
};
use fe2o3_compiler_execution_protocol::{
    NATIVE_PROOF_CUSTODIAN_CONFIGURATION_BYTES_V1 as BYTES,
    NativeApplicationProofCustodianConfigurationV1 as Config,
    NativeProofCustodianConfigurationErrorV1 as ConfigError,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use sha2::Digest;
use std::{fmt, fs::File, marker::PhantomData, mem::size_of, path::Path};

const DIRECTORIES: [&str; 3] = ["etc", "fe2o3", "proof-custodian"];
const FILES: [(&str, usize); 1] = [("application-native-deployment-v1", BYTES)];
const POLICY_FILE: &str = "native-conditional-root-policy-v1";
const MAX_SEMANTIC_POLICY: u64 = 131_688;
const IO_WORK: usize = 2 * 1024 * 1024;
const IO_STORAGE: usize = 768 * 1024;
type Result<T> = std::result::Result<T, ProductionNativeApplicationProofProfileErrorV1>;

#[derive(Debug)]
pub enum ProductionNativeApplicationProofProfileErrorV1 {
    Resource(Resource),
    Configuration(ConfigError),
    Compiler(super::ProductionCompilerExecutionDeploymentErrorV3),
    Filesystem(String),
    Binding,
}
type Error = ProductionNativeApplicationProofProfileErrorV1;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native installed proof profile: {self:?}")
    }
}
impl std::error::Error for Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionNativeApplicationProofProfileStorageV1(usize);
impl ProductionNativeApplicationProofProfileStorageV1 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}
use self::ProductionNativeApplicationProofProfileStorageV1 as Storage;

/// Original fixed root-owned native proof configuration in an unprivileged app.
///
/// Checks the actual compiler V3 deployment, exact policy identity, separate
/// controller credentials, original installed bytes/path and namespace custody.
/// Retains the second fixed root-owned semantic-policy file and its exact Config
/// SHA/length. This owner does not semantically decode that policy, recover source,
/// attest controller execution or approve a proof.
/// No caller config, alternative public path, legacy fallback or root conversion.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::ProductionNativeApplicationProofProfileV1 as Profile;
/// fn clone(value: Profile<'_>) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::ProductionNativeApplicationProofProfileV1 as Profile;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<Profile<'_>>();
/// ```
pub struct ProductionNativeApplicationProofProfileV1<'work> {
    config: Config,
    tree: InstalledTree<1>,
    policy: InstalledFile,
    policy_bytes: Vec<u8>,
    namespace: NamespaceCustody,
    ledger: Ledger,
    account: Option<Account>,
    lifetime: PhantomData<&'work Work>,
}
type Profile<'work> = ProductionNativeApplicationProofProfileV1<'work>;
impl fmt::Debug for Profile<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProductionNativeApplicationProofProfileV1")
            .field("identity", &self.config.identity())
            .finish_non_exhaustive()
    }
}
impl<'work> Profile<'work> {
    pub fn open(compiler: &Compiler<'work>, budget: &mut Budget<'work>) -> Result<(Self, Storage)> {
        Self::open_tree(Path::new("/"), 0, 0, compiler, budget)
    }
    #[cfg(test)]
    pub(super) fn open_test_tree(
        root: &Path,
        uid: u32,
        gid: u32,
        compiler: &Compiler<'work>,
        budget: &mut Budget<'work>,
    ) -> Result<(Self, Storage)> {
        Self::open_tree(root, uid, gid, compiler, budget)
    }
    fn open_tree(
        root: &Path,
        uid: u32,
        gid: u32,
        compiler: &Compiler<'work>,
        budget: &mut Budget<'work>,
    ) -> Result<(Self, Storage)> {
        let floor = budget.storage();
        budget.charge_work(IO_WORK)?;
        if floor < compiler.retained_storage().map_err(Error::Compiler)? {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(IO_STORAGE)?;
        if root.as_os_str().len() > 4096 {
            return Err(Error::Binding);
        }
        compiler.revalidate(budget).map_err(Error::Compiler)?;
        let namespace = NamespaceCustody::open_with_read_mode(true).map_err(Error::Filesystem)?;
        let (tree, bytes) =
            InstalledTree::open_path_roster(root, uid, gid, &DIRECTORIES, FILES, true)
                .map_err(Error::Filesystem)?;
        let (config, charge) = Config::decode(&bytes[0], budget).map_err(Error::Configuration)?;
        budget.reserve_storage(charge.retained_storage())?;
        let (digest, length) = config.parts().semantic_policy;
        if length > MAX_SEMANTIC_POLICY {
            return Err(Error::Binding);
        }
        let length = usize::try_from(length).map_err(|_| Error::Binding)?;
        let file = super::open_file_at(tree.directories.last().ok_or(Error::Binding)?, POLICY_FILE)
            .map_err(Error::Filesystem)?;
        let snapshot = super::validate_trusted_file(&file, uid, gid, POLICY_FILE, length)
            .map_err(Error::Filesystem)?;
        let policy_bytes = super::read_installed(&file, length, true).map_err(Error::Filesystem)?;
        if <[u8; 32]>::from(sha2::Sha256::digest(&policy_bytes)) != digest {
            return Err(Error::Binding);
        }
        let value = Self {
            config,
            tree,
            policy: InstalledFile { file, snapshot },
            policy_bytes,
            namespace,
            ledger: budget.work_ledger_identity_v1(),
            account: budget.storage_account_identity_v1(),
            lifetime: PhantomData,
        };
        value.revalidate_inner(compiler, budget)?;
        let retained = value.retained_storage()?;
        drop(bytes);
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        )?;
        Ok((value, Storage(retained)))
    }
    pub const fn configuration(&self) -> &Config {
        &self.config
    }
    /// Exact independently installed bytes; semantic recovery is a separate gate.
    pub fn semantic_policy_bytes(&self) -> &[u8] {
        &self.policy_bytes
    }
    pub fn retained_storage(&self) -> Result<usize> {
        let mut bytes = size_of::<(Self, Storage)>();
        for addition in [
            self.policy_bytes.capacity(),
            self.tree.root_path.capacity(),
            self.tree
                .directories
                .capacity()
                .checked_mul(size_of::<File>())
                .ok_or(Resource::Arithmetic)?,
            self.tree
                .files
                .capacity()
                .checked_mul(size_of::<InstalledFile>())
                .ok_or(Resource::Arithmetic)?,
        ] {
            bytes = bytes.checked_add(addition).ok_or(Resource::Arithmetic)?;
        }
        Ok(bytes)
    }
    pub fn revalidate(&self, compiler: &Compiler<'work>, budget: &mut Budget<'work>) -> Result<()> {
        if self.ledger != budget.work_ledger_identity_v1()
            || self.account != budget.storage_account_identity_v1()
            || budget.storage()
                < self
                    .retained_storage()?
                    .checked_add(compiler.retained_storage().map_err(Error::Compiler)?)
                    .ok_or(Resource::Arithmetic)?
        {
            return Err(Resource::Accounting.into());
        }
        budget.charge_work(IO_WORK)?;
        budget.reserve_storage(IO_STORAGE)?;
        self.revalidate_inner(compiler, budget)?;
        budget.release_storage(IO_STORAGE)?;
        Ok(())
    }
    fn revalidate_inner(
        &self,
        compiler: &Compiler<'work>,
        budget: &mut Budget<'work>,
    ) -> Result<()> {
        compiler.revalidate(budget).map_err(Error::Compiler)?;
        self.namespace.revalidate().map_err(Error::Filesystem)?;
        let parts = self.config.parts();
        if parts.compiler_policy_identity != *compiler.policy().identity().as_bytes()
            || parts.semantic_policy.1 > MAX_SEMANTIC_POLICY
            || [
                self.namespace.credentials.uid,
                compiler.profile().supervisor_uid(),
                compiler.anchor().service().uid(),
            ]
            .contains(&parts.credentials.0)
            || [
                self.namespace.credentials.gid,
                compiler.profile().supervisor_gid(),
                compiler.anchor().service().gid(),
            ]
            .contains(&parts.credentials.1)
        {
            return Err(Error::Binding);
        }
        self.tree
            .revalidate(&[self.config.canonical_bytes()])
            .map_err(Error::Filesystem)?;
        for _ in 0..2 {
            let current = self
                .tree
                .revalidate_directories()
                .map_err(Error::Filesystem)?;
            let named = super::open_file_at(&current, POLICY_FILE).map_err(Error::Filesystem)?;
            for file in [&self.policy.file, &named] {
                if super::validate_trusted_file(
                    file,
                    self.tree.owner_uid,
                    self.tree.owner_gid,
                    POLICY_FILE,
                    self.policy_bytes.len(),
                )
                .map_err(Error::Filesystem)?
                    != self.policy.snapshot
                    || super::read_installed(file, self.policy_bytes.len(), true)
                        .map_err(Error::Filesystem)?
                        != self.policy_bytes
                    || super::validate_trusted_file(
                        file,
                        self.tree.owner_uid,
                        self.tree.owner_gid,
                        POLICY_FILE,
                        self.policy_bytes.len(),
                    )
                    .map_err(Error::Filesystem)?
                        != self.policy.snapshot
                {
                    return Err(Error::Filesystem(
                        "installed native semantic policy changed".into(),
                    ));
                }
            }
        }
        self.namespace.revalidate().map_err(Error::Filesystem)
    }
}
