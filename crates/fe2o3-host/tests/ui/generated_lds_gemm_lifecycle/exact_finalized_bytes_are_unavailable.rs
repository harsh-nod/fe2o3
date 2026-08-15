use fe2o3_host::JoinedExactLdsGemmSlice1V1;

fn extract_bytes(joined: &JoinedExactLdsGemmSlice1V1<'_, '_, '_>) {
    let _bytes = joined.exact_finalized_bytes();
}

fn main() {}
