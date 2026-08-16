use fe2o3_hsaco_finalize::PreparedFinalizedMoeTop2V1HsacoV1;

fn replay(receipt: PreparedFinalizedMoeTop2V1HsacoV1) {
    let _copy = receipt.clone();
    drop(receipt);
}

fn main() {}
