// Private scan state shared by the Rust and Verus acquisition implementations.
context_read_declarations_v1! {
#[derive(Clone, Copy)]
struct StableReadAcquireScanV1 {
    previous: Option<(u64, u64, u64)>,
    group: usize,
}
}
