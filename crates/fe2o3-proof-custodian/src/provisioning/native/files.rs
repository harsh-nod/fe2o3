use super::*;
use fe2o3_compiler_execution_protocol::CompilerExecutionClientProfileV3 as Profile;
use fe2o3_protected_service_profile::ProofControllerCredentialProfileV1 as Credentials;
use fe2o3_protected_static_executable::{
    ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOwnerV1 as Owner, ProtectedStaticExecutableV2 as Image,
    RootInstalledFileV1,
};
use rustix::fs::{Mode, OFlags};
use std::{fs::File, os::unix::fs::MetadataExt};

const MAX_IMAGE: u64 = 128 * 1024 * 1024;
const IO_QUOTE: usize = 64 * 1024;

#[derive(PartialEq, Eq)]
struct Snapshot {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
    gid: u32,
    links: u64,
    length: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}

fn snapshot(file: &File) -> io::Result<Snapshot> {
    let m = file.metadata()?;
    Ok(Snapshot {
        device: m.dev(),
        inode: m.ino(),
        mode: m.mode(),
        uid: m.uid(),
        gid: m.gid(),
        links: m.nlink(),
        length: m.len(),
        modified: (m.mtime(), m.mtime_nsec()),
        changed: (m.ctime(), m.ctime_nsec()),
    })
}

fn finite_read(file: &File, maximum: usize, budget: &mut Budget<'_>) -> io::Result<Vec<u8>> {
    budget.charge_work(IO_QUOTE).map_err(other)?;
    let before = snapshot(file)?;
    let size = usize::try_from(before.length).map_err(other)?;
    require(
        before.mode & libc::S_IFMT == libc::S_IFREG
            && before.links == 1
            && (1..=maximum).contains(&size),
        "native approval input file shape",
    )?;
    budget.charge_work(size).map_err(other)?;
    budget.reserve_storage(size).map_err(other)?;
    let mut bytes = vec![0; size];
    for (index, chunk) in bytes.chunks_mut(64 * 1024).enumerate() {
        let offset = (index * 64 * 1024) as u64;
        require(
            rustix::io::pread(file, &mut *chunk, offset)? == chunk.len(),
            "native approval single-attempt read was short",
        )?;
    }
    require(
        rustix::io::pread(file, &mut [0; 1], size as u64)? == 0 && snapshot(file)? == before,
        "native approval input changed",
    )?;
    Ok(bytes)
}

pub(super) fn read_pinned(
    path: &Path,
    pin: [u8; 32],
    maximum: usize,
    budget: &mut Budget<'_>,
) -> io::Result<Vec<u8>> {
    budget.charge_work(IO_QUOTE).map_err(other)?;
    require(
        pin != [0; 32],
        "native approval requires an independent nonzero pin",
    )?;
    let file = File::from(rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )?);
    let bytes = finite_read(&file, maximum, budget)?;
    require(
        <[u8; 32]>::from(Sha256::digest(&bytes)) == pin,
        "native approval differs from independent SHA-256",
    )?;
    Ok(bytes)
}

pub(super) fn write_candidate(
    path: &Path,
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> io::Result<()> {
    budget.charge_work(bytes.len() + IO_QUOTE).map_err(other)?;
    let file = File::from(rustix::fs::open(
        path,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR,
    )?);
    require(
        rustix::io::write(&file, bytes)? == bytes.len(),
        "native candidate write was short",
    )?;
    rustix::fs::fsync(&file)?;
    Ok(())
}

/// A separate export path: publication is one no-replace rename after a full
/// write and fsync. The existing administrator candidate writer is unchanged.
pub(super) fn write_candidate_atomic(
    path: &Path,
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> io::Result<()> {
    write_candidate_atomic_using(path, bytes, budget, || Ok(()))
}

struct ExportStaging<'a> {
    parent: &'a File,
    file: File,
    name: String,
    published: bool,
}

impl Drop for ExportStaging<'_> {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        let original = self.file.metadata();
        let current = rustix::fs::statat(
            self.parent,
            &self.name,
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        );
        if let (Ok(original), Ok(current)) = (original, current)
            && original.dev() == current.st_dev
            && original.ino() == current.st_ino
        {
            let _ = rustix::fs::unlinkat(self.parent, &self.name, rustix::fs::AtFlags::empty());
        }
    }
}

