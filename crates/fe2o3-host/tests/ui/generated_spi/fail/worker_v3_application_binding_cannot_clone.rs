use fe2o3_host::WorkerV3ApplicationExecutionBindingV1;

fn require_clone<T: Clone>() {}

fn main() {
    require_clone::<WorkerV3ApplicationExecutionBindingV1<()>>();
}
