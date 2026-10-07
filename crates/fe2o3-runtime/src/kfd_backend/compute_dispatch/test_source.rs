// Preserve whole-unit source checks after splitting the dispatch implementation.
pub(in crate::kfd_backend) const TEST_SOURCE_V1: &str = concat!(
    include_str!("publication.rs"),
    include_str!("validation.rs"),
    include_str!("polling.rs"),
    include_str!("pending.rs"),
    include_str!("preparation.rs"),
    include_str!("persistent_publication.rs"),
    include_str!("shutdown.rs"),
    include_str!("resident_release.rs"),
    include_str!("semantic_contract.rs"),
    include_str!("../compute_dispatch.rs"),
);
