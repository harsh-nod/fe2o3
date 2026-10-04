//! Offline approval installation. Neither candidate bytes nor installation grant proof authority.

use crate::{
    ProofCustodianDeploymentV1,
    deployment::{APPLICATION_CONTROLLER_PATH, DEPLOYMENT_BYTES, RUNTIME_PATH, WORKER_PATH},
    other, require,
};
use fe2o3_broker_authority_service::ProofManagerDeploymentV1;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1, COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1,
    CompilerExecutionClientProfileV1,
};
use fe2o3_kernel_analysis::{
    AuthenticatedPhysicalMachineEffectLimitsV1, MAX_PHYSICAL_MACHINE_EFFECT_WORKER_BYTES_V1,
    PhysicalMachineWorkerExecutableIdentityV1, inspect_physical_machine_effect_worker_candidate_v1,
};
use fe2o3_protected_service_profile::ProofControllerCredentialProfileV1;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableMeasurementV1, ProtectedStaticExecutableOwnerV1,
    ProtectedStaticExecutableV1, RootInstalledFileV1,
};
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags, ResolveFlags};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fmt::Write as _,
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::unix::fs::{FileExt, MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};

const MANAGER_PATH: &str = "/usr/libexec/fe2o3/fe2o3-proof-manager";
const COORDINATOR_PATH: &str = "/usr/libexec/fe2o3/fe2o3-compiler-execution-coordinator";
const CONFIG_NAME: &str = "proof-custodian";
const STAGING_PREFIX: &str = ".proof-custodian-";
const RECORD_NAMES: [&str; 2] = ["application-deployment-v1", "manager-deployment-v1"];
const MAX_IMAGE: u64 = 128 * 1024 * 1024;
const CANDIDATE_BYTES: usize = DEPLOYMENT_BYTES + 160;
const USAGE: &str = "usage: fe2o3-proof-custodian-provision inspect-fixed-resources OUTPUT | install CANDIDATE EXPECTED_SHA256";

enum Command {
    Inspect(PathBuf),
    Install(PathBuf, [u8; 32]),
}

fn parse(args: &[OsString]) -> io::Result<Command> {
    match args {
        [command, path] if command == "inspect-fixed-resources" => {
            Ok(Command::Inspect(path.into()))
        }
        [command, path, pin] if command == "install" => {
            let pin = pin.to_str().ok_or_else(|| io::Error::other(USAGE))?;
            require(pin.len() == 64, USAGE)?;
            let mut hash = [0; 32];
            for (out, pair) in hash.iter_mut().zip(pin.as_bytes().chunks_exact(2)) {
                let nibble = |byte| match byte {
                    b'0'..=b'9' => Ok(byte - b'0'),
                    b'a'..=b'f' => Ok(byte - b'a' + 10),
                    _ => Err(io::Error::other(USAGE)),
                };
                *out = (nibble(pair[0])? << 4) | nibble(pair[1])?;
            }
            Ok(Command::Install(path.into(), hash))
        }
        _ => Err(io::Error::other(USAGE)),
    }
}

