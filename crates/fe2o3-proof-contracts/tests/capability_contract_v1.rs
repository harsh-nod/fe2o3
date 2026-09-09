use fe2o3_proof_contracts::{
    ArtifactIdentityV1, CapabilityAnalysisReportIdentityV1, CapabilityCheckerIdentityV1,
    CapabilityCodecErrorV1, CapabilityCompositionErrorV1, CapabilityDiagnosticIdV1,
    CapabilityObligationSpecV1, CapabilityOutcomeKindV1, CapabilityOutcomeV1,
    CapabilityPropertyIdV1, CapabilityRecordKindV1, CapabilityRefinementKindV1,
    CapabilityRefinementReceiptIdentityV1, CapabilityResourceV1, CapabilityResultSpecV1,
    CapabilitySubjectFieldV1, CapabilitySubjectV1, DigestV1, EvidenceIdentityV1,
    ExactToolIdentityV1, ExecutableKirIdentityV1, InertCapabilityObligationSetV1,
    InertCapabilityResultSetV1, KernelIdentityV1, KernelRootIdentityV1, LaunchContractIdentityV1,
    MAX_CAPABILITY_OBLIGATIONS_V1, MAX_CAPABILITY_REJECTED_WITNESS_BYTES_V1,
    MAX_CAPABILITY_RESULT_SET_BYTES_V1, MAX_CAPABILITY_WITNESS_BYTES_V1, StatementIdentityV1,
    TargetModelIdentityV1, validate_capability_composition_v1,
};

fn digest(byte: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([byte; 32])
}

fn subject_with(
    kernel: u8,
    root: u8,
    kir: u8,
    epoch: u64,
    target: u8,
    launch: u8,
) -> CapabilitySubjectV1 {
    CapabilitySubjectV1::new(
        KernelIdentityV1::from_untrusted_digest(digest(kernel)),
        KernelRootIdentityV1::from_untrusted_digest(digest(root)),
        ExecutableKirIdentityV1::from_untrusted_digest(digest(kir)),
        epoch,
        TargetModelIdentityV1::from_untrusted_digest(digest(target)),
        LaunchContractIdentityV1::from_untrusted_digest(digest(launch)),
    )
    .unwrap()
}

fn subject() -> CapabilitySubjectV1 {
    subject_with(1, 2, 3, 7, 4, 5)
}

fn statement(byte: u8) -> StatementIdentityV1 {
    StatementIdentityV1::from_untrusted_digest(digest(byte))
}

fn artifact(byte: u8) -> ArtifactIdentityV1 {
    ArtifactIdentityV1::new(digest(byte), digest(byte + 1))
}

fn obligations(subject: CapabilitySubjectV1) -> InertCapabilityObligationSetV1 {
    InertCapabilityObligationSetV1::from_specs(
        subject,
        vec![
            CapabilityObligationSpecV1::new(CapabilityPropertyIdV1::BOUNDS, statement(20)),
            CapabilityObligationSpecV1::new(
                CapabilityPropertyIdV1::BARRIER_CONVERGENCE,
                statement(21),
            ),
            CapabilityObligationSpecV1::new(
                CapabilityPropertyIdV1::TARGET_CAPABILITY_CLOSURE,
                statement(22),
            ),
            CapabilityObligationSpecV1::new(
                CapabilityPropertyIdV1::SOURCE_MIR_TO_KIR_REFINEMENT,
                statement(23),
            ),
            CapabilityObligationSpecV1::new(
                CapabilityPropertyIdV1::MACHINE_REFINEMENT,
                statement(24),
            ),
        ],
    )
    .unwrap()
}

fn outcomes() -> Vec<CapabilityOutcomeV1> {
    vec![
        CapabilityOutcomeV1::Proven {
            evidence: EvidenceIdentityV1::from_untrusted_digest(digest(40)),
            tool: ExactToolIdentityV1::new(digest(41), digest(42)),
            proof_artifact: artifact(43),
        },
        CapabilityOutcomeV1::Rejected {
            diagnostic: CapabilityDiagnosticIdV1::REJECTED_WITH_WITNESS,
            witness: b"block=3,operation=8".to_vec(),
        },
        CapabilityOutcomeV1::Incomplete {
            diagnostic: CapabilityDiagnosticIdV1::INCOMPLETE_ANALYSIS,
            detail: artifact(46),
        },
        CapabilityOutcomeV1::Unsupported {
            diagnostic: CapabilityDiagnosticIdV1::UNSUPPORTED_CAPABILITY,
            detail: artifact(48),
        },
        CapabilityOutcomeV1::Unreviewed {
            diagnostic: CapabilityDiagnosticIdV1::UNREVIEWED_CAPABILITY,
            detail: artifact(50),
        },
    ]
}

