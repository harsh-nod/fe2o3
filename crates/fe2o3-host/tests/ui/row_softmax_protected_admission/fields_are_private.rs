use fe2o3_host::ProtectedRowSoftmaxV1HostTokenV1;

fn extract(token: ProtectedRowSoftmaxV1HostTokenV1) {
    let ProtectedRowSoftmaxV1HostTokenV1 {
        identity,
        admission,
    } = token;
    let _ = (identity, admission);
}

fn main() {}
