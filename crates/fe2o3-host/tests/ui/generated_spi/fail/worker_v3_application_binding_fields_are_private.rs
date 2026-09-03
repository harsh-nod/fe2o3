use fe2o3_host::WorkerV3ApplicationExecutionBindingV1;

fn expose<K>(binding: WorkerV3ApplicationExecutionBindingV1<K>) {
    let WorkerV3ApplicationExecutionBindingV1 {
        authenticated,
        coordinates,
        packing,
    } = binding;
    let _ = (authenticated, coordinates, packing);
}

fn main() {}
