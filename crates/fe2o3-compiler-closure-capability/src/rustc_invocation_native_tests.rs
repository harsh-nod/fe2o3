//! Inert sealed-owner accounting only; no cargo authorship, runtime approval or exec.
use super::*;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_rustc_invocation::{CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcUnitV2};
use std::{fs, os::fd::AsRawFd, os::unix::fs::MetadataExt};

type Cap = RustcInvocationCapabilityV1;
const CWD: &str = "/workspace/project";
const PREFIX: usize = 31;
const WORK: usize =
    Cap::NATIVE_ADMISSION_WORK + Cap::NATIVE_REVALIDATION_WORK + Cap::NATIVE_TRANSFER_WORK + 1024;
const STORAGE: usize = 4 * Cap::NATIVE_MAX_RETAINED_STORAGE + Cap::NATIVE_TRANSFER_SCRATCH;

fn descriptor(cwd_capacity: usize) -> RustcInvocationDescriptorV3 {
    let mut cwd = String::with_capacity(cwd_capacity);
    cwd.push_str(CWD);
    let closure = CompilerClosureV2::new(
        [0x31; 32], [0x32; 32], [0x33; 32], [0x11; 32], [0x35; 32], [0x22; 32],
    )
    .unwrap();
    RustcInvocationDescriptorV3::new(
        RustcInvocationDescriptorV2::new(
            closure.rustc_executable_sha256(),
            closure.codegen_backend_sha256(),
            RustcUnitV2::new(
                cwd,
                vec![
                    "/toolchains/rustc".into(),
                    "-Zcodegen-backend=/proc/./self/fd/198".into(),
                ],
            )
            .unwrap(),
            CompileEnvironmentV2::from_child_environment([
                ("FE2O3_TARGET".into(), "gfx942:xnack-".into()),
                ("FE2O3_HSACO_DIR".into(), "/proc/self/fd/197".into()),
            ])
            .unwrap(),
        )
        .unwrap(),
        closure,
    )
    .unwrap()
}

fn actual_storage(cap: &Cap) -> usize {
    let decoded = cap.descriptor().retained_storage_bytes().unwrap();
    assert_eq!(cap.decoded_storage, decoded);
    size_of::<(Cap, NativeStorage)>() + decoded - size_of::<RustcInvocationDescriptorV3>()
        + envelope_overhead::<(Cap, NativeStorage), NativeError>()
        + cap.canonical_bytes.capacity()
        + usize::try_from(cap.image.as_file().metadata().unwrap().len()).unwrap()
}

fn capacity_rejected<T>(result: Result<T, NativeError>) {
    assert!(matches!(
        result,
        Err(NativeError::Rejected(
            "retained invocation capacity exceeds native owner bound"
        ))
    ));
}

fn history(b: &Budget<'_>) -> (usize, usize, usize, Option<usize>, Option<usize>) {
    (
        b.work(),
        b.storage(),
        b.peak_storage(),
        b.failed_work(),
        b.failed_storage(),
    )
}

#[test]
fn constructors_cache_create_file_and_inherited_decoded_capacity() {
    let descriptor = descriptor(8192);
    let decoded = descriptor.retained_storage_bytes().unwrap();
    let cwd = descriptor.rustc().working_directory().as_ptr();
    let source = Cap::create(descriptor).unwrap();
    assert_eq!(source.decoded_storage, decoded);
    assert_eq!(
        source.descriptor().rustc().working_directory().as_ptr(),
        cwd
    );
    assert!(actual_storage(&source) <= source.native_retained_storage().unwrap());

    let received = Cap::from_file(source.try_clone_for_transfer().unwrap()).unwrap();
    assert_eq!(received.canonical_bytes, source.canonical_bytes);
    assert!(received.decoded_storage < source.decoded_storage);
    assert!(actual_storage(&received) <= received.native_retained_storage().unwrap());

    let file = source.try_clone_for_transfer().unwrap();
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
    let inherited = Cap::from_inherited_at(file.as_raw_fd()).unwrap();
    drop(file);
    assert_eq!(inherited.canonical_bytes, source.canonical_bytes);
    assert!(actual_storage(&inherited) <= inherited.native_retained_storage().unwrap());
}

