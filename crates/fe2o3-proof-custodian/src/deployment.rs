use crate::{other, require};
use fe2o3_kernel_analysis::{
    PhysicalMachineAnalyzerIdentityV1, PhysicalMachineEffectWorkerPolicyV1,
    PhysicalMachineRuntimeClosureIdentityV1, PhysicalMachineToolchainIdentityV1,
    PhysicalMachineWorkerExecutableIdentityV1,
};
use fe2o3_protected_service_profile::{
    ProofControllerCredentialProfileV1, ProtectedServiceNamespaceSetV1,
    require_proof_controller_parent_v1,
};
use fe2o3_protected_service_spawn::require_exact_root_identity_v1;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableMeasurementV1, ProtectedStaticExecutableOwnerV1,
    ProtectedStaticExecutableV1,
};
use rustix::fs::{Mode, OFlags};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read},
    marker::PhantomData,
    os::unix::fs::{FileExt, MetadataExt},
};

pub(crate) const DEPLOYMENT_BYTES: usize = 280;
pub(crate) const CONFIG_PATH: &str = "/etc/fe2o3/proof-custodian/deployment-v1";
pub(crate) const CONTROLLER_PATH: &str =
    "/usr/libexec/fe2o3/fe2o3-conditional-fill-proof-controller";
pub(crate) const APPLICATION_CONFIG_PATH: &str =
    "/etc/fe2o3/proof-custodian/application-deployment-v1";
pub(crate) const APPLICATION_CONTROLLER_PATH: &str =
    "/usr/libexec/fe2o3/fe2o3-application-proof-controller";
pub(crate) const WORKER_PATH: &str = "/usr/libexec/fe2o3/fe2o3-llvm-link-worker";
pub(crate) const RUNTIME_PATH: &str =
    "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";
const MAX_CONTROLLER: u64 = 128 * 1024 * 1024;
const DOMAIN: &[u8] = b"FE2O3/CONDITIONAL-FILL/PROOF-CUSTODIAN-DEPLOYMENT/V1\0";

/// Canonical public measurements. Constructing bytes does not approve a deployment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofCustodianDeploymentV1 {
    bytes: [u8; DEPLOYMENT_BYTES],
}

impl ProofCustodianDeploymentV1 {
    /// Encodes one fixed controller, analyzer closure and protected Verus runtime.
    pub fn new(
        credentials: ProofControllerCredentialProfileV1,
        controller_sha256: [u8; 32],
        controller_bytes: u64,
        analyzer: PhysicalMachineEffectWorkerPolicyV1,
        verus_identity: [u8; 32],
    ) -> io::Result<Self> {
        let mut bytes = [0; DEPLOYMENT_BYTES];
        bytes[..8].copy_from_slice(b"F3PCDP1\0");
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&(DEPLOYMENT_BYTES as u32).to_le_bytes());
        bytes[24..28].copy_from_slice(&credentials.uid().to_le_bytes());
        bytes[28..32].copy_from_slice(&credentials.gid().to_le_bytes());
        bytes[32..64].copy_from_slice(&controller_sha256);
        bytes[64..72].copy_from_slice(&controller_bytes.to_le_bytes());
        bytes[72..104].copy_from_slice(&analyzer.executable().sha256());
        bytes[104..112].copy_from_slice(&analyzer.executable().byte_len().to_le_bytes());
        bytes[112..144].copy_from_slice(&analyzer.runtime_closure().sha256());
        bytes[144..152].copy_from_slice(&analyzer.runtime_closure().byte_len().to_le_bytes());
        bytes[152..184].copy_from_slice(&analyzer.analyzer().as_bytes());
        bytes[184..216].copy_from_slice(&analyzer.toolchain().as_bytes());
        bytes[216..248].copy_from_slice(&verus_identity);
        let identity = hash(&bytes[..248]);
        bytes[248..].copy_from_slice(&identity);
        Self::decode(&bytes)
    }

    pub fn decode(bytes: &[u8]) -> io::Result<Self> {
        require(bytes.len() == DEPLOYMENT_BYTES, "wrong deployment length")?;
        require(
            &bytes[..8] == b"F3PCDP1\0"
                && bytes[8..10] == 1u16.to_le_bytes()
                && bytes[10..12] == [0; 2]
                && bytes[12..16] == (DEPLOYMENT_BYTES as u32).to_le_bytes()
                && bytes[16..24] == [0; 8]
                && bytes[248..] == hash(&bytes[..248]),
            "noncanonical deployment header or identity",
        )?;
        let value = Self {
            bytes: bytes.try_into().unwrap(),
        };
        value.credentials()?;
        value.controller_measurement()?;
        value.analyzer_policy()?;
        require(
            value.verus_identity() != [0; 32],
            "zero Verus runtime identity",
        )?;
        Ok(value)
    }

    pub const fn canonical_bytes(&self) -> &[u8; DEPLOYMENT_BYTES] {
        &self.bytes
    }
    pub fn identity(&self) -> [u8; 32] {
        self.bytes[248..].try_into().unwrap()
    }
    pub fn credentials(&self) -> io::Result<ProofControllerCredentialProfileV1> {
        ProofControllerCredentialProfileV1::new(
            u32::from_le_bytes(self.bytes[24..28].try_into().unwrap()),
            u32::from_le_bytes(self.bytes[28..32].try_into().unwrap()),
        )
        .map_err(other)
    }
    pub fn analyzer_policy(&self) -> io::Result<PhysicalMachineEffectWorkerPolicyV1> {
        PhysicalMachineEffectWorkerPolicyV1::new(
            PhysicalMachineWorkerExecutableIdentityV1::from_parts(
                self.bytes[72..104].try_into().unwrap(),
                number(&self.bytes[104..112]),
            ),
            PhysicalMachineRuntimeClosureIdentityV1::from_parts(
                self.bytes[112..144].try_into().unwrap(),
                number(&self.bytes[144..152]),
            ),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes(
                self.bytes[152..184].try_into().unwrap(),
            ),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes(
                self.bytes[184..216].try_into().unwrap(),
            ),
        )
        .map_err(other)
    }
    pub fn verus_identity(&self) -> [u8; 32] {
        self.bytes[216..248].try_into().unwrap()
    }
    pub(crate) fn controller_measurement(
        &self,
    ) -> io::Result<ProtectedStaticExecutableMeasurementV1> {
        ProtectedStaticExecutableMeasurementV1::new(
            self.bytes[32..64].try_into().unwrap(),
            number(&self.bytes[64..72]),
            MAX_CONTROLLER,
        )
        .map_err(other)
    }
}