fn results(
    subject: CapabilitySubjectV1,
    obligations: &InertCapabilityObligationSetV1,
) -> InertCapabilityResultSetV1 {
    let specs = obligations
        .obligations()
        .iter()
        .zip(outcomes())
        .map(|(obligation, outcome)| CapabilityResultSpecV1::new(obligation.identity(), outcome))
        .collect();
    InertCapabilityResultSetV1::from_specs(subject, obligations.identity(), specs).unwrap()
}

#[test]
fn canonical_roundtrip_binds_all_five_outcomes() {
    let obligations = obligations(subject());
    let results = results(subject(), &obligations);

    let decoded_obligations =
        InertCapabilityObligationSetV1::decode_canonical(obligations.canonical_bytes()).unwrap();
    let decoded_results =
        InertCapabilityResultSetV1::decode_canonical(results.canonical_bytes()).unwrap();

    assert_eq!(decoded_obligations, obligations);
    assert_eq!(decoded_results, results);
    assert_eq!(decoded_results.schema_version(), 1);
    assert_eq!(
        decoded_results
            .results()
            .iter()
            .map(|result| result.outcome().kind())
            .collect::<Vec<_>>(),
        vec![
            CapabilityOutcomeKindV1::Proven,
            CapabilityOutcomeKindV1::Rejected,
            CapabilityOutcomeKindV1::Incomplete,
            CapabilityOutcomeKindV1::Unsupported,
            CapabilityOutcomeKindV1::Unreviewed,
        ]
    );
    assert_eq!(
        validate_capability_composition_v1(subject(), &obligations, &results),
        Ok(())
    );
}

#[test]
fn checked_and_refinement_receipt_are_distinct_v2_outcomes() {
    let subject = subject();
    let obligations = InertCapabilityObligationSetV1::from_specs(
        subject,
        vec![
            CapabilityObligationSpecV1::new(CapabilityPropertyIdV1::BOUNDS, statement(60)),
            CapabilityObligationSpecV1::new(
                CapabilityPropertyIdV1::MACHINE_REFINEMENT,
                statement(61),
            ),
        ],
    )
    .unwrap();
    let specs: Vec<_> = obligations
        .obligations()
        .iter()
        .map(|obligation| {
            let outcome = if obligation.property() == CapabilityPropertyIdV1::BOUNDS {
                CapabilityOutcomeV1::Checked {
                    evidence: EvidenceIdentityV1::from_untrusted_digest(digest(62)),
                    checker: CapabilityCheckerIdentityV1::from_untrusted_digest(digest(63)),
                    report: CapabilityAnalysisReportIdentityV1::from_untrusted_digest(digest(64)),
                    executable_kir: subject.executable_kir(),
                    executable_kir_epoch: subject.executable_kir_epoch(),
                    analysis_epoch: 9,
                }
            } else {
                CapabilityOutcomeV1::RefinementReceipt {
                    kind: CapabilityRefinementKindV1::Machine,
                    receipt: CapabilityRefinementReceiptIdentityV1::from_untrusted_parts(
                        digest(65),
                        4096,
                    ),
                }
            };
            CapabilityResultSpecV1::new(obligation.identity(), outcome)
        })
        .collect();
    assert!(matches!(
        InertCapabilityResultSetV1::from_specs(subject, obligations.identity(), specs.clone(),),
        Err(CapabilityCodecErrorV1::UnknownOutcome { tag: 6 | 7, .. })
    ));
    let results =
        InertCapabilityResultSetV1::from_specs_v2(subject, obligations.identity(), specs).unwrap();
    assert_eq!(results.schema_version(), 2);
    let decoded = InertCapabilityResultSetV1::decode_canonical(results.canonical_bytes()).unwrap();
    assert!(
        decoded
            .results()
            .iter()
            .any(|result| { result.outcome().kind() == CapabilityOutcomeKindV1::Checked })
    );
    assert!(
        decoded.results().iter().any(|result| {
            result.outcome().kind() == CapabilityOutcomeKindV1::RefinementReceipt
        })
    );

    let mut downgraded = results.canonical_bytes().to_vec();
    downgraded[8..10].copy_from_slice(&1_u16.to_le_bytes());
    assert!(matches!(
        InertCapabilityResultSetV1::decode_canonical(&downgraded),
        Err(CapabilityCodecErrorV1::UnknownOutcome { tag: 6 | 7, .. })
    ));
}

