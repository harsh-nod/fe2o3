//! Read existing fixed provisioning only. No record/key/profile generation.
use super::*;
use crate::{
    CompilerExecutionSupervisorProgramSourcesV1 as Sources,
    CompilerExecutionSupervisorTrustV3 as Trust, native_root_source as source,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionClientProfileCapabilityV3 as Profile,
    CompilerExecutionExternalAnchorDeploymentCapabilityV3 as AnchorCap,
    CompilerExecutionExternalAnchorProvisioningCapabilityV3 as ProvisioningCap,
    CompilerExecutionExternalAnchorSigningKeyCapabilityV3 as AnchorKey,
    CompilerExecutionPolicyCapabilityV3 as PolicyCap,
    CompilerExecutionSigningKeyCapabilityV3 as Key,
    CompilerExecutionSupervisorDeploymentCapabilityV3 as DeploymentCap,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V3 as ANCHOR_BYTES,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V3 as PROVISIONING_BYTES,
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V3 as POLICY_BYTES,
    COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V3 as DEPLOYMENT_BYTES,
    CompilerExecutionExternalAnchorDeploymentV3 as AnchorDeployment,
    CompilerExecutionExternalAnchorProvisioningV3 as Provisioning,
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionSupervisorDeploymentV3 as Deployment,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V3 as MAX_ANCHOR,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_HELPER_BYTES_V3 as MAX_HELPER,
    MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V3 as MAX_SUPERVISOR,
    MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V3 as MAX_LAUNCHER,
};
use fe2o3_protected_static_executable::ProtectedStaticExecutableV2 as Image;
use zeroize::Zeroizing;

pub(super) struct Provisioned {
    pub trust: Trust,
    pub anchor: AnchorCap,
    pub provisioning: ProvisioningCap,
    pub anchor_key: AnchorKey,
    pub programs: Sources,
    pub helper: File,
    pub daemon: File,
}

