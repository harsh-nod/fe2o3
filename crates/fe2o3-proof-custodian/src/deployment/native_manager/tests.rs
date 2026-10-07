use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn fixture() -> Config {
    let mut work = Work::new(CONFIG_WORK);
    let mut b = Budget::new(&mut work, CONFIG_SCRATCH);
    let (config, storage) =
        Config::new(([1; 32], 4096), [2; 32], [3; 32], ([4; 32], 648), &mut b).unwrap();
    assert_eq!(storage.additional_storage(), Config::RETAINED);
    assert_eq!(b.storage(), 0);
    config
}
fn decode(bytes: &[u8]) -> io::Result<Config> {
    let mut work = Work::new(CONFIG_WORK);
    let mut b = Budget::new(&mut work, BYTES + CONFIG_SCRATCH);
    b.reserve_storage(BYTES).unwrap();
    Config::decode(bytes, &mut b).map(|(c, _)| c)
}
fn reseal(bytes: &mut [u8; BYTES]) {
    let identity = checksum(&bytes[..176]);
    bytes[176..].copy_from_slice(&identity);
}
#[test]
fn native_manager_configuration_binds_every_byte_and_exact_frame() {
    let config = fixture();
    assert_eq!(decode(config.canonical_bytes()).unwrap(), config);
    assert_eq!(config.compiler_policy_identity(), [2; 32]);
    assert_eq!(config.proof_deployment_identity(), [3; 32]);
    assert_eq!(config.semantic_policy(), ([4; 32], 648));
    for offset in 0..BYTES {
        let mut changed = *config.canonical_bytes();
        changed[offset] ^= 1;
        assert!(decode(&changed).is_err(), "offset {offset}");
    }
    for length in 0..BYTES {
        assert!(decode(&config.canonical_bytes()[..length]).is_err());
    }
    let mut extended = config.canonical_bytes().to_vec();
    extended.push(0);
    assert!(decode(&extended).is_err());
}
#[test]
fn native_manager_resealed_missing_or_noncanonical_fields_refuse() {
    for range in [24..56, 56..64, 64..96, 96..128, 128..160, 160..168] {
        let mut changed = *fixture().canonical_bytes();
        changed[range.clone()].fill(0);
        reseal(&mut changed);
        assert!(decode(&changed).is_err(), "zero {range:?}");
    }
    for offset in [8, 10, 12, 16, 23, 168, 169, 175] {
        let mut changed = *fixture().canonical_bytes();
        changed[offset] ^= 1;
        reseal(&mut changed);
        assert!(
            decode(&changed).is_err(),
            "reserved/version/boundary {offset}"
        );
    }
    let mut oversized = *fixture().canonical_bytes();
    oversized[56..64].copy_from_slice(&(MAX_CONTROLLER + 1).to_le_bytes());
    reseal(&mut oversized);
    assert!(decode(&oversized).is_err());
}
#[test]
fn native_manager_each_independent_binding_changes_identity() {
    let config = fixture();
    for offset in [24, 56, 64, 96, 128, 160] {
        let mut changed = *config.canonical_bytes();
        changed[offset] ^= 1;
        reseal(&mut changed);
        let changed = decode(&changed).unwrap();
        assert_ne!(config.identity(), changed.identity(), "binding {offset}");
    }
    assert_ne!(CONFIG_PATH, super::super::native::CONFIG_PATH);
    assert_ne!(IMAGE_PATH, super::super::native::CONTROLLER_PATH);
    assert!(super::super::ProofCustodianDeploymentV1::decode(config.canonical_bytes()).is_err());
}
#[test]
fn native_manager_exact_original_work_storage_and_input_floor() {
    let config = fixture();
    let floor = BYTES + 73;
    for (work, storage, success) in [
        (CONFIG_WORK, floor + CONFIG_SCRATCH, true),
        (CONFIG_WORK - 1, floor + CONFIG_SCRATCH, false),
        (CONFIG_WORK, floor + CONFIG_SCRATCH - 1, false),
    ] {
        let mut work = Work::new(work);
        let mut b = Budget::new(&mut work, storage);
        b.reserve_storage(floor).unwrap();
        assert_eq!(
            Config::decode(config.canonical_bytes(), &mut b).is_ok(),
            success
        );
        assert_eq!(b.storage(), floor);
    }
    let mut work = Work::new(CONFIG_WORK);
    let mut b = Budget::new(&mut work, floor + CONFIG_SCRATCH);
    assert!(Config::decode(config.canonical_bytes(), &mut b).is_err());
    assert_eq!(b.storage(), 0);
}
