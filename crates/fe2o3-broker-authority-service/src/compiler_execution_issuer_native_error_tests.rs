use super::*;
use crate::compiler_execution_supervision::CompilerExecutionSupervisionErrorV1 as InspectionError;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_protected_service_spawn::RetainedResourceAccessErrorV2 as RetainedError;

// This module is instantiated by both nominal V2 and V3 issuer services.
fn assert_occurrence_resource(resource: Resource) {
    let errors = [
        OccurrenceError::Resource(resource),
        OccurrenceError::Subject(SubjectError::Resource(resource)),
        OccurrenceError::Handoff(resource.into()),
        OccurrenceError::Observation(NativeObservationError::Resource(resource)),
        OccurrenceError::Observation(NativeObservationError::Capability(KeyError::Resource(
            resource,
        ))),
        OccurrenceError::Observation(NativeObservationError::Service(resource.into())),
        OccurrenceError::Observation(NativeObservationError::Service(
            crate::LiveClientPidfdErrorV2::from(resource).into(),
        )),
        OccurrenceError::Observation(NativeObservationError::Spawn(resource.into())),
        OccurrenceError::Observation(NativeObservationError::Spawn(
            CleanupError::Resource(resource).into(),
        )),
    ];
    for error in errors {
        let issuer = NativeIssuerServiceError::from(error);
        assert_eq!(issuer.resource(), Some(resource), "{issuer:?}");
    }
}

#[test]
fn occurrence_wrappers_preserve_exact_work_and_storage_denials() {
    let mut work = Work::new(17);
    let mut budget = Budget::new(&mut work, 23);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(9).unwrap();
    let ledger = budget.work_ledger_identity_v1();

    let denied_work = budget.charge_work(11).unwrap_err();
    let Resource::Work(limit) = denied_work else {
        panic!("expected work denial");
    };
    assert_eq!((limit.actual(), limit.limit()), (18, 17));
    assert_occurrence_resource(denied_work);

    let denied_storage = budget.reserve_storage(15).unwrap_err();
    let Resource::Storage(limit) = denied_storage else {
        panic!("expected storage denial");
    };
    assert_eq!((limit.actual(), limit.limit()), (24, 23));
    assert_occurrence_resource(denied_storage);

    assert_eq!(budget.work(), 7);
    assert_eq!(budget.failed_work(), Some(18));
    assert_eq!(budget.failed_storage(), Some(24));
    assert_eq!(budget.storage(), 9);
    assert_eq!(budget.peak_storage(), 9);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn occurrence_wrappers_preserve_allocation_accounting_and_arithmetic_failures() {
    for resource in [
        Resource::Allocation,
        Resource::Accounting,
        Resource::Arithmetic,
    ] {
        assert_occurrence_resource(resource);
    }
}

#[test]
fn non_resource_occurrence_failures_remain_non_resource() {
    let errors = [
        OccurrenceError::Mismatch,
        OccurrenceError::Expected(
            crate::compiler_execution_occurrence::ProtectedCompilerExecutionOccurrenceErrorV1::MissingBuildAttempt,
        ),
        OccurrenceError::Subject(SubjectError::HandoffIdentityMismatch),
        OccurrenceError::Observation(NativeObservationError::Inspection(
            InspectionError::ProcessIdentityChanged,
        )),
        OccurrenceError::Observation(NativeObservationError::Inspection(InspectionError::Io {
            operation: "observe test compiler",
            source: std::io::Error::from_raw_os_error(libc::EPERM),
        })),
        OccurrenceError::Observation(NativeObservationError::Capability(KeyError::Rejected(
            "invocation changed",
        ))),
        OccurrenceError::Observation(NativeObservationError::Capability(KeyError::Io {
            operation: "read test invocation",
            errno: libc::EACCES,
        })),
        OccurrenceError::Observation(NativeObservationError::Spawn(SpawnError::State(
            "trace custody retired",
        ))),
        OccurrenceError::Observation(NativeObservationError::Spawn(SpawnError::Io {
            operation: "observe test trace",
            source: rustix::io::Errno::PERM,
        })),
        OccurrenceError::Observation(NativeObservationError::Spawn(SpawnError::Retained(
            RetainedError::Poisoned,
        ))),
        OccurrenceError::Observation(NativeObservationError::Spawn(SpawnError::Cleanup(
            CleanupError::Capacity,
        ))),
        OccurrenceError::Observation(NativeObservationError::Spawn(SpawnError::Cleanup(
            CleanupError::GuardIo(rustix::io::Errno::NOMEM),
        ))),
        OccurrenceError::Observation(NativeObservationError::Spawn(SpawnError::SpawnLease(
            fe2o3_artifact_transaction::ArtifactProcessSpawnLeaseErrorV1::CountOverflow,
        ))),
    ];
    for error in errors {
        let issuer = NativeIssuerServiceError::from(error);
        assert_eq!(issuer.resource(), None, "{issuer:?}");
    }
}

#[test]
fn nominal_handoff_diagnostics_are_not_reclassified_as_resource_failures() {
    use crate::compiler_execution_occurrence::{
        NativeOccurrenceError as OccurrenceV2, NativeOccurrenceErrorV3 as OccurrenceV3,
    };
    use crate::{
        ProtectedCompilerExecutionIssuerServiceErrorV2 as IssuerV2,
        ProtectedCompilerExecutionIssuerServiceErrorV3 as IssuerV3,
    };
    use fe2o3_artifact_transaction::{
        CompilerModuleHandoffErrorV1 as Coordination, CompilerModuleHandoffErrorV4 as HandoffV4,
        CompilerModuleHandoffErrorV5 as HandoffV5,
    };

    for handoff in [
        HandoffV4::Busy,
        HandoffV4::MismatchedCurrentnessToken,
        HandoffV4::Coordination(Coordination::Io(std::io::Error::other(
            Resource::Allocation,
        ))),
    ] {
        assert_eq!(
            IssuerV2::from(OccurrenceV2::Handoff(handoff)).resource(),
            None
        );
    }
    for handoff in [
        HandoffV5::Busy,
        HandoffV5::MismatchedCurrentnessToken,
        HandoffV5::Coordination(Coordination::Io(std::io::Error::other(
            Resource::Allocation,
        ))),
    ] {
        assert_eq!(
            IssuerV3::from(OccurrenceV3::Handoff(handoff)).resource(),
            None
        );
    }
}
