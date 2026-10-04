use super::*;
use fe2o3_compiler_closure_capability::{
    CompilerExecutionExternalAnchorDeploymentCapabilityV3 as AnchorDeploymentCap,
    CompilerExecutionExternalAnchorProvisioningCapabilityV3 as ProvisioningCap,
    CompilerExecutionExternalAnchorSigningKeyCapabilityV3 as AnchorKey,
    CompilerExecutionSigningKeyCapabilityV3 as Key,
    CompilerExecutionSupervisorDeploymentCapabilityV3 as DeploymentCap,
};
use fe2o3_compiler_execution_lifecycle::CompilerExecutionServiceLifecycleLeaseV2 as Lease;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorDeploymentV3 as AnchorDeployment,
    CompilerExecutionExternalAnchorProvisioningV3 as Provisioning,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionSupervisorDeploymentV3 as Deployment,
    sealed_static_issuer_runtime_measurement_v1,
};
use fe2o3_compiler_execution_supervisor::IssuerServiceCredentialProfileV1 as Credentials;
use fe2o3_external_anchor_coordinator::PreparedExternalAnchorOccurrenceV3 as Anchor;
use std::fs::File;

// Every constructor's returned delta is reserved immediately on the same budget.
macro_rules! charged {
    ($b:ident, $call:expr) => {{
        let (value, charge) = $call.unwrap();
        $b.reserve_storage(charge.additional_storage()).unwrap();
        value
    }};
}

pub(super) fn prepare(
    f: &mut fixtures::Fixture,
    pool: &mut Cleanup,
    b: &mut Budget<'_>,
) -> Prepared {
    let floor = b.storage();
    for root in [&f.service_root, &f.anchor_root] {
        drop(fs::openat(root, "..", fs::OFlags::RDONLY | fs::OFlags::DIRECTORY | fs::OFlags::CLOEXEC, fs::Mode::empty())
            .expect("actual lifecycle requires CAP_DAC_READ_SEARCH or CAP_DAC_OVERRIDE in the isolated container"));
    }
    let issuer_image = fixtures::measurement(&f.issuer);
    let helper_image = fixtures::measurement(&f.helper);
    let daemon_image = fixtures::measurement(&f.daemon);
    let public = |seed| {
        ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
            .verifying_key()
            .to_bytes()
    };
    let policy = charged!(
        b,
        Policy::new(
            1,
            issuer_image,
            sealed_static_issuer_runtime_measurement_v1(),
            public(7),
            public(9),
            b
        )
    );
    let policy = charged!(b, PolicyCap::create(policy, b));
    let deployment = charged!(
        b,
        Deployment::new(
            fixtures::ISSUER,
            fixtures::ISSUER,
            Service::new(fixtures::ANCHOR, fixtures::ANCHOR).unwrap(),
            daemon_image,
            helper_image,
            policy.policy(),
            b
        )
    );
    let deployment = charged!(b, DeploymentCap::create(deployment, b));
    let anchor_deployment = charged!(
        b,
        AnchorDeployment::new(deployment.deployment(), policy.policy(), daemon_image, b)
    );
    let anchor_deployment = charged!(b, AnchorDeploymentCap::create(anchor_deployment, b));
    let provisioning = charged!(
        b,
        Provisioning::new(anchor_deployment.deployment(), helper_image, b)
    );
    let provisioning = charged!(b, ProvisioningCap::create(provisioning, b));
    b.reserve_storage(32).unwrap();
    let mut seed = [9; 32];
    let anchor_key = charged!(
        b,
        AnchorKey::create_and_zeroize(&mut seed, anchor_deployment.deployment(), b)
    );
    assert_eq!(seed, [0; 32]);
    b.release_storage(32).unwrap();
    let anchor_root = f.anchor_root.try_clone().unwrap();
    b.reserve_storage(Anchor::ROOT_STORAGE).unwrap();
    let anchor_lease = charged!(b, Lease::open(&anchor_root, b));
    b.reserve_storage(
        Image::file_storage_for_length(helper_image.byte_len()).unwrap()
            + Image::file_storage_for_length(daemon_image.byte_len()).unwrap(),
    )
    .unwrap();
    let anchor = charged!(
        b,
        Anchor::prepare(
            File::open(&f.helper).unwrap(),
            File::open(&f.daemon).unwrap(),
            anchor_root,
            anchor_lease,
            anchor_deployment,
            provisioning,
            anchor_key,
            &deployment,
            &policy,
            b
        )
    );
    anchor
        .retain_cleanup_guard(&deployment, &policy, pool, b)
        .unwrap();
    let anchor = charged!(b, anchor.launch(&deployment, &policy, TIMEOUT, pool, b));
    anchor.validate_continuity(&deployment, &policy, b).unwrap();
    b.reserve_storage(32).unwrap();
    let mut seed = [7; 32];
    let key = charged!(b, Key::create_and_zeroize(&mut seed, policy.policy(), b));
    assert_eq!(seed, [0; 32]);
    b.release_storage(32).unwrap();
    let trust = charged!(
        b,
        crate::CompilerExecutionSupervisorTrustV3::new(deployment, policy, key, b)
    );
    let root = f.service_root.try_clone().unwrap();
    b.reserve_storage(Inputs::PAIR_STORAGE).unwrap();
    let supervisor_lease = charged!(b, Lease::open(&root, b));
    let root_lease = charged!(b, Lease::open(&root, b));
    let listener = f.bind();
    let inputs = charged!(
        b,
        Inputs::admit(
            listener,
            root,
            Credentials::new(fixtures::ISSUER, fixtures::ISSUER).unwrap(),
            b
        )
    );
    b.reserve_storage(
        Image::file_storage_for_length(daemon_image.byte_len()).unwrap()
            + Image::file_storage_for_length(helper_image.byte_len()).unwrap()
            + Image::file_storage_for_length(issuer_image.byte_len()).unwrap(),
    )
    .unwrap();
    let sources = crate::CompilerExecutionSupervisorProgramSourcesV1::new(
        File::open(&f.daemon).unwrap(),
        File::open(&f.helper).unwrap(),
        File::open(&f.issuer).unwrap(),
    );
    assert_eq!(
        b.storage(),
        floor
            + Prepared::prepare_input_storage(
                &trust,
                &inputs,
                &supervisor_lease,
                &root_lease,
                &anchor
            )
            .unwrap()
    );
    let prepared = charged!(
        b,
        Prepared::prepare(
            sources,
            trust,
            inputs,
            supervisor_lease,
            root_lease,
            anchor,
            b
        )
    );
    assert_eq!(b.storage(), floor + prepared.retained_storage());
    prepared.validate_cleanup_guard(pool, b).unwrap();
    prepared
}
