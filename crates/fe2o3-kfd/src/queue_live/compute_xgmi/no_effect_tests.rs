//! CPU envelope tests; the scripted model pair is not a native VM authority.

use super::super::tests::{persistent_compute_cancellation_test_session, test_queue_key};
use super::*;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

struct Pair {
    fault: Option<(&'static str, bool)>,
    trace: Vec<&'static str>,
}

impl Pair {
    fn step(&mut self, stage: &'static str) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.trace.push(stage);
        match self.fault {
            Some((failed, panic)) if failed == stage => {
                assert!(!panic, "{stage}");
                Err(ComputeAqlQueueSessionErrorV1::Contract(stage))
            }
            _ => Ok(()),
        }
    }
}

impl model_pair_loan::Context for Pair {
    type Loan = usize;
    type Error = ComputeAqlQueueSessionErrorV1;

    fn open(&mut self, endpoint: usize) -> Result<usize, Self::Error> {
        self.step(["open0", "open1"][endpoint])?;
        Ok(endpoint)
    }

    fn retake(&mut self, endpoint: usize, loan: usize) -> Result<(), Self::Error> {
        assert_eq!(endpoint, loan);
        self.step(["retake0", "retake1"][endpoint])
    }

    fn poison_endpoint(&mut self, endpoint: usize) {
        self.trace.push(["poison0", "poison1"][endpoint]);
    }

    fn poison_process(&mut self) {
        self.trace.push("process");
    }
}

fn identity() -> CreationNoEffect {
    CreationNoEffect {
        source: test_queue_key(11, 1),
        destination: test_queue_key(12, 1),
        route: crate::topology::tests::admitted_xgmi_routes()[0],
    }
}

#[test]
fn compute_xgmi_host_rejection_certification_follows_both_model_retakes() {
    for fault in std::iter::once(None).chain(
        [
            "open0",
            "open1",
            "host",
            "currentness",
            "retake0",
            "retake1",
        ]
        .into_iter()
        .flat_map(|stage| [Some((stage, false)), Some((stage, true))]),
    ) {
        let mut pair = Pair {
            fault,
            trace: Vec::new(),
        };
        let mut root = ManuallyDrop::new(Gfx942ComputeXgmiQueueCreationRootV1::new());
        root.armed = true;
        let mut candidate = false;
        let mut certificate = None;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let result = model_pair_loan::execute(&mut pair, |pair| {
                pair.step("host").map_err(|error| Failure {
                    error,
                    terminal: true,
                })?;
                pair.step("currentness").map_err(|error| Failure {
                    error,
                    terminal: true,
                })?;
                candidate = true;
                Err(Failure {
                    error: ComputeAqlQueueSessionErrorV1::Contract("host allocation denied"),
                    terminal: false,
                })
            });
            let failure = result.expect_err("script always rejects creation");
            certificate = settle_creation_failure_v1(&mut root, &failure, candidate, |_| {
                // This closure models only the final live-identity check. The
                // real certificate factory additionally checks both sessions.
                assert!(pair.trace.ends_with(&["retake1", "retake0"]));
                pair.trace.push("validate");
                Some(identity())
            });
            failure
        }));
        if fault.is_none() {
            assert!(certificate.is_some());
            assert!(root.is_vacant());
            assert!(!outcome.unwrap().terminal);
            assert_eq!(
                pair.trace,
                [
                    "open0",
                    "open1",
                    "host",
                    "currentness",
                    "retake1",
                    "retake0",
                    "validate"
                ]
            );
        } else {
            assert!(certificate.is_none());
            assert!(root.armed);
            assert!(pair.trace.ends_with(&["poison0", "poison1", "process"]));
            if fault.is_some_and(|(_, panics)| panics) {
                assert!(outcome.is_err());
            } else {
                assert!(outcome.unwrap().terminal);
            }
            if pair.trace.contains(&"host") {
                let first = pair
                    .trace
                    .iter()
                    .position(|step| *step == "retake1")
                    .unwrap();
                let second = pair
                    .trace
                    .iter()
                    .position(|step| *step == "retake0")
                    .unwrap();
                assert!(first < second);
            }
        }
        // No native owner was created in this CPU-only pair model.
        root.armed = false;
        drop(ManuallyDrop::into_inner(root));
    }
}

