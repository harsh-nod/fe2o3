use super::*;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableMeasurementV1 as Measurement, ProtectedStaticExecutableOwnerV1,
    ProtectedStaticExecutableV1, RootInstalledFileV1,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    os::unix::fs::{FileExt, MetadataExt},
};

pub(super) const CONFIG_PATH: &str = "/etc/fe2o3/proof-custodian/manager-deployment-v1";
pub(super) const MANAGER_PATH: &str = "/usr/libexec/fe2o3/fe2o3-proof-manager";
const COORDINATOR_PATH: &str = "/usr/libexec/fe2o3/fe2o3-compiler-execution-coordinator";
const BYTES: usize = 160;
const MAX_IMAGE: u64 = 128 * 1024 * 1024;

/// Canonical independent manager/coordinator measurements. Bytes alone approve no process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofManagerDeploymentV1 {
    bytes: [u8; BYTES],
}
impl ProofManagerDeploymentV1 {
    pub fn new(
        manager: ([u8; 32], u64),
        coordinator: ([u8; 32], u64),
        application_deployment: [u8; 32],
    ) -> Result<Self> {
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(b"F3PMDP1\0");
        bytes[8..10].copy_from_slice(&1u16.to_le_bytes());
        bytes[12..16].copy_from_slice(&(BYTES as u32).to_le_bytes());
        for (offset, value) in [(16, manager), (56, coordinator)] {
            bytes[offset..offset + 32].copy_from_slice(&value.0);
            bytes[offset + 32..offset + 40].copy_from_slice(&value.1.to_le_bytes());
        }
        bytes[96..128].copy_from_slice(&application_deployment);
        let identity = digest(&bytes[..128]);
        bytes[128..].copy_from_slice(&identity);
        Self::decode(&bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != BYTES
            || &bytes[..8] != b"F3PMDP1\0"
            || bytes[8..10] != 1u16.to_le_bytes()
            || bytes[10..12] != [0; 2]
            || bytes[12..16] != (BYTES as u32).to_le_bytes()
            || bytes[128..] != digest(&bytes[..128])
            || bytes[96..128] == [0; 32]
            || bytes[16..48] == bytes[56..88]
        {
            return Err(invalid("invalid proof-manager deployment"));
        }
        let value = Self {
            bytes: bytes.try_into().unwrap(),
        };
        value.measurement(true)?;
        value.measurement(false)?;
        Ok(value)
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn identity(&self) -> [u8; 32] {
        self.bytes[128..].try_into().unwrap()
    }
    pub fn application_deployment(&self) -> [u8; 32] {
        self.bytes[96..128].try_into().unwrap()
    }
    fn measurement(&self, manager: bool) -> Result<Measurement> {
        let start = if manager { 16 } else { 56 };
        Measurement::new(
            self.bytes[start..start + 32].try_into().unwrap(),
            u64::from_le_bytes(self.bytes[start + 32..start + 40].try_into().unwrap()),
            MAX_IMAGE,
        )
        .map_err(|_| invalid("invalid proof-manager image measurement"))
    }
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/PROOF-MANAGER-DEPLOYMENT/V1\0");
    hash.update(bytes);
    hash.finalize().into()
}

pub(super) struct Approval {
    config: RootInstalledFileV1,
    pub(super) deployment: ProofManagerDeploymentV1,
    installed_manager: RootInstalledFileV1,
    installed_coordinator: RootInstalledFileV1,
}
impl Approval {
    pub(super) fn open() -> Result<Self> {
        fe2o3_protected_service_spawn::require_exact_root_identity_v1()
            .map_err(|_| invalid("proof-manager channel requires exact root identity"))?;
        let config = RootInstalledFileV1::open(CONFIG_PATH, 0o444, BYTES as u64)?;
        let mut bytes = [0; BYTES];
        config.leaf().read_exact_at(&mut bytes, 0)?;
        let deployment = ProofManagerDeploymentV1::decode(&bytes)?;
        let installed_manager = RootInstalledFileV1::open(
            MANAGER_PATH,
            0o555,
            deployment.measurement(true)?.byte_len(),
        )?;
        let installed_coordinator = RootInstalledFileV1::open(
            COORDINATOR_PATH,
            0o555,
            deployment.measurement(false)?.byte_len(),
        )?;
        let value = Self {
            config,
            deployment,
            installed_manager,
            installed_coordinator,
        };
        value.revalidate()?;
        Ok(value)
    }
    pub(super) fn revalidate(&self) -> Result<()> {
        self.config.revalidate()?;
        self.installed_manager.revalidate()?;
        self.installed_coordinator.revalidate()?;
        let mut bytes = [0; BYTES];
        self.config.leaf().read_exact_at(&mut bytes, 0)?;
        if bytes != self.deployment.bytes {
            return Err(invalid("manager approval changed"));
        }
        Ok(())
    }
    pub(super) fn admit_process(
        &self,
        process: LiveClientPidfdIdentityV1,
        manager: bool,
    ) -> Result<ApprovedProcess> {
        self.revalidate()?;
        let image = ApprovedProcess::admit(process, self.deployment.measurement(manager)?)?;
        let installed = if manager {
            &self.installed_manager
        } else {
            &self.installed_coordinator
        };
        if object(&image.source)? != object(installed.leaf())? {
            return Err(invalid(
                "running manager/coordinator is not the independently installed image object",
            ));
        }
        self.revalidate()?;
        Ok(image)
    }
}

type Object = (u64, u64, u32, u32, u32, u64, u64, i64, i64, i64, i64);
fn object(file: &File) -> io::Result<Object> {
    let m = file.metadata()?;
    Ok((
        m.dev(),
        m.ino(),
        m.mode(),
        m.uid(),
        m.gid(),
        m.nlink(),
        m.len(),
        m.mtime(),
        m.mtime_nsec(),
        m.ctime(),
        m.ctime_nsec(),
    ))
}

pub(super) struct ApprovedProcess {
    pub(super) process: LiveClientPidfdIdentityV1,
    proc: File,
    source: File,
    snapshot: Object,
    _measured: ProtectedStaticExecutableV1,
    namespaces: Vec<(String, (u64, u64))>,
}
impl ApprovedProcess {
    fn admit(process: LiveClientPidfdIdentityV1, measurement: Measurement) -> Result<Self> {
        process.validate_liveness()?;
        if process.expected_client.uid != 0 || process.expected_client.gid != 0 {
            return Err(invalid("manager/coordinator peer is not root"));
        }
        let proc: File = rustix::fs::open(
            format!("/proc/{}", process.expected_client.pid),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?
        .into();
        if rustix::fs::fstatfs(&proc)?.f_type != libc::PROC_SUPER_MAGIC {
            return Err(invalid("manager process observation is not procfs"));
        }
        let source = executable(&proc)?;
        let snapshot = object(&source)?;
        let measured = ProtectedStaticExecutableV1::seal_source_for_owner(
            source.try_clone()?,
            measurement,
            ProtectedStaticExecutableOwnerV1::new(0, 0).map_err(|_| invalid("root image owner"))?,
            "proof-manager channel peer",
        )
        .map_err(|error| CompilerExecutionObserverErrorV1::Profile(error.to_string()))?;
        let mut namespaces = Vec::new();
        for name in ["pid", "user", "time", "net"] {
            let current = std::fs::metadata(format!("/proc/self/ns/{name}"))?;
            let peer = File::from(rustix::fs::openat(
                &proc,
                format!("ns/{name}"),
                OFlags::RDONLY | OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )?)
            .metadata()?;
            if (current.dev(), current.ino()) != (peer.dev(), peer.ino()) {
                return Err(invalid("manager peer namespace differs"));
            }
            namespaces.push((name.to_owned(), (peer.dev(), peer.ino())));
        }
        let value = Self {
            process,
            proc,
            source,
            snapshot,
            _measured: measured,
            namespaces,
        };
        value.revalidate()?;
        Ok(value)
    }
    pub(super) fn revalidate(&self) -> Result<()> {
        self.process.validate_liveness()?;
        if object(&self.source)? != self.snapshot
            || object(&executable(&self.proc)?)? != self.snapshot
        {
            return Err(invalid("approved manager/coordinator executable changed"));
        }
        let status: File = rustix::fs::openat(
            &self.proc,
            "status",
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )?
        .into();
        let mut bytes = String::new();
        status.take(65537).read_to_string(&mut bytes)?;
        if bytes.len() > 65536 {
            return Err(invalid("oversized manager process status"));
        }
        for (name, count) in [("Uid:", 4), ("Gid:", 4), ("TracerPid:", 1)] {
            let mut values = bytes.lines().filter_map(|line| line.strip_prefix(name));
            let value = values
                .next()
                .ok_or_else(|| invalid("missing manager credential status"))?;
            if values.next().is_some()
                || !value
                    .split_ascii_whitespace()
                    .eq(std::iter::repeat_n("0", count))
            {
                return Err(invalid(
                    "manager/coordinator root credentials or tracer changed",
                ));
            }
        }
        for (name, expected) in &self.namespaces {
            let metadata = File::from(rustix::fs::openat(
                &self.proc,
                format!("ns/{name}"),
                OFlags::RDONLY | OFlags::CLOEXEC,
                rustix::fs::Mode::empty(),
            )?)
            .metadata()?;
            if (metadata.dev(), metadata.ino()) != *expected {
                return Err(invalid("approved manager/coordinator namespace changed"));
            }
        }
        self.process.validate_liveness()?;
        Ok(())
    }
}
fn executable(proc: &File) -> io::Result<File> {
    Ok(rustix::fs::openat(
        proc,
        "exe",
        OFlags::RDONLY | OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )?
    .into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deployment_is_canonical_and_binds_each_field() {
        let good =
            ProofManagerDeploymentV1::new(([1; 32], 4096), ([2; 32], 8192), [3; 32]).unwrap();
        assert_eq!(
            ProofManagerDeploymentV1::decode(good.canonical_bytes()).unwrap(),
            good
        );
        for length in [0, 159, 161] {
            assert!(ProofManagerDeploymentV1::decode(&vec![0; length]).is_err());
        }
        for index in 0..BYTES {
            let mut bytes = good.bytes;
            bytes[index] ^= 1;
            assert!(
                ProofManagerDeploymentV1::decode(&bytes).is_err(),
                "byte {index}"
            );
        }
        for (manager, coordinator, application) in [
            (([0; 32], 4096), ([2; 32], 8192), [3; 32]),
            (([1; 32], 0), ([2; 32], 8192), [3; 32]),
            (([1; 32], MAX_IMAGE + 1), ([2; 32], 8192), [3; 32]),
            (([1; 32], 4096), ([1; 32], 8192), [3; 32]),
            (([1; 32], 4096), ([2; 32], 8192), [0; 32]),
        ] {
            // new recomputes the checksum, so these exercise semantic rejection.
            assert!(ProofManagerDeploymentV1::new(manager, coordinator, application).is_err());
        }
    }
}