/// Inspects fixed installed resources as the intended proof UID/GID, or installs a
/// separately reviewed candidate as root. Root never executes the candidate or
/// reruns the unprivileged analyzer/Verus inspection. The fixed controller must
/// still validate those approved resource identities before ResourcesReady.
pub fn run_proof_custodian_provisioner_v1() -> io::Result<()> {
    let command = parse(&std::env::args_os().skip(1).collect::<Vec<_>>())?;
    match command {
        Command::Inspect(path) => {
            let credentials = current_inspection_credentials()?;
            let profile =
                fe2o3_protected_service_profile::ProofControllerProcessProfileV1::capture(
                    credentials,
                )
                .map_err(other)?;
            let candidate = inspect(credentials)?;
            profile.revalidate_current().map_err(other)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o400)
                .open(path)?;
            output.write_all(&candidate.bytes())?;
            output.sync_all()?;
            println!("candidate_sha256={}", candidate.sha256_hex());
        }
        Command::Install(path, pin) => {
            fe2o3_protected_service_spawn::require_exact_root_identity_v1().map_err(other)?;
            let candidate = read_candidate(&path, pin)?;
            let resources = Resources::open()?;
            resources.validate_candidate(&candidate)?;
            // Sealing checks the same production static-image contract before publication.
            let mut sealed = Vec::with_capacity(3);
            for image in [
                &resources.controller,
                &resources.manager,
                &resources.coordinator,
            ] {
                let (hash, len) = image.measurement();
                sealed.push(
                    ProtectedStaticExecutableV1::seal_source_for_owner(
                        image.file.leaf().try_clone()?,
                        ProtectedStaticExecutableMeasurementV1::new(hash, len, MAX_IMAGE)
                            .map_err(other)?,
                        ProtectedStaticExecutableOwnerV1::new(0, 0).map_err(other)?,
                        "installed proof deployment image",
                    )
                    .map_err(other)?,
                );
            }
            resources.revalidate()?;
            let parent = open_config_parent()?;
            publish(&parent, &candidate, 0, 0, || {
                revalidate_config_parent(&parent)?;
                resources.revalidate()?;
                for image in &sealed {
                    image.revalidate().map_err(other)?;
                }
                Ok(())
            })?;
            println!("installed_candidate_sha256={}", candidate.sha256_hex());
        }
    }
    Ok(())
}

fn current_inspection_credentials() -> io::Result<ProofControllerCredentialProfileV1> {
    let uid = rustix::process::getuid();
    let gid = rustix::process::getgid();
    require(
        uid == rustix::process::geteuid() && gid == rustix::process::getegid(),
        "inspection requires stable non-root credentials",
    )?;
    ProofControllerCredentialProfileV1::new(uid.as_raw(), gid.as_raw()).map_err(other)
}

struct Candidate {
    application: ProofCustodianDeploymentV1,
    manager: ProofManagerDeploymentV1,
}

impl Candidate {
    fn sha256_hex(&self) -> String {
        let mut hex = String::with_capacity(64);
        for byte in Sha256::digest(self.bytes()) {
            write!(&mut hex, "{byte:02x}").expect("String formatting is infallible");
        }
        hex
    }

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        require(
            bytes.len() == CANDIDATE_BYTES,
            "wrong proof deployment candidate length",
        )?;
        let value = Self {
            application: ProofCustodianDeploymentV1::decode(&bytes[..DEPLOYMENT_BYTES])?,
            manager: ProofManagerDeploymentV1::decode(&bytes[DEPLOYMENT_BYTES..]).map_err(other)?,
        };
        require(
            value.manager.application_deployment() == value.application.identity(),
            "manager approval names a different application deployment",
        )?;
        Ok(value)
    }

    fn bytes(&self) -> [u8; CANDIDATE_BYTES] {
        let mut bytes = [0; CANDIDATE_BYTES];
        bytes[..DEPLOYMENT_BYTES].copy_from_slice(self.application.canonical_bytes());
        bytes[DEPLOYMENT_BYTES..].copy_from_slice(self.manager.canonical_bytes());
        bytes
    }

    fn records(&self) -> [&[u8]; 2] {
        [
            self.application.canonical_bytes(),
            self.manager.canonical_bytes(),
        ]
    }
}

