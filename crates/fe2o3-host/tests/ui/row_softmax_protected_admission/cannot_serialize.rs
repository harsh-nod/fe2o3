use fe2o3_host::ProtectedRowSoftmaxV1HostTokenV1;

fn serialize(token: &ProtectedRowSoftmaxV1HostTokenV1) {
    let _ = serde_json::to_vec(token);
}

fn main() {}
