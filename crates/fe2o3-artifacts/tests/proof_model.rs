#[allow(dead_code)]
mod common;

use common::{digest, name, text};
use fe2o3_artifacts::{
    ConfigurationEntry, DigestAlgorithm, MAX_CONFIGURATION_ENTRIES, MAX_PROOF_PROPERTIES,
    MAX_TRUSTED_ITEMS, MeasuredToolIdentity, PayloadDigest, ProofArtifactIdentity,
    ProofExecutionIdentity, ProofOutcome, ProofProperty, ProofRecordV1, ProofTargetIdentity,
    SourceContractIdentity, TrustedItem, ValidationError, VerificationModelIdentity,
};

fn sha(byte: u8) -> PayloadDigest {
    PayloadDigest::new(DigestAlgorithm::Sha256, digest(byte))
}

fn target() -> ProofTargetIdentity {
    ProofTargetIdentity::new(
        ProofArtifactIdentity::new(
            sha(1),
            sha(2),
            sha(3),
            sha(4),
            sha(5),
            sha(6),
            sha(7),
            sha(8),
        ),
        SourceContractIdentity::new(sha(9), sha(10), sha(11), sha(12), sha(13)),
    )
}

fn measured_tool(name: &str, version: &str, byte: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(text(name), text(version), sha(byte), sha(byte + 1))
}

fn execution() -> ProofExecutionIdentity {
    ProofExecutionIdentity::new(
        VerificationModelIdentity::new(text("fe2o3-gpu-v1"), sha(14)),
        measured_tool("verus", "0.2026.08.04", 15),
        measured_tool("z3", "4.15.2", 17),
        measured_tool("fe2o3-proof-driver", "0.1.0", 19),
        sha(21),
    )
}

#[test]
fn record_canonicalizes_sets_and_preserves_explicit_digest_algorithms() {
    let record = ProofRecordV1::new(
        target(),
        vec![
            ConfigurationEntry::new(name("feature_z"), text("enabled")),
            ConfigurationEntry::new(name("cfg_target"), text("amdgpu")),
        ],
        execution(),
        ProofOutcome::Proved,
        vec![ProofProperty::RaceFreedom, ProofProperty::Bounds],
        vec![
            TrustedItem::new(name("z_axiom"), sha(16)),
            TrustedItem::new(name("model_axiom"), sha(15)),
        ],
    )
    .unwrap();

    assert_eq!(record.target(), target());
    assert_eq!(
        record.target().artifact().kernel_id().algorithm(),
        DigestAlgorithm::Sha256
    );
    assert_eq!(record.outcome(), ProofOutcome::Proved);
    assert_eq!(record.configuration()[0].key().as_str(), "cfg_target");
    assert_eq!(
        record.proved_properties(),
        &[ProofProperty::Bounds, ProofProperty::RaceFreedom]
    );
    assert_eq!(record.trusted_items()[0].name().as_str(), "model_axiom");
    assert_eq!(
        record
            .target()
            .source_contracts()
            .functional_specification_digest(),
        sha(13)
    );
    assert_eq!(record.execution().verifier().executable_digest(), sha(15));
    assert_eq!(record.execution().solver().configuration_digest(), sha(18));
    assert_eq!(
        record.execution().evidence_recorder().executable_digest(),
        sha(19)
    );
    assert_eq!(record.execution().invocation_digest(), sha(21));
}

#[test]
fn proved_record_requires_at_least_one_property() {
    assert_eq!(
        ProofRecordV1::new(
            target(),
            vec![],
            execution(),
            ProofOutcome::Proved,
            vec![],
            vec![],
        ),
        Err(ValidationError::EmptyCollection {
            field: "proved properties"
        })
    );

    assert!(
        ProofRecordV1::new(
            target(),
            vec![],
            execution(),
            ProofOutcome::Failed,
            vec![],
            vec![],
        )
        .is_ok()
    );
}

#[test]
fn record_rejects_duplicate_logical_keys() {
    assert!(matches!(
        ProofRecordV1::new(
            target(),
            vec![
                ConfigurationEntry::new(name("feature"), text("one")),
                ConfigurationEntry::new(name("feature"), text("two")),
            ],
            execution(),
            ProofOutcome::Failed,
            vec![],
            vec![],
        ),
        Err(ValidationError::Duplicate {
            field: "proof configuration key"
        })
    ));

    assert!(matches!(
        ProofRecordV1::new(
            target(),
            vec![],
            execution(),
            ProofOutcome::Proved,
            vec![ProofProperty::Bounds, ProofProperty::Bounds],
            vec![],
        ),
        Err(ValidationError::Duplicate {
            field: "proved property"
        })
    ));

    assert!(matches!(
        ProofRecordV1::new(
            target(),
            vec![],
            execution(),
            ProofOutcome::Proved,
            vec![ProofProperty::Bounds],
            vec![
                TrustedItem::new(name("axiom"), sha(1)),
                TrustedItem::new(name("axiom"), sha(2)),
            ],
        ),
        Err(ValidationError::Duplicate {
            field: "trusted item name"
        })
    ));
}

#[test]
fn record_bounds_all_variable_collections() {
    let configurations = (0..=MAX_CONFIGURATION_ENTRIES)
        .map(|index| ConfigurationEntry::new(name(&format!("cfg_{index:03}")), text("on")))
        .collect();
    assert!(matches!(
        ProofRecordV1::new(
            target(),
            configurations,
            execution(),
            ProofOutcome::Failed,
            vec![],
            vec![],
        ),
        Err(ValidationError::TooMany {
            field: "proof configuration",
            max: MAX_CONFIGURATION_ENTRIES
        })
    ));

    let properties = vec![ProofProperty::Bounds; MAX_PROOF_PROPERTIES + 1];
    assert!(matches!(
        ProofRecordV1::new(
            target(),
            vec![],
            execution(),
            ProofOutcome::Proved,
            properties,
            vec![],
        ),
        Err(ValidationError::TooMany {
            field: "proved properties",
            max: MAX_PROOF_PROPERTIES
        })
    ));

    let trusted_items = (0..=MAX_TRUSTED_ITEMS)
        .map(|index| TrustedItem::new(name(&format!("trusted_{index:03}")), sha(1)))
        .collect();
    assert!(matches!(
        ProofRecordV1::new(
            target(),
            vec![],
            execution(),
            ProofOutcome::Failed,
            vec![],
            trusted_items,
        ),
        Err(ValidationError::TooMany {
            field: "trusted items",
            max: MAX_TRUSTED_ITEMS
        })
    ));
}
