use sha2::{Digest, Sha256};

use super::*;

#[test]
fn manifest_digest_is_frozen() {
    let digest = Sha256::digest(GFX942_PERSISTENT_LOCAL_COMPUTE_ADAPTER_MANIFEST_V1);
    let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(
        rendered,
        GFX942_PERSISTENT_LOCAL_COMPUTE_ADAPTER_MANIFEST_SHA256_V1
    );
}

#[test]
fn coexistence_manifest_has_its_own_frozen_identity() {
    let digest = Sha256::digest(GFX942_COMPUTE_SDMA_COEXISTENCE_MANIFEST_V1);
    let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(rendered, GFX942_COMPUTE_SDMA_COEXISTENCE_MANIFEST_SHA256_V1);
    assert_ne!(
        rendered,
        GFX942_PERSISTENT_LOCAL_COMPUTE_ADAPTER_MANIFEST_SHA256_V1
    );
    assert!(GFX942_PERSISTENT_LOCAL_COMPUTE_ADAPTER_MANIFEST_V1.contains("no-concurrent-sdma"));
    assert!(
        GFX942_COMPUTE_SDMA_COEXISTENCE_MANIFEST_V1
            .contains("no-device-timeline-overlap-or-performance-claim")
    );
}

#[test]
fn metadata_effect_reports_only_write_capability() {
    assert!(!Gfx942PersistentComputeEffectV1::Read.writes());
    assert!(Gfx942PersistentComputeEffectV1::Write.writes());
    assert!(Gfx942PersistentComputeEffectV1::ReadWrite.writes());
}

#[test]
fn readwrite_completion_never_reuses_predispatch_authenticated_digest() {
    let stale = Some([0x5a; 32]);
    assert_eq!(
        replay_authenticated_sha256_v1(Gfx942PersistentComputeEffectV1::Read, stale),
        stale
    );
    assert_eq!(
        replay_authenticated_sha256_v1(Gfx942PersistentComputeEffectV1::Write, stale),
        None
    );
    assert_eq!(
        replay_authenticated_sha256_v1(Gfx942PersistentComputeEffectV1::ReadWrite, stale),
        None
    );
}

#[test]
fn terminal_custody_observation_is_address_free_and_stage_exact() {
    assert_eq!(
        PersistentComputeTerminalNativeCustodyV1::Attached.stage(),
        Some(Gfx942PersistentComputeTerminalStageV1::Attached)
    );
    assert_eq!(
        PersistentComputeTerminalNativeCustodyV1::Data(PersistentComputeTerminalDataV1::from_vec(
            Vec::new()
        ),)
        .stage(),
        Some(Gfx942PersistentComputeTerminalStageV1::DataDetached)
    );
    assert_eq!(
        PersistentComputeTerminalNativeCustodyV1::Restored.stage(),
        Some(Gfx942PersistentComputeTerminalStageV1::Restored)
    );
}

#[test]
fn transition_failure_into_parts_preserves_retryable_or_terminal_custody() {
    let retryable = Gfx942PersistentComputeTransitionFailureV1 {
        error: ComputeAqlQueueSessionErrorV1::Contract("foreign receipt"),
        recovered: Some(37_u64),
        retained: None,
    };
    let (_, custody) = retryable.into_parts();
    assert!(matches!(
        custody,
        Gfx942PersistentComputeTransitionFailureCustodyV1::Retryable(37)
    ));

    let terminal = Gfx942PersistentComputeTransitionFailureV1::<u64> {
        error: ComputeAqlQueueSessionErrorV1::Contract("terminal receipt"),
        recovered: None,
        retained: Some(PersistentComputeTerminalNativeCustodyV1::Attached),
    };
    let (_, custody) = terminal.into_parts();
    let Gfx942PersistentComputeTransitionFailureCustodyV1::ProcessTeardown(custody) = custody
    else {
        panic!("terminal native custody must not be dropped by into_parts")
    };
    assert_eq!(
        custody.stage(),
        Some(Gfx942PersistentComputeTerminalStageV1::Attached)
    );
}
