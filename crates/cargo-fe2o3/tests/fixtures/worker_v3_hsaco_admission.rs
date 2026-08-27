// Keep the vertical cargo test on the same adversarial corpus as HSACO finalization.
#[path = "../../../fe2o3-hsaco-finalize/tests/worker_v3_hsaco_admission.rs"]
mod shared;

pub(crate) use shared::{
    PublishedWorkerV3Fixture, TestDirectory, publish_worker_v3_fixture_in_directory,
    published_worker_v3_fixture,
};
