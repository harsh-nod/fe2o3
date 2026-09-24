//! Pure controls only; no fabricated Kernel, VM, device, trap execution or ioctl.
use super::*;

#[test]
fn artifact_preflight_is_bounded_before_hashing_or_native_work() {
    assert!(check_object_bound(0).is_err());
    assert!(check_object_bound(1).is_ok());
    assert!(check_object_bound(fe2o3_hsaco::MAX_HSACO_BYTES).is_ok());
    assert!(check_object_bound(fe2o3_hsaco::MAX_HSACO_BYTES + 1).is_err());
    assert!(check_object_bound(usize::MAX).is_err());
}

#[test]
fn compiled_trap_text_is_exact_offline_structural_input() {
    assert_eq!(trap::text().len(), 1116);
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(trap::text())),
        trap::TEXT_SHA256
    );
    assert_eq!(trap::text().len() % 4, 0);
    assert!(trap::check_text().is_ok());
}

#[test]
fn facts_expose_only_immutable_preparation_counts_and_content_digests() {
    // Synthetic facts, not an actual native preparation result.
    let facts = Gfx950DebugColdPreparationFactsV1 {
        artifact_sha256: [1; 32],
        artifact_bytes: 128,
        trap_sha256: [2; 32],
        trap_bytes: 1116,
        mapped_backing_bytes: 8192,
        metadata_retained_bytes: 336,
    };
    assert_eq!(facts.artifact_sha256(), [1; 32]);
    assert_eq!(facts.artifact_bytes(), 128);
    assert_eq!(facts.trap_sha256(), [2; 32]);
    assert_eq!(facts.trap_bytes(), 1116);
    assert_eq!(facts.mapped_backing_bytes(), 8192);
    assert_eq!(facts.metadata_retained_bytes(), 336);
}