fn read_candidate(path: &Path, pin: [u8; 32]) -> io::Result<Candidate> {
    let mut file = File::from(rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )?);
    let metadata = file.metadata()?;
    require(
        metadata.is_file() && metadata.nlink() == 1 && metadata.len() == CANDIDATE_BYTES as u64,
        "candidate must be one bounded regular file",
    )?;
    let mut bytes = Vec::with_capacity(CANDIDATE_BYTES + 1);
    (&mut file)
        .take((CANDIDATE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    require(
        <[u8; 32]>::from(Sha256::digest(&bytes)) == pin,
        "candidate differs from approved SHA-256",
    )?;
    Candidate::decode(&bytes)
}

struct InstalledBytes {
    file: RootInstalledFileV1,
    bytes: Vec<u8>,
}

impl InstalledBytes {
    fn open(path: &'static str, mode: u32, maximum: u64) -> io::Result<Self> {
        let len = std::fs::symlink_metadata(path)?.len();
        require(
            len != 0 && len <= maximum,
            "installed resource length is invalid",
        )?;
        let file = RootInstalledFileV1::open(path, mode, len)?;
        let mut bytes = vec![0; len as usize];
        file.leaf().read_exact_at(&mut bytes, 0)?;
        file.revalidate()?;
        Ok(Self { file, bytes })
    }

    fn static_image(path: &'static str) -> io::Result<Self> {
        let value = Self::open(path, 0o555, MAX_IMAGE)?;
        fe2o3_runtime_protocol::sealed_static_application_identity_v1(&value.bytes)
            .map_err(other)?;
        Ok(value)
    }

    fn measurement(&self) -> ([u8; 32], u64) {
        (Sha256::digest(&self.bytes).into(), self.bytes.len() as u64)
    }
}

struct Resources {
    controller: InstalledBytes,
    manager: InstalledBytes,
    coordinator: InstalledBytes,
    worker: InstalledBytes,
    profile_file: InstalledBytes,
    profile: CompilerExecutionClientProfileV1,
}

impl Resources {
    fn open() -> io::Result<Self> {
        let profile_file = InstalledBytes::open(
            COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V1,
            0o444,
            COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1 as u64,
        )?;
        let profile =
            CompilerExecutionClientProfileV1::decode(&profile_file.bytes).map_err(other)?;
        Ok(Self {
            controller: InstalledBytes::static_image(APPLICATION_CONTROLLER_PATH)?,
            manager: InstalledBytes::static_image(MANAGER_PATH)?,
            coordinator: InstalledBytes::static_image(COORDINATOR_PATH)?,
            worker: InstalledBytes::open(
                WORKER_PATH,
                0o555,
                MAX_PHYSICAL_MACHINE_EFFECT_WORKER_BYTES_V1,
            )?,
            profile_file,
            profile,
        })
    }

    fn revalidate(&self) -> io::Result<()> {
        for image in [
            &self.controller,
            &self.manager,
            &self.coordinator,
            &self.worker,
            &self.profile_file,
        ] {
            image.file.revalidate()?;
        }
        Ok(())
    }

    fn validate_candidate(&self, candidate: &Candidate) -> io::Result<()> {
        separate_credentials(candidate.application.credentials()?, &self.profile)?;
        let (hash, len) = self.controller.measurement();
        let application = ProofCustodianDeploymentV1::new(
            candidate.application.credentials()?,
            hash,
            len,
            candidate.application.analyzer_policy()?,
            candidate.application.verus_identity(),
        )?;
        let manager = ProofManagerDeploymentV1::new(
            self.manager.measurement(),
            self.coordinator.measurement(),
            application.identity(),
        )
        .map_err(other)?;
        require(
            application == candidate.application && manager == candidate.manager,
            "installed static images differ from approved candidate",
        )?;
        require(
            PhysicalMachineWorkerExecutableIdentityV1::calculate(&self.worker.bytes)
                == candidate.application.analyzer_policy()?.executable(),
            "installed analyzer image differs from approved candidate",
        )?;
        self.revalidate()
    }
}

fn separate_credentials(
    credentials: ProofControllerCredentialProfileV1,
    profile: &CompilerExecutionClientProfileV1,
) -> io::Result<()> {
    require(
        ![
            profile.supervisor_uid(),
            profile.external_anchor_service().uid(),
        ]
        .contains(&credentials.uid())
            && ![
                profile.supervisor_gid(),
                profile.external_anchor_service().gid(),
            ]
            .contains(&credentials.gid()),
        "proof credentials overlap compiler or anchor service",
    )
}

fn inspect(credentials: ProofControllerCredentialProfileV1) -> io::Result<Candidate> {
    let resources = Resources::open()?;
    separate_credentials(credentials, &resources.profile)?;
    let limits = AuthenticatedPhysicalMachineEffectLimitsV1::new(
        Duration::from_secs(60),
        1024 * 1024,
        16384,
    )
    .map_err(other)?;
    let policy = inspect_physical_machine_effect_worker_candidate_v1(WORKER_PATH, limits)
        .map_err(other)?
        .policy();
    let runtime = fe2o3_verifier::FunctionalRefinementVerusRuntimeLeaseV1::open(RUNTIME_PATH)
        .map_err(other)?;
    let (hash, len) = resources.controller.measurement();
    let application = ProofCustodianDeploymentV1::new(
        credentials,
        hash,
        len,
        policy,
        runtime.identity().as_bytes(),
    )?;
    let manager = ProofManagerDeploymentV1::new(
        resources.manager.measurement(),
        resources.coordinator.measurement(),
        application.identity(),
    )
    .map_err(other)?;
    let candidate = Candidate {
        application,
        manager,
    };
    resources.validate_candidate(&candidate)?;
    runtime.revalidate().map_err(other)?;
    Ok(candidate)
}

fn open_config_parent() -> io::Result<File> {
    let root = rustix::fs::open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let mut parent = File::from(root);
    for component in ["etc", "fe2o3"] {
        let next = File::from(rustix::fs::openat2(
            &parent,
            component,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        )?);
        let m = next.metadata()?;
        require(
            m.uid() == 0 && m.gid() == 0 && m.mode() & 0o022 == 0,
            "proof configuration parent is not root protected",
        )?;
        no_attributes(&next)?;
        parent = next;
    }
    Ok(parent)
}

fn revalidate_config_parent(parent: &File) -> io::Result<()> {
    require(
        same_object(parent, &open_config_parent()?)?,
        "proof configuration parent changed",
    )
}

fn same_object(left: &File, right: &File) -> io::Result<bool> {
    let key = |m: std::fs::Metadata| (m.dev(), m.ino(), m.mode(), m.uid(), m.gid());
    Ok(key(left.metadata()?) == key(right.metadata()?))
}

fn no_attributes(file: &File) -> io::Result<()> {
    require(
        rustix::fs::flistxattr(file, &mut [0u8; 1])? == 0,
        "proof deployment object has extended attributes",
    )
}

fn open_directory(parent: &File, name: &str) -> io::Result<File> {
    Ok(File::from(rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )?))
}

