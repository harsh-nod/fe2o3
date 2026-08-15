use fe2o3_host::ProtectedRowSoftmaxV1HostTokenV1;

fn consume(_: ProtectedRowSoftmaxV1HostTokenV1) {}

fn replay(token: ProtectedRowSoftmaxV1HostTokenV1) {
    consume(token);
    consume(token);
}

fn main() {}
