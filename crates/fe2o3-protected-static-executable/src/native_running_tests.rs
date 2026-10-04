use super::*;
use crate::tests::Fixture;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[test]
fn running_admission_prepays_before_open_and_returns_full_storage() {
    let f = Fixture::new();
    let m = f.measurement();
    let source = Image::seal_source_for_owner(f.open(), m, Owner::current(), "fixture").unwrap();
    let quota =
        ProtectedStaticExecutableV2::quota(m, ProtectedStaticExecutableOperationV2::Admit).unwrap();
    for mode in 0..4 {
        let mut w = Work::new(if mode == 0 {
            7
        } else {
            quota.work() - usize::from(mode == 1)
        });
        let mut b = Budget::new(&mut w, 17 + quota.scratch() - usize::from(mode == 2));
        b.reserve_storage(17).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let opened = Cell::new(false);
        let result = ProtectedStaticExecutableV2::admit_running_with(
            m,
            Owner::current(),
            "fixture",
            &mut b,
            || {
                opened.set(true);
                source.try_clone_for_exec()
            },
        );
        assert_eq!(b.storage(), 17);
        assert!(ledger == b.work_ledger_identity_v1());
        assert_eq!(opened.get(), mode == 3);
        match mode {
            0 | 1 => assert!(matches!(
                result,
                Err(ProtectedStaticExecutableErrorV2::Resource(Resource::Work(
                    _
                )))
            )),
            2 => assert!(matches!(
                result,
                Err(ProtectedStaticExecutableErrorV2::Resource(
                    Resource::Storage(_)
                ))
            )),
            _ => {
                let (image, charge) = result.unwrap();
                let full = charge.additional_storage();
                assert_eq!(full, image.retained_storage());
                b.reserve_storage(full).unwrap();
                assert_eq!(image.object_identity(), source.object_identity());
                drop((image, charge));
                b.release_storage(full).unwrap();
                assert_eq!(b.peak_storage(), 17 + quota.scratch());
            }
        }
        assert_eq!(
            b.work(),
            match mode {
                0 => 0,
                1 => 8,
                _ => quota.work(),
            }
        );
    }
}

#[test]
fn running_open_failure_and_unwind_preserve_history() {
    let f = Fixture::new();
    let m = f.measurement();
    let quota =
        ProtectedStaticExecutableV2::quota(m, ProtectedStaticExecutableOperationV2::Admit).unwrap();
    for unwind in [false, true] {
        let mut w = Work::new(quota.work());
        let mut b = Budget::new(&mut w, 17 + quota.scratch());
        b.reserve_storage(17).unwrap();
        assert!(b.charge_work(usize::MAX).is_err());
        assert!(b.reserve_storage(usize::MAX).is_err());
        let denials = (b.failed_work(), b.failed_storage());
        let result = catch_unwind(AssertUnwindSafe(|| {
            ProtectedStaticExecutableV2::admit_running_with(
                m,
                Owner::current(),
                "fixture",
                &mut b,
                || {
                    assert!(!unwind, "injected running-image open unwind");
                    Err(ImageError::Io {
                        operation: "injected open failure",
                        source: std::io::Error::from_raw_os_error(libc::ENOENT),
                    })
                },
            )
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result.unwrap(),
                Err(ProtectedStaticExecutableErrorV2::Image(_))
            ));
        }
        assert_eq!(b.storage(), 17);
        assert_eq!(b.work(), quota.work());
        assert_eq!((b.failed_work(), b.failed_storage()), denials);
    }
}

#[test]
fn unsealed_running_test_process_is_not_admitted() {
    let f = Fixture::new();
    let quota = ProtectedStaticExecutableV2::quota(
        f.measurement(),
        ProtectedStaticExecutableOperationV2::Admit,
    )
    .unwrap();
    let mut w = Work::new(quota.work());
    let mut b = Budget::new(&mut w, quota.scratch());
    assert!(matches!(
        ProtectedStaticExecutableV2::admit_running(
            f.measurement(),
            Owner::current(),
            "fixture",
            &mut b
        ),
        Err(ProtectedStaticExecutableErrorV2::Image(_))
    ));
    assert_eq!(b.storage(), 0);
    assert_eq!(b.work(), quota.work());
}

#[test]
fn interrupted_running_open_refuses_after_one_attempt() {
    let f = Fixture::new();
    let m = f.measurement();
    let quota =
        ProtectedStaticExecutableV2::quota(m, ProtectedStaticExecutableOperationV2::Admit).unwrap();
    let mut w = Work::new(quota.work());
    let mut b = Budget::new(&mut w, quota.scratch());
    let attempts = Cell::new(0);
    let result = ProtectedStaticExecutableV2::admit_running_with(
        m,
        Owner::current(),
        "fixture",
        &mut b,
        || {
            attempts.set(attempts.get() + 1);
            Err(ImageError::Io {
                operation: "interrupted open",
                source: std::io::Error::from_raw_os_error(libc::EINTR),
            })
        },
    );
    assert!(
        matches!(result, Err(ProtectedStaticExecutableErrorV2::Image(ImageError::Io { source, .. }))
        if source.kind() == std::io::ErrorKind::Interrupted)
    );
    assert_eq!(attempts.get(), 1);
    assert_eq!(b.storage(), 0);
    assert_eq!(b.work(), quota.work());
}
