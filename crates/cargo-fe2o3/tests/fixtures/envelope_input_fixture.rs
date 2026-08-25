use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use fe2o3_artifacts::{
    BundleIndexV1, DigestAlgorithm, DigestBytes, DirectLinkBindingExpectationV1,
    DirectLinkBindingSourceV1, DirectLinkBundleEvidenceV1, DirectLinkFfiClosureIdentityV1,
    DirectLinkFinalizationIdentityV1, DirectLinkFinalizedPayloadIdentityV1,
    DirectLinkLinkedOutputIdentityV1, DirectLinkRequestIdentityV1, DirectLinkResponseIdentityV1,
    DirectLinkToolchainConfigurationIdentityV1, DirectLinkToolchainExecutableIdentityV1,
    DirectLinkToolchainIdentityV1, DirectLinkTransformationIdentityV1,
    DirectLinkWorkerConfigurationIdentityV1, DirectLinkWorkerExecutableIdentityV1,
    DirectLinkWorkerIdentityV1, IdentityText, MeasuredToolIdentity, PayloadDigest,
    ProofArtifactIdentity, ProofExecutionIdentity, ProofOutcome, ProofRecordV1,
    ProofTargetIdentity, SourceContractIdentity, VerificationModelIdentity,
};
use fe2o3_worker_v2_bundle::{ExactRawHsacoV1, WorkerV2EnvelopeInputsV1};

#[allow(dead_code)]
#[path = "../../src/worker_v2_artifact_container.rs"]
mod worker_v2_artifact_container;

#[allow(dead_code)]
#[path = "../../src/worker_v2_envelope_mode.rs"]
mod worker_v2_envelope_mode;

#[allow(dead_code)]
#[path = "../../src/worker_v2_restart.rs"]
mod worker_v2_restart;

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if !(3..=4).contains(&arguments.len()) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "usage: envelope-input-fixture RAW FINALIZED OUTPUT [IDENTITY-SEED]",
        )
        .into());
    }
    let raw_path = PathBuf::from(arguments[0].as_os_str());
    let finalized_path = PathBuf::from(arguments[1].as_os_str());
    let output_path = PathBuf::from(arguments[2].as_os_str());
    let identity_seed = arguments
        .get(3)
        .map(|value| {
            value
                .to_str()
                .ok_or_else(|| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "identity seed is not UTF-8",
                    )
                })?
                .parse::<u8>()
                .map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "identity seed is not a byte",
                    )
                })
        })
        .transpose()?
        .unwrap_or(0);
    let raw = fs::read(raw_path)?;
    let finalized = fs::read(finalized_path)?;
    let container =
        worker_v2_artifact_container::canonical_worker_v2_container_for_fixture_v1(&finalized)?;
    let bundle = BundleIndexV1::from_containers(std::slice::from_ref(&container))?;
    let raw_hsaco = ExactRawHsacoV1::from_bytes(raw)?;
    let finalized_identity = DigestAlgorithm::Sha256.calculate(&finalized);
    let tagged = |seed: u8| payload_digest([seed.wrapping_add(identity_seed); 32]);
    let expectation = DirectLinkBindingExpectationV1::new(
        DirectLinkRequestIdentityV1::new(tagged(0x71)),
        DirectLinkWorkerIdentityV1::new(
            text("fixture-worker"),
            text("1"),
            DirectLinkWorkerExecutableIdentityV1::new(tagged(0x72)),
            DirectLinkWorkerConfigurationIdentityV1::new(tagged(0x73)),
        ),
        DirectLinkToolchainIdentityV1::new(
            text("fixture-toolchain"),
            text("1"),
            DirectLinkToolchainExecutableIdentityV1::new(tagged(0x74)),
            DirectLinkToolchainConfigurationIdentityV1::new(tagged(0x75)),
        ),
        DirectLinkResponseIdentityV1::new(tagged(0x76)),
        DirectLinkTransformationIdentityV1::new(
            DirectLinkLinkedOutputIdentityV1::new(raw_hsaco.identity()),
            DirectLinkFinalizationIdentityV1::new(tagged(0x77)),
            DirectLinkFinalizedPayloadIdentityV1::new(finalized_identity),
        ),
        DirectLinkFfiClosureIdentityV1::new(tagged(0x78)),
    );
    let direct_link = DirectLinkBundleEvidenceV1::bind(
        &bundle,
        &[&container],
        &[DirectLinkBindingSourceV1::new(&container, expectation)],
    )?;
    let proofs = container
        .manifest()
        .kernels()
        .iter()
        .map(proof_record)
        .collect::<Result<Vec<_>, _>>()?;
    let capsule = WorkerV2EnvelopeInputsV1::new(direct_link, proofs, raw_hsaco)?;
    capsule.validate_against_container(&container)?;

    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(output_path)?;
    output.write_all(&capsule.to_bytes())?;
    output.sync_all()?;
    Ok(())
}

fn proof_record(
    kernel: &fe2o3_artifacts::KernelEntry,
) -> Result<ProofRecordV1, fe2o3_artifacts::ValidationError> {
    let tagged = |seed| payload_digest([seed; 32]);
    ProofRecordV1::new(
        ProofTargetIdentity::new(
            ProofArtifactIdentity::new(
                payload_digest(*kernel.kernel_id().as_bytes()),
                tagged(0x41),
                payload_digest(*kernel.source_digest().as_bytes()),
                tagged(0x42),
                payload_digest(*kernel.executable_digest().as_bytes()),
                tagged(0x43),
                tagged(0x44),
                tagged(0x45),
            ),
            SourceContractIdentity::new(
                tagged(0x51),
                tagged(0x52),
                tagged(0x53),
                tagged(0x54),
                tagged(0x55),
            ),
        ),
        vec![],
        ProofExecutionIdentity::new(
            VerificationModelIdentity::new(text("fixture-model"), tagged(0x61)),
            MeasuredToolIdentity::new(
                text("fixture-verifier"),
                text("1"),
                tagged(0x62),
                tagged(0x63),
            ),
            MeasuredToolIdentity::new(
                text("fixture-solver"),
                text("1"),
                tagged(0x64),
                tagged(0x65),
            ),
            MeasuredToolIdentity::new(
                text("fixture-recorder"),
                text("1"),
                tagged(0x66),
                tagged(0x67),
            ),
            tagged(0x68),
        ),
        ProofOutcome::Failed,
        vec![],
        vec![],
    )
}

fn payload_digest(bytes: [u8; 32]) -> PayloadDigest {
    PayloadDigest::new(DigestAlgorithm::Sha256, DigestBytes::from_bytes(bytes))
}

fn text(value: &str) -> IdentityText {
    IdentityText::new(value).expect("fixture identity text")
}
