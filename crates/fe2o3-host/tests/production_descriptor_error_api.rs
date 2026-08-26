use fe2o3_host::{
    ApplicationDescriptorHandoffErrorV1, WorkerV3ApplicationDescriptorHandoffErrorV1,
};

#[test]
fn production_descriptor_errors_are_public_without_worker_v2() {
    fn accepts_descriptor(_: Option<ApplicationDescriptorHandoffErrorV1>) {}
    fn accepts_handoff(_: Option<WorkerV3ApplicationDescriptorHandoffErrorV1>) {}

    accepts_descriptor(None);
    accepts_handoff(None);
}
