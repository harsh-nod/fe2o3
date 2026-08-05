use fe2o3_host::LoadedKernel;

struct Kernel;

fn duplicate(loaded: LoadedKernel<Kernel>) {
    let _first = loaded;
    let _second = loaded;
}

fn main() {}
