use std::error::Error;
use std::fs;
use std::path::PathBuf;

use fe2o3_artifacts::{
    DigestAlgorithm, DirectLinkContainerIdentityV1, IdentityText, MeasuredToolIdentity,
    PayloadDigest,
};
use fe2o3_rustc_invocation::InvocationDigestV2;
use fe2o3_worker_v2_bundle::{
    CallerMeasuredBackendInvocationIdentityV2, CallerMeasuredKernelIrIdentityV2,
    CallerMeasuredSemanticWitnessIdentityV2, CallerMeasuredSourceRootIdentityV2,
    CompilerSourceClosureV2, CompilerTransactionEvidenceCapsuleV2,
    CompilerTransactionEvidencePartsV2, WorkerV2LoadEnvelopeV1,
};

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 3 {
        return Err("usage: host-consumer-input-fixture ENVELOPE CAPSULE KERNEL-ID".into());
    }
    let envelope = WorkerV2LoadEnvelopeV1::from_bytes(&fs::read(PathBuf::from(&arguments[0]))?)?;
    let binding = envelope
        .direct_link_evidence()
        .bindings()
        .first()
        .ok_or("envelope has no direct-link binding")?;
    let expectation = binding.expectation();
    let capsule = CompilerTransactionEvidenceCapsuleV2::new(CompilerTransactionEvidencePartsV2 {
        source_closure: CompilerSourceClosureV2::new(
            CallerMeasuredSourceRootIdentityV2::try_from_sha256([0x81; 32])?,
            vec![],
            vec![],
        )?,
        rustc_tool: measured_tool("rustc", 0x82),
        rustc_invocation: InvocationDigestV2::from_bytes([0x84; 32])?,
        backend_tool: measured_tool("rustc-codegen-fe2o3", 0x85),
        backend_invocation: CallerMeasuredBackendInvocationIdentityV2::try_from_sha256([0x87; 32])?,
        semantic_witness: CallerMeasuredSemanticWitnessIdentityV2::try_from_sha256([0x88; 32])?,
        kernel_ir: CallerMeasuredKernelIrIdentityV2::try_from_sha256([0x89; 32])?,
        worker_request: expectation.request_identity(),
        worker_response: expectation.response_identity(),
        target: envelope.published_claim().plan().scope().target(),
        raw_hsaco: expectation.linked_output_identity(),
        finalized_hsaco: expectation.finalized_payload_identity(),
        artifact: DirectLinkContainerIdentityV1::new(
            DigestAlgorithm::Sha256.calculate(&envelope.container().to_bytes()),
        ),
    })?;
    let kernel = envelope
        .descriptor_lineage()
        .table()
        .kernels()
        .first()
        .ok_or("envelope has no kernel")?
        .kernel_id();
    fs::write(PathBuf::from(&arguments[1]), capsule.to_bytes())?;
    fs::write(PathBuf::from(&arguments[2]), hex(kernel.as_bytes()))?;
    Ok(())
}

fn measured_tool(name: &str, seed: u8) -> MeasuredToolIdentity {
    MeasuredToolIdentity::new(
        IdentityText::new(name).expect("fixture tool name"),
        IdentityText::new("fixture-v1").expect("fixture tool version"),
        tagged(seed),
        tagged(seed.wrapping_add(1)),
    )
}

fn tagged(seed: u8) -> PayloadDigest {
    DigestAlgorithm::Sha256.calculate(&[seed; 32])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
