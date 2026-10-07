//! Native root application-manager approval is independent of compiler-coordinator approval.
use super::{
    InstalledFile, MAX_CONTROLLER, NativeApplicationProofCustodianDeploymentV1 as ProofConfig,
    ProductionNativeApplicationProofCustodianDeploymentV1 as ProofDeployment,
};
use crate::{other, require};
use fe2o3_compiler_closure_capability::RootProductionCompilerExecutionDeploymentV3 as Compiler;
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_profile::{
    ProtectedServiceNamespaceSetV1 as Namespaces, observations, require_proof_controller_parent_v1,
};
use fe2o3_protected_service_spawn::require_exact_root_identity_v1;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOwnerV1 as Owner, ProtectedStaticExecutableV2 as Image,
};
use sha2::{Digest, Sha256};
use std::{fs::File, io};

#[allow(unsafe_code)]
mod bootstrap;

pub(crate) const CONFIG_PATH: &str = "/etc/fe2o3/proof-custodian/native-manager-deployment-v1";
pub(crate) const IMAGE_PATH: &str = "/usr/libexec/fe2o3/fe2o3-native-application-manager";
const BYTES: usize = 208;
const DOMAIN: &[u8] = b"FE2O3/NATIVE-APPLICATION-MANAGER-DEPLOYMENT/V1\0";
const CONFIG_WORK: usize = 4096;
const CONFIG_SCRATCH: usize = 4 * BYTES;
const IO_WORK: usize = 2
    * (observations::PROCESS_CURRENT_WORK
        + observations::NAMESPACE_CAPTURE_WORK
        + observations::NAMESPACE_SELF_WORK)
    + 256 * 1024;
const IO_STORAGE: usize = 128 * 1024
    + observations::PROCESS_CURRENT_SCRATCH
    + observations::NAMESPACE_CAPTURE_SCRATCH
    + observations::NAMESPACE_SELF_SCRATCH;

/// Matching bytes only. Construction, decoding and a checksum confer no manager custody.
#[derive(Debug, Eq, PartialEq)]
pub struct NativeApplicationManagerConfigurationV1 {
    bytes: [u8; BYTES],
}
type Config = NativeApplicationManagerConfigurationV1;

