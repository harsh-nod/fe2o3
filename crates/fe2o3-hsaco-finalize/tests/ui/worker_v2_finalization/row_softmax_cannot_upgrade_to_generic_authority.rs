use fe2o3_artifact_transaction::ProducerIdentity;
use fe2o3_hsaco_finalize::{
    FinalizedRowSoftmaxV1StructuralHsacoV1, InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1,
    PreparedFinalizedWorkerV2HsacoPublicationV1, PreparedFinalizedWorkerV2HsacoV1,
    PreparedWorkerV2HsacoPublicationV1, WorkerV2HsacoFinalizationError,
    WorkerV2HsacoPublicationError, finalize_inspected_worker_v2_hsaco_v1,
    prepare_finalized_worker_v2_hsaco_publication_v1, prepare_worker_v2_hsaco_publication_v1,
};

fn generic_finalize(
    inspected: InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1,
) -> Result<PreparedFinalizedWorkerV2HsacoV1, WorkerV2HsacoFinalizationError> {
    finalize_inspected_worker_v2_hsaco_v1(inspected)
}

fn raw_publication(
    producer: &ProducerIdentity,
    inspected: InspectedRowSoftmaxV1StructuralWorkerV2HsacoV1,
) -> Result<PreparedWorkerV2HsacoPublicationV1, WorkerV2HsacoPublicationError> {
    prepare_worker_v2_hsaco_publication_v1(producer, inspected)
}

fn finalized_publication(
    producer: &ProducerIdentity,
    finalized: FinalizedRowSoftmaxV1StructuralHsacoV1,
) -> Result<PreparedFinalizedWorkerV2HsacoPublicationV1, WorkerV2HsacoPublicationError> {
    prepare_finalized_worker_v2_hsaco_publication_v1(producer, finalized)
}

fn main() {}
