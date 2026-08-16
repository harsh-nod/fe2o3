use fe2o3_host::JoinedMoeTop2V1;

fn expose(joined: &JoinedMoeTop2V1<'_, '_, '_, '_, '_, '_, '_, '_>) {
    let _executable = joined.executable_object();
    let _kernel = joined.kernel_object();
}

fn main() {}