fn validate_records(directory: &File, candidate: &Candidate, uid: u32, gid: u32) -> io::Result<()> {
    let m = directory.metadata()?;
    require(
        m.uid() == uid && m.gid() == gid && m.mode() == libc::S_IFDIR | 0o755,
        "proof configuration directory metadata differs",
    )?;
    no_attributes(directory)?;
    let mut names = rustix::fs::Dir::read_from(directory)?
        .take(5)
        .map(|entry| entry.map(|entry| entry.file_name().to_bytes().to_vec()))
        .collect::<Result<Vec<_>, _>>()?;
    names.retain(|name| name != b"." && name != b"..");
    names.sort_unstable();
    require(
        names == RECORD_NAMES.map(|name| name.as_bytes().to_vec()),
        "unexpected proof configuration inventory",
    )?;
    for (name, bytes) in RECORD_NAMES.into_iter().zip(candidate.records()) {
        let file = File::from(rustix::fs::openat(
            directory,
            name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )?);
        let m = file.metadata()?;
        require(
            m.uid() == uid
                && m.gid() == gid
                && m.mode() == libc::S_IFREG | 0o444
                && m.nlink() == 1
                && m.len() == bytes.len() as u64,
            "proof record metadata differs",
        )?;
        no_attributes(&file)?;
        let mut actual = vec![0; bytes.len()];
        file.read_exact_at(&mut actual, 0)?;
        require(
            actual == bytes,
            "existing proof approval differs; refusing replacement",
        )?;
    }
    Ok(())
}

