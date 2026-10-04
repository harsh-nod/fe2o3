use super::*;
use crate::production_pipeline::checked_output_progress_v1::{
    self as timing, Event, Phase, PhaseOutcome, Route,
};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, PartialEq, Eq)]
struct ActualArtifacts {
    canonical: Vec<u8>,
    llvm: String,
    descriptor: Vec<u8>,
    work: usize,
    floor: usize,
    peak: usize,
    receipt: usize,
}

fn erased_artifacts(observed: bool) -> ActualArtifacts {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    let mut actual = None;
    with_backend_erased_bound_v1(true, 2, profile, |source, bound, budget| {
        let typed = erased_typed_roots(&source);
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, budget)
                .unwrap();
        let checked_storage = checked.retained_storage();
        budget.reserve_storage(checked_storage).unwrap();
        assert_eq!(checked.forwarding_rows().len(), 2);
        let admitted = Admitted::try_admit_v1(source, bound, checked, budget).unwrap();
        let floor = budget.storage();
        let work = budget.work();
        let events = Rc::new(RefCell::new(Vec::new()));
        let records = events.clone();
        let action =
            || prepare_erased_checked_output_artifacts_v1(admitted, profile, &typed, None, budget);
        let (artifacts, receipt) = if observed {
            timing::with_sink(
                Rc::new(move |event| records.borrow_mut().push(event)),
                action,
            )
        } else {
            action()
        }
        .unwrap();
        assert_eq!(budget.storage(), floor);
        let work = budget.work() - work;
        let peak = budget.peak_storage();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        actual = Some(ActualArtifacts {
            canonical: artifacts
                .admitted()
                .output()
                .canonical()
                .canonical_bytes()
                .to_vec(),
            llvm: artifacts.llvm_ir().to_owned(),
            descriptor: artifacts.descriptor_source().canonical_bytes().to_vec(),
            work,
            floor,
            peak,
            receipt: receipt.retained_storage(),
        });
        if observed {
            assert!(matches!(
                events.borrow().as_slice(),
                [
                    Event::Started {
                        route: Route::SilentUnitErased,
                        phase: Phase::ArtifactPreparation
                    },
                    Event::Finished {
                        route: Route::SilentUnitErased,
                        phase: Phase::ArtifactPreparation,
                        outcome: PhaseOutcome::Complete,
                        ..
                    }
                ]
            ));
        } else {
            assert!(events.borrow().is_empty());
        }
        drop(artifacts);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.release_storage(checked_storage).unwrap();
    });
    actual.unwrap()
}

#[test]
fn erased_artifact_observer_preserves_actual_bytes_and_canonical_resources() {
    assert_eq!(erased_artifacts(false), erased_artifacts(true));
}
