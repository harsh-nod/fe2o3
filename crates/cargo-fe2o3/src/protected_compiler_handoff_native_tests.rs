//! Portable parent/sealed-image checks, not live capture or protected execution.
use super::*;
use crate::inert_rustc_invocation_capture::{
    InertPreparedRustcInvocationCapture, InertRustcInvocationCaptureV2,
};
use fe2o3_compiler_closure_capability::RustcInvocationCapabilityV1;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_process_identity::PinnedWorkingDirectoryV3;
use std::{ffi::OsString, path::Path, process::Command};

const LIMIT: usize = 4_000_000;
const WORK: usize = 30_000_000;

fn parent(mutation: usize) -> ParentRustcInvocationCustody {
    parent_with_directory(
        mutation,
        PinnedWorkingDirectoryV3::open(Path::new("/")).unwrap(),
        Path::new(if mutation == 4 {
            "/other"
        } else {
            "/workspace"
        }),
    )
}

fn capture_v2(mutation: usize, path: &Path) -> InertRustcInvocationCaptureV2 {
    let mut command = Command::new("/proc/self/fd/9");
    command.args([
        "--crate-name",
        if mutation == 1 { "other" } else { "example" },
        "--crate-type",
        "cdylib",
        if mutation == 2 {
            "other.rs"
        } else {
            "kernel.rs"
        },
        "-Zcodegen-backend=/toolchains/backend.so",
    ]);
    let environment = [
        ("PATH", "/usr/bin"),
        ("FE2O3_HSACO_DIR", "/proc/self/fd/197"),
        (
            "FE2O3_TARGET",
            if mutation == 3 {
                "gfx950:xnack-"
            } else {
                "gfx942:xnack-"
            },
        ),
        ("FE2O3_VERIFY_KERNEL_IR", "1"),
    ]
    .map(|(k, v)| (OsString::from(k), OsString::from(v)));
    InertRustcInvocationCaptureV2::capture(
        &command,
        Path::new("/toolchains/rustc").as_os_str(),
        path,
        &environment,
        [4; 32],
        [6; 32],
    )
    .unwrap()
}

fn parent_with_directory(
    mutation: usize,
    directory: PinnedWorkingDirectoryV3,
    path: &Path,
) -> ParentRustcInvocationCustody {
    let capture = capture_v2(mutation, path);
    let closure = CompilerClosureV2::new(
        [if mutation == 5 { 9 } else { 1 }; 32],
        [2; 32],
        [3; 32],
        [4; 32],
        [5; 32],
        [6; 32],
    )
    .unwrap();
    let capture = capture.upgrade(closure).unwrap();
    let capability = RustcInvocationCapabilityV1::create(capture.descriptor().clone()).unwrap();
    ParentRustcInvocationCustody::retain(
        Some(InertPreparedRustcInvocationCapture::V3(Box::new(capture))),
        Some(capability),
        None,
        directory,
    )
    .unwrap()
    .unwrap()
}

#[path = "protected_compiler_handoff_directory_tests.rs"]
mod directory;

#[test]
fn native_parent_requires_exact_capture_seal_and_observed_invocation() {
    let parent = parent(0);
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(parent.native_retained_storage().unwrap())
        .unwrap();
    let ledger = b.work_ledger_identity_v1();
    parent.revalidate_native(&mut b).unwrap();
    assert_eq!(
        parent
            .match_native_invocation(parent.invocation.descriptor(), &mut b)
            .unwrap(),
        *parent.invocation.descriptor().compiler_closure()
    );
    assert!(!parent.grants_compiler_authority());
    for mutation in 1..=5 {
        let other = self::parent(mutation);
        let storage = other.native_retained_storage().unwrap();
        b.reserve_storage(storage).unwrap();
        let floor = b.storage();
        assert!(matches!(
            parent.match_native_invocation(other.invocation.descriptor(), &mut b),
            Err(Error::Rejected("parent rustc invocation differs"))
        ));
        assert_eq!(b.storage(), floor);
        drop(other);
        b.release_storage(storage).unwrap();
    }
    assert!(b.work_ledger_identity_v1() == ledger);
}

#[test]
fn native_parent_rejects_sealed_capability_substitution() {
    let mut selected = parent(0);
    let foreign = parent(1);
    selected.capability = foreign.capability;
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, LIMIT);
    b.reserve_storage(selected.native_retained_storage().unwrap())
        .unwrap();
    assert!(matches!(
        selected.revalidate_native(&mut b),
        Err(Error::Rejected(_))
    ));
}

#[test]
fn native_parent_exact_and_short_resources_keep_the_original_account() {
    let parent = parent(0);
    let floor = parent.native_retained_storage().unwrap();
    let mut measured = (WORK, LIMIT);
    for case in 0..5 {
        let quota = measured.0 - usize::from(case == 2);
        let limit = measured.1 - usize::from(case == 3);
        let input = floor - usize::from(case == 4);
        let mut work = Work::new(quota);
        let mut b = Budget::new(&mut work, limit);
        b.reserve_storage(input).unwrap();
        b.charge_work(19).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let result = parent.revalidate_native(&mut b);
        assert_eq!(result.is_ok(), case < 2, "case {case}: {result:?}");
        if case == 0 {
            measured = (b.work(), b.peak_storage());
        }
        if case >= 2 {
            assert!(matches!(result, Err(Error::Resource(_))));
        }
        if case == 3 {
            assert!(b.failed_storage().is_some());
        }
        assert_eq!(b.storage(), input);
        assert!(b.work_ledger_identity_v1() == ledger);
    }
}
