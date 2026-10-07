//! Fixed native controller configuration, distinct from the legacy producer profile.
use super::{
    InstalledFile, MAX_CONTROLLER, PhysicalMachineAnalyzerIdentityV1,
    PhysicalMachineEffectWorkerPolicyV1, PhysicalMachineRuntimeClosureIdentityV1,
    PhysicalMachineToolchainIdentityV1, PhysicalMachineWorkerExecutableIdentityV1,
    ProofControllerCredentialProfileV1, ProtectedServiceNamespaceSetV1,
    ProtectedStaticExecutableMeasurementV1, ProtectedStaticExecutableOwnerV1, open_procfs,
    require_exact_root_identity_v1, require_proof_controller_parent_v1,
};
use crate::native_application::resources::{self, Policy};
use crate::{other, require};
use fe2o3_compiler_execution_protocol::{
    NATIVE_PROOF_CUSTODIAN_CONFIGURATION_BYTES_V1,
    NativeApplicationProofCustodianConfigurationV1 as WireConfig,
    NativeProofCustodianConfigurationPartsV1 as Parts,
};
use fe2o3_functional_proof::FunctionalRefinementBoundaryV2;
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_protected_service_profile::observations;
use fe2o3_protected_static_executable::ProtectedStaticExecutableV2 as Image;
use rustix::fs::{Mode, OFlags, ResolveFlags};
#[cfg(test)]
use sha2::{Digest, Sha256};
use std::{fs::File, io, marker::PhantomData, os::unix::fs::MetadataExt};

pub(crate) const CONFIG_PATH: &str = "/etc/fe2o3/proof-custodian/application-native-deployment-v1";
pub(crate) const CONTROLLER_PATH: &str =
    "/usr/libexec/fe2o3/fe2o3-native-application-proof-controller";
pub(crate) const POLICY_PATH: &str = "/etc/fe2o3/proof-custodian/native-conditional-root-policy-v1";
const BYTES: usize = NATIVE_PROOF_CUSTODIAN_CONFIGURATION_BYTES_V1;
#[cfg(test)]
const IDENTITY: usize = BYTES - 32;
#[cfg(test)]
const DOMAIN: &[u8] = b"FE2O3/NATIVE-APPLICATION/PROOF-CUSTODIAN-DEPLOYMENT/V1\0";
// Open also performs a final revalidation. Both passes pay the bounded profile
// and namespace observers, in addition to path/descriptor and map operations.
const IO_WORK: usize = 2
    * (observations::PROCESS_CURRENT_WORK
        + observations::NAMESPACE_CAPTURE_WORK
        + observations::NAMESPACE_SELF_WORK)
    + 256 * 1024;
const IO_STORAGE: usize = 128 * 1024
    + observations::PROCESS_CURRENT_SCRATCH
    + observations::NAMESPACE_CAPTURE_SCRATCH
    + observations::NAMESPACE_SELF_SCRATCH;
#[cfg(test)]
const CONFIG_WORK: usize = 4096;
#[cfg(test)]
const CONFIG_SCRATCH: usize = 4 * WireConfig::RETAINED;

/// Public matching data only. Even a valid checksum grants no deployment authority.
/// The native compiler-policy identity and final-IR-to-machine boundary are explicit.
#[derive(Debug, Eq, PartialEq)]
pub struct NativeApplicationProofCustodianDeploymentV1 {
    wire: WireConfig,
}
type Config = NativeApplicationProofCustodianDeploymentV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeProofCustodianDeploymentStorageV1(usize);
impl NativeProofCustodianDeploymentStorageV1 {
    /// Full unreserved output-owner charge on the original account.
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}

impl Config {
    pub const RETAINED: usize =
        size_of::<Self>() + size_of::<NativeProofCustodianDeploymentStorageV1>();

