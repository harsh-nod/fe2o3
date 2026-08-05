use fe2o3_host::LoadedKernel;

struct Kernel;

fn expose(loaded: &LoadedKernel<Kernel>) {
    let _ = loaded.function();
    let _ = &loaded.ownership;
}

fn main() {}
