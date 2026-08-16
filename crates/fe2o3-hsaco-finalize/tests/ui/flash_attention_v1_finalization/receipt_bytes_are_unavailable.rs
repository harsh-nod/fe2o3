use fe2o3_hsaco_finalize::FinalizedFlashAttentionV1ReceiptV1;

fn expose(receipt: &FinalizedFlashAttentionV1ReceiptV1) {
    let _bytes = receipt.exact_finalized_bytes();
}

fn main() {}