#[test]
fn checked_result_identity_binds_checker_report_graph_and_both_epochs() {
    let subject = subject();
    let obligations = InertCapabilityObligationSetV1::from_specs(
        subject,
        vec![CapabilityObligationSpecV1::new(
            CapabilityPropertyIdV1::BOUNDS,
            statement(70),
        )],
    )
    .unwrap();
    let make = |checker, report, kir, final_epoch, analysis_epoch| {
        InertCapabilityResultSetV1::from_specs_v2(
            subject,
            obligations.identity(),
            vec![CapabilityResultSpecV1::new(
                obligations.obligations()[0].identity(),
                CapabilityOutcomeV1::Checked {
                    evidence: EvidenceIdentityV1::from_untrusted_digest(digest(71)),
                    checker: CapabilityCheckerIdentityV1::from_untrusted_digest(digest(checker)),
                    report: CapabilityAnalysisReportIdentityV1::from_untrusted_digest(digest(
                        report,
                    )),
                    executable_kir: ExecutableKirIdentityV1::from_untrusted_digest(digest(kir)),
                    executable_kir_epoch: final_epoch,
                    analysis_epoch,
                },
            )],
        )
        .unwrap()
        .identity()
    };
    let exact = make(72, 73, 3, 7, 9);
    for substituted in [
        make(74, 73, 3, 7, 9),
        make(72, 74, 3, 7, 9),
        make(72, 73, 4, 7, 9),
        make(72, 73, 3, 8, 9),
        make(72, 73, 3, 7, 10),
    ] {
        assert_ne!(exact, substituted);
    }
}

#[test]
fn construction_is_deterministic_across_input_order() {
    let subject = subject();
    let first = obligations(subject);
    let mut reversed: Vec<_> = first
        .obligations()
        .iter()
        .map(|record| CapabilityObligationSpecV1::new(record.property(), record.statement()))
        .collect();
    reversed.reverse();
    let second = InertCapabilityObligationSetV1::from_specs(subject, reversed).unwrap();

    assert_eq!(first.identity(), second.identity());
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
}

#[test]
fn exact_composition_rejects_every_subject_axis() {
    let base = subject();
    let obligations = obligations(base);
    let base_results = results(base, &obligations);
    let substitutions = [
        (
            subject_with(9, 2, 3, 7, 4, 5),
            CapabilitySubjectFieldV1::Kernel,
        ),
        (
            subject_with(1, 9, 3, 7, 4, 5),
            CapabilitySubjectFieldV1::Root,
        ),
        (
            subject_with(1, 2, 9, 7, 4, 5),
            CapabilitySubjectFieldV1::ExecutableKir,
        ),
        (
            subject_with(1, 2, 3, 8, 4, 5),
            CapabilitySubjectFieldV1::ExecutableKirEpoch,
        ),
        (
            subject_with(1, 2, 3, 7, 9, 5),
            CapabilitySubjectFieldV1::TargetModel,
        ),
        (
            subject_with(1, 2, 3, 7, 4, 9),
            CapabilitySubjectFieldV1::LaunchContract,
        ),
    ];

    for (substitution, field) in substitutions {
        assert_eq!(
            validate_capability_composition_v1(substitution, &obligations, &base_results),
            Err(CapabilityCompositionErrorV1::ObligationSubjectMismatch(
                field
            ))
        );
    }

    let other_results = results(subject_with(1, 2, 3, 8, 4, 5), &obligations);
    assert_eq!(
        validate_capability_composition_v1(base, &obligations, &other_results),
        Err(CapabilityCompositionErrorV1::ResultSubjectMismatch(
            CapabilitySubjectFieldV1::ExecutableKirEpoch
        ))
    );
}

