//! Isolated end-to-end hardware evidence for the repository scalar-add slice.

#[path = "support/gfx942_repository_scalar_add_v1_runner.rs"]
mod runner;

/// Emits deterministic source and handoff identities without creating an HSA context.
#[test]
fn canonical_source_non_authoritative_observation() -> Result<(), Box<dyn std::error::Error>> {
    let observation = runner::observe_canonical_source()?;
    assert!(!observation.grants_authority());
    assert!(!observation.hsa_touched());
    Ok(())
}

/// Emits the reviewed adapter's runtime-stack and physical-device observation
/// for explicit repository requalification. This grants no artifact authority.
#[test]
#[ignore = "requires the exact MI300X GPU-6ced1647a296545c lane"]
fn repository_scalar_add_v1_mi300x_environment_observation()
-> Result<(), Box<dyn std::error::Error>> {
    runner::observe_runtime_environment()
}

/// Builds canonical source through the hardened worker and consumes the sealed
/// receipt on the exact MI300X lane. This dedicated integration-test binary
/// isolates terminal HSA behavior from all host-only tests.
#[test]
#[ignore = "requires the pinned LLVM 22.1.8 worker and exact MI300X GPU-6ced1647a296545c lane"]
fn repository_scalar_add_v1_isolated_mi300x() -> Result<(), Box<dyn std::error::Error>> {
    runner::run()
}
