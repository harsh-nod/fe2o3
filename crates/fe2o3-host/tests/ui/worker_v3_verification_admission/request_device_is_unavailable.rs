use fe2o3_host::{CompilerGeneratedKernelExpectationV1, WorkerV3VerificationRequestV1};

fn extract_device<K: CompilerGeneratedKernelExpectationV1>(
    request: &WorkerV3VerificationRequestV1<'_, K>,
) {
    let _device = request.device();
}

fn main() {}
