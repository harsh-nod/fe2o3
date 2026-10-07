// Compiled from the same declaration tokens by Rust and Verus.
enrollment_declarations_v1! {
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContextAllocationEnrollmentV1 {
    pub key: ContextAllocationKeyV1,
    pub device: ContextJournalDeviceKeyV1,
    pub byte_extent: u64,
}
}