    pub fn new(
        credentials: ProofControllerCredentialProfileV1,
        controller_sha256: [u8; 32],
        controller_bytes: u64,
        analyzer: PhysicalMachineEffectWorkerPolicyV1,
        verus_identity: [u8; 32],
        compiler_policy_identity: [u8; 32],
        semantic_policy: ([u8; 32], u64),
        budget: &mut Budget<'_>,
    ) -> io::Result<(Self, NativeProofCustodianDeploymentStorageV1)> {
        let parts = Parts {
            credentials: (credentials.uid(), credentials.gid()),
            controller: (controller_sha256, controller_bytes),
            analyzer_executable: (
                analyzer.executable().sha256(),
                analyzer.executable().byte_len(),
            ),
            analyzer_runtime_closure: (
                analyzer.runtime_closure().sha256(),
                analyzer.runtime_closure().byte_len(),
            ),
            analyzer_identity: analyzer.analyzer().as_bytes(),
            toolchain_identity: analyzer.toolchain().as_bytes(),
            verus_identity,
            compiler_policy_identity,
            semantic_policy,
        };
        let (wire, _) = WireConfig::new(parts, budget).map_err(other)?;
        Self::checked(wire)
    }

    pub fn decode(
        bytes: &[u8],
        budget: &mut Budget<'_>,
    ) -> io::Result<(Self, NativeProofCustodianDeploymentStorageV1)> {
        let (wire, _) = WireConfig::decode(bytes, budget).map_err(other)?;
        Self::checked(wire)
    }

    fn checked(wire: WireConfig) -> io::Result<(Self, NativeProofCustodianDeploymentStorageV1)> {
        require(
            wire.boundary()
                == FunctionalRefinementBoundaryV2::FinalKernelIrToGfx942FillDispatchConditional
                    as u8,
            "native deployment refinement boundary",
        )?;
        let value = Self { wire };
        value.credentials()?;
        value.controller_measurement()?;
        value.analyzer_policy()?;
        require(
            (1..=resources::MAX_POLICY_BYTES as u64).contains(&value.semantic_policy_byte_len()),
            "native semantic policy extent",
        )?;
        Ok((
            value,
            NativeProofCustodianDeploymentStorageV1(Self::RETAINED),
        ))
    }

    pub const fn canonical_bytes(&self) -> &[u8; BYTES] {
        self.wire.canonical_bytes()
    }
    pub fn identity(&self) -> [u8; 32] {
        self.wire.identity()
    }
    pub fn compiler_policy_identity(&self) -> [u8; 32] {
        self.wire.parts().compiler_policy_identity
    }
    pub fn verus_identity(&self) -> [u8; 32] {
        self.wire.parts().verus_identity
    }
    pub fn semantic_policy(&self) -> ([u8; 32], u64) {
        self.wire.parts().semantic_policy
    }
    pub fn semantic_policy_sha256(&self) -> [u8; 32] {
        self.semantic_policy().0
    }
    pub fn semantic_policy_byte_len(&self) -> u64 {
        self.semantic_policy().1
    }
    pub fn credentials(&self) -> io::Result<ProofControllerCredentialProfileV1> {
        ProofControllerCredentialProfileV1::new(
            self.wire.parts().credentials.0,
            self.wire.parts().credentials.1,
        )
        .map_err(other)
    }
    pub fn analyzer_policy(&self) -> io::Result<PhysicalMachineEffectWorkerPolicyV1> {
        let parts = self.wire.parts();
        PhysicalMachineEffectWorkerPolicyV1::new(
            PhysicalMachineWorkerExecutableIdentityV1::from_parts(
                parts.analyzer_executable.0,
                parts.analyzer_executable.1,
            ),
            PhysicalMachineRuntimeClosureIdentityV1::from_parts(
                parts.analyzer_runtime_closure.0,
                parts.analyzer_runtime_closure.1,
            ),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes(parts.analyzer_identity),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes(parts.toolchain_identity),
        )
        .map_err(other)
    }
    pub(crate) fn controller_measurement(
        &self,
    ) -> io::Result<ProtectedStaticExecutableMeasurementV1> {
        ProtectedStaticExecutableMeasurementV1::new(
            self.wire.parts().controller.0,
            self.wire.parts().controller.1,
            MAX_CONTROLLER,
        )
        .map_err(other)
    }
}

#[cfg(test)]
fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