#[test]
fn omission_and_cross_obligation_set_substitution_fail_closed() {
    let subject = subject();
    let obligations = obligations(subject);
    let one = &obligations.obligations()[0];
    let omitted = InertCapabilityResultSetV1::from_specs(
        subject,
        obligations.identity(),
        vec![CapabilityResultSpecV1::new(
            one.identity(),
            outcomes().remove(0),
        )],
    )
    .unwrap();
    assert_eq!(
        validate_capability_composition_v1(subject, &obligations, &omitted),
        Err(CapabilityCompositionErrorV1::ResultCountMismatch {
            expected: 5,
            actual: 1,
        })
    );

    let other = InertCapabilityObligationSetV1::from_specs(
        subject,
        vec![CapabilityObligationSpecV1::new(
            CapabilityPropertyIdV1::BOUNDS,
            statement(90),
        )],
    )
    .unwrap();
    let spliced = results(subject, &other);
    assert_eq!(
        validate_capability_composition_v1(subject, &obligations, &spliced),
        Err(CapabilityCompositionErrorV1::ObligationSetIdentityMismatch)
    );
}

#[test]
fn duplicate_properties_and_results_are_rejected() {
    let subject = subject();
    assert!(matches!(
        InertCapabilityObligationSetV1::from_specs(
            subject,
            vec![
                CapabilityObligationSpecV1::new(CapabilityPropertyIdV1::BOUNDS, statement(20),),
                CapabilityObligationSpecV1::new(CapabilityPropertyIdV1::BOUNDS, statement(21),),
            ],
        ),
        Err(CapabilityCodecErrorV1::DuplicateProperty { .. })
    ));

    let obligations = obligations(subject);
    let obligation = obligations.obligations()[0].identity();
    assert_eq!(
        InertCapabilityResultSetV1::from_specs(
            subject,
            obligations.identity(),
            vec![
                CapabilityResultSpecV1::new(obligation, outcomes().remove(0)),
                CapabilityResultSpecV1::new(obligation, outcomes().remove(0)),
            ],
        ),
        Err(CapabilityCodecErrorV1::DuplicateResult { index: 1 })
    );
}

#[test]
fn duplicate_and_reordered_wire_records_are_rejected_before_set_identity() {
    let subject = subject();
    let obligations = InertCapabilityObligationSetV1::from_specs(
        subject,
        vec![
            CapabilityObligationSpecV1::new(CapabilityPropertyIdV1::BOUNDS, statement(20)),
            CapabilityObligationSpecV1::new(CapabilityPropertyIdV1::EFFECTS, statement(21)),
        ],
    )
    .unwrap();
    let mut duplicate = obligations.canonical_bytes().to_vec();
    duplicate.copy_within(188..292, 292);
    assert_eq!(
        InertCapabilityObligationSetV1::decode_canonical(&duplicate),
        Err(CapabilityCodecErrorV1::DuplicateObligation { index: 1 })
    );

    let mut reordered = obligations.canonical_bytes().to_vec();
    let first = reordered[188..292].to_vec();
    reordered.copy_within(292..396, 188);
    reordered[292..396].copy_from_slice(&first);
    assert_eq!(
        InertCapabilityObligationSetV1::decode_canonical(&reordered),
        Err(CapabilityCodecErrorV1::NonCanonicalOrder {
            record: CapabilityRecordKindV1::Obligation,
            index: 1,
        })
    );

    let rejected = CapabilityOutcomeV1::Rejected {
        diagnostic: CapabilityDiagnosticIdV1::REJECTED_WITH_WITNESS,
        witness: vec![1],
    };
    let results = InertCapabilityResultSetV1::from_specs(
        subject,
        obligations.identity(),
        obligations
            .obligations()
            .iter()
            .map(|obligation| CapabilityResultSpecV1::new(obligation.identity(), rejected.clone()))
            .collect(),
    )
    .unwrap();
    let mut duplicate = results.canonical_bytes().to_vec();
    duplicate.copy_within(220..333, 333);
    assert_eq!(
        InertCapabilityResultSetV1::decode_canonical(&duplicate),
        Err(CapabilityCodecErrorV1::DuplicateResult { index: 1 })
    );
}

#[test]
fn changing_a_disposition_changes_exact_result_identities() {
    let subject = subject();
    let obligations = obligations(subject);
    let proven = InertCapabilityResultSetV1::from_specs(
        subject,
        obligations.identity(),
        vec![CapabilityResultSpecV1::new(
            obligations.obligations()[0].identity(),
            outcomes().remove(0),
        )],
    )
    .unwrap();
    let incomplete = InertCapabilityResultSetV1::from_specs(
        subject,
        obligations.identity(),
        vec![CapabilityResultSpecV1::new(
            obligations.obligations()[0].identity(),
            CapabilityOutcomeV1::Incomplete {
                diagnostic: CapabilityDiagnosticIdV1::INCOMPLETE_ANALYSIS,
                detail: artifact(90),
            },
        )],
    )
    .unwrap();

    assert_ne!(
        proven.results()[0].identity(),
        incomplete.results()[0].identity()
    );
    assert_ne!(proven.identity(), incomplete.identity());
}

