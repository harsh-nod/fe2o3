use std::collections::BTreeSet;

use fe2o3_differential::{
    CorpusError, MAX_CASES_PER_FEATURE, MAX_SEMANTIC_CANONICAL_BYTES, SemanticCase,
    SemanticCorpusConfig, SemanticFeature, SemanticSpec, classify_semantic_outcome,
    encode_semantic_case_v1, evaluate_semantic_case, generate_semantic_case,
    generate_semantic_corpus, replay_semantic_case_v1, semantic_replay_identity_v1,
};

#[test]
fn corpus_is_seeded_bounded_deterministic_and_feature_complete() {
    let config = SemanticCorpusConfig::new(8).unwrap();
    let first = generate_semantic_corpus(0x1234_5678, config);
    let second = generate_semantic_corpus(0x1234_5678, config);
    assert_eq!(first, second);
    assert_ne!(first, generate_semantic_corpus(0x1234_5679, config));
    assert_eq!(first.len(), SemanticFeature::ALL.len() * 8);

    let features: BTreeSet<_> = first.iter().map(SemanticCase::feature).collect();
    assert_eq!(features, SemanticFeature::ALL.into_iter().collect());
    for case in &first {
        assert!(encode_semantic_case_v1(case).unwrap().len() <= MAX_SEMANTIC_CANONICAL_BYTES);
    }
}

#[test]
fn generated_corpus_contains_success_and_expected_rejection_for_every_feature() {
    let corpus = generate_semantic_corpus(99, SemanticCorpusConfig::new(8).unwrap());
    for feature in SemanticFeature::ALL {
        let outcomes: Vec<_> = corpus
            .iter()
            .filter(|case| case.feature() == feature)
            .map(evaluate_semantic_case)
            .collect();
        assert!(
            outcomes.iter().any(|outcome| matches!(
                outcome,
                fe2o3_differential::ReferenceOutcome::Execution(_)
            )),
            "{feature:?} has no supported case"
        );
        assert!(
            outcomes.iter().any(|outcome| matches!(
                outcome,
                fe2o3_differential::ReferenceOutcome::CompileRejection(_)
            )),
            "{feature:?} has no rejection case"
        );
    }
}

#[test]
fn replay_identity_round_trips_and_rejects_all_identity_field_mutations() {
    let case = generate_semantic_case(0xa5a5, SemanticFeature::AtomicScopes, 3).unwrap();
    let identity = semantic_replay_identity_v1(&case).unwrap();
    assert_eq!(replay_semantic_case_v1(identity).unwrap(), case);

    let mut mutations = Vec::new();
    let mut version = identity;
    version.corpus_version = version.corpus_version.wrapping_add(1);
    mutations.push(version);
    let mut seed = identity;
    seed.seed ^= 1;
    mutations.push(seed);
    let mut feature = identity;
    feature.feature = SemanticFeature::IntegerSwitch;
    mutations.push(feature);
    let mut ordinal = identity;
    ordinal.ordinal ^= 1;
    mutations.push(ordinal);
    for byte in 0..identity.canonical_fingerprint.len() {
        let mut fingerprint = identity;
        fingerprint.canonical_fingerprint[byte] ^= 1;
        mutations.push(fingerprint);
    }

    for mutation in mutations {
        assert!(replay_semantic_case_v1(mutation).is_err());
    }
}

#[test]
fn replay_identity_rejects_a_hand_substituted_case() {
    let generated = generate_semantic_case(7, SemanticFeature::IntegerSwitch, 0).unwrap();
    let SemanticSpec::IntegerSwitch(mut specification) = generated.specification().clone() else {
        unreachable!()
    };
    specification.default ^= 1;
    let substituted = SemanticCase::new(
        generated.seed(),
        generated.ordinal(),
        generated.feature(),
        SemanticSpec::IntegerSwitch(specification),
    )
    .unwrap();
    assert_eq!(
        semantic_replay_identity_v1(&substituted),
        Err(CorpusError::CaseNotGenerated)
    );
}

#[test]
fn generator_configuration_and_coordinates_fail_closed() {
    assert_eq!(
        SemanticCorpusConfig::new(0),
        Err(CorpusError::InvalidCasesPerFeature { actual: 0 })
    );
    assert_eq!(
        SemanticCorpusConfig::new(MAX_CASES_PER_FEATURE + 1),
        Err(CorpusError::InvalidCasesPerFeature {
            actual: MAX_CASES_PER_FEATURE + 1,
        })
    );
    assert_eq!(
        generate_semantic_case(
            0,
            SemanticFeature::PointerDistance,
            u16::from(MAX_CASES_PER_FEATURE)
        ),
        Err(CorpusError::OrdinalOutOfRange {
            ordinal: u16::from(MAX_CASES_PER_FEATURE),
        })
    );
}

#[test]
fn canonical_encoding_changes_for_semantic_mutations() {
    let corpus = generate_semantic_corpus(123, SemanticCorpusConfig::new(8).unwrap());
    for window in corpus.windows(2) {
        assert_ne!(
            encode_semantic_case_v1(&window[0]).unwrap(),
            encode_semantic_case_v1(&window[1]).unwrap()
        );
    }
}

#[test]
fn cpu_oracle_is_sufficient_to_classify_generated_reference_runs() {
    let corpus = generate_semantic_corpus(456, SemanticCorpusConfig::default());
    for case in corpus {
        let backend = match evaluate_semantic_case(&case) {
            fe2o3_differential::ReferenceOutcome::Execution(observation) => {
                fe2o3_differential::BackendOutcome::Execution(observation)
            }
            fe2o3_differential::ReferenceOutcome::CompileRejection(reason) => {
                fe2o3_differential::BackendOutcome::CompileRejection(reason)
            }
        };
        assert!(matches!(
            classify_semantic_outcome(&case, backend),
            fe2o3_differential::ConformanceOutcome::SupportedPass
                | fe2o3_differential::ConformanceOutcome::ExpectedCompileRejection
        ));
    }
}
