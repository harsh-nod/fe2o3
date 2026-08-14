use fe2o3_artifact_transaction::ProducerIdentity;
use fe2o3_hsaco_finalize::{
    InspectedRawWorkerV2HsacoV1, InspectedScalarGemmV1WorkerV2HsacoV1,
    PreparedFinalizedWorkerV2HsacoV1, PreparedWorkerV2HsacoPublicationV1,
    WorkerV2HsacoFinalizationError, WorkerV2HsacoPublicationError,
    finalize_inspected_worker_v2_hsaco_v1, prepare_worker_v2_hsaco_publication_v1,
};

fn finalize_scalar_inspection(
    inspected: InspectedScalarGemmV1WorkerV2HsacoV1,
) -> Result<PreparedFinalizedWorkerV2HsacoV1, WorkerV2HsacoFinalizationError> {
    finalize_inspected_worker_v2_hsaco_v1(inspected.into_raw())
}

fn prepare_scalar_publication(
    producer: &ProducerIdentity,
    inspected: InspectedScalarGemmV1WorkerV2HsacoV1,
) -> Result<PreparedWorkerV2HsacoPublicationV1, WorkerV2HsacoPublicationError> {
    prepare_worker_v2_hsaco_publication_v1(producer, inspected.into_raw())
}

#[test]
fn scalar_inspection_exposes_only_a_consuming_raw_ownership_bridge() {
    let bridge: fn(InspectedScalarGemmV1WorkerV2HsacoV1) -> InspectedRawWorkerV2HsacoV1 =
        InspectedScalarGemmV1WorkerV2HsacoV1::into_raw;
    let finalize: fn(
        InspectedScalarGemmV1WorkerV2HsacoV1,
    )
        -> Result<PreparedFinalizedWorkerV2HsacoV1, WorkerV2HsacoFinalizationError> =
        finalize_scalar_inspection;
    let prepare_publication: fn(
        &ProducerIdentity,
        InspectedScalarGemmV1WorkerV2HsacoV1,
    ) -> Result<
        PreparedWorkerV2HsacoPublicationV1,
        WorkerV2HsacoPublicationError,
    > = prepare_scalar_publication;

    let _ = (bridge, finalize, prepare_publication);
}