#[test]
fn record_and_terminal_identity_mutation_is_rejected() {
    let obligations = obligations(subject());
    let mut obligation_record = obligations.canonical_bytes().to_vec();
    obligation_record[188 + 40] ^= 1;
    assert!(matches!(
        InertCapabilityObligationSetV1::decode_canonical(&obligation_record),
        Err(CapabilityCodecErrorV1::IdentityMismatch {
            record: CapabilityRecordKindV1::Obligation,
            ..
        })
    ));

    let mut obligation_terminal = obligations.canonical_bytes().to_vec();
    *obligation_terminal.last_mut().unwrap() ^= 1;
    assert_eq!(
        InertCapabilityObligationSetV1::decode_canonical(&obligation_terminal),
        Err(CapabilityCodecErrorV1::IdentityMismatch {
            record: CapabilityRecordKindV1::ObligationSet,
            index: None,
        })
    );

    let results = results(subject(), &obligations);
    let mut result_record = results.canonical_bytes().to_vec();
    result_record[220 + 32] ^= 1;
    assert!(matches!(
        InertCapabilityResultSetV1::decode_canonical(&result_record),
        Err(CapabilityCodecErrorV1::IdentityMismatch {
            record: CapabilityRecordKindV1::Result,
            ..
        })
    ));

    let mut result_terminal = results.canonical_bytes().to_vec();
    *result_terminal.last_mut().unwrap() ^= 1;
    assert_eq!(
        InertCapabilityResultSetV1::decode_canonical(&result_terminal),
        Err(CapabilityCodecErrorV1::IdentityMismatch {
            record: CapabilityRecordKindV1::ResultSet,
            index: None,
        })
    );
}

#[test]
fn framing_versions_reserved_fields_and_outcome_tags_are_strict() {
    let obligations = obligations(subject());
    let canonical = obligations.canonical_bytes();

    assert_eq!(
        InertCapabilityObligationSetV1::decode_canonical(&canonical[..canonical.len() - 1]),
        Err(CapabilityCodecErrorV1::Truncated)
    );
    let mut trailing = canonical.to_vec();
    trailing.push(0);
    assert_eq!(
        InertCapabilityObligationSetV1::decode_canonical(&trailing),
        Err(CapabilityCodecErrorV1::TrailingBytes)
    );
    let mut version = canonical.to_vec();
    version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        InertCapabilityObligationSetV1::decode_canonical(&version),
        Err(CapabilityCodecErrorV1::UnsupportedVersion {
            record: CapabilityRecordKindV1::ObligationSet,
            version: 2,
        })
    );
    let mut flags = canonical.to_vec();
    flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(
        InertCapabilityObligationSetV1::decode_canonical(&flags),
        Err(CapabilityCodecErrorV1::UnsupportedFlags {
            record: CapabilityRecordKindV1::ObligationSet,
            flags: 1,
        })
    );
    let mut reserved = canonical.to_vec();
    reserved[188 + 34] = 1;
    assert_eq!(
        InertCapabilityObligationSetV1::decode_canonical(&reserved),
        Err(CapabilityCodecErrorV1::NonzeroReserved {
            record: CapabilityRecordKindV1::Obligation,
            index: Some(0),
        })
    );

    let results = results(subject(), &obligations);
    let mut outcome = results.canonical_bytes().to_vec();
    outcome[220 + 64] = 99;
    assert_eq!(
        InertCapabilityResultSetV1::decode_canonical(&outcome),
        Err(CapabilityCodecErrorV1::UnknownOutcome { index: 0, tag: 99 })
    );
}

