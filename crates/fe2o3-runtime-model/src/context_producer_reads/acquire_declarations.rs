// Private scan state shared by Rust and Verus producer admission.
context_producer_read_declarations_v1! {
#[derive(Clone, Copy)]
struct ProducerReadAcquireScanV1 {
    previous: Option<(u64, u64, u64)>,
    group: usize,
}
}
