// Keep the vertical cargo test on the same adversarial corpus as HSACO finalization.
#[path = "../../../fe2o3-hsaco-finalize/tests/worker_v3_hsaco_admission.rs"]
mod shared;

#[allow(
    unused_imports,
    reason = "different vertical test binaries consume different shared fixture constructors"
)]
pub(crate) use shared::{
    PublishedWorkerV3Fixture, TestDirectory, publish_worker_v3_fixture_in_directory,
    published_synthetic_two_kernel_worker_v3_fixture,
    published_synthetic_two_kernel_worker_v3_fixture_with_llvm_build_identity,
    published_worker_v3_fixture, published_worker_v3_fixture_with_llvm_build_identity,
};
