use super::*;
use ed25519_dalek::SigningKey;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    io::Write,
    os::unix::fs::{FileExt, PermissionsExt},
};

const EXTRA: usize = 23;
const PREFIX_WORK: usize = 19;

#[test]
fn fresh_work_meter_cannot_restart_a_retained_issuers_budget() {
    let mut original = Work::new(usize::MAX);
    let mut replacement = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut original, usize::MAX);
    let mut foreign = Budget::new(&mut replacement, usize::MAX);
    for current in [&mut budget, &mut foreign] {
        current.reserve_storage(EXTRA).unwrap();
        current.charge_work(PREFIX_WORK).unwrap();
    }
    let ledger = budget.work_ledger_identity_v1();
    require_work_ledger(ledger, &budget).unwrap();
    assert_eq!(
        require_work_ledger(ledger, &foreign)
            .unwrap_err()
            .resource(),
        Some(Resource::Accounting)
    );
    for current in [&budget, &foreign] {
        assert_eq!(current.work(), PREFIX_WORK);
        assert_eq!(current.storage(), EXTRA);
        assert_eq!(current.peak_storage(), EXTRA);
        assert_eq!(current.failed_storage(), None);
    }
}

// A parser fixture only. No public admission accepts this supplied image in
// place of /proc/self/exe, and these tests manufacture no issuer authority.
fn static_elf() -> Vec<u8> {
    let mut bytes = vec![0; 4097];
    bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    bytes[16..18].copy_from_slice(&2_u16.to_le_bytes());
    bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
    bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
    bytes[24..32].copy_from_slice(&0x401000_u64.to_le_bytes());
    bytes[32..40].copy_from_slice(&64_u64.to_le_bytes());
    bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
    bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
    bytes[56..58].copy_from_slice(&4_u16.to_le_bytes());
    for (index, (kind, flags, offset, address, size, align)) in [
        (6_u32, 4_u32, 64_u64, 0x400040_u64, 224_u64, 8_u64),
        (1, 4, 0, 0x400000, 288, 4096),
        (1, 5, 4096, 0x401000, 1, 4096),
        (0x6474_e551, 6, 0, 0, 0, 16),
    ]
    .into_iter()
    .enumerate()
    {
        let start = 64 + index * 56;
        bytes[start..start + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[start + 4..start + 8].copy_from_slice(&flags.to_le_bytes());
        for (field, value) in [
            (8, offset),
            (16, address),
            (32, size),
            (40, size),
            (48, align),
        ] {
            bytes[start + field..start + field + 8].copy_from_slice(&value.to_le_bytes());
        }
    }
    bytes[4096] = 0xc3;
    bytes
}

fn image(bytes: &[u8]) -> (File, Snapshot) {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(bytes).unwrap();
    file.set_permissions(std::fs::Permissions::from_mode(0o700))
        .unwrap();
    let snapshot = inspect_executable(&file).unwrap();
    (file, snapshot)
}

fn budget<T>(floor: usize, operation: impl FnOnce(&mut Budget<'_>) -> T) -> T {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, usize::MAX);
    b.reserve_storage(floor + EXTRA).unwrap();
    b.charge_work(PREFIX_WORK).unwrap();
    let result = operation(&mut b);
    assert_eq!(b.storage(), floor + EXTRA);
    result
}

fn policy(measurements: Measurements, generation: u64, b: &mut Budget<'_>) -> Policy {
    let (policy, storage) = Policy::new(
        generation,
        measurements.executable,
        measurements.runtime,
        SigningKey::from_bytes(&[0x31; 32])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[0x71; 32])
            .verifying_key()
            .to_bytes(),
        b,
    )
    .unwrap();
    b.reserve_storage(storage.additional_storage()).unwrap();
    policy
}

#[test]
fn native_and_legacy_static_measurements_agree() {
    let (file, snapshot) = image(&static_elf());
    let legacy = super::super::measure_static_executable(&file, snapshot).unwrap();
    budget(size_of::<File>(), |b| {
        let native = measure_executable(&file, snapshot, b).unwrap();
        assert_eq!(native, legacy);
        assert_eq!(
            b.work(),
            PREFIX_WORK + image_resources(snapshot.size as usize).unwrap().0
        );
    });
}

#[test]
fn exact_image_budget_and_each_one_short_refuse_before_io() {
    let (file, snapshot) = image(&static_elf());
    let floor = size_of::<File>() + EXTRA;
    let (work, storage) = image_resources(snapshot.size as usize).unwrap();
    for (work_limit, storage_limit, expected) in [
        (PREFIX_WORK + work, floor + storage, "ok"),
        (PREFIX_WORK + work - 1, floor + storage, "work"),
        (PREFIX_WORK + work, floor + storage - 1, "storage"),
    ] {
        let mut w = Work::new(work_limit);
        let mut b = Budget::new(&mut w, storage_limit);
        b.charge_work(PREFIX_WORK).unwrap();
        b.reserve_storage(floor).unwrap();
        let result = measure_executable(&file, snapshot, &mut b);
        assert_eq!(b.storage(), floor);
        match expected {
            "ok" => {
                result.unwrap();
                assert_eq!(b.peak_storage(), floor + storage);
            }
            "work" => assert!(matches!(
                result.unwrap_err().resource(),
                Some(Resource::Work(_))
            )),
            "storage" => {
                assert!(matches!(
                    result.unwrap_err().resource(),
                    Some(Resource::Storage(_))
                ));
                assert_eq!(b.failed_storage(), Some(floor + storage));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn image_scope_requires_prepaid_input_and_preserves_prior_work() {
    let (file, snapshot) = image(&static_elf());
    let mut w = Work::new(usize::MAX);
    let mut b = Budget::new(&mut w, usize::MAX);
    b.charge_work(PREFIX_WORK).unwrap();
    assert_eq!(
        measure_executable(&file, snapshot, &mut b)
            .unwrap_err()
            .resource(),
        Some(Resource::Accounting)
    );
    assert_eq!(b.work(), PREFIX_WORK + ENTRY_WORK);
    assert_eq!(b.storage(), 0);
}

#[test]
fn changed_and_short_images_fail_closed() {
    for grow in [false, true] {
        let (file, snapshot) = image(&static_elf());
        file.set_len(if grow {
            snapshot.size + 1
        } else {
            snapshot.size - 1
        })
        .unwrap();
        budget(size_of::<File>(), |b| {
            let error = measure_executable(&file, snapshot, b).unwrap_err();
            assert_eq!(error.kind(), Some(Kind::ExecutableChanged));
        });
    }
}

#[test]
fn retained_image_rechecks_bytes_not_only_supplied_measurements() {
    let (file, snapshot) = image(&static_elf());
    let measurements = budget(size_of::<File>(), |b| {
        measure_executable(&file, snapshot, b).unwrap()
    });
    let mut retained = Executable {
        image: file,
        snapshot,
        measurements,
    };
    retained.image.write_all_at(&[0xcc], 4096).unwrap();
    // Even a new metadata snapshot cannot make stale measurements match bytes.
    retained.snapshot = inspect_executable(&retained.image).unwrap();
    budget(size_of::<File>(), |b| {
        assert_eq!(
            validate_executable(&retained, b).unwrap_err().kind(),
            Some(Kind::ExecutableChanged)
        );
    });
}

#[test]
fn malformed_static_images_keep_parser_error_and_charges() {
    let mut bytes = static_elf();
    bytes[64..68].copy_from_slice(&3_u32.to_le_bytes()); // PT_INTERP
    let (file, snapshot) = image(&bytes);
    budget(size_of::<File>(), |b| {
        let error = measure_executable(&file, snapshot, b).unwrap_err();
        assert!(matches!(
            error.0,
            Failure::StaticImage(SealedStaticApplicationErrorV1::InterpreterPresent)
        ));
        assert_eq!(
            b.work(),
            PREFIX_WORK + image_resources(bytes.len()).unwrap().0
        );
    });
}

#[test]
fn executable_shape_and_cloexec_are_shared_with_legacy() {
    let (file, _) = image(&static_elf());
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
    assert_eq!(
        inspect_executable(&file).unwrap_err().kind(),
        Some(Kind::ExecutableCloseOnExec)
    );
    rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::CLOEXEC).unwrap();
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .unwrap();
    assert_eq!(
        inspect_executable(&file).unwrap_err().kind(),
        Some(Kind::ExecutableShape)
    );
    file.set_permissions(std::fs::Permissions::from_mode(0o700))
        .unwrap();
    file.set_len(0).unwrap();
    assert_eq!(
        inspect_executable(&file).unwrap_err().kind(),
        Some(Kind::ExecutableSize)
    );
}

#[test]
fn native_policy_checks_both_independently_measured_axes() {
    let (file, snapshot) = image(&static_elf());
    let measurements = budget(size_of::<File>(), |b| {
        measure_executable(&file, snapshot, b).unwrap()
    });
    let mut w = Work::new(usize::MAX);
    let mut b = Budget::new(&mut w, usize::MAX);
    let pinned = policy(measurements, 1, &mut b);
    check_policy(measurements, &pinned).unwrap();
    let mut substituted = measurements;
    substituted.executable = CompilerExecutionIssuerMeasurementV1::new([8; 32], 1).unwrap();
    assert_eq!(
        check_policy(substituted, &pinned).unwrap_err().kind(),
        Some(Kind::ExecutablePolicyMismatch)
    );
    substituted = measurements;
    substituted.runtime = CompilerExecutionIssuerMeasurementV1::new([9; 32], 1).unwrap();
    assert_eq!(
        check_policy(substituted, &pinned).unwrap_err().kind(),
        Some(Kind::RuntimePolicyMismatch)
    );
}

#[test]
fn retained_native_key_rejects_same_key_different_policy() {
    let measurements = Measurements {
        executable: CompilerExecutionIssuerMeasurementV1::new([1; 32], 1).unwrap(),
        runtime: sealed_static_issuer_runtime_measurement_v1(),
        sealed_static_identity: [2; 32],
    };
    let mut w = Work::new(usize::MAX);
    let mut b = Budget::new(&mut w, usize::MAX);
    let first = policy(measurements, 1, &mut b);
    let second = policy(measurements, 2, &mut b);
    let mut seed = [0x31; 32];
    b.reserve_storage(seed.len()).unwrap();
    let (key, growth) = Key::create_and_zeroize(&mut seed, &first, &mut b).unwrap();
    b.reserve_storage(growth.additional_storage()).unwrap();
    assert_eq!(seed, [0; 32]);
    let floor = b.storage();
    key.revalidate(&first, &mut b).unwrap();
    let error = Error::from(key.revalidate(&second, &mut b).unwrap_err());
    assert_eq!(error.kind(), Some(Kind::KeyCapability));
    assert_eq!(b.storage(), floor);
}

#[test]
fn outer_scope_restores_storage_on_refusal_and_unwind_without_refunding_work() {
    budget(17, |b| {
        let floor = b.storage();
        let error = Admission::scope::<()>(b, floor, |b| {
            b.reserve_storage(13)?;
            Err(Resource::Accounting.into())
        })
        .unwrap_err();
        assert_eq!(error.resource(), Some(Resource::Accounting));
        assert_eq!(b.storage(), floor);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<()> = Admission::scope(b, floor, |b| {
                b.reserve_storage(7)?;
                panic!("native issuer scope unwind fixture");
            });
        }));
        assert!(caught.is_err());
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work(), PREFIX_WORK + 2 * OUTER_WORK);
    });
}

#[test]
fn image_resource_arithmetic_and_nested_resource_errors_are_not_reclassified() {
    assert_eq!(
        image_resources(usize::MAX).unwrap_err().resource(),
        Some(Resource::Arithmetic)
    );
    let error = Error::from(KeyError::Resource(Resource::Accounting));
    assert_eq!(error.resource(), Some(Resource::Accounting));
}
