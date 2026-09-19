//! Genuine unsigned source/J component checks, never signed owner construction.
use super::*;
use crate::compiler_descriptor::TypedDescriptorRootV1;
use fe2o3_compiler_lineage::{
    InertNativeNeutralSubjectV1 as Subject, MultiRootProofRosterInputsV3,
    MultiRootProofRosterKindV3 as Kind, MultiRootProofRosterRootInputV3 as Root,
    MultiRootProofRosterTranscriptV3 as Roster, encode_native_neutral_module_v1,
};
use fe2o3_kernel_ir::{FormalMemoryObligations, InertFormalMemoryReceiptFormatV4 as Formal};

#[derive(Clone, Copy)]
enum Fault {
    None,
    Root,
    Workgroup,
    Formal,
    HistoricalFormal,
    Roster,
}

fn subject(graph: &Graph, artifacts: &PreparedPolicy7ArtifactsV1) -> Subject {
    let catalog = artifacts.native_worker_output_v1().catalog;
    Subject::new(
        *graph.canonical().identity().digest(),
        graph.canonical().identity().canonical_length(),
        *catalog.digest(),
        catalog.canonical_bytes().len() as u64,
    )
    .unwrap()
}
fn envelope(graph: &Graph, artifacts: &PreparedPolicy7ArtifactsV1) -> Vec<u8> {
    encode_native_neutral_module_v1(
        &subject(graph, artifacts),
        graph.canonical().canonical_bytes(),
        artifacts
            .native_worker_output_v1()
            .catalog
            .canonical_bytes(),
    )
    .unwrap()
}
fn reports(artifacts: &PreparedPolicy7ArtifactsV1, historical: bool) -> &[FormalMemoryObligations] {
    match &artifacts.admitted {
        Admitted7::Direct(v) => {
            if historical {
                v.prefix().kernels()
            } else {
                v.kernels()
            }
        }
        Admitted7::Erased(v) => {
            if historical {
                v.prefix().kernels()
            } else {
                v.kernels()
            }
        }
    }
}
fn formal(artifacts: &PreparedPolicy7ArtifactsV1, ranked: &Ranked, fault: Fault) -> Vec<u8> {
    let mut payloads = reports(artifacts, matches!(fault, Fault::HistoricalFormal))
        .iter()
        .map(|r| {
            Formal::from_current_obligations(r)
                .unwrap()
                .canonical_bytes()
                .to_vec()
        })
        .collect::<Vec<_>>();
    if matches!(fault, Fault::Formal) {
        let end = payloads[0].len() - 1;
        payloads[0][end] ^= 1;
    }
    let inputs = artifacts.native_worker_output_v1();
    let source = inputs.owner.source(inputs.catalog).unwrap();
    let mut rows = ranked
        .roots()
        .iter()
        .enumerate()
        .map(|(ordinal, root)| {
            let export = std::str::from_utf8(root.export_symbol()).unwrap();
            let index = artifacts
                .output()
                .module()
                .kernels
                .iter()
                .position(|k| k.id.as_str() == export)
                .unwrap();
            Root {
                semantic_root: root.semantic_root().index(),
                semantic_root_identity: *root.semantic_root_identity().as_bytes(),
                kernel_binding: *root.kernel_binding(),
                source_rank: root.source_rank(),
                workgroup: source.launch.roots()[ordinal]
                    .source_launch()
                    .exact_workgroup()
                    .unwrap(),
                logical_name: root.logical_name(),
                export_symbol: export,
                kernel_id: export,
                payload: &payloads[index],
            }
        })
        .collect::<Vec<_>>();
    if matches!(fault, Fault::Root) {
        rows[0].semantic_root_identity[0] ^= 1;
    }
    if matches!(fault, Fault::Workgroup) {
        rows[0].workgroup[0] += 1;
    }
    let mut identity = *ranked.canonical_roster_identity().as_bytes();
    if matches!(fault, Fault::Roster) {
        identity[0] ^= 1;
    }
    let order = ranked
        .canonical_kernel_order()
        .iter()
        .map(|i| *i as u32)
        .collect::<Vec<_>>();
    Roster::new(MultiRootProofRosterInputsV3 {
        kind: Kind::FormalMemory,
        semantic_mir_sha256: *source.semantic.semantic_sha256().as_bytes(),
        native_neutral_subject: subject(artifacts.output(), artifacts),
        roster_identity: identity,
        canonical_kernel_order: &order,
        roots: &rows,
    })
    .unwrap()
    .into_canonical_bytes()
}
fn check_case(
    artifacts: &PreparedPolicy7ArtifactsV1,
    ranked: &Ranked,
    typed: &[TypedDescriptorRootV1],
    kernel: &[u8],
    formal: &[u8],
    expected: Option<&'static str>,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let (receipts, storage) = Final::try_from_preimages_v1(kernel, formal, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let result = final_receipts::check_component(artifacts, ranked, typed, &receipts, budget);
    match expected {
        None => result.unwrap(),
        Some(expected) => assert!(
            matches!(result,
            Err(ProductionPipelineError::CheckedOutputPolicy7Stage(CheckedOutputPolicy7StageErrorV1::Native(ref e)))
            if matches!(e.as_ref(), NativeOutputHandoffErrorV1::Mismatch(actual) if *actual == expected)),
            "{result:?}"
        ),
    }
    assert_eq!(budget.storage(), floor + storage.retained_storage());
    assert!(budget.work_ledger_identity_v1() == ledger);
    drop(receipts);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

pub(crate) fn exercise_final_j_components(
    artifacts: &PreparedPolicy7ArtifactsV1,
    ranked: &Ranked,
    typed: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let kernel = envelope(artifacts.output(), artifacts);
    let good_formal = formal(artifacts, ranked, Fault::None);
    assert_eq!(kernel, envelope(artifacts.output(), artifacts));
    assert_eq!(good_formal, formal(artifacts, ranked, Fault::None));
    check_case(
        artifacts,
        ranked,
        typed,
        &kernel,
        &good_formal,
        None,
        budget,
    );
    {
        use crate::production_pipeline::native_checked_output_handoff_v1::policy6::final_receipts::{
            NativeFinalOutputReceiptsPolicy6V1, joins,
        };
        let (payloads, storage) = NativeFinalOutputReceiptsPolicy6V1::try_from_preimages_v1(
            &kernel,
            &good_formal,
            budget,
        )
        .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let mut historical_view = artifacts.native_worker_output_v1();
        historical_view.owner = match &artifacts.admitted {
            Admitted7::Direct(v) => OutputOwnerV1::Direct6(v.prefix()),
            Admitted7::Erased(v) => OutputOwnerV1::Erased6(v.prefix()),
        };
        assert!(matches!(
            joins::check_policy7(historical_view, ranked, &payloads, typed, budget),
            Err(NativeOutputHandoffErrorV1::Mismatch(
                "fixed Policy7 receipt owner"
            ))
        ));
        drop(payloads);
        budget.release_storage(storage.retained_storage()).unwrap();
    }
    let mut permuted = typed.to_vec();
    permuted.reverse();
    check_case(
        artifacts,
        ranked,
        &permuted,
        &kernel,
        &good_formal,
        None,
        budget,
    );
    for (index, expected) in [
        (112, "final KernelIr graph bytes"),
        (
            112 + artifacts.output().canonical().canonical_bytes().len(),
            "final KernelIr catalog bytes",
        ),
    ] {
        let mut bad = kernel.clone();
        bad[index] ^= 1;
        check_case(
            artifacts,
            ranked,
            typed,
            &bad,
            &good_formal,
            Some(expected),
            budget,
        );
    }
    for (fault, expected) in [
        (Fault::Root, "exact final receipt root axes"),
        (Fault::Workgroup, "exact final receipt root axes"),
        (Fault::Roster, "complete final FormalMemory roster"),
        (Fault::Formal, "fresh final-J formal payload"),
    ] {
        check_case(
            artifacts,
            ranked,
            typed,
            &kernel,
            &formal(artifacts, ranked, fault),
            Some(expected),
            budget,
        );
    }
    let (old_o, erased) = match &artifacts.admitted {
        Admitted7::Direct(v) => (
            v.prefix().checked_output().intermediate_policy5().owner(),
            None,
        ),
        Admitted7::Erased(v) => (
            v.prefix().checked_output().intermediate_policy5().owner(),
            Some(v.prefix().erased()),
        ),
    };
    for old in [
        Some(artifacts.admitted.original()),
        Some(artifacts.admitted.input()),
        Some(old_o),
        erased,
    ]
    .into_iter()
    .flatten()
    {
        if old.canonical().canonical_bytes() != artifacts.output().canonical().canonical_bytes() {
            check_case(
                artifacts,
                ranked,
                typed,
                &envelope(old, artifacts),
                &good_formal,
                Some("final KernelIr subject"),
                budget,
            );
        }
    }
    if !artifacts.admitted.continuation().rows().is_empty() {
        let historical = formal(artifacts, ranked, Fault::HistoricalFormal);
        assert_ne!(
            artifacts.admitted.input().canonical().identity(),
            artifacts.output().canonical().identity(),
            "actual Store deletion must still change the graph"
        );
        let expected = match &artifacts.admitted {
            Admitted7::Direct(_) => {
                // This fixture has only private Stores. Formal memory rows
                // describe external accesses, so their empty payload is stable.
                assert!(
                    reports(artifacts, false)
                        .iter()
                        .chain(reports(artifacts, true))
                        .all(|r| r.accesses().is_empty())
                );
                assert_eq!(
                    historical, good_formal,
                    "private-only deletion preserves external obligations"
                );
                None
            }
            Admitted7::Erased(_) => {
                // Its real global Store follows the deleted private Store;
                // both roots must retain nonempty, shifted final-J access rows.
                assert!(
                    reports(artifacts, false)
                        .iter()
                        .chain(reports(artifacts, true))
                        .all(|r| !r.accesses().is_empty())
                );
                assert_ne!(
                    historical, good_formal,
                    "global Store coordinates must change after deletion"
                );
                Some("fresh final-J formal payload")
            }
        };
        check_case(
            artifacts,
            ranked,
            typed,
            &kernel,
            &historical,
            expected,
            budget,
        );
    }
    // Fresh independent invocations measure this component only. No production
    // continuation swaps or resets its live cumulative work ledger.
    let (receipts, storage) = Final::try_from_preimages_v1(&kernel, &good_formal, budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let retained = budget.storage();
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut trial = Budget::new(&mut work, storage_limit);
        trial.reserve_storage(retained).unwrap();
        let result =
            final_receipts::check_component(artifacts, ranked, typed, &receipts, &mut trial);
        assert_eq!(trial.storage(), retained);
        (result.is_ok(), trial.work(), trial.peak_storage())
    };
    let (ok, work, peak) = run(1_000_000_000, 1024 * 1024 * 1024);
    assert!(ok);
    assert!(run(work, peak).0);
    assert!(!run(work - 1, peak).0);
    assert!(!run(work, peak - 1).0);
    drop(receipts);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}
