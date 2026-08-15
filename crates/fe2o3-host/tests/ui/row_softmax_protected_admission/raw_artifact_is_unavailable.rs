use fe2o3_host::ProtectedRowSoftmaxV1HostTokenV1;

fn extract_bytes(token: &ProtectedRowSoftmaxV1HostTokenV1) {
    let _ = token.exact_finalized_bytes();
}

fn main() {}
