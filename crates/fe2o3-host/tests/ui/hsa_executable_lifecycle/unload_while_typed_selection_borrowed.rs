use fe2o3_host::{
    CompilerGeneratedKernelContractV1, LoadedHsaExecutableV1,
    ReviewedHsaExecutableLifecycleAdapterV1,
};

fn unload_while_selected<K, S, A>(loaded: LoadedHsaExecutableV1<K, A>)
where
    S: CompilerGeneratedKernelContractV1,
    A: ReviewedHsaExecutableLifecycleAdapterV1,
{
    let selection = loaded.select_typed_kernel::<S>();
    let _unloaded = loaded.unload();
    drop(selection);
}

fn main() {}
