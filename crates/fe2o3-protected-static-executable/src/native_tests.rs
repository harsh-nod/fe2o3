use crate::{
    ProtectedStaticExecutableMeasurementV1 as Measurement,
    ProtectedStaticExecutableOwnerV1 as Owner, native::*, tests::Fixture,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::{fs::File, os::unix::fs::MetadataExt};

type Image = ProtectedStaticExecutableV2;
type Error = ProtectedStaticExecutableErrorV2;
type Op = ProtectedStaticExecutableOperationV2;
fn admit(fixture: &Fixture) -> Image {
    let mut w = Work::new(1_000_000_000);
    let mut b = Budget::new(&mut w, 10_000_000);
    b.reserve_storage(Image::file_storage(fixture.measurement()).unwrap())
        .unwrap();
    Image::seal_source_for_owner(
        fixture.open(),
        fixture.measurement(),
        Owner::current(),
        "test executable",
        &mut b,
    )
    .unwrap()
    .0
}

#[test]
fn native_image_admission_exact_work_storage_and_floor_boundaries() {
    let f = Fixture::new();
    let quota = Image::quota(f.measurement(), Op::Admit).unwrap();
    let file = Image::file_storage(f.measurement()).unwrap();
    for mode in 0..4 {
        let mut w = Work::new(quota.work() - usize::from(mode == 0));
        let mut b = Budget::new(&mut w, file + quota.scratch() - usize::from(mode == 1));
        let floor = file - usize::from(mode == 2);
        b.reserve_storage(floor).unwrap();
        let result = Image::seal_source_for_owner(
            f.open(),
            f.measurement(),
            Owner::current(),
            "test executable",
            &mut b,
        );
        assert_eq!(b.storage(), floor);
        match mode {
            0 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert_eq!(b.work(), 8);
            }
            1 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                assert_eq!(b.work(), quota.work());
            }
            2 => {
                assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                assert_eq!(b.work(), 8);
            }
            _ => {
                let (image, delta) = result.unwrap();
                assert_eq!(file + delta.additional_storage(), image.retained_storage());
                assert_eq!(b.peak_storage(), file + quota.scratch());
                assert_eq!(b.work(), quota.work());
            }
        }
    }
}

