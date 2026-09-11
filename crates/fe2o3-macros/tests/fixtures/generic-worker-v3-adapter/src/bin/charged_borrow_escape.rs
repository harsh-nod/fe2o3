fn main() {
    let values = [1u32];
    let _ = gpu_host::GeneratedRuntimeReadWriteSlice::new_charged(&values[..]);
}
