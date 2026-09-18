use super::*;
use sha2::{Digest as _, Sha256};

struct Fixture {
    function: SemanticFunctionIdV1,
    identity: SemanticFunctionIdentityV1,
    sources: RetainedRawBodySourceProducerV1,
    raw_to_semantic: [SemanticLocalIdV1; 3],
    locals: [RetainedSemanticLocalProducerV1; 3],
    inserted: ReceiverLocalV1,
}

impl Fixture {
    fn new() -> Self {
        let function = SemanticFunctionIdV1::from_index(4);
        let identity = SemanticFunctionIdentityV1::from_sha256([17; 32]);
        let source = RetainedSemanticSourceProducerV1 {
            provenance: SemanticSourceProvenanceV1::unavailable(),
            expansion_chain_sha256: [55; 32],
        };
        let mut next_to_last = [u8::MAX; 32];
        next_to_last[31] -= 1;
        let locals =
            [(2, [0; 32]), (0, next_to_last), (1, [u8::MAX; 32])].map(|(rustc_local, identity)| {
                RetainedSemanticLocalProducerV1 {
                    identity: SemanticLocalIdentityV1::from_sha256(identity),
                    rustc_local,
                    ty: SemanticTypeIdV1::from_index(0),
                    source,
                }
            });
        let inserted = ReceiverLocalV1::derive(
            identity,
            SemanticBlockIdentityV1::from_sha256([34; 32]),
            locals.iter().map(|local| local.identity),
        )
        .unwrap();
        assert_eq!(inserted.local.index(), 1);
        let variables = [
            ("below", RetainedRawDebugSourceVariableClassV2::Local(2)),
            ("at", RetainedRawDebugSourceVariableClassV2::Local(0)),
            ("above", RetainedRawDebugSourceVariableClassV2::Local(1)),
            (
                "unrepresented",
                RetainedRawDebugSourceVariableClassV2::Unrepresented,
            ),
        ]
        .into_iter()
        .enumerate()
        .map(
            |(ordinal, (name, class))| RetainedRawDebugSourceVariableV2 {
                ordinal: u32::try_from(ordinal).unwrap(),
                exact_name: Some(name.to_owned()),
                name_sha256: Sha256::digest(name.as_bytes()).into(),
                raw_scope: u32::from(ordinal != 3),
                class,
                entry_value_preserved: matches!(ordinal, 0 | 2),
            },
        )
        .collect();
        Self {
            function,
            identity,
            sources: RetainedRawBodySourceProducerV1 {
                source,
                locals: Box::default(),
                blocks: Box::default(),
                debug_scopes: vec![
                    RetainedRawDebugSourceScopeV2 {
                        raw_scope: 0,
                        parent_raw_scope: None,
                        depth: 0,
                        source,
                    },
                    RetainedRawDebugSourceScopeV2 {
                        raw_scope: 1,
                        parent_raw_scope: Some(0),
                        depth: 1,
                        source,
                    },
                ]
                .into_boxed_slice(),
                debug_variables: variables,
                debug_capture_gap: None,
            },
            raw_to_semantic: [1, 2, 0].map(SemanticLocalIdV1::from_index),
            locals,
            inserted,
        }
    }

    fn convert(
        &self,
        inserted: Option<ReceiverLocalV1>,
        counts: &mut RawMirPreflightCountsV1,
        limits: SemanticMirLimitsV1,
    ) -> OptionalDebugSourcesV2 {
        convert_receiver_debug_sources_v2(
            self.function,
            self.identity,
            &self.sources,
            &self.raw_to_semantic,
            &self.locals,
            inserted,
            (counts, limits),
        )
    }
}

fn work_limits(maximum: u64) -> SemanticMirLimitsV1 {
    SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::ValidationWork, maximum)
        .unwrap()
}

