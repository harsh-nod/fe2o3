use fe2o3_host::JoinedExactLdsGemmSlice1V1;

fn no_raw_load(joined: JoinedExactLdsGemmSlice1V1<'_, '_, '_>) {
    joined.load_executable();
}

fn main() {}