#[test]
fn record_count_and_witness_limits_are_enforced_before_allocation() {
    let obligations = obligations(subject());
    let mut count = obligations.canonical_bytes().to_vec();
    count[184..188].copy_from_slice(
        &u32::try_from(MAX_CAPABILITY_OBLIGATIONS_V1 + 1)
            .unwrap()
            .to_le_bytes(),
    );
    assert_eq!(
        InertCapabilityObligationSetV1::decode_canonical(&count),
        Err(CapabilityCodecErrorV1::LimitExceeded {
            resource: CapabilityResourceV1::Obligations,
            actual: MAX_CAPABILITY_OBLIGATIONS_V1 + 1,
            maximum: MAX_CAPABILITY_OBLIGATIONS_V1,
        })
    );

    let obligation = obligations.obligations()[0].identity();
    assert_eq!(
        InertCapabilityResultSetV1::from_specs(
            subject(),
            obligations.identity(),
            vec![CapabilityResultSpecV1::new(
                obligation,
                CapabilityOutcomeV1::Rejected {
                    diagnostic: CapabilityDiagnosticIdV1::REJECTED_WITH_WITNESS,
                    witness: vec![0; MAX_CAPABILITY_REJECTED_WITNESS_BYTES_V1 + 1],
                },
            )],
        ),
        Err(CapabilityCodecErrorV1::LimitExceeded {
            resource: CapabilityResourceV1::RejectedWitnessBytes,
            actual: MAX_CAPABILITY_REJECTED_WITNESS_BYTES_V1 + 1,
            maximum: MAX_CAPABILITY_REJECTED_WITNESS_BYTES_V1,
        })
    );

    let many_obligations = InertCapabilityObligationSetV1::from_specs(
        subject(),
        (1..=65)
            .map(|code| {
                CapabilityObligationSpecV1::new(
                    CapabilityPropertyIdV1::new(digest(70), 1, code),
                    statement(u8::try_from(code).unwrap()),
                )
            })
            .collect(),
    )
    .unwrap();
    let specs = many_obligations
        .obligations()
        .iter()
        .map(|obligation| {
            CapabilityResultSpecV1::new(
                obligation.identity(),
                CapabilityOutcomeV1::Rejected {
                    diagnostic: CapabilityDiagnosticIdV1::REJECTED_WITH_WITNESS,
                    witness: vec![0; MAX_CAPABILITY_REJECTED_WITNESS_BYTES_V1],
                },
            )
        })
        .collect();
    assert_eq!(
        InertCapabilityResultSetV1::from_specs(subject(), many_obligations.identity(), specs,),
        Err(CapabilityCodecErrorV1::LimitExceeded {
            resource: CapabilityResourceV1::AggregateWitnessBytes,
            actual: MAX_CAPABILITY_WITNESS_BYTES_V1 + MAX_CAPABILITY_REJECTED_WITNESS_BYTES_V1,
            maximum: MAX_CAPABILITY_WITNESS_BYTES_V1,
        })
    );

    assert_eq!(
        InertCapabilityResultSetV1::decode_canonical(&vec![
            0;
            MAX_CAPABILITY_RESULT_SET_BYTES_V1 + 1
        ]),
        Err(CapabilityCodecErrorV1::LimitExceeded {
            resource: CapabilityResourceV1::ResultSetBytes,
            actual: MAX_CAPABILITY_RESULT_SET_BYTES_V1 + 1,
            maximum: MAX_CAPABILITY_RESULT_SET_BYTES_V1,
        })
    );
}

#[test]
fn zero_epoch_and_empty_rejection_witness_are_not_valid_records() {
    assert_eq!(
        CapabilitySubjectV1::new(
            KernelIdentityV1::from_untrusted_digest(digest(1)),
            KernelRootIdentityV1::from_untrusted_digest(digest(2)),
            ExecutableKirIdentityV1::from_untrusted_digest(digest(3)),
            0,
            TargetModelIdentityV1::from_untrusted_digest(digest(4)),
            LaunchContractIdentityV1::from_untrusted_digest(digest(5)),
        ),
        Err(CapabilityCodecErrorV1::InvalidKirEpoch)
    );

    let obligations = obligations(subject());
    assert_eq!(
        InertCapabilityResultSetV1::from_specs(
            subject(),
            obligations.identity(),
            vec![CapabilityResultSpecV1::new(
                obligations.obligations()[0].identity(),
                CapabilityOutcomeV1::Rejected {
                    diagnostic: CapabilityDiagnosticIdV1::REJECTED_WITH_WITNESS,
                    witness: Vec::new(),
                },
            )],
        ),
        Err(CapabilityCodecErrorV1::EmptyWitness { index: 0 })
    );
}
