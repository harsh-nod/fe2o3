//! Signed two-root consistency; public test keys confer no execution authority.
use super::*;
use fe2o3_functional_proof::FunctionalRefinementImportErrorV2;
use fe2o3_kernel_ir::{Constant, OperationKind};
use fe2o3_lower_mir_kernel::{
    NativeSourceReplayErrorV1, ProductionMirPlironTranslationErrorV1, ProductionSemanticKirErrorV1,
};

fn assert_store_values(graph: &VerifiedCanonicalKernelIrModuleV12) {
    assert_eq!(graph.module().kernels.len(), STORES.len());
    for spec in STORES {
        let kernel = graph
            .module()
            .kernels
            .iter()
            .find(|kernel| kernel.id.as_str() == spec.name)
            .unwrap();
        let function = graph
            .module()
            .functions
            .iter()
            .find(|function| function.id == kernel.entry)
            .unwrap();
        let operations: Vec<_> = function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .collect();
        let values: Vec<_> = operations
            .iter()
            .filter_map(|operation| {
                let OperationKind::Store { value, .. } = operation.kind else {
                    return None;
                };
                let definition = operations
                    .iter()
                    .find(|operation| operation.results.iter().any(|result| result.id == value))
                    .unwrap();
                let OperationKind::Constant(Constant::U32(bits)) = definition.kind else {
                    panic!("nonconstant store: {definition:?}")
                };
                Some(bits)
            })
            .collect();
        assert_eq!(values, [spec.value]);
    }
}

fn multi_fixture() -> Fixture {
    fixture_for_stores(&source_for_stores(&STORES), &STORES)
}

fn with_typed_inputs<T>(
    fixture: &Fixture,
    budget: &mut Budget<'_>,
    replay: impl FnOnce(NativeCompilerRankedSourceProofInputsV1<'_>, &mut Budget<'_>) -> T,
) -> T {
    let launch: Vec<_> = fixture
        .roots
        .iter()
        .map(|root| {
            ProductionSourceLaunchRootInputV1::new(
                root.spec.name,
                root.spec.binding,
                ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
            )
        })
        .collect();
    let staging: Vec<_> = fixture
        .roots
        .iter()
        .enumerate()
        .map(|(ordinal, root)| NativeCompilerRootStagingV1 {
            semantic_root: ordinal as u32,
            commitments: std::slice::from_ref(&root.staging),
        })
        .collect();
    let access = access();
    let roots: Vec<_> = fixture
        .roots
        .iter()
        .enumerate()
        .map(|(ordinal, root)| NativeCompilerRankedRootV1 {
            candidate: NativeRankedSourceCandidateV1::from_untrusted_parts(
                ordinal as u32,
                1,
                &root.kernel,
                &access,
                &[],
                TEXT,
            ),
            effect_receipts: std::slice::from_ref(&root.signature),
        })
        .collect();
    replay(
        NativeCompilerRankedSourceProofInputsV1 {
            source: NativeCompilerSourceProofInputsV1 {
                semantic_mir: &fixture.semantic,
                native_module: &fixture.native,
                middle_end_roster: fixture.middle.canonical_bytes(),
                correspondence_roster: fixture.correspondence.canonical_bytes(),
                verus_roster: fixture.verus.canonical_bytes(),
                launch_inputs: &launch,
                staging_roots: &staging,
            },
            ranked_roots: &roots,
        },
        budget,
    )
}

