use fe2o3_host::GeneratedLdsGemmSlice1HostAdapterV1;

fn extract(adapter: &GeneratedLdsGemmSlice1HostAdapterV1<'_, '_, '_>) {
    let _bytes = adapter.explicit_kernarg_bytes_v1();
}

fn main() {}
