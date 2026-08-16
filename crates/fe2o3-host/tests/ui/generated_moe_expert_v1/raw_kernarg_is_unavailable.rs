use fe2o3_host::GeneratedMoeExpertV1HostAdapterV1;

fn expose(host: &GeneratedMoeExpertV1HostAdapterV1<'_, '_, '_, '_, '_, '_, '_, '_>) {
    let _ = host.gemm_kernarg_bytes_v1(0);
    let _ = host.combine_kernarg_bytes_v1();
}

fn main() {}
