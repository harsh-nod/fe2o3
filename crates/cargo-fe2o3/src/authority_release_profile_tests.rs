use super::*;
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionClientProfileV1, CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionIssuerPolicyV3,
};
use std::{
    io::Write,
    os::unix::fs::{MetadataExt, PermissionsExt},
};

pub(crate) fn account(work: usize, storage: usize) -> ClientProfileAccountV3 {
    Arc::new(Mutex::new(OwnedBudget::new(Work::new(work), storage)))
}

// Synthetic trust inputs for codec/custody tests only, never fixed-origin admission.
pub(crate) fn native_profile(seed: u8) -> FundedClientProfileV3 {
    let account = account(100_000_000, 16_000_000);
    let capability = with_account(&account, |budget| {
        budget.reserve_storage(PROFILE_OWNER_STORAGE).unwrap();
        let (policy, growth) = CompilerExecutionIssuerPolicyV3::new(
            u64::from(seed),
            Measurement::new([seed; 32], 123).unwrap(),
            Measurement::new([seed + 1; 32], 456).unwrap(),
            SigningKey::from_bytes(&[seed; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[seed + 1; 32])
                .verifying_key()
                .to_bytes(),
            budget,
        )
        .unwrap();
        budget.reserve_storage(growth.additional_storage()).unwrap();
        let (profile, growth) = CompilerExecutionClientProfileV3::new(
            1000,
            1000,
            Service::new(1001, 1001).unwrap(),
            policy,
            budget,
        )
        .unwrap();
        budget.reserve_storage(growth.additional_storage()).unwrap();
        let (capability, growth) = Capability::create(profile, budget).unwrap();
        budget.reserve_storage(growth.additional_storage()).unwrap();
        capability
    });
    FundedClientProfileV3 {
        capability,
        account,
    }
}

pub(crate) fn legacy_profile_file(seed: u8) -> File {
    let policy = CompilerExecutionIssuerPolicyV1::new(
        u64::from(seed),
        Measurement::new([seed; 32], 123).unwrap(),
        Measurement::new([seed + 1; 32], 456).unwrap(),
        SigningKey::from_bytes(&[seed; 32])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[seed + 1; 32])
            .verifying_key()
            .to_bytes(),
    )
    .unwrap();
    let profile = CompilerExecutionClientProfileV1::new(
        1000,
        1000,
        Service::new(1001, 1001).unwrap(),
        policy,
    )
    .unwrap();
    fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV1::create(profile)
        .unwrap()
        .try_clone_for_transfer()
        .unwrap()
}

