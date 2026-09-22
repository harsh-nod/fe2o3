//! End-to-end source replay from one inert byte buffer, without producer custody.
use super::*;
use fe2o3_lower_mir_kernel::{
    NativeSourceReplayErrorV1, ProductionPreRankedKirErrorV1, ProductionSemanticKirErrorV1,
};

fn assert_erasure_rejection(error: E, expected: &str) {
    assert!(
        matches!(error, E::Source(NativeSourceReplayErrorV1::Materialize(
        ProductionPreRankedKirErrorV1::Lowering(ProductionSemanticKirErrorV1::Unsupported { function: 0, block: None, statement: None, detail })
    )) if detail == expected),
        "{error:?}"
    );
}

#[test]
fn complete_packet_direct_recovery_drops_every_producer() {
    let durable = DurableFixture::capture(fixture());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (_, baseline) = durable.direct(&[durable.signature], &mut budget).unwrap();
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
    assert_eq!(
        checked
            .source()
            .source()
            .semantic()
            .semantic()
            .canonical_encoding(),
        expected.0
    );
    assert_eq!(
        checked
            .source()
            .source()
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
    checked.source().source().verify_equivalence().unwrap();
    assert!(!checked.authenticates_compiler_or_launch_origin());
    assert!(!checked.grants_artifact_or_launch_authority());
    drop(checked);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn complete_packet_unit_local_recovers_actual_e_and_rejects_n_as_e() {
    let (fixture, producer) = unit_local_erased_fixture::unit_fixture();
    let erased_bytes = producer.erased().canonical().canonical_bytes().to_vec();
    let durable = DurableFixture::capture(fixture);
    drop(producer);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (erased, erased_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_canonical_bytes_with_verification_budget_v12(
            &erased_bytes,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(erased_storage.retained_storage())
        .unwrap();
    let (_, baseline) = durable
        .replay(
            &[durable.signature],
            &mut budget,
            |original, ranked_roots, budget| {
                validate_native_compiler_unit_local_erased_recipe_source_proof_v1(
                    NativeCompilerUnitLocalErasedRecipeSourceProofInputsV1 {
                        original,
                        ranked_roots,
                        erased: &erased,
                    },
                    budget,
                )
            },
        )
        .unwrap();
    drop(erased);
    budget
        .release_storage(erased_storage.retained_storage())
        .unwrap();
    let (bytes, wire_storage) = durable.packet(Some(&erased_bytes), &mut budget).unwrap();
    budget
        .reserve_storage(wire_storage.retained_storage())
        .unwrap();
    let (wrong, wrong_storage) = durable
        .packet(Some(&durable.native_graph), &mut budget)
        .unwrap();
    budget
        .reserve_storage(wrong_storage.retained_storage())
        .unwrap();
    let expected = (
        durable.native_graph.clone(),
        durable.middle.canonical_bytes().to_vec(),
        durable.correspondence.canonical_bytes().to_vec(),
        durable.verus.canonical_bytes().to_vec(),
        durable.semantic.clone(),
        NativeNeutralModuleRefV1::decode(&durable.native)
            .unwrap()
            .catalog_bytes()
            .to_vec(),
    );
    drop(durable);
    budget.reserve_storage(37).unwrap();
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
    assert_eq!(
        checked
            .source()
            .source()
            .erased()
            .canonical()
            .canonical_bytes(),
        erased_bytes
    );
    assert_eq!(
        checked
            .source()
            .source()
            .original_source()
            .executable()
            .canonical()
            .canonical_bytes(),
        expected.0
    );
    assert_eq!(checked.middle_end_roster().canonical_bytes(), expected.1);
    assert_eq!(
        checked.correspondence_roster().canonical_bytes(),
        expected.2
    );
    assert_eq!(checked.verus_roster().canonical_bytes(), expected.3);
    assert_eq!(
        checked
            .source()
            .source()
            .original_source()
            .semantic_ssa()
            .source_semantic()
            .canonical_encoding(),
        expected.4
    );
    assert_eq!(checked.source().catalog().canonical_bytes(), expected.5);
    checked
        .source()
        .source()
        .verify_equivalence(&mut budget)
        .unwrap();
    assert!(!checked.authenticates_compiler_or_launch_origin());
    assert!(!checked.grants_artifact_or_launch_authority());
    drop(checked);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 37 + wrong_storage.retained_storage());
    let error = validate_native_compiler_unit_local_erased_source_packet_v1(&wrong, &mut budget)
        .err()
        .unwrap();
    assert_erasure_rejection(
        error,
        "Unit-local deletion added or retained an extra operation",
    );
    drop(wrong);
    budget
        .release_storage(wrong_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn complete_packet_distinguishes_malformed_e_from_valid_but_changed_e() {
    let (fixture, producer) = unit_local_erased_fixture::unit_fixture();
    let changed = unit_local_erased_fixture::changed_erased_module(producer.erased());
    drop(producer);
    let durable = DurableFixture::capture(fixture);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    assert!(budget.reserve_storage(usize::MAX).is_err());
    let (changed, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &changed,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (bytes, wire_storage) = durable
        .packet(Some(changed.canonical().canonical_bytes()), &mut budget)
        .unwrap();
    budget
        .reserve_storage(wire_storage.retained_storage())
        .unwrap();
    drop(changed);
    budget.release_storage(storage.retained_storage()).unwrap();
    let error = validate_native_compiler_unit_local_erased_source_packet_v1(&bytes, &mut budget)
        .err()
        .unwrap();
    assert_erasure_rejection(
        error,
        "Unit-local deletion changed a retained operation or order",
    );
    drop(bytes);
    budget
        .release_storage(wire_storage.retained_storage())
        .unwrap();
    let (bytes, wire_storage) = durable
        .packet(Some(b"not a V12 graph"), &mut budget)
        .unwrap();
    budget
        .reserve_storage(wire_storage.retained_storage())
        .unwrap();
    let error = validate_native_compiler_unit_local_erased_source_packet_v1(&bytes, &mut budget)
        .err()
        .unwrap();
    assert!(
        matches!(
            error,
            E::ErasedAdmission(
                fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Decode(_)
            )
        ),
        "{error:?}"
    );
    drop(bytes);
    budget
        .release_storage(wire_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 37);
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
}

#[test]
fn complete_packet_wire_valid_semantic_mutations_reach_existing_replay() {
    let base = DurableFixture::capture(fixture());
    for mutation in 0..8 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let (bytes, _) = base
            .replay(&[base.signature], &mut budget, |source, ranked, budget| {
                let mut root = ranked[0];
                let mut signatures = root.effect_receipts.to_vec();
                let mut row = base.staging;
                let mut stage_id = 0;
                match mutation {
                    0 => {
                        signatures[0] =
                            InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
                                *signatures[0].wire(),
                                [91; 32],
                            )
                    }
                    1 => row.toolchain[4][0] ^= 1,
                    2 => root.launch_rank = 2,
                    3 => root.ranked_ir = "changed diagnostic text",
                    4 => {
                        root.source_rows_bytes =
                            &root.source_rows_bytes[..root.source_rows_bytes.len() - 1]
                    }
                    5 => root.recipe_bytes = &root.recipe_bytes[..root.recipe_bytes.len() - 1],
                    6 => stage_id = 7,
                    7 => root.semantic_root = 7,
                    _ => unreachable!(),
                }
                root.effect_receipts = &signatures;
                let commitments = [row];
                let staging = [NativeCompilerRootStagingV1 {
                    semantic_root: stage_id,
                    commitments: &commitments,
                }];
                let source = NativeCompilerSourceProofInputsV1 {
                    staging_roots: &staging,
                    ..source
                };
                encode_native_compiler_source_packet_v1(
                    NativeCompilerRankedRecipeSourceProofInputsV1 {
                        source,
                        ranked_roots: &[root],
                    },
                    None,
                    budget,
                )
            })
            .unwrap();
        budget.reserve_storage(37).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        let error = validate_native_compiler_ranked_source_packet_v1(&bytes, &mut budget)
            .err()
            .unwrap();
        let expected = match mutation {
            0 => matches!(error, E::EffectReceipt(_)),
            1 => matches!(error, E::Mismatch("signed aggregate obligation binding")),
            2 | 7 => matches!(error, E::Mismatch("ordered typed ranked root/rank")),
            3 => matches!(error, E::Mismatch("exact typed ranked diagnostic text")),
            4 => matches!(error, E::RankedSourceRowsWire(_)),
            5 => matches!(error, E::RankedRecipeWire(_)),
            6 => matches!(error, E::Mismatch("ordered staging semantic root")),
            _ => unreachable!(),
        };
        assert!(expected, "mutation {mutation}: {error:?}");
        assert_eq!(budget.storage(), 37);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
    }
}

#[test]
fn complete_packet_both_replay_routes_preserve_resource_boundary_and_route() {
    for unit_local in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let (bytes, _) = if unit_local {
            let (fixture, producer) = unit_local_erased_fixture::unit_fixture();
            DurableFixture::capture(fixture)
                .packet(
                    Some(producer.erased().canonical().canonical_bytes()),
                    &mut budget,
                )
                .unwrap()
        } else {
            DurableFixture::capture(fixture())
                .packet(None, &mut budget)
                .unwrap()
        };
        let run = |budget: &mut Budget<'_>| {
            if unit_local {
                validate_native_compiler_unit_local_erased_source_packet_v1(&bytes, budget)
                    .map(drop)
            } else {
                validate_native_compiler_ranked_source_packet_v1(&bytes, budget).map(drop)
            }
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(37).unwrap();
        run(&mut budget).unwrap();
        let used = budget.work();
        let peak = budget.peak_storage();
        for (work_limit, storage_limit, success) in [
            (used, peak, true),
            (used - 1, peak, false),
            (used, peak - 1, false),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(37).unwrap();
            assert!(budget.reserve_storage(usize::MAX).is_err());
            assert_eq!(run(&mut budget).is_ok(), success);
            assert_eq!(budget.storage(), 37);
            assert_eq!(budget.failed_storage(), Some(usize::MAX));
        }
        let error = if unit_local {
            validate_native_compiler_ranked_source_packet_v1(&bytes, &mut budget)
                .err()
                .unwrap()
        } else {
            validate_native_compiler_unit_local_erased_source_packet_v1(&bytes, &mut budget)
                .err()
                .unwrap()
        };
        assert!(matches!(error, E::PacketWire(_)), "{error:?}");
    }
}
