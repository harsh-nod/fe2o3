use fe2o3_host::{
    ApplicationDescriptorHandoffErrorV1, ProductionWorkerV3KfdApplicationErrorV1,
    ProductionWorkerV3KfdPreparationErrorV1, WorkerV3ApplicationDescriptorHandoffErrorV1,
};

#[test]
fn production_descriptor_errors_are_public_without_worker_v2() {
    fn accepts_descriptor(_: Option<ApplicationDescriptorHandoffErrorV1>) {}
    fn accepts_handoff(_: Option<WorkerV3ApplicationDescriptorHandoffErrorV1>) {}
    fn accepts_kfd_application(_: Option<ProductionWorkerV3KfdApplicationErrorV1<std::io::Error>>) {
    }
    fn accepts_kfd_preparation(_: Option<ProductionWorkerV3KfdPreparationErrorV1<std::io::Error>>) {
    }

    accepts_descriptor(None);
    accepts_handoff(None);
    accepts_kfd_application(None);
    accepts_kfd_preparation(None);
}
