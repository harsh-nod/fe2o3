use super::*;
use crate::tests::Fixture;
use std::os::unix::fs::MetadataExt as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn assert_image(file: &File, owner: ProtectedStaticExecutableOwnerV1, bytes: u64) {
    let metadata = file.metadata().unwrap();
    assert_eq!(metadata.uid(), owner.uid());
    assert_eq!(metadata.gid(), owner.gid());
    assert_eq!(metadata.mode() & 0o7777, 0o555);
    assert_eq!(metadata.len(), bytes);
    assert_eq!(metadata.nlink(), 0);
    assert_eq!(
        rustix::fs::fcntl_getfl(file).unwrap() & OFlags::ACCMODE,
        OFlags::RDONLY
    );
    assert!(
        rustix::io::fcntl_getfd(file)
            .unwrap()
            .contains(rustix::io::FdFlags::CLOEXEC)
    );
    let seals = rustix::fs::fcntl_get_seals(file).unwrap();
    assert!(
        seals == REQUIRED_EXECUTABLE_SEALS_V1
            || seals == REQUIRED_EXECUTABLE_SEALS_V1 | SealFlags::FUTURE_WRITE
    );
}

fn both_public_families(owner: ProtectedStaticExecutableOwnerV1) {
    let fixture = Fixture::new();
    let measurement = fixture.measurement();
    let image = ProtectedStaticExecutableV1::seal_source_for_owner(
        fixture.open(),
        measurement,
        owner,
        "owner transition regression",
    )
    .unwrap();
    image.revalidate().unwrap();
    let clone = image.try_clone_for_exec().unwrap();
    assert_image(&clone, owner, measurement.byte_len());
    image.revalidate_exec_clone(&clone).unwrap();
    let admitted = ProtectedStaticExecutableV1::admit_sealed(
        clone,
        measurement,
        owner,
        "owner transition regression",
    )
    .unwrap();
    admitted.revalidate().unwrap();

    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    type Image = crate::native::ProtectedStaticExecutableV2;
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let floor = Image::file_storage(measurement).unwrap();
    budget.reserve_storage(floor).unwrap();
    let (image, delta) = Image::seal_source_for_owner(
        fixture.open(),
        measurement,
        owner,
        "bounded owner transition regression",
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(delta.additional_storage()).unwrap();
    image.revalidate(&mut budget).unwrap();
    let (clone, charge) = image.try_clone_for_exec(&mut budget).unwrap();
    budget.reserve_storage(charge.additional_storage()).unwrap();
    assert_image(&clone, owner, measurement.byte_len());
    image.revalidate_exec_clone(&clone, &mut budget).unwrap();
}

#[test]
fn canonical_creator_mode_and_both_public_families_remain_exact() {
    both_public_families(ProtectedStaticExecutableOwnerV1::current());
}

#[test]
#[ignore = "requires explicit root opt-in; child retains only the five production coordinator capabilities"]
fn root_coordinator_transfers_sealed_images_without_fowner() {
    assert_eq!(
        std::env::var("FE2O3_RUN_ROOT_SEALED_OWNER_V90").as_deref(),
        Ok("1")
    );
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    if std::env::var_os("FE2O3_ROOT_SEALED_OWNER_CHILD_V90").is_some() {
        root_child();
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "mode_owner_tests::root_coordinator_transfers_sealed_images_without_fowner",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("FE2O3_RUN_ROOT_SEALED_OWNER_V90", "1")
        .env("FE2O3_ROOT_SEALED_OWNER_CHILD_V90", "1")
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            child.wait().unwrap();
            panic!("root sealed-owner child exceeded its deadline");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn root_child() {
    use rustix::thread::{CapabilitySet, CapabilitySets};
    let exact = CapabilitySet::CHOWN
        | CapabilitySet::KILL
        | CapabilitySet::SETGID
        | CapabilitySet::SETPCAP
        | CapabilitySet::SETUID;
    rustix::thread::set_capabilities(
        None,
        CapabilitySets {
            effective: exact,
            permitted: exact,
            inheritable: CapabilitySet::empty(),
        },
    )
    .unwrap();
    let caps = rustix::thread::capabilities(None).unwrap();
    assert_eq!(caps.effective, exact);
    assert_eq!(caps.permitted, exact);
    assert!(caps.inheritable.is_empty());
    assert_eq!(rustix::process::geteuid().as_raw(), 0);
    let owner = ProtectedStaticExecutableOwnerV1::new(65534, 65534).unwrap();
    assert_ne!(owner, ProtectedStaticExecutableOwnerV1::current());

    // Reproduce the old ordering under the real capability restriction.
    let old = rustix::fs::memfd_create(
        c"fe2o3-old-owner-order-regression",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING | MemfdFlags::EXEC,
    )
    .unwrap();
    rustix::fs::fchown(
        &old,
        Some(Uid::from_raw(owner.uid())),
        Some(Gid::from_raw(owner.gid())),
    )
    .unwrap();
    assert_eq!(
        rustix::fs::fchmod(&old, Mode::from_bits_truncate(0o555)).unwrap_err(),
        rustix::io::Errno::PERM
    );
    drop(old);
    both_public_families(owner);
    let after = rustix::thread::capabilities(None).unwrap();
    assert_eq!(after.effective, exact);
    assert_eq!(after.permitted, exact);
    assert!(after.inheritable.is_empty());
}