#[test]
fn same_wire_oversized_capacity_is_rejected_without_native_work_or_cap_inflation() {
    let source = Cap::create(descriptor(CWD.len())).unwrap();
    let bound = source.native_retained_storage().unwrap();
    let oversized = Cap::create(descriptor(2 * bound)).unwrap();
    assert_eq!(oversized.canonical_bytes, source.canonical_bytes);
    assert_eq!(
        Cap::native_storage_for(oversized.canonical_bytes.len()).unwrap(),
        bound
    );
    assert!(oversized.decoded_storage > bound);
    capacity_rejected(oversized.native_retained_storage());

    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    b.reserve_storage(PREFIX + actual_storage(&source) + actual_storage(&oversized))
        .unwrap();
    b.charge_work(PREFIX).unwrap();
    assert!(b.charge_work(WORK).is_err());
    assert!(b.reserve_storage(STORAGE).is_err());
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_>;
    let before = history(&b);
    capacity_rejected(oversized.revalidate_native(&mut b));
    capacity_rejected(oversized.try_clone_for_transfer_native(&mut b));
    assert_eq!(history(&b), before);
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(address, &b as *const Budget<'_>);
    drop(oversized);
    drop(source);
    assert_eq!(history(&b), before);
    b.release_storage(before.1).unwrap();
}

#[test]
fn complete_owner_boundary_includes_canonical_capacity_sealed_bytes_and_envelope() {
    let source = Cap::create(descriptor(CWD.len())).unwrap();
    let bound = source.native_retained_storage().unwrap();
    let spare = bound.checked_sub(actual_storage(&source)).unwrap();
    for excess in [0, 1] {
        let cap = Cap::create(descriptor(CWD.len() + spare + excess)).unwrap();
        assert_eq!(cap.canonical_bytes, source.canonical_bytes);
        assert!(cap.decoded_storage < bound);
        assert_eq!(actual_storage(&cap), bound + excess);
        if excess == 0 {
            assert_eq!(cap.native_retained_storage().unwrap(), bound);
        } else {
            capacity_rejected(cap.native_retained_storage());
        }
    }
}

#[test]
fn canonical_spare_capacity_is_counted_without_changing_sealed_bytes_or_bound() {
    let mut cap = Cap::create(descriptor(CWD.len())).unwrap();
    let bound = cap.native_retained_storage().unwrap();
    let decoded = cap.decoded_storage;
    let length = cap.canonical_bytes.len();
    // Model an allocator returning spare canonical-buffer capacity. Only this
    // private test can grow it; the cached decoded owner remains unchanged.
    cap.canonical_bytes.reserve_exact(2 * bound);
    assert_eq!(cap.decoded_storage, decoded);
    assert_eq!(cap.canonical_bytes.len(), length);
    assert_eq!(
        cap.image.read_bounded_native().unwrap(),
        cap.canonical_bytes
    );
    assert_eq!(Cap::native_storage_for(length).unwrap(), bound);
    assert!(actual_storage(&cap) > bound);
    capacity_rejected(cap.native_retained_storage());
}

#[test]
fn native_received_cache_and_charge_preserve_original_fd_and_account() {
    let source = Cap::create(descriptor(8192)).unwrap();
    let source_storage = source.native_retained_storage().unwrap();
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    b.reserve_storage(PREFIX + source_storage).unwrap();
    b.charge_work(PREFIX).unwrap();
    assert!(b.charge_work(WORK).is_err());
    assert!(b.reserve_storage(STORAGE).is_err());
    let denials = (b.failed_work(), b.failed_storage());
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_>;
    let (file, charge) = source.try_clone_for_transfer_native(&mut b).unwrap();
    assert_eq!(charge.additional_storage(), Cap::NATIVE_FILE_STORAGE);
    b.reserve_storage(charge.additional_storage()).unwrap();
    let original_fd = file.as_raw_fd();
    let original = file.metadata().unwrap();
    let floor = b.storage();
    let spent = b.work();
    let (received, growth) = Cap::from_file_native(file, &mut b).unwrap();
    assert_eq!(b.storage(), floor);
    assert!(b.work() > spent);
    assert!(b.work() - spent <= Cap::NATIVE_ADMISSION_WORK);
    let full = received.native_retained_storage().unwrap();
    assert_eq!(full, Cap::NATIVE_FILE_STORAGE + growth.additional_storage());
    assert_eq!(
        full,
        Cap::native_storage_for(received.canonical_bytes.len()).unwrap()
    );
    assert!(actual_storage(&received) <= full);
    assert!(full <= Cap::NATIVE_MAX_RETAINED_STORAGE);
    b.reserve_storage(growth.additional_storage()).unwrap();
    assert_eq!(received.image.as_file().as_raw_fd(), original_fd);
    let retained = received.image.as_file().metadata().unwrap();
    assert_eq!(
        (original.dev(), original.ino()),
        (retained.dev(), retained.ino())
    );
    assert_eq!(received.canonical_bytes, source.canonical_bytes);
    assert!(received.decoded_storage < source.decoded_storage);
    received.revalidate_native(&mut b).unwrap();
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(address, &b as *const Budget<'_>);
    assert_eq!((b.failed_work(), b.failed_storage()), denials);
    let before_drop = history(&b);
    drop(received);
    drop(source);
    assert_eq!(history(&b), before_drop);
    b.release_storage(full + source_storage).unwrap();
    assert_eq!(b.storage(), PREFIX);
}

fn references(file: &File) -> usize {
    let original = file.metadata().unwrap();
    let mut count = 0;
    for (index, entry) in fs::read_dir("/proc/self/fd").unwrap().enumerate() {
        assert!(index < 4096, "descriptor census exceeds fixture bound");
        if let Ok(metadata) = fs::metadata(entry.unwrap().path()) {
            count +=
                usize::from((metadata.dev(), metadata.ino()) == (original.dev(), original.ino()));
        }
    }
    count
}

#[test]
fn scoped_capacity_refusal_closes_consumed_inert_owner_and_preserves_history() {
    let ordinary = Cap::create(descriptor(CWD.len())).unwrap();
    let bound = ordinary.native_retained_storage().unwrap();
    drop(ordinary);
    let cap = Cap::create(descriptor(2 * bound)).unwrap();
    let full = actual_storage(&cap);
    let witness_storage = Cap::NATIVE_FILE_STORAGE + cap.canonical_bytes.len();
    let witness = cap.image.as_file().try_clone().unwrap();
    assert_eq!(references(&witness), 2);
    let mut work = Work::new(WORK);
    let mut b = Budget::new(&mut work, STORAGE);
    b.reserve_storage(PREFIX + full + witness_storage).unwrap();
    b.charge_work(PREFIX).unwrap();
    assert!(b.charge_work(WORK).is_err());
    assert!(b.reserve_storage(STORAGE).is_err());
    let ledger = b.work_ledger_identity_v1();
    let address = &b as *const Budget<'_>;
    let before = history(&b);
    // This is only consuming sealed-owner mechanics, not compiler preparation.
    capacity_rejected(b.with_prepaid_scope(full, 8, 16, 128, move |b| {
        cap.revalidate_native(b)?;
        drop(cap);
        Ok(())
    }));
    assert_eq!(references(&witness), 1);
    assert_eq!(b.storage(), before.1);
    assert_eq!(b.work(), before.0 + 16);
    assert_eq!(b.peak_storage(), before.1 + 128);
    assert_eq!((b.failed_work(), b.failed_storage()), (before.3, before.4));
    assert!(ledger == b.work_ledger_identity_v1());
    assert_eq!(address, &b as *const Budget<'_>);
    drop(witness);
    b.release_storage(full + witness_storage).unwrap();
    assert_eq!(b.storage(), PREFIX);
}
