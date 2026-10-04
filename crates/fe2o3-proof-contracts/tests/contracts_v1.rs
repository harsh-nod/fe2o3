use fe2o3_proof_contracts::{
    ArtifactIdentityV1, CheckedEvidenceV1, ContractSetV1, ContractedEvidenceV1,
    CorrespondenceIdentityV1, CorrespondenceKindV1, CorrespondenceReferenceV1, DigestV1,
    EvidenceBindingV1, EvidenceIdentityV1, ExactInputIdentityV1, ExactModelIdentityV1,
    ExactToolIdentityV1, IdentityFieldV1, MAX_CORRESPONDENCES_V1, MAX_OBLIGATIONS_V1,
    MAX_PROPERTIES_V1, MAX_TCB_ENTRIES_V1, MAX_TCB_REFERENCES_PER_EVIDENCE_V1,
    ObligationIdentityV1, ObligationRecordV1, ObligationSatisfactionV1, PropertyEvidenceV1,
    PropertyIdentityV1, PropertyKindV1, PropertyRecordV1, PropertyStatusV1, ProvedEvidenceV1,
    SectionV1, StatementIdentityV1, TcbEntryIdentityV1, TcbEntryKindV1, TcbEntryV1,
    UnsupportedEvidenceV1, UnsupportedReasonV1, ValidatedEvidenceV1, ValidationErrorV1,
};

fn digest(byte: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([byte; 32])
}

fn artifact(byte: u8) -> ArtifactIdentityV1 {
    ArtifactIdentityV1::new(digest(byte), digest(byte + 1))
}

fn input(byte: u8) -> ExactInputIdentityV1 {
    ExactInputIdentityV1::new(digest(byte), digest(byte + 1))
}

fn model(byte: u8) -> ExactModelIdentityV1 {
    ExactModelIdentityV1::new(digest(byte), digest(byte + 1))
}

fn tool(byte: u8) -> ExactToolIdentityV1 {
    ExactToolIdentityV1::new(digest(byte), digest(byte + 1))
}

fn property_id(byte: u8) -> PropertyIdentityV1 {
    PropertyIdentityV1::from_untrusted_digest(digest(byte))
}

fn statement_id(byte: u8) -> StatementIdentityV1 {
    StatementIdentityV1::from_untrusted_digest(digest(byte))
}

fn evidence_id(byte: u8) -> EvidenceIdentityV1 {
    EvidenceIdentityV1::from_untrusted_digest(digest(byte))
}

fn tcb_id(byte: u8) -> TcbEntryIdentityV1 {
    TcbEntryIdentityV1::from_untrusted_digest(digest(byte))
}

fn binding(property: u8, statement: u8, evidence: u8) -> EvidenceBindingV1 {
    EvidenceBindingV1 {
        identity: evidence_id(evidence),
        property: property_id(property),
        statement: statement_id(statement),
    }
}

fn obligation(
    identity: u8,
    property: &PropertyRecordV1,
    required_status: PropertyStatusV1,
    satisfied: bool,
) -> ObligationRecordV1 {
    ObligationRecordV1 {
        identity: ObligationIdentityV1::from_untrusted_digest(digest(identity)),
        property: property.identity,
        statement: property.statement,
        required_status,
        satisfaction: satisfied.then(|| ObligationSatisfactionV1 {
            evidence: property.evidence.binding().identity,
            property: property.identity,
            statement: property.statement,
            status: required_status,
        }),
    }
}