impl Provisioned {
    pub(super) fn read(b: &mut Budget<'_>) -> Self {
        let policy = record::<POLICY_BYTES, _>("issuer-policy-v3", b, |bytes, b| {
            let (record, charge) = Policy::decode(bytes, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (cap, charge) = PolicyCap::create(record, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let retained = cap.retained_storage();
            Ok((cap, retained))
        });
        let deployment =
            record::<DEPLOYMENT_BYTES, _>("supervisor-deployment-v3", b, |bytes, b| {
                let (record, charge) = Deployment::decode(bytes, policy.policy(), b)?;
                b.reserve_storage(charge.additional_storage())?;
                let (cap, charge) = DeploymentCap::create(record, b)?;
                b.reserve_storage(charge.additional_storage())?;
                let retained = cap.retained_storage();
                Ok((cap, retained))
            });
        let anchor = record::<ANCHOR_BYTES, _>("anchor-deployment-v3", b, |bytes, b| {
            let (record, charge) =
                AnchorDeployment::decode(bytes, deployment.deployment(), policy.policy(), b)?;
            b.reserve_storage(charge.additional_storage())?;
            let (cap, charge) = AnchorCap::create(record, b)?;
            b.reserve_storage(charge.additional_storage())?;
            let retained = cap.retained_storage();
            Ok((cap, retained))
        });
        let provisioning =
            record::<PROVISIONING_BYTES, _>("anchor-provisioning-v3", b, |bytes, b| {
                let (record, charge) = Provisioning::decode(bytes, anchor.deployment(), b)?;
                b.reserve_storage(charge.additional_storage())?;
                let (cap, charge) = ProvisioningCap::create(record, b)?;
                b.reserve_storage(charge.additional_storage())?;
                let retained = cap.retained_storage();
                Ok((cap, retained))
            });
        // Load the real profile independently, never derive one to fit test keys.
        let (profile, charge) = Profile::from_production_profile(b)
            .expect("genuinely provisioned fixed V3 client profile required");
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(
            profile.profile().policy().canonical_bytes(),
            policy.policy().canonical_bytes()
        );
        assert_eq!(
            profile.profile().supervisor_uid(),
            deployment.deployment().service_uid()
        );
        assert_eq!(
            profile.profile().supervisor_gid(),
            deployment.deployment().service_gid()
        );
        assert_eq!(
            profile.profile().external_anchor_service(),
            deployment.deployment().external_anchor_service()
        );
        drop(profile);
        b.release_storage(charge.additional_storage()).unwrap();

        let supervisor = program(
            "fe2o3-compiler-execution-supervisor-v3",
            deployment.deployment().executable(),
            MAX_SUPERVISOR,
            b,
        );
        let launcher = program(
            "fe2o3-static-preexec-launcher",
            deployment.deployment().launcher(),
            MAX_LAUNCHER,
            b,
        );
        let issuer = program(
            "fe2o3-compiler-execution-issuer-conditional",
            policy.policy().executable(),
            MAX_LAUNCHER,
            b,
        );
        let helper = program(
            "fe2o3-external-anchor-provisioning-helper-v3",
            provisioning.provisioning().helper(),
            MAX_HELPER,
            b,
        );
        let daemon = program(
            "fe2o3-external-anchor-service-v3",
            anchor.deployment().executable(),
            MAX_ANCHOR,
            b,
        );

        let (mut seed, retained) = read_seed("issuer-signing-key-seed-v3", b);
        let (key, charge) = Key::create_and_zeroize(&mut seed, policy.policy(), b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert!(
            seed.iter().all(|byte| *byte == 0),
            "issuer seed scratch must be wiped"
        );
        drop(seed);
        b.release_storage(retained).unwrap();
        let (mut seed, retained) = read_seed("anchor-signing-key-seed-v3", b);
        let (anchor_key, charge) =
            AnchorKey::create_and_zeroize(&mut seed, anchor.deployment(), b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert!(
            seed.iter().all(|byte| *byte == 0),
            "anchor seed scratch must be wiped"
        );
        drop(seed);
        b.release_storage(retained).unwrap();
        let (trust, charge) = Trust::new(deployment, policy, key, b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        Self {
            trust,
            anchor,
            provisioning,
            anchor_key,
            programs: Sources::new(supervisor, launcher, issuer),
            helper,
            daemon,
        }
    }
}

fn record<const N: usize, T>(
    name: &'static str,
    b: &mut Budget<'_>,
    decode: impl FnOnce(&[u8; N], &mut Budget<'_>) -> root::Result<(T, usize)>,
) -> T {
    let input = source::record_input_storage::<N>().unwrap();
    b.reserve_storage(input).unwrap();
    let file = fixed_file(["etc", "fe2o3", "compiler-execution", name], b);
    let (value, retained) = root::record(file, b, decode)
        .expect("existing root-provisioned V3 record must pass production source/codec admission");
    b.reserve_storage(retained).unwrap();
    b.release_storage(input).unwrap(); // The consumed source File is now closed.
    value
}

fn read_seed(name: &'static str, b: &mut Budget<'_>) -> (Zeroizing<[u8; 32]>, usize) {
    let input = source::record_input_storage::<32>().unwrap();
    b.reserve_storage(input).unwrap();
    let file = fixed_file(["etc", "fe2o3", "compiler-execution", name], b);
    let (seed, charge) = source::read_seed(&file, b)
        .expect("existing root-owned mode0400 seed required; never generate a test key");
    b.reserve_storage(charge.additional_storage()).unwrap();
    drop(file);
    b.release_storage(input).unwrap();
    (seed, charge.additional_storage())
}

fn program(name: &'static str, measurement: Measurement, maximum: u64, b: &mut Budget<'_>) -> File {
    let measured = crate::native::measurement(measurement, maximum).unwrap();
    b.reserve_storage(Image::file_storage(measured).unwrap())
        .unwrap();
    let file = fixed_file(["usr", "libexec", "fe2o3", name], b);
    source::validate_executable(&file, usize::try_from(measurement.byte_len()).unwrap(), b)
        .unwrap();
    // Actual content/static-image admission is performed by Anchor/Prepared below.
    file
}

// Fixed depth and fixed call sites; no environment-selected trust root. The
// production source readers subsequently enforce exact file metadata and bytes.
fn fixed_file(parts: [&str; 4], b: &mut Budget<'_>) -> File {
    b.charge_work(32 * 1024).unwrap();
    let flags = fs::OFlags::RDONLY | fs::OFlags::CLOEXEC | fs::OFlags::NOFOLLOW;
    let mut parent = fs::open("/", flags | fs::OFlags::DIRECTORY, fs::Mode::empty()).unwrap();
    for &part in &parts[..3] {
        assert_directory(&parent);
        parent = fs::openat(
            &parent,
            part,
            flags | fs::OFlags::DIRECTORY,
            fs::Mode::empty(),
        )
        .unwrap();
    }
    assert_directory(&parent);
    File::from(
        fs::openat(
            &parent,
            parts[3],
            flags | fs::OFlags::NONBLOCK,
            fs::Mode::empty(),
        )
        .unwrap(),
    )
}

fn assert_directory(file: &OwnedFd) {
    let m = fs::fstat(file).unwrap();
    assert_eq!((m.st_uid, m.st_gid), (0, 0));
    assert_eq!(
        fs::FileType::from_raw_mode(m.st_mode),
        fs::FileType::Directory
    );
    assert_eq!(m.st_mode & 0o7022, 0, "protected parent mode");
    assert_ne!(m.st_mode & 0o100, 0, "root must traverse parent");
    for name in [
        "security.capability",
        "system.posix_acl_access",
        "system.posix_acl_default",
    ] {
        assert!(matches!(
            fs::fgetxattr(file, name, &mut [0; 1]),
            Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP)
        ));
    }
}
