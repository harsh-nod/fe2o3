use fe2o3_host::ProtectedRowSoftmaxV1HostTokenV1;

fn require_clone<T: Clone>() {}
fn require_copy<T: Copy>() {}

fn authority_is_linear() {
    require_clone::<ProtectedRowSoftmaxV1HostTokenV1>();
    require_copy::<ProtectedRowSoftmaxV1HostTokenV1>();
}

fn main() {}