fn valid_contract_set() -> ContractSetV1 {
    let proof_tool = tool(70);
    let validation_tool = tool(72);
    let check_tool = tool(74);
    let proof_input = input(60);

    let properties = vec![
        PropertyRecordV1 {
            identity: property_id(1),
            kind: PropertyKindV1::MemorySafety,
            statement: statement_id(21),
            status: PropertyStatusV1::Proved,
            evidence: PropertyEvidenceV1::Proved(ProvedEvidenceV1 {
                binding: binding(1, 21, 11),
                input: proof_input,
                model: model(64),
                tool: proof_tool,
                proof_artifact: artifact(80),
                correspondence: CorrespondenceIdentityV1::from_untrusted_digest(digest(41)),
                trusted_computing_base: vec![tcb_id(31)],
            }),
        },
        PropertyRecordV1 {
            identity: property_id(2),
            kind: PropertyKindV1::DataRaceFreedom,
            statement: statement_id(22),
            status: PropertyStatusV1::Validated,
            evidence: PropertyEvidenceV1::Validated(ValidatedEvidenceV1 {
                binding: binding(2, 22, 12),
                input: input(62),
                model: model(66),
                tool: validation_tool,
                validation_artifact: artifact(82),
                trusted_computing_base: vec![tcb_id(32)],
            }),
        },
        PropertyRecordV1 {
            identity: property_id(3),
            kind: PropertyKindV1::FunctionalCorrectness,
            statement: statement_id(23),
            status: PropertyStatusV1::Contracted,
            evidence: PropertyEvidenceV1::Contracted(ContractedEvidenceV1 {
                binding: binding(3, 23, 13),
                contract_artifact: artifact(84),
            }),
        },
        PropertyRecordV1 {
            identity: property_id(4),
            kind: PropertyKindV1::ResourceBounds,
            statement: statement_id(24),
            status: PropertyStatusV1::Checked,
            evidence: PropertyEvidenceV1::Checked(CheckedEvidenceV1 {
                binding: binding(4, 24, 14),
                input: input(64),
                tool: check_tool,
                check_artifact: artifact(86),
                trusted_computing_base: vec![tcb_id(33)],
            }),
        },
        PropertyRecordV1 {
            identity: property_id(5),
            kind: PropertyKindV1::Progress,
            statement: statement_id(25),
            status: PropertyStatusV1::Unsupported,
            evidence: PropertyEvidenceV1::Unsupported(UnsupportedEvidenceV1 {
                binding: binding(5, 25, 15),
                reason: UnsupportedReasonV1::OutsideDeclaredScope,
                rationale_artifact: artifact(88),
            }),
        },
    ];

    let obligations = properties
        .iter()
        .enumerate()
        .map(|(index, property)| obligation(51 + index as u8, property, property.status, true))
        .collect();

    ContractSetV1 {
        properties,
        obligations,
        trusted_computing_base: vec![
            TcbEntryV1 {
                identity: tcb_id(31),
                kind: TcbEntryKindV1::Tool,
                component: ArtifactIdentityV1::new(proof_tool.executable, digest(91)),
                exact_tool: Some(proof_tool),
                rationale: artifact(92),
            },
            TcbEntryV1 {
                identity: tcb_id(32),
                kind: TcbEntryKindV1::Tool,
                component: ArtifactIdentityV1::new(validation_tool.executable, digest(95)),
                exact_tool: Some(validation_tool),
                rationale: artifact(96),
            },
            TcbEntryV1 {
                identity: tcb_id(33),
                kind: TcbEntryKindV1::Tool,
                component: ArtifactIdentityV1::new(check_tool.executable, digest(99)),
                exact_tool: Some(check_tool),
                rationale: artifact(100),
            },
        ],
        correspondences: vec![CorrespondenceReferenceV1 {
            identity: CorrespondenceIdentityV1::from_untrusted_digest(digest(41)),
            kind: CorrespondenceKindV1::ProofErasure,
            property: property_id(1),
            statement: statement_id(21),
            from: proof_input,
            to: input(68),
            witness_artifact: artifact(102),
        }],
    }
}

#[test]
fn all_five_statuses_validate_with_exact_local_evidence() {
    let contracts = valid_contract_set();

    assert_eq!(contracts.validate(), Ok(()));
    assert_eq!(contracts.validate_closed(), Ok(()));
    assert_eq!(contracts.clone(), contracts);
}

#[test]
fn status_and_evidence_variant_must_match_exactly() {
    let mut contracts = valid_contract_set();
    contracts.properties[0].status = PropertyStatusV1::Validated;

    assert_eq!(
        contracts.validate(),
        Err(ValidationErrorV1::EvidenceStatusMismatch { property_index: 0 })
    );
}