/// Original native fixed-path configuration and sealed executable, not a proof lease.
///
/// The actual work lifetime and both resource identities are retained. Failed opens
/// and revalidations are terminal and keep their admission charges; successful calls
/// retire only their own scratch. Quotas are logical, not syscall-latency or RSS bounds.
/// Only real unremapped root can open this owner; there is no legacy conversion.
///
/// ```compile_fail
/// use fe2o3_proof_custodian::ProductionNativeApplicationProofCustodianDeploymentV1 as Native;
/// fn cloneable<T: Clone>() {}
/// cloneable::<Native<'_>>();
/// ```
/// ```compile_fail
/// use fe2o3_proof_custodian::ProductionNativeApplicationProofCustodianDeploymentV1 as Native;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<Native<'_>>();
/// ```
pub struct ProductionNativeApplicationProofCustodianDeploymentV1<'work> {
    config: Config,
    pub(crate) executable: Image,
    config_tree: InstalledFile,
    controller_tree: InstalledFile,
    policy_tree: InstalledFile,
    semantic_policy: Policy<'work>,
    namespaces: ProtectedServiceNamespaceSetV1,
    procfs: File,
    pid: u32,
    tid: u32,
    ledger: Ledger,
    account: Option<Account>,
    lifetime: PhantomData<&'work Work>,
    thread: PhantomData<*mut ()>,
}
type Deployment<'work> = ProductionNativeApplicationProofCustodianDeploymentV1<'work>;