struct Staging<'a> {
    parent: &'a File,
    directory: File,
    name: String,
    published: bool,
}
impl Drop for Staging<'_> {
    fn drop(&mut self) {
        if !self.published {
            for name in RECORD_NAMES {
                let _ = rustix::fs::unlinkat(&self.directory, name, AtFlags::empty());
            }
            if open_directory(self.parent, &self.name)
                .and_then(|d| same_object(&d, &self.directory))
                .unwrap_or(false)
            {
                let _ = rustix::fs::unlinkat(self.parent, &self.name, AtFlags::REMOVEDIR);
            }
        }
    }
}

fn publish(
    parent: &File,
    candidate: &Candidate,
    uid: u32,
    gid: u32,
    mut revalidate: impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
    revalidate()?;
    for (index, entry) in rustix::fs::Dir::read_from(parent)?.enumerate() {
        require(
            index < 4096,
            "proof configuration parent inventory exceeds bound",
        )?;
        require(
            !entry?
                .file_name()
                .to_bytes()
                .starts_with(STAGING_PREFIX.as_bytes()),
            "interrupted proof staging requires administrator inspection; refusing automatic cleanup",
        )?;
    }
    match open_directory(parent, CONFIG_NAME) {
        Ok(existing) => {
            validate_records(&existing, candidate, uid, gid)?;
            existing.sync_all()?;
            parent.sync_all()?;
            revalidate()?;
            require(
                same_object(&existing, &open_directory(parent, CONFIG_NAME)?)?,
                "existing proof configuration path changed",
            )?;
            return validate_records(&existing, candidate, uid, gid);
        }
        Err(error) if error.raw_os_error() == Some(libc::ENOENT) => (),
        Err(error) => return Err(error),
    }
    let mut nonce = [0; 16];
    let mut filled = 0;
    while filled < nonce.len() {
        match rustix::rand::getrandom(&mut nonce[filled..], rustix::rand::GetRandomFlags::empty()) {
            Ok(0) => return Err(io::Error::other("empty proof provisioning randomness")),
            Ok(count) => filled += count,
            Err(rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        }
    }
    let name = format!("{STAGING_PREFIX}{:032x}", u128::from_le_bytes(nonce));
    rustix::fs::mkdirat(parent, &name, Mode::RWXU)?;
    let directory = match open_directory(parent, &name) {
        Ok(directory) => directory,
        Err(error) => {
            return match rustix::fs::unlinkat(parent, &name, AtFlags::REMOVEDIR) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(io::Error::other(format!(
                    "cannot open proof staging: {error}; cannot remove its new empty directory: {cleanup}"
                ))),
            };
        }
    };
    let mut staging = Staging {
        parent,
        directory,
        name,
        published: false,
    };
    for (name, bytes) in RECORD_NAMES.into_iter().zip(candidate.records()) {
        let mut file = File::from(rustix::fs::openat(
            &staging.directory,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )?);
        file.write_all(bytes)?;
        rustix::fs::fchmod(&file, Mode::from_raw_mode(0o444))?;
        file.sync_all()?;
        revalidate()?;
    }
    rustix::fs::fchmod(&staging.directory, Mode::from_raw_mode(0o755))?;
    validate_records(&staging.directory, candidate, uid, gid)?;
    staging.directory.sync_all()?;
    revalidate()?;
    require(
        same_object(&staging.directory, &open_directory(parent, &staging.name)?)?,
        "staging proof configuration path changed",
    )?;
    rustix::fs::renameat_with(
        parent,
        &staging.name,
        parent,
        CONFIG_NAME,
        RenameFlags::NOREPLACE,
    )?;
    staging.published = true;
    let mut finish = || {
        parent.sync_all()?;
        revalidate()?;
        let published = open_directory(parent, CONFIG_NAME)?;
        require(
            same_object(&staging.directory, &published)?,
            "published proof directory changed",
        )?;
        validate_records(&published, candidate, uid, gid)
    };
    finish().map_err(|error| {
        io::Error::other(format!(
            "proof approvals may already be installed; post-publication validation failed: {error}"
        ))
    })
}

#[cfg(test)]
mod tests;