#[test]
fn evidence_cannot_be_rebound_to_another_property_or_statement() {
    let mut contracts = valid_contract_set();
    if let PropertyEvidenceV1::Validated(record) = &mut contracts.properties[1].evidence {
        record.binding.property = property_id(1);
    }

    assert_eq!(
        contracts.validate(),
        Err(ValidationErrorV1::EvidenceBindingMismatch { property_index: 1 })
    );
}

#[test]
fn evidence_identity_cannot_be_reused_across_properties() {
    let mut contracts = valid_contract_set();
    if let PropertyEvidenceV1::Validated(record) = &mut contracts.properties[1].evidence {
        record.binding.identity = evidence_id(11);
    }

    assert_eq!(
        contracts.validate(),
        Err(ValidationErrorV1::EvidenceIdentityReused { property_index: 1 })
    );
}

#[test]
fn proved_memory_safety_does_not_promote_data_race_freedom() {
    let mut contracts = valid_contract_set();
    let PropertyEvidenceV1::Proved(mut proof) = contracts.properties[0].evidence.clone() else {
        panic!("fixture must contain proof evidence");
    };
    proof.binding = binding(2, 22, 12);
    contracts.properties[1].status = PropertyStatusV1::Proved;
    contracts.properties[1].evidence = PropertyEvidenceV1::Proved(proof);
    contracts.obligations[1] =
        obligation(52, &contracts.properties[1], PropertyStatusV1::Proved, true);

    assert_eq!(
        contracts.validate(),
        Err(ValidationErrorV1::CorrespondenceBindingMismatch { property_index: 1 })
    );
}

#[test]
fn obligation_requires_exact_status_not_a_stronger_or_weaker_status() {
    let mut contracts = valid_contract_set();
    contracts.obligations[0].required_status = PropertyStatusV1::Validated;
    contracts.obligations[0]
        .satisfaction
        .as_mut()
        .unwrap()
        .status = PropertyStatusV1::Validated;

    assert_eq!(
        contracts.validate(),
        Err(ValidationErrorV1::SatisfactionStatusMismatch {
            obligation_index: 0
        })
    );
}

#[test]
fn obligation_cannot_cite_another_properties_evidence() {
    let mut contracts = valid_contract_set();
    contracts.obligations[1]
        .satisfaction
        .as_mut()
        .unwrap()
        .evidence = evidence_id(11);

    assert_eq!(
        contracts.validate(),
        Err(ValidationErrorV1::SatisfactionBindingMismatch {
            obligation_index: 1
        })
    );
}

#[test]
fn exact_input_model_and_tool_substitution_fail_closed() {
    let mut input_substitution = valid_contract_set();
    if let PropertyEvidenceV1::Proved(record) = &mut input_substitution.properties[0].evidence {
        record.input = input(110);
    }
    assert_eq!(
        input_substitution.validate(),
        Err(ValidationErrorV1::CorrespondenceBindingMismatch { property_index: 0 })
    );

    let mut model_substitution = valid_contract_set();
    if let PropertyEvidenceV1::Validated(record) = &mut model_substitution.properties[1].evidence {
        record.model = ExactModelIdentityV1::new(DigestV1::ZERO, digest(1));
    }
    assert_eq!(
        model_substitution.validate(),
        Err(ValidationErrorV1::InvalidIdentity {
            section: SectionV1::Properties,
            index: 1,
            field: IdentityFieldV1::Model,
        })
    );

    let mut tool_substitution = valid_contract_set();
    if let PropertyEvidenceV1::Checked(record) = &mut tool_substitution.properties[3].evidence {
        record.tool = tool(112);
    }
    assert_eq!(
        tool_substitution.validate(),
        Err(ValidationErrorV1::MissingToolTcb { property_index: 3 })
    );
}