#[test]
fn compute_xgmi_vacancy_and_retryable_diagnostics_do_not_certify_no_effect() {
    for host_preparation in [false, true] {
        let mut root = ManuallyDrop::new(Gfx942ComputeXgmiQueueCreationRootV1::new());
        root.armed = true;
        let failure = Failure {
            error: ComputeAqlQueueSessionErrorV1::Contract("host allocation denied"),
            terminal: false,
        };
        let mut validations = 0;
        let certificate = settle_creation_failure_v1(&mut root, &failure, host_preparation, |_| {
            validations += 1;
            None // Live identity/currentness/attachment validation did not pass.
        });
        assert!(certificate.is_none());
        assert!(root.is_vacant());
        assert_eq!(validations, usize::from(host_preparation));
        drop(ManuallyDrop::into_inner(root));
    }
}

#[test]
fn compute_xgmi_additive_outcome_rejects_missing_live_engine_and_occupied_root() {
    let id = identity();
    for occupied in [false, true] {
        let mut source = persistent_compute_cancellation_test_session(id.source, None, None);
        let mut destination =
            persistent_compute_cancellation_test_session(id.destination, None, None);
        let mut root = ManuallyDrop::new(Gfx942ComputeXgmiQueueCreationRootV1::new());
        root.armed = occupied;
        let (result, disposition) = source
            .create_native_xgmi_queue_with_peer_outcome_v1(&mut destination, id.route, &mut root)
            .into_classified_parts_v1(&source, &destination, id.route, &root);
        assert!(result.is_err());
        assert_eq!(
            disposition,
            Gfx942ComputeXgmiQueueCreationDispositionV1::Unclassified
        );
        assert!(!id.matches_v1(&source, &destination, id.route, &root));
        assert_eq!(root.armed, occupied);
        assert!(root.native.is_vacant() && root.returned.is_none());
        assert!(source.xgmi_attachment.is_none() && destination.xgmi_attachment.is_none());
        root.armed = false;
        drop(ManuallyDrop::into_inner(root));
    }
}

#[test]
fn compute_xgmi_consuming_classification_preserves_only_its_enclosed_error() {
    for evidence in [false, true] {
        let id = identity();
        let source = persistent_compute_cancellation_test_session(id.source, None, None);
        let destination = persistent_compute_cancellation_test_session(id.destination, None, None);
        let root = Gfx942ComputeXgmiQueueCreationRootV1::new();
        let outcome = Gfx942ComputeXgmiQueueCreationOutcomeV1 {
            result: Err(ComputeAqlQueueSessionErrorV1::Contract(
                "this exact attempt",
            )),
            no_effect: evidence.then_some(id),
        };
        let (result, disposition) = outcome.into_classified_parts_v1(
            &source,
            &destination,
            crate::topology::tests::admitted_xgmi_routes()[0],
            &root,
        );
        assert!(matches!(
            result,
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "this exact attempt"
            ))
        ));
        assert_eq!(
            disposition,
            Gfx942ComputeXgmiQueueCreationDispositionV1::Unclassified
        );
        // Even private fixture evidence is insufficient without live engines.
        // The public API returned no reusable identity/certificate to attach
        // to a different error or later creation outcome.
        assert!(root.is_vacant());
    }
}

#[cfg(feature = "hardware-qualification")]
#[test]
fn qualified_capacity_denial_cannot_bypass_creation_endpoint_preflight() {
    let id = identity();
    for occupied in [false, true] {
        let mut source = persistent_compute_cancellation_test_session(id.source, None, None);
        let mut destination =
            persistent_compute_cancellation_test_session(id.destination, None, None);
        let mut root = ManuallyDrop::new(Gfx942ComputeXgmiQueueCreationRootV1::new());
        root.armed = occupied;
        let (result, disposition) = source
            .create_native_xgmi_queue_with_peer_host_preparation_denial_v1(
                &mut destination,
                id.route,
                &mut root,
            )
            .into_classified_parts_v1(&source, &destination, id.route, &root);
        assert!(result.is_err());
        assert_eq!(
            disposition,
            Gfx942ComputeXgmiQueueCreationDispositionV1::Unclassified
        );
        assert_eq!(root.armed, occupied);
        assert!(root.native.is_vacant() && root.returned.is_none());
        assert!(source.xgmi_attachment.is_none() && destination.xgmi_attachment.is_none());
        assert!(!source.terminal_poisoned && !destination.terminal_poisoned);
        root.armed = false;
        drop(ManuallyDrop::into_inner(root));
    }
}
