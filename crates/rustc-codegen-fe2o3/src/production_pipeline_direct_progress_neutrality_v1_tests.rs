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

fn direct_artifacts(observed: bool) -> ActualArtifacts {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    let mut actual = None;
    with_backend_checked_output_policy4_owned_v1(profile, |owner, budget| {
        let roots = typed_roots(&owner);
        let floor = budget.storage();
        let work = budget.work();
        let events = Rc::new(RefCell::new(Vec::new()));
        let records = events.clone();
        let action = || prepare_checked_output_artifacts_v1(owner, profile, &roots, None, budget);
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
                        route: Route::Direct,
                        phase: Phase::ArtifactPreparation
                    },
                    Event::Finished {
                        route: Route::Direct,
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
    });
    actual.unwrap()
}

#[test]
fn direct_artifact_observer_preserves_actual_bytes_and_canonical_resources() {
    assert_eq!(direct_artifacts(false), direct_artifacts(true));
}

#[test]
fn actual_artifact_resource_refusal_survives_observer_panic() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_backend_checked_output_policy4_owned_v1(profile, |owner, original| {
        let roots = typed_roots(&owner);
        let floor = original.storage();
        let mut work = Work::new(3);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        let outcomes = Rc::new(RefCell::new(Vec::new()));
        let records = outcomes.clone();
        let error = timing::with_sink(
            Rc::new(move |event| {
                if let Event::Finished { outcome, .. } = event {
                    records.borrow_mut().push(outcome);
                    panic!("artifact observer only");
                }
            }),
            || prepare_checked_output_artifacts_v1(owner, profile, &roots, None, &mut budget),
        )
        .err()
        .unwrap();
        let ProductionPipelineError::CheckedOutputStage(CheckedOutputStageErrorV1::Resource(
            Resource::Work(error),
        )) = error
        else {
            panic!("observer replaced the original resource refusal");
        };
        assert_eq!((error.actual(), error.limit()), (4, 3));
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (0, floor, floor)
        );
        assert_eq!(outcomes.borrow().as_slice(), [PhaseOutcome::Refused]);
    });
}