#[test]
fn correspondence_must_be_nonvacuous_and_property_local() {
    let mut wrong_property = valid_contract_set();
    wrong_property.correspondences[0].property = property_id(2);
    assert_eq!(
        wrong_property.validate(),
        Err(ValidationErrorV1::CorrespondenceBindingMismatch { property_index: 0 })
    );

    let mut vacuous = valid_contract_set();
    vacuous.correspondences[0].to = vacuous.correspondences[0].from;
    assert_eq!(
        vacuous.validate(),
        Err(ValidationErrorV1::VacuousCorrespondence {
            correspondence_index: 0
        })
    );
}

#[test]
fn tool_evidence_requires_an_explicit_exact_tcb_entry() {
    let mut missing = valid_contract_set();
    if let PropertyEvidenceV1::Proved(record) = &mut missing.properties[0].evidence {
        record.trusted_computing_base.clear();
    }
    assert_eq!(
        missing.validate(),
        Err(ValidationErrorV1::MissingToolTcb { property_index: 0 })
    );

    let mut malformed = valid_contract_set();
    malformed.trusted_computing_base[0].exact_tool = None;
    assert_eq!(
        malformed.validate(),
        Err(ValidationErrorV1::InvalidTcbEntryKind { tcb_index: 0 })
    );

    let mut contradictory = valid_contract_set();
    contradictory.trusted_computing_base[0].component.bytes = digest(109);
    assert_eq!(
        contradictory.validate(),
        Err(ValidationErrorV1::InvalidTcbEntryKind { tcb_index: 0 })
    );
}

#[test]
fn unknown_tcb_and_correspondence_references_fail_closed() {
    let mut tcb = valid_contract_set();
    if let PropertyEvidenceV1::Checked(record) = &mut tcb.properties[3].evidence {
        record.trusted_computing_base = vec![tcb_id(34)];
    }
    assert_eq!(
        tcb.validate(),
        Err(ValidationErrorV1::UnknownTcbReference {
            property_index: 3,
            reference_index: 0,
        })
    );

    let mut correspondence = valid_contract_set();
    if let PropertyEvidenceV1::Proved(record) = &mut correspondence.properties[0].evidence {
        record.correspondence = CorrespondenceIdentityV1::from_untrusted_digest(digest(42));
    }
    assert_eq!(
        correspondence.validate(),
        Err(ValidationErrorV1::UnknownCorrespondence { property_index: 0 })
    );
}

#[test]
fn unreferenced_tcb_and_correspondence_records_are_rejected() {
    let mut tcb = valid_contract_set();
    tcb.trusted_computing_base.push(TcbEntryV1 {
        identity: tcb_id(34),
        kind: TcbEntryKindV1::HardwareAssumption,
        component: artifact(104),
        exact_tool: None,
        rationale: artifact(106),
    });
    assert_eq!(
        tcb.validate(),
        Err(ValidationErrorV1::UnreferencedTcb { tcb_index: 3 })
    );

    let mut correspondence = valid_contract_set();
    let mut extra = correspondence.correspondences[0];
    extra.identity = CorrespondenceIdentityV1::from_untrusted_digest(digest(42));
    correspondence.correspondences.push(extra);
    assert_eq!(
        correspondence.validate(),
        Err(ValidationErrorV1::UnreferencedCorrespondence {
            correspondence_index: 1
        })
    );
}

#[test]
fn records_and_nested_references_must_be_canonically_ordered() {
    let mut records = valid_contract_set();
    records.properties.swap(0, 1);
    assert_eq!(
        records.validate(),
        Err(ValidationErrorV1::NonCanonicalOrder {
            section: SectionV1::Properties,
            index: 1,
        })
    );

    let mut references = valid_contract_set();
    if let PropertyEvidenceV1::Proved(record) = &mut references.properties[0].evidence {
        record.trusted_computing_base = vec![tcb_id(31), tcb_id(31)];
    }
    assert_eq!(
        references.validate(),
        Err(ValidationErrorV1::NonCanonicalOrder {
            section: SectionV1::EvidenceTcbReferences,
            index: 1,
        })
    );
}