impl<'work> Deployment<'work> {
    pub fn open(
        budget: &mut Budget<'work>,
    ) -> io::Result<(Self, NativeProofCustodianDeploymentStorageV1)> {
        let floor = budget.storage();
        budget.charge_work(IO_WORK).map_err(other)?;
        budget.reserve_storage(IO_STORAGE).map_err(other)?;
        require_exact_root_identity_v1().map_err(other)?;
        require_proof_controller_parent_v1().map_err(other)?;
        let namespaces = ProtectedServiceNamespaceSetV1::capture_current_thread().map_err(other)?;
        let procfs = open_procfs()?;
        check_maps(&procfs)?;
        let config_tree = InstalledFile::open(CONFIG_PATH, 0o444, BYTES as u64)?;
        let bytes = read_config(config_tree.leaf())?;
        let (config, storage) = Config::decode(&bytes, budget)?;
        budget
            .reserve_storage(storage.additional_storage())
            .map_err(other)?;
        let policy_tree =
            InstalledFile::open(POLICY_PATH, 0o444, config.semantic_policy_byte_len())?;
        let (policy_file, charge) =
            resources::seal_policy_source(policy_tree.leaf(), config.semantic_policy(), budget)?;
        budget.reserve_storage(charge).map_err(other)?;
        policy_tree.revalidate()?;
        let (semantic_policy, charge) = Policy::admit(policy_file, &config, budget)?;
        budget.reserve_storage(charge).map_err(other)?;
        let measurement = config.controller_measurement()?;
        let controller_tree = InstalledFile::open(CONTROLLER_PATH, 0o555, measurement.byte_len())?;
        let source_charge = Image::file_storage(measurement).map_err(other)?;
        budget.reserve_storage(source_charge).map_err(other)?;
        let (executable, storage) = Image::seal_source_for_owner(
            controller_tree.leaf().try_clone()?,
            measurement,
            ProtectedStaticExecutableOwnerV1::new(0, 0).map_err(other)?,
            "native application proof controller",
            budget,
        )
        .map_err(other)?;
        budget
            .reserve_storage(storage.additional_storage())
            .map_err(other)?;
        let value = Self {
            config,
            executable,
            config_tree,
            controller_tree,
            policy_tree,
            semantic_policy,
            namespaces,
            procfs,
            pid: std::process::id(),
            tid: rustix::thread::gettid().as_raw_pid() as u32,
            ledger: budget.work_ledger_identity_v1(),
            account: budget.storage_account_identity_v1(),
            lifetime: PhantomData,
            thread: PhantomData,
        };
        value.revalidate_inner(budget)?;
        let retained = value.retained_storage()?;
        budget
            .release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or_else(|| io::Error::other("native deployment accounting"))?,
            )
            .map_err(other)?;
        Ok((value, NativeProofCustodianDeploymentStorageV1(retained)))
    }

    pub const fn deployment(&self) -> &Config {
        &self.config
    }
    pub fn retained_storage(&self) -> io::Result<usize> {
        // The three fixed paths have <=6 components each; this allowance
        // covers their retained path vectors, snapshots and namespace descriptors.
        IO_STORAGE
            .checked_add(self.executable.retained_storage())
            .and_then(|n| n.checked_add(self.semantic_policy.retained_storage()))
            .ok_or_else(|| io::Error::other("native deployment retained storage overflow"))
    }
    pub(crate) fn clone_policy_for_handoff(
        &self,
        budget: &mut Budget<'work>,
    ) -> io::Result<(File, usize)> {
        self.revalidate(budget)?;
        let result = self.semantic_policy.try_clone_for_handoff(budget)?;
        // Reserve the unreturned output while revalidating its independent origin.
        budget.reserve_storage(result.1).map_err(other)?;
        self.revalidate(budget)?;
        budget.release_storage(result.1).map_err(other)?;
        Ok(result)
    }
    pub fn revalidate(&self, budget: &mut Budget<'work>) -> io::Result<()> {
        budget.charge_work(IO_WORK).map_err(other)?;
        require(
            self.ledger == budget.work_ledger_identity_v1()
                && self.account == budget.storage_account_identity_v1(),
            "native deployment account replaced",
        )?;
        require(
            budget.storage() >= self.retained_storage()?,
            "native deployment owner is not prepaid",
        )?;
        budget.reserve_storage(IO_STORAGE).map_err(other)?;
        self.revalidate_inner(budget)?;
        budget.release_storage(IO_STORAGE).map_err(other)
    }
    fn revalidate_inner(&self, budget: &mut Budget<'work>) -> io::Result<()> {
        require(
            self.pid == std::process::id()
                && self.tid == rustix::thread::gettid().as_raw_pid() as u32,
            "native deployment moved process/thread",
        )?;
        require_exact_root_identity_v1().map_err(other)?;
        require_proof_controller_parent_v1().map_err(other)?;
        self.namespaces.revalidate_current_thread().map_err(other)?;
        let procfs = open_procfs()?;
        let old = self.procfs.metadata()?;
        let now = procfs.metadata()?;
        require(
            (old.dev(), old.ino()) == (now.dev(), now.ino()),
            "native deployment procfs changed",
        )?;
        check_maps(&self.procfs)?;
        check_maps(&procfs)?;
        self.config_tree.revalidate()?;
        self.controller_tree.revalidate()?;
        self.policy_tree.revalidate()?;
        require(
            read_config(self.config_tree.leaf())? == *self.config.canonical_bytes(),
            "native deployment bytes changed",
        )?;
        self.executable.revalidate(budget).map_err(other)?;
        self.semantic_policy.revalidate(budget)?;
        self.config_tree.revalidate()?;
        self.controller_tree.revalidate()?;
        self.policy_tree.revalidate()
    }
}

fn read_config(file: &File) -> io::Result<[u8; BYTES]> {
    let mut bytes = [0; BYTES];
    require(
        rustix::io::pread(file, &mut bytes, 0)? == BYTES,
        "short native deployment read",
    )?;
    Ok(bytes)
}
fn check_maps(procfs: &File) -> io::Result<()> {
    for path in ["thread-self/uid_map", "thread-self/gid_map"] {
        let file = File::from(rustix::fs::openat2(
            procfs,
            path,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_XDEV | ResolveFlags::NO_MAGICLINKS,
        )?);
        require(
            rustix::fs::fstatfs(&file)?.f_type == libc::PROC_SUPER_MAGIC,
            "native mapping is not procfs",
        )?;
        let mut bytes = [0; 4097];
        let len = rustix::io::read(&file, &mut bytes)?;
        require(
            len > 0 && len <= 4096 && bytes[len - 1] == b'\n',
            "nonexact native namespace map",
        )?;
        let mut fields = std::str::from_utf8(&bytes[..len])
            .map_err(other)?
            .split_ascii_whitespace();
        require(
            fields.next() == Some("0")
                && fields.next() == Some("0")
                && fields.next() == Some("4294967295")
                && fields.next().is_none(),
            "remapped native deployment namespace",
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
