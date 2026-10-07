// Private scan state shared by Rust and Verus release implementations.
context_producer_read_declarations_v1! {
#[derive(Clone, Copy)]
struct ProducerReadReleaseScanV1 {
    previous: Option<(u64, u64, u64, u64)>,
    group: usize,
}
}