#[test]
fn complete_packet_two_root_recovery_preserves_independent_orders_after_producers_drop() {
    let fixture = multi_fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (proof, baseline) = with_typed_inputs(
        &fixture,
        &mut budget,
        validate_native_compiler_ranked_source_proof_v1,
    )
    .unwrap();
    assert_eq!(proof.root_count(), 2);
    drop(proof);
    let durable = DurableFixture::capture(fixture);
    assert_ne!(durable.roots[0].recipe, durable.roots[1].recipe);
    assert_ne!(
        durable.roots[0].signature.wire(),
        durable.roots[1].signature.wire()
    );
    assert_ne!(durable.roots[0].staging, durable.roots[1].staging);
    // Source coordinates are local to each root, not global root identifiers.
    assert_eq!(durable.roots[0].source_rows, durable.roots[1].source_rows);
    for roster in [&durable.middle, &durable.correspondence, &durable.verus] {
        assert_eq!(roster.canonical_kernel_order(), &[1, 0]);
        assert_ne!(
            roster.root(0).unwrap().payload(),
            roster.root(1).unwrap().payload()
        );
        for (ordinal, spec) in STORES.iter().enumerate() {
            let root = roster.root(ordinal).unwrap();
            assert_eq!(root.semantic_root(), ordinal as u32);
            assert_eq!(root.kernel_binding(), spec.binding);
            assert_eq!(root.logical_name(), spec.name);
        }
    }
    let (proof, recipe_storage) = durable
        .with_inputs(&mut budget, |source, ranked_roots, budget| {
            validate_native_compiler_ranked_recipe_source_proof_v1(
                NativeCompilerRankedRecipeSourceProofInputsV1 {
                    source,
                    ranked_roots,
                },
                budget,
            )
        })
        .unwrap();
    assert_eq!(recipe_storage, baseline);
    drop(proof);
    let expected = (
        durable.semantic.clone(),
        durable.native_graph.clone(),
        durable.middle.canonical_bytes().to_vec(),
        durable.correspondence.canonical_bytes().to_vec(),
        durable.verus.canonical_bytes().to_vec(),
        NativeNeutralModuleRefV1::decode(&durable.native)
            .unwrap()
            .catalog_bytes()
            .to_vec(),
    );
    let (bytes, wire_storage) = durable.packet(None, &mut budget).unwrap();
    drop(durable);
    budget
        .reserve_storage(37 + wire_storage.retained_storage())
        .unwrap();
    assert!(budget.reserve_storage(usize::MAX).is_err());
    let floor = budget.storage();
    let (checked, storage) =
        validate_native_compiler_ranked_source_packet_v1(&bytes, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(storage, baseline);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    drop(bytes);
    budget
        .release_storage(wire_storage.retained_storage())
        .unwrap();
    assert_eq!(checked.root_count(), 2);
    let source = checked.source().source();
    assert_eq!(
        source.semantic().semantic().canonical_encoding(),
        expected.0
    );
    assert_eq!(
        source
            .pre_ranked_executable()
            .unwrap()
            .canonical()
            .canonical_bytes(),
        expected.1
    );
    assert_eq!(checked.middle_end_roster().canonical_bytes(), expected.2);
    assert_eq!(
        checked.correspondence_roster().canonical_bytes(),
        expected.3
    );
    assert_eq!(checked.verus_roster().canonical_bytes(), expected.4);
    assert_eq!(checked.source().catalog().canonical_bytes(), expected.5);
    assert_store_values(source.pre_ranked_executable().unwrap());
    source.verify_equivalence().unwrap();
    assert!(!checked.authenticates_compiler_or_launch_origin());
    assert!(!checked.grants_artifact_or_launch_authority());
    drop(checked);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 37);
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
}