/// Full unreserved output charge, or the explicit growth returned by a consuming transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeApplicationManagerStorageV1(usize);
impl NativeApplicationManagerStorageV1 {
    /// Reserve this amount on the original account before using the returned owner.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}
use self::NativeApplicationManagerStorageV1 as Storage;

impl Config {
    /// Complete logical storage retained by one configuration and its receipt.
    pub const RETAINED: usize = size_of::<Self>() + size_of::<Storage>();
    /// Administrative bytes explicitly pin all independently approved identities.
    pub fn new(
        image: ([u8; 32], u64),
        compiler_policy: [u8; 32],
        proof_deployment: [u8; 32],
        semantic_policy: ([u8; 32], u64),
        budget: &mut Budget<'_>,
    ) -> io::Result<(Self, Storage)> {
        budget.charge_work(CONFIG_WORK).map_err(other)?;
        budget.reserve_storage(CONFIG_SCRATCH).map_err(other)?;
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(b"F3NAMC1\0");
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
        bytes[24..56].copy_from_slice(&image.0);
        bytes[56..64].copy_from_slice(&image.1.to_le_bytes());
        bytes[64..96].copy_from_slice(&compiler_policy);
        bytes[96..128].copy_from_slice(&proof_deployment);
        bytes[128..160].copy_from_slice(&semantic_policy.0);
        bytes[160..168].copy_from_slice(&semantic_policy.1.to_le_bytes());
        bytes[168] = 6;
        let identity = checksum(&bytes[..176]);
        bytes[176..].copy_from_slice(&identity);
        let value = Self { bytes };
        value.validate()?;
        budget.release_storage(CONFIG_SCRATCH).map_err(other)?;
        Ok((value, Storage(Self::RETAINED)))
    }
    /// Decodes exactly one prepaid frame; failures retain partial resource charges.
    pub fn decode(bytes: &[u8], budget: &mut Budget<'_>) -> io::Result<(Self, Storage)> {
        budget.charge_work(CONFIG_WORK).map_err(other)?;
        require(
            bytes.len() == BYTES && budget.storage() >= BYTES,
            "native manager input frame",
        )?;
        budget.reserve_storage(CONFIG_SCRATCH).map_err(other)?;
        let value = Self {
            bytes: bytes.try_into().map_err(other)?,
        };
        value.validate()?;
        budget.release_storage(CONFIG_SCRATCH).map_err(other)?;
        Ok((value, Storage(Self::RETAINED)))
    }
    fn validate(&self) -> io::Result<()> {
        let b = &self.bytes;
        require(
            &b[..8] == b"F3NAMC1\0"
                && b[8..10] == 1u16.to_le_bytes()
                && b[10..12] == [0; 2]
                && b[12..16] == (BYTES as u32).to_le_bytes()
                && b[16..24] == [0; 8]
                && b[168] == 6
                && b[169..176] == [0; 7]
                && b[176..] == checksum(&b[..176]),
            "noncanonical native manager configuration",
        )?;
        require(
            self.compiler_policy_identity() != [0; 32]
                && self.proof_deployment_identity() != [0; 32]
                && self.semantic_policy().0 != [0; 32]
                && (1..=fe2o3_verifier::MAX_NATIVE_CONDITIONAL_ROOT_POLICY_FILE_BYTES_V1 as u64)
                    .contains(&self.semantic_policy().1),
            "empty native manager binding",
        )?;
        self.measurement().map(drop)
    }
    /// Exact inert deployment frame; matching bytes alone confer no custody.
    pub const fn canonical_bytes(&self) -> &[u8; BYTES] {
        &self.bytes
    }
    /// Domain-separated identity of the complete configuration.
    pub fn identity(&self) -> [u8; 32] {
        self.bytes[176..].try_into().unwrap()
    }
    /// Independently approved native compiler policy identity.
    pub fn compiler_policy_identity(&self) -> [u8; 32] {
        self.bytes[64..96].try_into().unwrap()
    }
    /// Independently approved native proof-controller deployment identity.
    pub fn proof_deployment_identity(&self) -> [u8; 32] {
        self.bytes[96..128].try_into().unwrap()
    }
    /// Exact semantic-policy file digest and length.
    pub fn semantic_policy(&self) -> ([u8; 32], u64) {
        (
            self.bytes[128..160].try_into().unwrap(),
            u64::from_le_bytes(self.bytes[160..168].try_into().unwrap()),
        )
    }
    fn measurement(&self) -> io::Result<Measurement> {
        Measurement::new(
            self.bytes[24..56].try_into().unwrap(),
            u64::from_le_bytes(self.bytes[56..64].try_into().unwrap()),
            MAX_CONTROLLER,
        )
        .map_err(other)
    }
    fn matches(&self, compiler: &Compiler<'_>, proof: &ProofConfig) -> bool {
        self.compiler_policy_identity() == *compiler.policy().identity().as_bytes()
            && self.compiler_policy_identity() == proof.compiler_policy_identity()
            && self.proof_deployment_identity() == proof.identity()
            && self.semantic_policy() == proof.semantic_policy()
    }
}
fn checksum(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(DOMAIN);
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    h.finalize().into()
}

/// Original root-installed native manager approval and sealed image, not a running service.
/// Both independent deployments remain borrowed on their original work/account lifetime.
///
/// ```compile_fail
/// use fe2o3_proof_custodian::ProductionNativeApplicationManagerDeploymentV1 as D;
/// fn cloneable<T: Clone>() {} fn test() { cloneable::<D<'_, '_>>(); }
/// ```
pub struct ProductionNativeApplicationManagerDeploymentV1<'root, 'work> {
    compiler: &'root Compiler<'work>,
    proof: &'root ProofDeployment<'work>,
    config: Config,
    config_tree: InstalledFile,
    image_tree: InstalledFile,
    image: Image,
    namespaces: Namespaces,
    pid: u32,
    tid: u32,
    ledger: Ledger,
    account: Option<Account>,
}
type Deployment<'r, 'w> = ProductionNativeApplicationManagerDeploymentV1<'r, 'w>;
impl<'root, 'work> Deployment<'root, 'work> {
    /// Opens only fixed protected paths and compares with both original admitted deployments.
    /// Returned storage is this new owner's full unreserved charge, excluding borrowed inputs.
    pub fn open(
        compiler: &'root Compiler<'work>,
        proof: &'root ProofDeployment<'work>,
        budget: &mut Budget<'work>,
    ) -> io::Result<(Self, Storage)> {
        let floor = budget.storage();
        budget.charge_work(IO_WORK).map_err(other)?;
        let borrowed = compiler
            .retained_storage()
            .map_err(other)?
            .checked_add(proof.retained_storage()?)
            .ok_or_else(|| io::Error::other("manager input storage"))?;
        require(floor >= borrowed, "native manager deployments not prepaid")?;
        budget.reserve_storage(IO_STORAGE).map_err(other)?;
        require_exact_root_identity_v1().map_err(other)?;
        require_proof_controller_parent_v1().map_err(other)?;
        compiler.revalidate(budget).map_err(other)?;
        proof.revalidate(budget)?;
        let namespaces = Namespaces::capture_current_thread().map_err(other)?;
        let config_tree = InstalledFile::open(CONFIG_PATH, 0o444, BYTES as u64)?;
        let bytes = read_config(config_tree.leaf())?;
        let (config, charge) = Config::decode(&bytes, budget)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        require(
            config.matches(compiler, proof.deployment()),
            "native manager independent deployment differs",
        )?;
        let measurement = config.measurement()?;
        let image_tree = InstalledFile::open(IMAGE_PATH, 0o555, measurement.byte_len())?;
        budget
            .reserve_storage(Image::file_storage(measurement).map_err(other)?)
            .map_err(other)?;
        let (image, charge) = Image::seal_source_for_owner(
            image_tree.leaf().try_clone()?,
            measurement,
            Owner::new(0, 0).map_err(other)?,
            "native root application manager",
            budget,
        )
        .map_err(other)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let value = Self {
            compiler,
            proof,
            config,
            config_tree,
            image_tree,
            image,
            namespaces,
            pid: std::process::id(),
            tid: rustix::thread::gettid().as_raw_pid() as u32,
            ledger: budget.work_ledger_identity_v1(),
            account: budget.storage_account_identity_v1(),
        };
        value.revalidate_inner(budget)?;
        let retained = value.retained_storage()?;
        budget
            .release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or_else(|| io::Error::other("manager admission accounting"))?,
            )
            .map_err(other)?;
        Ok((value, Storage(retained)))
    }
    /// This owner's reservation, excluding its two borrowed deployment owners.
    pub fn retained_storage(&self) -> io::Result<usize> {
        IO_STORAGE
            .checked_add(size_of::<Self>())
            .and_then(|n| n.checked_add(self.image.retained_storage()))
            .ok_or_else(|| io::Error::other("manager owner storage overflow"))
    }
    /// Matching description backed by the retained fixed-path installation.
    pub const fn configuration(&self) -> &Config {
        &self.config
    }
    /// The original independently admitted root compiler installation.
    pub const fn compiler(&self) -> &'root Compiler<'work> {
        self.compiler
    }
    /// The original independently admitted native proof installation.
    pub const fn proof(&self) -> &'root ProofDeployment<'work> {
        self.proof
    }
    /// Rechecks original account, process, namespace, image and installed paths.
    pub fn revalidate(&self, budget: &mut Budget<'work>) -> io::Result<()> {
        budget.charge_work(IO_WORK).map_err(other)?;
        let proof_storage = self.proof.retained_storage()?;
        let retained = self
            .retained_storage()?
            .checked_add(self.compiler.retained_storage().map_err(other)?)
            .and_then(|n| n.checked_add(proof_storage))
            .ok_or_else(|| io::Error::other("manager aggregate owner storage"))?;
        require(
            self.ledger == budget.work_ledger_identity_v1()
                && self.account == budget.storage_account_identity_v1()
                && budget.storage() >= retained,
            "native manager original account/floor differs",
        )?;
        budget.reserve_storage(IO_STORAGE).map_err(other)?;
        self.revalidate_inner(budget)?;
        budget.release_storage(IO_STORAGE).map_err(other)
    }
    fn revalidate_inner(&self, budget: &mut Budget<'work>) -> io::Result<()> {
        require(
            self.pid == std::process::id()
                && self.tid == rustix::thread::gettid().as_raw_pid() as u32,
            "native manager moved process/thread",
        )?;
        require_exact_root_identity_v1().map_err(other)?;
        require_proof_controller_parent_v1().map_err(other)?;
        self.namespaces.revalidate_current_thread().map_err(other)?;
        self.compiler.revalidate(budget).map_err(other)?;
        self.proof.revalidate(budget)?;
        self.config_tree.revalidate()?;
        self.image_tree.revalidate()?;
        require(
            read_config(self.config_tree.leaf())? == *self.config.canonical_bytes()
                && self.config.matches(self.compiler, self.proof.deployment()),
            "native manager configuration changed",
        )?;
        self.image.revalidate(budget).map_err(other)?;
        self.config_tree.revalidate()?;
        self.image_tree.revalidate()
    }
    /// Admits the actual running sealed native manager image, not its installed description.
    /// The dedicated root launcher must execute the approved sealed image first.
    /// Returned storage is growth over the consumed, already prepaid deployment owner.
    pub fn admit_running(
        self,
        budget: &mut Budget<'work>,
    ) -> io::Result<(RunningNativeApplicationManagerV1<'root, 'work>, Storage)> {
        self.revalidate(budget)?;
        let floor = budget.storage();
        let (running, charge) = Image::admit_running(
            self.config.measurement()?,
            Owner::new(0, 0).map_err(other)?,
            "running native root application manager",
            budget,
        )
        .map_err(other)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        self.revalidate(budget)?;
        let growth = running
            .retained_storage()
            .checked_add(size_of::<Image>())
            .ok_or_else(|| io::Error::other("running manager storage overflow"))?;
        let value = RunningNativeApplicationManagerV1 {
            deployment: self,
            running,
        };
        budget
            .release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or_else(|| io::Error::other("running manager accounting"))?,
            )
            .map_err(other)?;
        Ok((value, Storage(growth)))
    }
}
fn read_config(file: &File) -> io::Result<[u8; BYTES]> {
    let mut bytes = [0; BYTES];
    require(
        rustix::io::pread(file, &mut bytes, 0)? == BYTES
            && rustix::io::pread(file, &mut [0u8; 1], BYTES as u64)? == 0,
        "native manager exact fixed frame",
    )?;
    Ok(bytes)
}

