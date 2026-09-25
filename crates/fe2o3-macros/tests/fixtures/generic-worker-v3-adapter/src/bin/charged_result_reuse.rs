use gpu_host::{ChargedTypedResultV1, GeneratedRuntimeReadSlice};

fn reuse(result: ChargedTypedResultV1<f32>) {
    let _first = GeneratedRuntimeReadSlice::from_charged_result(result);
    let _second = GeneratedRuntimeReadSlice::from_charged_result(result);
}

fn main() {}
