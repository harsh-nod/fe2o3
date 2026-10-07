use super::*;

mod private_root;

fn analyzer() -> PhysicalMachineEffectWorkerPolicyV1 {
    PhysicalMachineEffectWorkerPolicyV1::new(
        PhysicalMachineWorkerExecutableIdentityV1::from_parts([2; 32], 1024),
        PhysicalMachineRuntimeClosureIdentityV1::from_parts([3; 32], 2048),
        PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([4; 32]),
        PhysicalMachineToolchainIdentityV1::from_sha256_bytes([5; 32]),
    )
    .unwrap()
}
fn config() -> Config {
    let mut work = Work::new(CONFIG_WORK);
    let mut b = Budget::new(&mut work, CONFIG_SCRATCH);
    let (value, storage) = Config::new(
        ProofControllerCredentialProfileV1::new(61000, 61000).unwrap(),
        [1; 32],
        4096,
        analyzer(),
        [6; 32],
        [7; 32],
        ([8; 32], 1024),
        &mut b,
    )
    .unwrap();
    assert_eq!(storage.additional_storage(), Config::RETAINED);
    assert_eq!(b.storage(), 0);
    value
}
fn decode(bytes: &[u8]) -> io::Result<Config> {
    let mut work = Work::new(CONFIG_WORK);
    let mut b = Budget::new(&mut work, BYTES + CONFIG_SCRATCH);
    b.reserve_storage(BYTES).unwrap();
    Config::decode(bytes, &mut b).map(|(v, _)| v)
}

#[test]
fn native_deployment_binds_every_byte_and_exact_frame() {
    let config = config();
    assert_eq!(decode(config.canonical_bytes()).unwrap(), config);
    for offset in 0..BYTES {
        let mut bytes = *config.canonical_bytes();
        bytes[offset] ^= 1;
        assert!(decode(&bytes).is_err(), "byte {offset}");
    }
    for len in 0..BYTES {
        assert!(decode(&config.canonical_bytes()[..len]).is_err());
    }
    let mut extended = config.canonical_bytes().to_vec();
    extended.push(0);
    assert!(decode(&extended).is_err());
}

#[test]
fn resealed_zero_measurements_policy_and_legacy_boundary_reject() {
    for range in [
        24..28,
        28..32,
        32..64,
        64..72,
        72..104,
        104..112,
        112..144,
        144..152,
        152..184,
        184..216,
        216..248,
        248..280,
        280..281,
    ] {
        let mut bytes = *config().canonical_bytes();
        bytes[range.clone()].fill(0);
        let identity = hash(&bytes[..IDENTITY]);
        bytes[IDENTITY..].copy_from_slice(&identity);
        assert!(decode(&bytes).is_err(), "range {range:?}");
    }
    for boundary in [1, 2, 3, 4, 5, 7, 255] {
        let mut bytes = *config().canonical_bytes();
        bytes[280] = boundary;
        let identity = hash(&bytes[..IDENTITY]);
        bytes[IDENTITY..].copy_from_slice(&identity);
        assert!(decode(&bytes).is_err());
    }
}

#[test]
fn native_and_legacy_configs_have_no_cross_decode() {
    let native = config();
    assert!(super::super::ProofCustodianDeploymentV1::decode(native.canonical_bytes()).is_err());
    let legacy = super::super::ProofCustodianDeploymentV1::new(
        native.credentials().unwrap(),
        [1; 32],
        4096,
        analyzer(),
        [6; 32],
    )
    .unwrap();
    assert!(decode(legacy.canonical_bytes()).is_err());
    assert_ne!(CONFIG_PATH, super::super::APPLICATION_CONFIG_PATH);
    assert_ne!(CONTROLLER_PATH, super::super::APPLICATION_CONTROLLER_PATH);
    assert_ne!(native.identity(), legacy.identity());
}

#[test]
fn exact_and_one_short_config_accounts_keep_work_and_floor() {
    let config = config();
    let floor = BYTES + 73;
    for (work_limit, storage_limit, succeeds) in [
        (CONFIG_WORK, floor + CONFIG_SCRATCH, true),
        (CONFIG_WORK - 1, floor + CONFIG_SCRATCH, false),
        (CONFIG_WORK, floor + CONFIG_SCRATCH - 1, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, storage_limit);
        b.reserve_storage(floor).unwrap();
        assert_eq!(
            Config::decode(config.canonical_bytes(), &mut b).is_ok(),
            succeeds
        );
        assert_eq!(b.storage(), floor);
    }
    let mut work = Work::new(2 * CONFIG_WORK);
    let mut b = Budget::new(&mut work, floor + CONFIG_SCRATCH);
    assert!(Config::decode(config.canonical_bytes(), &mut b).is_err());
    assert_eq!(b.storage(), 0);
    b.reserve_storage(floor).unwrap();
    let mut invalid = *config.canonical_bytes();
    invalid[0] ^= 1;
    assert!(Config::decode(&invalid, &mut b).is_err());
    assert!(b.storage() >= floor);
}

#[test]
fn changed_compiler_policy_is_an_independent_matching_axis() {
    let original = config();
    let mut changed = *original.canonical_bytes();
    changed[248] ^= 1;
    let identity = hash(&changed[..IDENTITY]);
    changed[IDENTITY..].copy_from_slice(&identity);
    let changed = decode(&changed).unwrap();
    assert_ne!(changed.identity(), original.identity());
    assert_ne!(
        changed.compiler_policy_identity(),
        original.compiler_policy_identity()
    );
    assert_eq!(changed.verus_identity(), original.verus_identity());
}
