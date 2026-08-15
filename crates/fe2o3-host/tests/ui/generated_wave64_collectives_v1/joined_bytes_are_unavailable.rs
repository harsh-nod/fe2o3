use fe2o3_host::JoinedWave64CollectivesV1;

fn extract(value: &JoinedWave64CollectivesV1<'_, '_, '_, '_>) {
    let _ = value.exact_finalized_bytes();
}

fn main() {}
