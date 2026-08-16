use fe2o3_hsaco_finalize::FinalizedFlashAttentionV1ReceiptV1;

fn replay(receipt: FinalizedFlashAttentionV1ReceiptV1) {
    let _copy = receipt.clone();
    drop(receipt);
}

fn main() {}
