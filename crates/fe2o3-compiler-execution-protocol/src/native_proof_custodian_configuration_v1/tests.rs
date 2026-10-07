use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn parts() -> Parts {
    Parts {
        credentials: (61_021, 61_022),
        controller: ([1; 32], 1234),
        analyzer_executable: ([2; 32], 5678),
        analyzer_runtime_closure: ([3; 32], 9012),
        analyzer_identity: [4; 32],
        toolchain_identity: [5; 32],
        verus_identity: [6; 32],
        compiler_policy_identity: [7; 32],
        semantic_policy: ([8; 32], 256),
    }
}
fn bytes() -> [u8; BYTES] {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    *Config::new(parts(), &mut budget)
        .unwrap()
        .0
        .canonical_bytes()
}
fn reseal(bytes: &mut [u8; BYTES]) {
    let digest = hash(&bytes[..IDENTITY]);
    bytes[IDENTITY..].copy_from_slice(&digest);
}

#[test]
fn native_configuration_roundtrip_retains_every_axis_and_original_account() {
    let mut work = Work::new(2 * WORK);
    let mut budget = Budget::new(&mut work, BYTES + SCRATCH);
    budget.reserve_storage(BYTES).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let (value, charge) = Config::new(parts(), &mut budget).unwrap();
    let (decoded, decoded_charge) = Config::decode(value.canonical_bytes(), &mut budget).unwrap();
    assert_eq!(value, decoded);
    assert_eq!(value.parts(), parts());
    assert_eq!(value.boundary(), 6);
    assert!(!value.authenticates_deployment());
    assert_eq!(charge, decoded_charge);
    assert_eq!(charge.retained_storage(), Config::RETAINED);
    assert_eq!(budget.storage(), BYTES);
    assert_eq!(budget.work(), 2 * WORK);
    assert!(ledger == budget.work_ledger_identity_v1());
}

#[test]
fn native_configuration_rejects_old_shape_header_boundary_and_digest_substitution() {
    for length in [0, 320, BYTES - 1, BYTES + 1] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SCRATCH);
        assert!(matches!(
            Config::decode(&vec![0; length], &mut budget),
            Err(Error::Length)
        ));
        assert_eq!(budget.storage(), SCRATCH);
    }
    for index in [0, 8, 10, 12, 16, 280, 281, IDENTITY] {
        let mut data = bytes();
        data[index] ^= 1;
        if index != IDENTITY {
            reseal(&mut data);
        }
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, BYTES + SCRATCH);
        budget.reserve_storage(BYTES).unwrap();
        assert!(Config::decode(&data, &mut budget).is_err());
        assert_eq!(budget.storage(), BYTES + SCRATCH);
    }
}

#[test]
fn native_configuration_rejects_zero_measurements_and_invalid_credentials() {
    for offset in [32, 72, 112, 152, 184, 216, 248, 288] {
        let mut data = bytes();
        data[offset..offset + 32].fill(0);
        reseal(&mut data);
        assert!(matches!(
            Config::decode_inner(&data),
            Err(Error::Measurement)
        ));
    }
    for offset in [64, 104, 144, 320] {
        let mut data = bytes();
        data[offset..offset + 8].fill(0);
        reseal(&mut data);
        assert!(matches!(
            Config::decode_inner(&data),
            Err(Error::Measurement)
        ));
    }
    for offset in [24, 28] {
        for id in [0_u32, u32::MAX] {
            let mut data = bytes();
            data[offset..offset + 4].copy_from_slice(&id.to_le_bytes());
            reseal(&mut data);
            assert!(matches!(
                Config::decode_inner(&data),
                Err(Error::Credentials)
            ));
        }
    }
}

#[test]
fn native_configuration_independent_policy_pin_changes_exact_identity() {
    let original = Config::decode_inner(&bytes()).unwrap();
    for offset in [248, 288, 320] {
        let mut data = *original.canonical_bytes();
        data[offset] ^= 1;
        reseal(&mut data);
        let substituted = Config::decode_inner(&data).unwrap();
        assert_ne!(substituted.identity(), original.identity());
    }
}

#[test]
fn native_configuration_exact_and_one_short_limits_preserve_terminal_prefix() {
    for (work_limit, storage_limit, prepaid, success) in [
        (WORK, BYTES + SCRATCH, BYTES, true),
        (WORK - 1, BYTES + SCRATCH, BYTES, false),
        (WORK, BYTES + SCRATCH - 1, BYTES, false),
        (WORK, BYTES + SCRATCH, BYTES - 1, false),
    ] {
        let data = bytes();
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(prepaid).unwrap();
        assert_eq!(Config::decode(&data, &mut budget).is_ok(), success);
        assert_eq!(budget.storage(), prepaid);
        if work_limit < WORK {
            assert!(budget.failed_work().is_some());
        }
        if storage_limit < BYTES + SCRATCH {
            assert!(budget.failed_storage().is_some());
        }
    }
}
