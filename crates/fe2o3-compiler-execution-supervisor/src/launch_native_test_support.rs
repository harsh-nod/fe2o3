//! Policy-neutral descriptor witnesses for isolated native launch fixtures.
use crate::launch_checks as checks;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_static_preexec_manifest::StaticPreexecObjectIdentityV1 as Object;
use std::{
    collections::BTreeMap,
    error::Error as StdError,
    os::{
        fd::{AsFd, OwnedFd},
        unix::fs::MetadataExt,
    },
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct FdIdentity {
    pub(crate) object: (u64, u64, u32),
    pub(crate) flags: u32,
    pub(crate) pid: Option<i32>,
}

// Only the isolated single-test supervisor role may take a process-wide census.
pub(crate) fn fd_inventory() -> BTreeMap<i32, FdIdentity> {
    let descriptors: Vec<i32> = {
        let directory = std::fs::read_dir("/proc/self/fd").unwrap();
        directory
            .map(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .to_str()
                    .unwrap()
                    .parse()
                    .unwrap()
            })
            .collect()
    };
    let mut inventory = BTreeMap::new();
    let mut disappeared = 0;
    // Finish all metadata queries before fdinfo reads can reuse the collector FD.
    for fd in descriptors {
        match std::fs::metadata(format!("/proc/self/fd/{fd}")) {
            Ok(metadata) => {
                assert!(
                    inventory
                        .insert(
                            fd,
                            FdIdentity {
                                object: (metadata.dev(), metadata.ino(), metadata.mode()),
                                flags: 0,
                                pid: None,
                            }
                        )
                        .is_none()
                );
            }
            Err(error) if error.raw_os_error() == Some(libc::ENOENT) => disappeared += 1,
            Err(error) => panic!("cannot inventory fd {fd}: {error}"),
        }
    }
    assert_eq!(
        disappeared, 1,
        "only the closed ReadDir descriptor may disappear"
    );
    for (fd, identity) in &mut inventory {
        let info = std::fs::read_to_string(format!("/proc/self/fdinfo/{fd}")).unwrap();
        let flags = info
            .lines()
            .find_map(|line| line.strip_prefix("flags:"))
            .unwrap();
        identity.flags = u32::from_str_radix(flags.trim(), 8).unwrap();
        identity.pid = info
            .lines()
            .find_map(|line| line.strip_prefix("Pid:"))
            .map(|pid| pid.trim().parse().unwrap());
    }
    inventory
}

pub(crate) fn references(object: Object) -> usize {
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == (object.device(), object.inode()))
        .count()
}

// Keep the object alive while checking cleanup, excluding fd/inode reuse.
pub(crate) struct Witness {
    pub(crate) pin: OwnedFd,
    pub(crate) object: Object,
    remaining: usize,
}
impl Witness {
    pub(crate) fn new(file: &impl AsFd, owned_references: usize) -> Self {
        let pin = rustix::io::fcntl_dupfd_cloexec(file, 3).unwrap();
        let object = checks::object_identity(&pin, "cleanup witness").unwrap();
        Self {
            pin,
            object,
            remaining: references(object).checked_sub(owned_references).unwrap(),
        }
    }
    pub(crate) fn assert_released(&self) {
        assert_eq!(
            checks::object_identity(&self.pin, "cleanup witness").unwrap(),
            self.object
        );
        assert_eq!(
            references(self.object),
            self.remaining,
            "object {:?}",
            self.object
        );
    }
}

pub(crate) fn pidfd_references(pid: u32) -> usize {
    std::fs::read_dir("/proc/self/fdinfo")
        .unwrap()
        .filter_map(|entry| std::fs::read_to_string(entry.ok()?.path()).ok())
        .filter(|record| {
            record.lines().any(|line| {
                line.strip_prefix("Pid:\t")
                    .and_then(|value| value.parse::<u32>().ok())
                    == Some(pid)
            })
        })
        .count()
}

pub(crate) fn resource(mut error: &(dyn StdError + 'static)) -> Resource {
    loop {
        if let Some(resource) = error.downcast_ref::<Resource>() {
            return *resource;
        }
        error = error.source().expect("expected a typed resource refusal");
    }
}
