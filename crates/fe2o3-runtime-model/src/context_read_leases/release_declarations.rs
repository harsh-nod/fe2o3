// Private scan state shared by the Rust and Verus release implementations.
context_read_declarations_v1! {
#[derive(Clone, Copy)]
struct StableReadReleaseScanV1 {
    previous: Option<(u64, u64, u64, u64)>,
    group: usize,
}
}