pub(crate) fn image(bytes: &[u8], seals: rustix::fs::SealFlags) -> File {
    let mut file = File::from(
        rustix::fs::memfd_create(
            "release-profile-test",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .unwrap(),
    );
    file.set_permissions(std::fs::Permissions::from_mode(0o400))
        .unwrap();
    file.write_all(bytes).unwrap();
    rustix::fs::fcntl_add_seals(&file, seals).unwrap();
    file
}

#[test]
fn received_native_profile_uses_exact_original_account_with_nested_quota() {
    let source = native_profile(7);
    let (work, _) = client_profile_receive_quota_v3().unwrap();
    let floor = 19 + Capability::FILE_STORAGE + TRANSFER_OWNER_STORAGE + PROFILE_OWNER_STORAGE;
    let peak = floor + Capability::IO_STORAGE + COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V3;
    for case in 0..3 {
        let original = account(work - usize::from(case == 0), peak - usize::from(case == 1));
        with_account(&original, |b| b.reserve_storage(19).unwrap());
        let result = FundedClientProfileV3::from_received_with(Arc::clone(&original), || {
            source
                .try_clone_for_transfer()
                .map(|transfer| transfer.file.try_clone().unwrap())
        });
        if case == 2 {
            let received = result.unwrap();
            assert!(Arc::ptr_eq(&received.account, &original));
            assert_eq!(received.profile(), source.profile());
            assert_eq!(original.lock().unwrap().work(), work);
            assert_eq!(original.lock().unwrap().peak_storage(), peak);
            let retained = original.lock().unwrap().storage();
            drop(received);
            assert_eq!(original.lock().unwrap().storage(), retained);
        } else {
            assert!(result.is_err());
            let account = original.lock().unwrap();
            assert_eq!(account.storage(), floor);
            assert!(account.failed_work().is_some() || account.failed_storage().is_some());
        }
    }
}

#[test]
fn transfer_thread_move_and_unwind_keep_original_source_account() {
    let source = native_profile(8);
    let original = Arc::clone(&source.account);
    let initial = original.lock().unwrap().storage();
    let source_id = *source.profile().identity().as_bytes();
    let transferred = source.try_clone_for_transfer().unwrap();
    let inode = transferred.file().metadata().unwrap().ino();
    assert!(original.lock().unwrap().storage() > initial);
    let retained = FundedClientProfileV3::from_funded_file(transferred).unwrap();
    assert!(Arc::ptr_eq(&retained.account, &original));
    let retained = std::thread::spawn(move || {
        assert_eq!(*retained.profile().identity().as_bytes(), source_id);
        assert_eq!(
            retained
                .try_clone_for_transfer()
                .unwrap()
                .file()
                .metadata()
                .unwrap()
                .ino(),
            inode
        );
        retained
    })
    .join()
    .unwrap();
    let transfer = retained.try_clone_for_transfer().unwrap();
    let funded = original.lock().unwrap().storage();
    retained.with_profile_budget(|_, budget| {
        drop(transfer); // No recursively acquired account mutex and no refund.
        assert_eq!(budget.storage(), funded);
    });
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        retained.with_profile_budget(|_, budget| {
            budget.charge_work(17).unwrap();
            panic!("injected profile use failure");
        });
    }));
    assert!(unwind.is_err());
    let after = with_account(&original, |budget| (budget.work(), budget.storage()));
    assert_eq!(after.1, funded);
    assert_eq!(source.profile().identity().as_bytes(), &source_id);
    drop(retained);
    assert_eq!(with_account(&original, |b| b.storage()), funded);
    source.with_profile_budget(|cap, b| cap.revalidate(b).unwrap());
    assert!(with_account(&original, |b| b.work()) > after.0);
}

#[test]
fn descriptor_reservation_precedes_receive_and_refusal_never_resets_account() {
    let denied = account(100_000, Capability::FILE_STORAGE - 1);
    let called = std::cell::Cell::new(false);
    assert!(
        FundedClientProfileV3::from_received_with(Arc::clone(&denied), || {
            called.set(true);
            Err("must not open a descriptor".to_owned())
        })
        .is_err()
    );
    assert!(!called.get());
    assert!(denied.lock().unwrap().failed_storage().is_some());
    let transfer_storage = Capability::FILE_STORAGE + TRANSFER_OWNER_STORAGE;
    let original = account(100_000, transfer_storage);
    assert!(
        FundedClientProfileV3::from_received_with(Arc::clone(&original), || {
            Err("injected read failure".to_owned())
        })
        .is_err()
    );
    assert_eq!(original.lock().unwrap().storage(), transfer_storage);
    assert!(
        FundedClientProfileV3::from_received_with(Arc::clone(&original), || {
            panic!("must not retry on a new account")
        })
        .is_err()
    );
}

#[test]
fn legacy_bytes_and_missing_seals_never_admit_as_native_profile() {
    let source = native_profile(9);
    let full = rustix::fs::SealFlags::WRITE
        | rustix::fs::SealFlags::GROW
        | rustix::fs::SealFlags::SHRINK
        | rustix::fs::SealFlags::SEAL;
    for file in [
        legacy_profile_file(9),
        image(
            source.profile().canonical_bytes(),
            full & !rustix::fs::SealFlags::WRITE,
        ),
        image(
            source.profile().canonical_bytes(),
            rustix::fs::SealFlags::empty(),
        ),
    ] {
        let original = account(100_000_000, 16_000_000);
        assert!(FundedClientProfileV3::from_received_file(file, Arc::clone(&original)).is_err());
        assert!(original.lock().unwrap().work() > 0);
        assert_eq!(
            original.lock().unwrap().storage(),
            Capability::FILE_STORAGE + TRANSFER_OWNER_STORAGE + PROFILE_OWNER_STORAGE
        );
    }
}