#[test]
fn bounded_sections_reject_oversized_untrusted_records_first() {
    let mut properties = valid_contract_set();
    properties
        .properties
        .resize(MAX_PROPERTIES_V1 + 1, properties.properties[0].clone());
    assert_eq!(
        properties.validate(),
        Err(ValidationErrorV1::LimitExceeded {
            section: SectionV1::Properties,
            actual: MAX_PROPERTIES_V1 + 1,
            maximum: MAX_PROPERTIES_V1,
        })
    );

    let mut obligations = valid_contract_set();
    obligations
        .obligations
        .resize(MAX_OBLIGATIONS_V1 + 1, obligations.obligations[0]);
    assert_eq!(
        obligations.validate(),
        Err(ValidationErrorV1::LimitExceeded {
            section: SectionV1::Obligations,
            actual: MAX_OBLIGATIONS_V1 + 1,
            maximum: MAX_OBLIGATIONS_V1,
        })
    );

    let mut tcb = valid_contract_set();
    tcb.trusted_computing_base
        .resize(MAX_TCB_ENTRIES_V1 + 1, tcb.trusted_computing_base[0]);
    assert_eq!(
        tcb.validate(),
        Err(ValidationErrorV1::LimitExceeded {
            section: SectionV1::TrustedComputingBase,
            actual: MAX_TCB_ENTRIES_V1 + 1,
            maximum: MAX_TCB_ENTRIES_V1,
        })
    );

    let mut correspondences = valid_contract_set();
    correspondences.correspondences.resize(
        MAX_CORRESPONDENCES_V1 + 1,
        correspondences.correspondences[0],
    );
    assert_eq!(
        correspondences.validate(),
        Err(ValidationErrorV1::LimitExceeded {
            section: SectionV1::Correspondences,
            actual: MAX_CORRESPONDENCES_V1 + 1,
            maximum: MAX_CORRESPONDENCES_V1,
        })
    );

    let mut references = valid_contract_set();
    if let PropertyEvidenceV1::Proved(record) = &mut references.properties[0].evidence {
        record
            .trusted_computing_base
            .resize(MAX_TCB_REFERENCES_PER_EVIDENCE_V1 + 1, tcb_id(31));
    }
    assert_eq!(
        references.validate(),
        Err(ValidationErrorV1::LimitExceeded {
            section: SectionV1::EvidenceTcbReferences,
            actual: MAX_TCB_REFERENCES_PER_EVIDENCE_V1 + 1,
            maximum: MAX_TCB_REFERENCES_PER_EVIDENCE_V1,
        })
    );
}

#[test]
fn every_property_requires_a_satisfied_obligation_at_its_exact_status() {
    let mut contracts = valid_contract_set();
    contracts.obligations.remove(2);

    assert_eq!(
        contracts.validate(),
        Err(ValidationErrorV1::PropertyWithoutExactObligation { property_index: 2 })
    );
}

#[test]
fn open_obligations_are_describable_but_not_closed() {
    let mut contracts = valid_contract_set();
    contracts.obligations.push(obligation(
        56,
        &contracts.properties[0],
        PropertyStatusV1::Checked,
        false,
    ));

    assert_eq!(contracts.validate(), Ok(()));
    assert_eq!(
        contracts.validate_closed(),
        Err(ValidationErrorV1::OpenObligation {
            obligation_index: 5
        })
    );
}

#[test]
fn zero_and_malformed_extension_identities_are_rejected() {
    let mut zero = valid_contract_set();
    zero.properties[0].statement = StatementIdentityV1::from_untrusted_digest(DigestV1::ZERO);
    assert_eq!(
        zero.validate(),
        Err(ValidationErrorV1::InvalidIdentity {
            section: SectionV1::Properties,
            index: 0,
            field: IdentityFieldV1::Statement,
        })
    );

    let mut extension = valid_contract_set();
    extension.properties[0].kind = PropertyKindV1::Extension {
        namespace: DigestV1::ZERO,
        code: 0,
    };
    assert_eq!(
        extension.validate(),
        Err(ValidationErrorV1::InvalidExtensionCode {
            section: SectionV1::Properties,
            index: 0,
        })
    );
}
