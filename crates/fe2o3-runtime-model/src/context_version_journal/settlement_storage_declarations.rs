settlement_storage_declarations_v1! {
#[derive(Clone, Copy)]
pub(super) struct SettlementReturnStorageV1 {
    pub(super) writer_free_len: usize,
    pub(super) member_free_len: usize,
    pub(super) writer_limit: usize,
    pub(super) writer_storage: usize,
    pub(super) member_limit: usize,
    pub(super) member_storage: usize,
    pub(super) scratch_len: usize,
}
}
