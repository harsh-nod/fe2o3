use fe2o3_hsaco_finalize::FinalizedFlashAttentionV1ReceiptV1;

fn expose(receipt: &FinalizedFlashAttentionV1ReceiptV1) {
    receipt.with_exact_finalized_bytes_for_reviewed_flash_runtime_v1(|bytes, _identity| {
        let _ = bytes;
    });
}

fn main() {}
