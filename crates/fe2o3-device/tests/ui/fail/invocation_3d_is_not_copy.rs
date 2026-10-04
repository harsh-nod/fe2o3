use fe2o3_device::Invocation3D;

fn duplicate(invocation: Invocation3D) {
    let first = invocation;
    let second = invocation;
    let _ = (first, second);
}

fn main() {}