#[test]
fn complete_packet_two_root_unit_local_recovers_n_e_and_signed_rosters() {
    let (fixture, producer) = unit_local_erased_fixture::unit_fixture_for_stores(&STORES);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(producer.retained_storage_floor_v1())
        .unwrap();
    let (proof, baseline) = with_typed_inputs(&fixture, &mut budget, |inputs, budget| {
        validate_native_compiler_unit_local_erased_source_proof_v1(
            NativeCompilerUnitLocalErasedSourceProofInputsV1 {
                original: inputs.source,
                ranked_roots: inputs.ranked_roots,
                erased: producer.erased(),
            },
            budget,
        )
    })
    .unwrap();
    assert_eq!(proof.root_count(), 2);
    drop(proof);
    let erased = producer.erased().canonical().canonical_bytes().to_vec();
    let producer_storage = producer.retained_storage_floor_v1();
    drop(producer);
    budget.release_storage(producer_storage).unwrap();
    let durable = DurableFixture::capture(fixture);
    assert_ne!(durable.native_graph, erased);
    let expected = (
        durable.semantic.clone(),
        durable.native_graph.clone(),
        durable.middle.canonical_bytes().to_vec(),
        durable.correspondence.canonical_bytes().to_vec(),
        durable.verus.canonical_bytes().to_vec(),
        NativeNeutralModuleRefV1::decode(&durable.native)
            .unwrap()
            .catalog_bytes()
            .to_vec(),
    );
    let (bytes, wire_storage) = durable.packet(Some(&erased), &mut budget).unwrap();
    drop(durable);
    budget
        .reserve_storage(37 + wire_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    let (checked, storage) =
        validate_native_compiler_unit_local_erased_source_packet_v1(&bytes, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(storage, baseline);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    drop(bytes);
    budget
        .release_storage(wire_storage.retained_storage())
        .unwrap();
    assert_eq!(checked.root_count(), 2);
    let source = checked.source().source();
    assert_eq!(
        source
            .original_source()
            .semantic_ssa()
            .source_semantic()
            .canonical_encoding(),
        expected.0
    );
    assert_eq!(
        source
            .original_source()
            .executable()
            .canonical()
            .canonical_bytes(),
        expected.1
    );
    assert_eq!(source.erased().canonical().canonical_bytes(), erased);
    assert_eq!(checked.middle_end_roster().canonical_bytes(), expected.2);
    assert_eq!(
        checked.correspondence_roster().canonical_bytes(),
        expected.3
    );
    assert_eq!(checked.verus_roster().canonical_bytes(), expected.4);
    assert_eq!(checked.source().catalog().canonical_bytes(), expected.5);
    assert_store_values(source.original_source().executable());
    assert_store_values(source.erased());
    for roster in [
        checked.middle_end_roster(),
        checked.correspondence_roster(),
        checked.verus_roster(),
    ] {
        assert_eq!(roster.canonical_kernel_order(), &[1, 0]);
    }
    source.verify_equivalence(&mut budget).unwrap();
    assert!(!checked.authenticates_compiler_or_launch_origin());
    assert!(!checked.grants_artifact_or_launch_authority());
    drop(checked);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn complete_packet_two_root_rejects_cross_root_substitution_and_late_truncation() {
    let durable = DurableFixture::capture(multi_fixture());
    for mutation in 0..8 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let (bytes, wire_storage) = durable
            .with_inputs(&mut budget, |source, ranked, budget| {
                let mut roots = ranked.to_vec();
                let mut staging = source.staging_roots.to_vec();
                match mutation {
                    0 => roots[1].recipe_bytes = roots[0].recipe_bytes,
                    1 => roots[1].effect_receipts = roots[0].effect_receipts,
                    2 => staging[1].commitments = staging[0].commitments,
                    3 => roots[1].semantic_root = 0,
                    4 => {
                        roots[1].source_rows_bytes =
                            &roots[1].source_rows_bytes[..roots[1].source_rows_bytes.len() - 1]
                    }
                    5 => {
                        roots[1].recipe_bytes =
                            &roots[1].recipe_bytes[..roots[1].recipe_bytes.len() - 1]
                    }
                    6 => {
                        roots[1].recipe_bytes = roots[0].recipe_bytes;
                        roots[1].effect_receipts = roots[0].effect_receipts;
                    }
                    7 => roots.swap(0, 1),
                    _ => unreachable!(),
                }
                encode_native_compiler_source_packet_v1(
                    NativeCompilerRankedRecipeSourceProofInputsV1 {
                        source: NativeCompilerSourceProofInputsV1 {
                            staging_roots: &staging,
                            ..source
                        },
                        ranked_roots: &roots,
                    },
                    None,
                    budget,
                )
            })
            .unwrap();
        // Re-encode complete framing so inner replay, not a corrupt header, rejects it.
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = 37 + wire_storage.retained_storage();
        budget.reserve_storage(floor).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let ledger = budget.work_ledger_identity_v1();
        let error = validate_native_compiler_ranked_source_packet_v1(&bytes, &mut budget)
            .err()
            .unwrap();
        let expected = match mutation {
            0 | 1 => matches!(
                error,
                E::EffectReceipt(
                    FunctionalRefinementImportErrorV2::StaleNormalizedObligationEffectIr
                )
            ),
            2 => matches!(error, E::Mismatch("signed aggregate obligation binding")),
            3 => matches!(error, E::Mismatch("ordered typed ranked root/rank")),
            4 => matches!(error, E::RankedSourceRowsWire(_)),
            5 => matches!(error, E::RankedRecipeWire(_)),
            6 | 7 => matches!(
                error,
                E::Mismatch("exact signed effect identity and staging row")
            ),
            _ => unreachable!(),
        };
        assert!(expected, "mutation {mutation}: {error:?}");
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
        assert!(budget.work() > 0);
        assert!(budget.peak_storage() > floor);
        drop(bytes);
        budget
            .release_storage(wire_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn complete_packet_two_root_exact_and_one_short_resource_limits() {
    let durable = DurableFixture::capture(multi_fixture());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (bytes, wire_storage) = durable.packet(None, &mut budget).unwrap();
    drop(durable);
    let floor = 37 + wire_storage.retained_storage();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    validate_native_compiler_ranked_source_packet_v1(&bytes, &mut budget).unwrap();
    let used = budget.work();
    let peak = budget.peak_storage();
    for (work_limit, storage_limit, success) in [
        (used, peak, true),
        (used - 1, peak, false),
        (used, peak - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let ledger = budget.work_ledger_identity_v1();
        let result = validate_native_compiler_ranked_source_packet_v1(&bytes, &mut budget);
        assert_eq!(result.is_ok(), success);
        if let Err(error) = result {
            if work_limit < used {
                assert!(
                    matches!(
                        error,
                        E::Source(NativeSourceReplayErrorV1::RankedSource(
                            ProductionSemanticKirErrorV1::MirPlironTranslation(
                                ProductionMirPlironTranslationErrorV1::ResourceLimit
                            )
                        ))
                    ),
                    "{error:?}"
                );
                assert!(budget.work() <= work_limit);
            } else {
                assert!(
                    matches!(error, E::Source(NativeSourceReplayErrorV1::Resource(Resource::Storage(limit))) if limit.actual() == peak && limit.limit() == peak - 1),
                    "{error:?}"
                );
            }
        }
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(used);
    let mut budget = Budget::new(&mut work, peak - 1);
    budget.reserve_storage(floor).unwrap();
    let error = validate_native_compiler_ranked_source_packet_v1(&bytes, &mut budget)
        .err()
        .unwrap();
    assert!(
        matches!(error, E::Source(NativeSourceReplayErrorV1::Resource(Resource::Storage(limit))) if limit.actual() == peak && limit.limit() == peak - 1),
        "{error:?}"
    );
    assert_eq!(budget.failed_storage(), Some(peak));
    assert_eq!(budget.storage(), floor);
}
