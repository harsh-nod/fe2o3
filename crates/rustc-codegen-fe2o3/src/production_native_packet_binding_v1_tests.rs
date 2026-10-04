//! Inert original-N packet checks over genuine retained source inputs.
//! These tests create neither typed signed proofs nor a source-proof stage.
use super::*;
use crate::production_pipeline::native_checked_output_handoff_v1::check_original_native_packet_components_v1 as check;
use fe2o3_compiler_lineage::{
    InertNativeNeutralSubjectV1 as Subject, NativeNeutralModuleRefV1 as Packet,
    encode_native_neutral_module_v1 as encode,
};
use sha2::Sha256;
use std::mem::size_of;

fn subject(source: SourceInputsV1<'_>) -> Subject {
    Subject::new(
        *source.original.canonical().identity().digest(),
        source
            .original
            .canonical()
            .canonical_bytes()
            .len()
            .try_into()
            .unwrap(),
        *source.catalog.digest(),
        source.catalog.canonical_bytes().len().try_into().unwrap(),
    )
    .unwrap()
}

#[test]
fn original_packet_requires_exact_n_catalog_route_and_each_roster_subject() {
    for erased in [false, true] {
        for profile in [
            ProductionAmdTargetProfileV1::Gfx942,
            ProductionAmdTargetProfileV1::Gfx950,
        ] {
            with_inputs(erased, profile, |inputs, _, budget| {
                let source = inputs.owner.source(inputs.catalog).unwrap();
                let expected = subject(source);
                let n = source.original.canonical().canonical_bytes();
                let catalog = source.catalog.canonical_bytes();
                let bytes = encode(&expected, n, catalog).unwrap();
                let floor = budget.storage();
                let ledger = budget.work_ledger_identity_v1();
                check(source, erased, [&expected; 3], &bytes, budget).unwrap();
                assert!(!Packet::decode(&bytes).unwrap().grants_authority());
                for (offset, detail) in [
                    (112, "exact original packet N bytes"),
                    (112 + n.len(), "exact original packet catalog bytes"),
                ] {
                    let mut changed = bytes.clone();
                    changed[offset] ^= 1;
                    assert_eq!(Packet::decode(&changed).unwrap().subject(), &expected);
                    assert!(
                        matches!(check(source, erased, [&expected; 3], &changed, budget),
                        Err(E::Mismatch(actual)) if actual == detail)
                    );
                    assert_eq!(budget.storage(), floor);
                }
                let foreign_n = Subject::new(
                    [9; 32],
                    n.len() as u64,
                    *source.catalog.digest(),
                    catalog.len() as u64,
                )
                .unwrap();
                let foreign_catalog = Subject::new(
                    *source.original.canonical().identity().digest(),
                    n.len() as u64,
                    [8; 32],
                    catalog.len() as u64,
                )
                .unwrap();
                for foreign in [&foreign_n, &foreign_catalog] {
                    let changed = encode(foreign, n, catalog).unwrap();
                    assert!(matches!(
                        check(source, erased, [foreign; 3], &changed, budget),
                        Err(E::Mismatch("exact original packet subject"))
                    ));
                }
                for index in 0..3 {
                    let mut subjects = [&expected; 3];
                    subjects[index] = &foreign_n;
                    assert!(matches!(
                        check(source, erased, subjects, &bytes, budget),
                        Err(E::Mismatch("each signed-source roster original subject"))
                    ));
                }
                // A self-consistent E/O packet is still not an original-N packet.
                let other_graph = source.erased.unwrap_or_else(|| inputs.owner.output());
                assert_ne!(n, other_graph.canonical().canonical_bytes());
                let substituted = SourceInputsV1 {
                    original: other_graph,
                    ..source
                };
                let foreign = subject(substituted);
                let changed =
                    encode(&foreign, other_graph.canonical().canonical_bytes(), catalog).unwrap();
                assert!(matches!(
                    check(source, erased, [&foreign; 3], &changed, budget),
                    Err(E::Mismatch("exact original packet subject"))
                ));
                assert!(matches!(
                    check(source, !erased, [&expected; 3], &bytes, budget),
                    Err(E::Mismatch("original packet direct/erased route"))
                ));
                let mut trailing = bytes.clone();
                trailing.push(0);
                for malformed in [&bytes[..bytes.len() - 1], trailing.as_slice()] {
                    assert!(matches!(
                        check(source, erased, [&expected; 3], malformed, budget),
                        Err(E::NativePacket(_))
                    ));
                }
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
            });
        }
    }
}

#[test]
fn original_packet_has_independent_exact_work_and_storage_boundaries() {
    for erased in [false, true] {
        with_inputs(
            erased,
            ProductionAmdTargetProfileV1::Gfx950,
            |inputs, _, outer| {
                let source = inputs.owner.source(inputs.catalog).unwrap();
                let expected = subject(source);
                let n = source.original.canonical().canonical_bytes();
                let catalog = source.catalog.canonical_bytes();
                let bytes = encode(&expected, n, catalog).unwrap();
                let floor = outer.storage();
                let required_work = bytes.len()
                    + n.len()
                    + catalog.len()
                    + 4 * (2 * size_of::<Subject>() + 1)
                    + 512;
                let scratch = size_of::<Packet<'_>>() + size_of::<Subject>() + size_of::<Sha256>();
                for (work_limit, storage_limit) in [
                    (required_work, floor + scratch),
                    (required_work - 1, floor + scratch),
                    (required_work, floor + scratch - 1),
                ] {
                    let mut work = Work::new(work_limit);
                    let mut budget = Budget::new(&mut work, storage_limit);
                    budget.reserve_storage(floor).unwrap();
                    let ledger = budget.work_ledger_identity_v1();
                    let result = check(source, erased, [&expected; 3], &bytes, &mut budget);
                    if work_limit < required_work {
                        assert!(matches!(result, Err(E::Resource(Resource::Work(e)))
                        if e.actual() == required_work && e.limit() == work_limit));
                        assert_eq!(budget.work(), 0);
                    } else if storage_limit < floor + scratch {
                        assert!(matches!(result, Err(E::Resource(Resource::Storage(e)))
                        if e.actual() == floor + scratch && e.limit() == storage_limit));
                        assert_eq!(budget.work(), required_work);
                    } else {
                        result.unwrap();
                        assert_eq!(budget.work(), required_work);
                        assert_eq!(budget.peak_storage(), floor + scratch);
                    }
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work_ledger_identity_v1() == ledger);
                }
            },
        );
    }
}