#[test]
fn fresh_sealed_transfer_and_revalidation_share_the_ledger_and_retain_offsets() {
    let f = Fixture::new();
    let m = f.measurement();
    let mut w = Work::new(1_000_000_000);
    let mut b = Budget::new(&mut w, 10_000_000);
    let source = Image::file_storage(m).unwrap();
    b.reserve_storage(source).unwrap();
    let (image, delta) =
        Image::seal_source_for_owner(f.open(), m, Owner::current(), "test executable", &mut b)
            .unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    let retained = image.retained_storage();
    let identity = image.static_identity();
    let (file, charge) = image.try_clone_for_exec(&mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    assert_eq!(
        file.metadata().unwrap().ino(),
        image.object_identity().inode()
    );
    rustix::fs::seek(&file, rustix::fs::SeekFrom::Start(77)).unwrap();
    image.revalidate_exec_clone(&file, &mut b).unwrap();
    image.revalidate(&mut b).unwrap();
    assert_eq!(
        rustix::fs::seek(&file, rustix::fs::SeekFrom::Current(0)).unwrap(),
        77
    );
    let other = admit(&f);
    let mut other_w = Work::new(1_000_000_000);
    let mut other_b = Budget::new(&mut other_w, 10_000_000);
    other_b.reserve_storage(other.retained_storage()).unwrap();
    let (different, _) = other.try_clone_for_exec(&mut other_b).unwrap();
    b.reserve_storage(source).unwrap();
    assert!(image.revalidate_exec_clone(&different, &mut b).is_err());
    drop(different);
    b.release_storage(source).unwrap();
    drop(image);
    b.release_storage(retained).unwrap();
    let (recovered, delta) =
        Image::admit_sealed(file, m, Owner::current(), "test executable", &mut b).unwrap();
    b.reserve_storage(delta.additional_storage()).unwrap();
    assert_eq!(recovered.static_identity(), identity);
    assert_eq!(b.storage(), recovered.retained_storage());
    drop(recovered);
    b.release_storage(retained).unwrap();
    assert_eq!(b.storage(), 0);
}

#[test]
fn borrowed_owner_and_transfer_floors_are_not_interchangeable() {
    let f = Fixture::new();
    let image = admit(&f);
    let m = f.measurement();
    for op in [Op::Revalidate, Op::Transfer] {
        let q = Image::quota(m, op).unwrap();
        let floor = image.retained_storage();
        for mode in 0..4 {
            let mut w = Work::new(q.work() - usize::from(mode == 0));
            let mut b = Budget::new(&mut w, floor + q.scratch() - usize::from(mode == 1));
            b.reserve_storage(floor - usize::from(mode == 2)).unwrap();
            let result = match op {
                Op::Revalidate => image.revalidate(&mut b),
                _ => image.try_clone_for_exec(&mut b).map(|_| ()),
            };
            assert_eq!(b.storage(), floor - usize::from(mode == 2));
            match mode {
                0 => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
                1 => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
                2 => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
                _ => {
                    result.unwrap();
                    assert_eq!(b.work(), q.work());
                    assert_eq!(b.peak_storage(), floor + q.scratch());
                }
            }
        }
    }
}

#[test]
fn changed_sealed_metadata_and_invalid_source_refuse_without_descriptor_leaks() {
    let f = Fixture::new();
    let image = admit(&f);
    let mut w = Work::new(1_000_000_000);
    let mut b = Budget::new(&mut w, 10_000_000);
    b.reserve_storage(image.retained_storage()).unwrap();
    let (file, charge) = image.try_clone_for_exec(&mut b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
    assert!(image.revalidate_exec_clone(&file, &mut b).is_err());
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::CLOEXEC).unwrap();
    let source = f.open();
    let witness = rustix::io::fcntl_dupfd_cloexec(&source, 0).unwrap();
    let stat = rustix::fs::fstat(&witness).unwrap();
    let references = || {
        std::fs::read_dir("/proc/self/fd")
            .unwrap()
            .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
            .filter(|m| m.dev() == stat.st_dev && m.ino() == stat.st_ino)
            .count()
    };
    assert_eq!(references(), 2);
    rustix::io::fcntl_setfd(&source, rustix::io::FdFlags::empty()).unwrap();
    b.reserve_storage(Image::file_storage(f.measurement()).unwrap())
        .unwrap();
    assert!(
        Image::seal_source_for_owner(
            source,
            f.measurement(),
            Owner::current(),
            "invalid flags",
            &mut b
        )
        .is_err()
    );
    assert!(rustix::fs::fstat(&witness).is_ok());
    assert_eq!(references(), 1);
}

#[test]
fn checked_quota_overflow_is_refused_before_any_image_io() {
    for length in [usize::MAX, usize::MAX / 1024] {
        let m = Measurement::new([1; 32], length as u64, u64::MAX).unwrap();
        if length < usize::MAX {
            assert!(Image::file_storage(m).is_ok());
        }
        assert!(matches!(
            Image::quota(m, Op::Admit),
            Err(Error::Resource(Resource::Arithmetic))
        ));
        let mut w = Work::new(8);
        let mut b = Budget::new(&mut w, 0);
        assert!(matches!(
            Image::seal_source_for_owner(
                File::open("/dev/null").unwrap(),
                m,
                Owner::current(),
                "overflow",
                &mut b
            ),
            Err(Error::Resource(Resource::Arithmetic))
        ));
        assert_eq!(b.work(), 8);
        assert_eq!(b.storage(), 0);
    }
}

#[test]
fn image_quotas_match_independent_fixture_visit_counts() {
    let m = Measurement::new([1; 32], 4097, 4097).unwrap();
    assert_eq!(Image::quota(m, Op::Admit).unwrap().work(), 34_824_473);
    assert_eq!(Image::quota(m, Op::Revalidate).unwrap().work(), 17_479_825);
    assert_eq!(Image::quota(m, Op::Transfer).unwrap().work(), 26_152_149);
}
