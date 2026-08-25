use fe2o3_host::LoadedKernel;

fn main() {
    let _ = core::mem::size_of::<LoadedKernel<()>>();
}
