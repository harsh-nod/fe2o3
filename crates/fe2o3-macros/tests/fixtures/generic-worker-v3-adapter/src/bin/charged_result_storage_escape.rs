fn shed_charge(result: gpu_host::ChargedTypedResultV1<u32>) -> Box<[u32]> {
    result.into_boxed_slice()
}
fn main() {}