fn number(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes.try_into().unwrap())
}
fn hash(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}

/// Original fixed-path root configuration and exact sealed controller image.
///
/// Only `open` establishes installed provenance. No caller path, raw descriptor,
/// candidate measurement or receipt can be promoted to this owner. This trusts
/// the local root/kernel and does not claim rollback resistance or GPU authority.
/// The administrator must reserve a dedicated proof UID/GID distinct from the
/// issuer and external anchor; this opener does not admit those other deployments.
///
/// ```compile_fail
/// use fe2o3_proof_custodian::ProductionProofCustodianDeploymentV1;
/// fn cloneable<T: Clone>() {}
/// cloneable::<ProductionProofCustodianDeploymentV1>();
/// ```
pub struct ProductionProofCustodianDeploymentV1 {
    pub(crate) config: ProofCustodianDeploymentV1,
    pub(crate) executable: ProtectedStaticExecutableV1,
    config_tree: InstalledFile,
    controller_tree: InstalledFile,
    namespaces: ProtectedServiceNamespaceSetV1,
    procfs: File,
    pid: u32,
    tid: u32,
    _thread: PhantomData<*mut ()>,
}

impl ProductionProofCustodianDeploymentV1 {
    /// Opens the sole root-owned configuration and controller at their fixed paths.
    pub fn open() -> io::Result<Self> {
        Self::open_fixed(CONFIG_PATH, CONTROLLER_PATH)
    }
    fn open_fixed(config_path: &'static str, controller_path: &'static str) -> io::Result<Self> {
        require_exact_root_identity_v1().map_err(other)?;
        require_proof_controller_parent_v1().map_err(other)?;
        let namespaces = ProtectedServiceNamespaceSetV1::capture_current_thread().map_err(other)?;
        let procfs = open_procfs()?;
        check_maps(&procfs)?;
        let config_tree = InstalledFile::open(config_path, 0o444, DEPLOYMENT_BYTES as u64)?;
        let mut bytes = [0; DEPLOYMENT_BYTES];
        config_tree.leaf().read_exact_at(&mut bytes, 0)?;
        let config = ProofCustodianDeploymentV1::decode(&bytes)?;
        let measurement = config.controller_measurement()?;
        let controller_tree = InstalledFile::open(controller_path, 0o555, measurement.byte_len())?;
        let executable = ProtectedStaticExecutableV1::seal_source_for_owner(
            controller_tree.leaf().try_clone()?,
            measurement,
            ProtectedStaticExecutableOwnerV1::new(0, 0).map_err(other)?,
            "conditional-fill proof controller",
        )
        .map_err(other)?;
        let value = Self {
            config,
            executable,
            config_tree,
            controller_tree,
            namespaces,
            procfs,
            pid: std::process::id(),
            tid: rustix::thread::gettid().as_raw_pid() as u32,
            _thread: PhantomData,
        };
        value.revalidate()?;
        Ok(value)
    }