fn write_candidate_atomic_using(
    path: &Path,
    bytes: &[u8],
    budget: &mut Budget<'_>,
    before_publish: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    use std::os::unix::ffi::OsStrExt;
    require(
        path.as_os_str().as_bytes().len() <= 4096,
        "native export output path exceeds its bound",
    )?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("native export requires one file name"))?;
    budget
        .charge_work(
            bytes
                .len()
                .checked_add(8 * IO_QUOTE)
                .ok_or_else(|| io::Error::other("native export publication work overflow"))?,
        )
        .map_err(other)?;
    budget.reserve_storage(IO_QUOTE).map_err(other)?;
    let parent_path = path
        .parent()
        .filter(|value| !value.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let parent = File::from(rustix::fs::open(
        parent_path,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )?);
    let metadata = parent.metadata()?;
    require(
        metadata.is_dir()
            && metadata.uid() == rustix::process::geteuid().as_raw()
            && metadata.mode() & 0o022 == 0,
        "native policy export requires a caller-owned directory without group/other write access",
    )?;
    let mut nonce = [0; 16];
    require(
        rustix::rand::getrandom(&mut nonce, rustix::rand::GetRandomFlags::NONBLOCK)? == nonce.len()
            && nonce != [0; 16],
        "native export randomness unavailable",
    )?;
    let stage_name = format!(".fe2o3-native-policy-{:032x}", u128::from_le_bytes(nonce));
    let file = File::from(rustix::fs::openat(
        &parent,
        &stage_name,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::RUSR | Mode::WUSR,
    )?);
    let mut staging = ExportStaging {
        parent: &parent,
        file,
        name: stage_name,
        published: false,
    };
    require(
        rustix::io::write(&staging.file, bytes)? == bytes.len(),
        "native policy export single-attempt write was short",
    )?;
    rustix::fs::fchmod(&staging.file, Mode::RUSR)?;
    rustix::fs::fsync(&staging.file)?;
    before_publish()?;
    let original = staging.file.metadata()?;
    let current = rustix::fs::statat(
        &parent,
        &staging.name,
        rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
    )?;
    require(
        original.dev() == current.st_dev
            && original.ino() == current.st_ino
            && original.mode() == libc::S_IFREG | 0o400
            && original.nlink() == 1
            && original.len() == bytes.len() as u64,
        "native policy export staging changed",
    )?;
    rustix::fs::renameat_with(
        &parent,
        &staging.name,
        &parent,
        name,
        rustix::fs::RenameFlags::NOREPLACE,
    )?;
    staging.published = true;
    rustix::fs::fsync(&parent).map_err(|error| {
        io::Error::other(format!(
            "native policy candidate published but directory fsync failed: {error}"
        ))
    })?;
    Ok(())
}

pub(super) struct Installed {
    owner: RootInstalledFileV1,
    bytes: Vec<u8>,
}
impl Installed {
    fn open(path: &'static str, maximum: u64, budget: &mut Budget<'_>) -> io::Result<Self> {
        budget.charge_work(IO_QUOTE).map_err(other)?;
        budget.reserve_storage(IO_QUOTE).map_err(other)?;
        let length = std::fs::symlink_metadata(path)?.len();
        require(
            (1..=maximum).contains(&length),
            "native installed image extent",
        )?;
        let owner = RootInstalledFileV1::open(path, 0o555, length)?;
        let bytes = finite_read(owner.leaf(), maximum as usize, budget)?;
        owner.revalidate()?;
        Ok(Self { owner, bytes })
    }
    pub fn measurement(&self) -> ([u8; 32], u64) {
        (Sha256::digest(&self.bytes).into(), self.bytes.len() as u64)
    }
    fn seal(&self, budget: &mut Budget<'_>) -> io::Result<Image> {
        let (hash, size) = self.measurement();
        let measurement = Measurement::new(hash, size, MAX_IMAGE).map_err(other)?;
        budget
            .reserve_storage(Image::file_storage(measurement).map_err(other)?)
            .map_err(other)?;
        let (image, charge) = Image::seal_source_for_owner(
            self.owner.leaf().try_clone()?,
            measurement,
            Owner::new(0, 0).map_err(other)?,
            "native approval image",
            budget,
        )
        .map_err(other)?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        Ok(image)
    }
}

