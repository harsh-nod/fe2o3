use fe2o3_artifacts::{
    AbiLayout, CodeObjectFormat, CodeObjectIdentity, DigestAlgorithm, DigestBytes, Endianness,
    LaunchContract, Name, PayloadDigest, PointerWidth, TargetIdentity,
};
use fe2o3_host::{ArtifactKernelIdentityV1, BlockSizeV1, DimensionsV1, KernelId, LaunchConstraintsV1};

fn forge(
    manifest_digest: PayloadDigest,
    code_object: CodeObjectIdentity,
    target: TargetIdentity,
    abi: AbiLayout,
    launch: LaunchContract,
) {
    let dimensions = DimensionsV1::new(1, 1, 1).unwrap();
    let effective_launch =
        LaunchConstraintsV1::new(1, BlockSizeV1::Any, dimensions, 1, 0, 0).unwrap();
    let _identity = ArtifactKernelIdentityV1 {
        manifest_digest,
        kernel_id: KernelId::from_bytes([0; 32]),
        name: Name::new("kernel").unwrap(),
        symbol: Name::new("kernel.kd").unwrap(),
        source_digest: DigestBytes::from_bytes([1; 32]),
        executable_digest: DigestBytes::from_bytes([2; 32]),
        code_object,
        payload_digest: PayloadDigest::new(
            DigestAlgorithm::Sha256,
            DigestBytes::from_bytes([3; 32]),
        ),
        target,
        required_capabilities: vec![],
        abi,
        launch,
        effective_launch,
    };
}

fn main() {
    let _ = (CodeObjectFormat::NativeExecutable, Endianness::Little, PointerWidth::Bits64);
}