/// Actual running sealed manager plus both original independent deployment owners.
/// No public constructor accepts descriptors, matching IDs, or caller validation callbacks.
///
/// ```compile_fail
/// use fe2o3_proof_custodian::{NativeApplicationManagerConfigurationV1 as C, RunningNativeApplicationManagerV1 as R};
/// fn fake(c:C) -> R<'static,'static> { c.into() }
/// ```
pub struct RunningNativeApplicationManagerV1<'root, 'work> {
    deployment: Deployment<'root, 'work>,
    running: Image,
}
impl<'root, 'work> RunningNativeApplicationManagerV1<'root, 'work> {
    /// The consumed original installation, still retaining both borrowed owners.
    pub const fn deployment(&self) -> &Deployment<'root, 'work> {
        &self.deployment
    }
    /// Combined installation and running-image reservation, excluding borrowed owners.
    pub fn retained_storage(&self) -> io::Result<usize> {
        self.deployment
            .retained_storage()?
            .checked_add(self.running.retained_storage())
            .and_then(|n| n.checked_add(size_of::<Image>()))
            .ok_or_else(|| io::Error::other("running manager storage"))
    }
    /// Rechecks the actual running image and complete original deployment custody.
    pub fn revalidate(&self, budget: &mut Budget<'work>) -> io::Result<()> {
        let borrowed = self
            .deployment
            .compiler
            .retained_storage()
            .map_err(other)?
            .checked_add(self.deployment.proof.retained_storage()?)
            .ok_or_else(|| io::Error::other("running manager borrowed storage"))?;
        let retained = self
            .retained_storage()?
            .checked_add(borrowed)
            .ok_or_else(|| io::Error::other("running manager aggregate storage"))?;
        require(
            budget.storage() >= retained,
            "running native manager unpaid",
        )?;
        self.deployment.revalidate(budget)?;
        self.running.revalidate(budget).map_err(other)
    }
}

#[cfg(test)]
mod tests;