    pub fn deployment(&self) -> &ProofCustodianDeploymentV1 {
        &self.config
    }

    /// Rechecks original files, current path edges, calling thread and namespace.
    pub fn revalidate(&self) -> io::Result<()> {
        require(
            self.pid == std::process::id()
                && self.tid == rustix::thread::gettid().as_raw_pid() as u32,
            "deployment owner moved process/thread",
        )?;
        require_exact_root_identity_v1().map_err(other)?;
        require_proof_controller_parent_v1().map_err(other)?;
        self.namespaces.revalidate_current_thread().map_err(other)?;
        let procfs = open_procfs()?;
        let old = self.procfs.metadata()?;
        let now = procfs.metadata()?;
        require(
            (old.dev(), old.ino()) == (now.dev(), now.ino()),
            "deployment procfs changed",
        )?;
        check_maps(&self.procfs)?;
        check_maps(&procfs)?;
        self.config_tree.revalidate()?;
        self.controller_tree.revalidate()?;
        let mut bytes = [0; DEPLOYMENT_BYTES];
        self.config_tree.leaf().read_exact_at(&mut bytes, 0)?;
        require(
            bytes == *self.config.canonical_bytes(),
            "installed deployment bytes changed",
        )?;
        self.executable.revalidate().map_err(other)?;
        self.config_tree.revalidate()?;
        self.controller_tree.revalidate()
    }
}

/// Independently installed application-controller image, distinct from the root-only producer.
/// This root-owned admission is neither application registration nor a remote GPU proof lease.
pub struct ProductionApplicationProofCustodianDeploymentV1(
    pub(crate) ProductionProofCustodianDeploymentV1,
);
impl ProductionApplicationProofCustodianDeploymentV1 {
    pub fn open() -> io::Result<Self> {
        ProductionProofCustodianDeploymentV1::open_fixed(
            APPLICATION_CONFIG_PATH,
            APPLICATION_CONTROLLER_PATH,
        )
        .map(Self)
    }
    pub fn deployment(&self) -> &ProofCustodianDeploymentV1 {
        self.0.deployment()
    }
    pub fn revalidate(&self) -> io::Result<()> {
        self.0.revalidate()
    }
}

