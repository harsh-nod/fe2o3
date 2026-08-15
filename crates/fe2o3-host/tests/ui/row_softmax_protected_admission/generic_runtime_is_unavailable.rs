use fe2o3_host::ProtectedRowSoftmaxV1HostTokenV1;

fn bypass_exact_path(token: ProtectedRowSoftmaxV1HostTokenV1) {
    let loaded = token.load_executable();
    loaded.launch();
}

fn main() {}
