use fe2o3_hsaco_finalize::PreparedFinalizedMoeTop2V1HsacoV1;

fn expose(receipt: &PreparedFinalizedMoeTop2V1HsacoV1) {
    receipt.with_exact_finalized_bytes_for_reviewed_moe_top2_runtime_v1(|bytes, _identity| {
        let _ = bytes;
    });
}

fn main() {}
