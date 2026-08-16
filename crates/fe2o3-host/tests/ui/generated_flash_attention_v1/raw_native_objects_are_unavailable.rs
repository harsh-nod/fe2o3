use fe2o3_host::JoinedFlashAttentionV1;

fn expose(joined: &JoinedFlashAttentionV1<'_, '_, '_, '_>) {
    let _executable = joined.executable_object();
    let _kernel = joined.kernel_object();
}

fn main() {}