#[test]
fn nonempty_debug_remap_shifts_only_emitted_ids_after_logical_identity_conversion() {
    let fixture = Fixture::new();
    let (original_scopes, original_variables) = convert_debug_sources_v2(
        fixture.function,
        fixture.identity,
        &fixture.sources.debug_scopes,
        &fixture.sources.debug_variables,
        &fixture.raw_to_semantic,
        &fixture.locals,
    )
    .unwrap();
    let mut counts = RawMirPreflightCountsV1 {
        validation_work: 3,
        ..RawMirPreflightCountsV1::default()
    };
    let (scopes, variables, gap) =
        fixture.convert(Some(fixture.inserted), &mut counts, work_limits(7));
    assert!(gap.is_none());
    assert_eq!(
        counts,
        RawMirPreflightCountsV1 {
            validation_work: 7,
            ..RawMirPreflightCountsV1::default()
        }
    );
    assert_eq!(scopes.len(), 2);
    assert_eq!(scopes.len(), original_scopes.len());
    for (scope, original) in scopes.iter().zip(&original_scopes) {
        assert_eq!(scope.identity, original.identity);
        assert_eq!(scope.function, original.function);
        assert_eq!(scope.parent_identity, original.parent_identity);
        assert_eq!(scope.depth, original.depth);
        assert_eq!(scope.source, original.source);
    }
    assert_eq!(variables.len(), 4);
    assert_eq!(variables.len(), original_variables.len());
    let local =
        |index| RetainedDebugSourceVariableClassV2::Local(SemanticLocalIdV1::from_index(index));
    let unrepresented = RetainedDebugSourceVariableClassV2::Unrepresented;
    assert_eq!(
        original_variables
            .iter()
            .map(|variable| variable.class)
            .collect::<Vec<_>>(),
        [local(0), local(1), local(2), unrepresented],
    );
    assert_eq!(
        variables
            .iter()
            .map(|variable| variable.class)
            .collect::<Vec<_>>(),
        [local(0), local(2), local(3), unrepresented],
    );
    for (variable, original) in variables.iter().zip(&original_variables) {
        assert_eq!(variable.identity, original.identity);
        assert_eq!(variable.function, original.function);
        assert_eq!(variable.name, original.name);
        assert_eq!(variable.scope_identity, original.scope_identity);
        assert_eq!(
            variable.entry_value_preserved,
            original.entry_value_preserved
        );
        assert_ne!(variable.class, local(fixture.inserted.local.index()));
    }
    assert_eq!(
        fixture.raw_to_semantic,
        [1, 2, 0].map(SemanticLocalIdV1::from_index)
    );
}

#[test]
fn optional_remap_exhaustion_and_overflow_discard_all_debug_records() {
    let fixture = Fixture::new();
    for (used, expected) in [(3, 7), (u64::MAX, u64::MAX)] {
        let mut counts = RawMirPreflightCountsV1 {
            validation_work: used,
            ..RawMirPreflightCountsV1::default()
        };
        let (scopes, variables, gap) =
            fixture.convert(Some(fixture.inserted), &mut counts, work_limits(6));
        assert_eq!(
            gap,
            Some(ProductionSemanticDebugProducerGapV1::ResourceLimit)
        );
        assert!(scopes.is_empty());
        assert!(variables.is_empty());
        assert_eq!(counts.validation_work, expected);
    }
}

#[test]
fn optional_remap_budget_continues_across_bodies() {
    let fixture = Fixture::new();
    let mut counts = RawMirPreflightCountsV1 {
        validation_work: 3,
        ..RawMirPreflightCountsV1::default()
    };
    let (_, variables, gap) = fixture.convert(Some(fixture.inserted), &mut counts, work_limits(10));
    assert!(gap.is_none());
    assert_eq!(variables.len(), 4);
    assert_eq!(counts.validation_work, 7);
    let (scopes, variables, gap) =
        fixture.convert(Some(fixture.inserted), &mut counts, work_limits(10));
    assert_eq!(
        gap,
        Some(ProductionSemanticDebugProducerGapV1::ResourceLimit)
    );
    assert!(scopes.is_empty());
    assert!(variables.is_empty());
    assert_eq!(counts.validation_work, 11);
}

#[test]
fn no_receiver_insertion_spends_no_remap_work() {
    let fixture = Fixture::new();
    let mut counts = RawMirPreflightCountsV1::default();
    let (scopes, variables, gap) = fixture.convert(None, &mut counts, work_limits(0));
    assert!(gap.is_none());
    assert_eq!(scopes.len(), 2);
    assert_eq!(variables.len(), 4);
    assert_eq!(counts, RawMirPreflightCountsV1::default());
    for (index, variable) in variables[..3].iter().enumerate() {
        assert_eq!(
            variable.class,
            RetainedDebugSourceVariableClassV2::Local(SemanticLocalIdV1::from_index(index as u32)),
        );
    }
}

#[test]
fn prior_optional_capture_gap_is_preserved_without_remapping() {
    let mut fixture = Fixture::new();
    fixture.sources.debug_capture_gap =
        Some(ProductionSemanticDebugProducerGapV1::SourceObservationUnrepresentable);
    let mut counts = RawMirPreflightCountsV1::default();
    let (scopes, variables, gap) =
        fixture.convert(Some(fixture.inserted), &mut counts, work_limits(0));
    assert_eq!(gap, fixture.sources.debug_capture_gap);
    assert!(scopes.is_empty());
    assert!(variables.is_empty());
    assert_eq!(counts, RawMirPreflightCountsV1::default());
}

#[test]
fn invalid_debug_mapping_retains_the_existing_typed_gap() {
    let mut fixture = Fixture::new();
    fixture.raw_to_semantic[2] = SemanticLocalIdV1::from_index(3);
    let mut counts = RawMirPreflightCountsV1::default();
    let (scopes, variables, gap) =
        fixture.convert(Some(fixture.inserted), &mut counts, work_limits(4));
    assert_eq!(
        gap,
        Some(ProductionSemanticDebugProducerGapV1::ResourceLimit)
    );
    assert!(scopes.is_empty());
    assert!(variables.is_empty());
    assert_eq!(counts, RawMirPreflightCountsV1::default());
}