fn open_procfs() -> io::Result<File> {
    let file: File = rustix::fs::open(
        "/proc",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?
    .into();
    require(
        rustix::fs::fstatfs(&file)?.f_type == libc::PROC_SUPER_MAGIC,
        "deployment needs kernel procfs",
    )?;
    Ok(file)
}
fn check_maps(procfs: &File) -> io::Result<()> {
    for path in ["thread-self/uid_map", "thread-self/gid_map"] {
        let file: File = rustix::fs::openat2(
            procfs,
            path,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
            rustix::fs::ResolveFlags::BENEATH
                | rustix::fs::ResolveFlags::NO_XDEV
                | rustix::fs::ResolveFlags::NO_MAGICLINKS,
        )?
        .into();
        require(
            rustix::fs::fstatfs(&file)?.f_type == libc::PROC_SUPER_MAGIC,
            "deployment mapping is not procfs",
        )?;
        let mut bytes = Vec::new();
        file.take(4097).read_to_end(&mut bytes)?;
        require(bytes.len() <= 4096, "oversized UID/GID map")?;
        let fields = std::str::from_utf8(&bytes)
            .map_err(other)?
            .split_ascii_whitespace()
            .collect::<Vec<_>>();
        require(
            fields == ["0", "0", "4294967295"],
            "remapped user namespace rejected",
        )?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Snapshot {
    dev: u64,
    ino: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    len: u64,
    mtime: i64,
    mtime_ns: i64,
    ctime: i64,
    ctime_ns: i64,
}
fn snapshot(file: &File) -> io::Result<Snapshot> {
    let m = file.metadata()?;
    Ok(Snapshot {
        dev: m.dev(),
        ino: m.ino(),
        mode: m.mode(),
        uid: m.uid(),
        gid: m.gid(),
        links: m.nlink(),
        len: m.len(),
        mtime: m.mtime(),
        mtime_ns: m.mtime_nsec(),
        ctime: m.ctime(),
        ctime_ns: m.ctime_nsec(),
    })
}
fn check(file: &File, directory: bool, mode: u32, len: u64) -> io::Result<Snapshot> {
    let s = snapshot(file)?;
    require(
        s.uid == 0 && s.gid == 0,
        "installed object is not root owned",
    )?;
    require(
        if directory {
            s.mode & libc::S_IFMT == libc::S_IFDIR && s.mode & 0o022 == 0 && s.mode & 0o100 != 0
        } else {
            s.mode == libc::S_IFREG | mode && s.links == 1 && s.len == len
        },
        "installed object metadata differs",
    )?;
    let mut attributes = [0u8; 1];
    require(
        rustix::fs::flistxattr(file, &mut attributes)? == 0,
        "installed object has extended attributes",
    )?;
    require(
        rustix::io::fcntl_getfd(file)? == rustix::io::FdFlags::CLOEXEC
            && rustix::fs::fcntl_getfl(file)?
                & (OFlags::ACCMODE
                    | OFlags::PATH
                    | OFlags::APPEND
                    | OFlags::ASYNC
                    | OFlags::DIRECT
                    | OFlags::NONBLOCK)
                == OFlags::RDONLY
                    | if directory {
                        OFlags::empty()
                    } else {
                        OFlags::NONBLOCK
                    },
        "installed descriptor flags differ",
    )?;
    Ok(s)
}

struct InstalledFile {
    path: &'static str,
    handles: Vec<File>,
    snapshots: Vec<Snapshot>,
    mode: u32,
    len: u64,
}
impl InstalledFile {
    fn open(path: &'static str, mode: u32, len: u64) -> io::Result<Self> {
        let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW;
        let root = File::from(rustix::fs::open(
            "/",
            flags | OFlags::DIRECTORY,
            Mode::empty(),
        )?);
        let mut snapshots = vec![check(&root, true, 0, 0)?];
        let mut handles = vec![root];
        let parts: Vec<_> = path.split('/').filter(|part| !part.is_empty()).collect();
        for (index, part) in parts.iter().enumerate() {
            let directory = index + 1 < parts.len();
            let file = File::from(rustix::fs::openat(
                handles.last().unwrap(),
                *part,
                flags
                    | if directory {
                        OFlags::DIRECTORY
                    } else {
                        OFlags::NONBLOCK
                    },
                Mode::empty(),
            )?);
            snapshots.push(check(&file, directory, mode, len)?);
            handles.push(file);
        }
        Ok(Self {
            path,
            handles,
            snapshots,
            mode,
            len,
        })
    }
    fn leaf(&self) -> &File {
        self.handles.last().unwrap()
    }
    fn revalidate(&self) -> io::Result<()> {
        let reopened = Self::open(self.path, self.mode, self.len)?;
        for (index, ((old, expected), actual)) in self
            .handles
            .iter()
            .zip(&self.snapshots)
            .zip(&reopened.snapshots)
            .enumerate()
        {
            let directory = index + 1 < self.handles.len();
            let retained = check(old, directory, self.mode, self.len)?;
            // Directory entries may change; the original path, ownership and mode may not.
            let same = |s: &Snapshot| (s.dev, s.ino, s.mode, s.uid, s.gid);
            require(
                if directory {
                    same(&retained) == same(expected) && same(actual) == same(expected)
                } else {
                    retained == *expected && actual == expected
                },
                "installed path or original object changed",
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deployment() -> ProofCustodianDeploymentV1 {
        let analyzer = PhysicalMachineEffectWorkerPolicyV1::new(
            PhysicalMachineWorkerExecutableIdentityV1::from_parts([2; 32], 1024),
            PhysicalMachineRuntimeClosureIdentityV1::from_parts([3; 32], 2048),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([4; 32]),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes([5; 32]),
        )
        .unwrap();
        ProofCustodianDeploymentV1::new(
            ProofControllerCredentialProfileV1::new(61000, 61000).unwrap(),
            [1; 32],
            4096,
            analyzer,
            [6; 32],
        )
        .unwrap()
    }

    #[test]
    fn canonical_deployment_binds_every_byte_and_rejects_extensions() {
        let config = deployment();
        assert_eq!(
            ProofCustodianDeploymentV1::decode(config.canonical_bytes()).unwrap(),
            config
        );
        for index in 0..DEPLOYMENT_BYTES {
            let mut bytes = *config.canonical_bytes();
            bytes[index] ^= 1;
            assert!(
                ProofCustodianDeploymentV1::decode(&bytes).is_err(),
                "byte {index}"
            );
        }
        assert!(ProofCustodianDeploymentV1::decode(&config.canonical_bytes()[..279]).is_err());
        let mut extended = config.canonical_bytes().to_vec();
        extended.push(0);
        assert!(ProofCustodianDeploymentV1::decode(&extended).is_err());
    }

    #[test]
    fn matching_hash_cannot_authorize_invalid_measurements_or_credentials() {
        for range in [
            24..28,
            28..32,
            32..64,
            64..72,
            72..104,
            104..112,
            112..144,
            144..152,
            152..184,
            184..216,
            216..248,
        ] {
            let mut bytes = *deployment().canonical_bytes();
            bytes[range.clone()].fill(0);
            let digest = hash(&bytes[..248]);
            bytes[248..].copy_from_slice(&digest);
            assert!(
                ProofCustodianDeploymentV1::decode(&bytes).is_err(),
                "{range:?}"
            );
        }
    }
}