pub(super) struct Images {
    pub controller: Installed,
    pub manager: Installed,
    worker: Installed,
}
impl Images {
    pub fn open(budget: &mut Budget<'_>) -> io::Result<Self> {
        let controller = Installed::open(
            crate::deployment::NATIVE_APPLICATION_CONTROLLER_PATH,
            MAX_IMAGE,
            budget,
        )?;
        let manager = Installed::open(crate::deployment::NATIVE_MANAGER_PATH, MAX_IMAGE, budget)?;
        for image in [&controller, &manager] {
            budget.charge_work(image.bytes.len()).map_err(other)?;
            fe2o3_runtime_protocol::sealed_static_application_identity_v1(&image.bytes)
                .map_err(other)?;
        }
        let worker = Installed::open(
            crate::deployment::WORKER_PATH,
            fe2o3_kernel_analysis::MAX_PHYSICAL_MACHINE_EFFECT_WORKER_BYTES_V1,
            budget,
        )?;
        Ok(Self {
            controller,
            manager,
            worker,
        })
    }
    pub fn validate(
        &self,
        candidate: &Candidate,
        profile: &Profile,
        budget: &mut Budget<'_>,
    ) -> io::Result<()> {
        budget
            .charge_work(
                self.controller.bytes.len()
                    + self.manager.bytes.len()
                    + self.worker.bytes.len()
                    + IO_QUOTE,
            )
            .map_err(other)?;
        separate_credentials(candidate.application.credentials()?, profile)?;
        require(
            candidate.application.compiler_policy_identity()
                == *profile.policy().identity().as_bytes(),
            "native candidate compiler policy differs from actual V3 deployment",
        )?;
        let (hash, length) = self.controller.measurement();
        let (application, charge) = Config::new(
            candidate.application.credentials()?,
            hash,
            length,
            candidate.application.analyzer_policy()?,
            candidate.application.verus_identity(),
            *profile.policy().identity().as_bytes(),
            candidate.application.semantic_policy(),
            budget,
        )?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        let (manager, charge) = ManagerConfig::new(
            self.manager.measurement(),
            *profile.policy().identity().as_bytes(),
            application.identity(),
            application.semantic_policy(),
            budget,
        )?;
        budget
            .reserve_storage(charge.additional_storage())
            .map_err(other)?;
        require(
            application == candidate.application && manager == candidate.manager,
            "native static image differs from independently approved candidate",
        )?;
        require(
            fe2o3_kernel_analysis::PhysicalMachineWorkerExecutableIdentityV1::calculate(
                &self.worker.bytes,
            ) == application.analyzer_policy()?.executable(),
            "native analyzer executable differs from approval",
        )?;
        for image in [&self.controller, &self.manager, &self.worker] {
            image.owner.revalidate()?;
        }
        Ok(())
    }
    pub fn seal(&self, budget: &mut Budget<'_>) -> io::Result<[Image; 2]> {
        Ok([self.controller.seal(budget)?, self.manager.seal(budget)?])
    }
}

pub(super) fn separate_credentials(credentials: Credentials, profile: &Profile) -> io::Result<()> {
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
        "native proof credentials overlap compiler or anchor service",
    )
}

#[cfg(test)]
mod export_tests {
    use super::*;

    fn private_dir() -> tempfile::TempDir {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        dir
    }

    #[test]
    fn policy_export_is_invisible_until_full_write_and_refusal_removes_only_owned_stage() {
        let dir = private_dir();
        let path = dir.path().join("candidate");
        let mut account = Owned::new(Work::new(ACCOUNT_WORK), ACCOUNT_STORAGE);
        account.with_budget(|budget| {
            let result = write_candidate_atomic_using(&path, b"inert candidate", budget, || {
                assert!(!path.exists());
                assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
                Err(io::Error::other("injected before atomic publication"))
            });
            assert!(result.is_err());
        });
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        account.with_budget(|budget| {
            write_candidate_atomic_using(&path, b"inert candidate", budget, || {
                assert!(!path.exists());
                Ok(())
            })
            .unwrap();
        });
        assert_eq!(std::fs::read(&path).unwrap(), b"inert candidate");
        assert_eq!(path.metadata().unwrap().mode() & 0o7777, 0o400);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn policy_export_racing_destination_is_not_replaced_or_removed() {
        let dir = private_dir();
        let path = dir.path().join("candidate");
        Owned::new(Work::new(ACCOUNT_WORK), ACCOUNT_STORAGE).with_budget(|budget| {
            assert!(
                write_candidate_atomic_using(&path, b"new candidate", budget, || {
                    std::fs::write(&path, b"other writer")
                })
                .is_err()
            );
        });
        assert_eq!(std::fs::read(&path).unwrap(), b"other writer");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn policy_export_refuses_shared_writable_output_parents_before_staging() {
        use std::os::unix::fs::PermissionsExt;
        let dir = private_dir();
        let path = dir.path().join("candidate");
        for mode in [0o720, 0o702, 0o777] {
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(mode)).unwrap();
            Owned::new(Work::new(ACCOUNT_WORK), ACCOUNT_STORAGE).with_budget(|budget| {
                assert!(write_candidate_atomic(&path, b"inert candidate", budget).is_err());
            });
            assert!(!path.exists());
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        }
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
}
