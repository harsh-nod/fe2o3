use fe2o3_host::{AuthenticatedKernelArtifactV1, CompilerGeneratedKernelContractV1};

fn open<K: CompilerGeneratedKernelContractV1>(token: AuthenticatedKernelArtifactV1<K>) {
    let AuthenticatedKernelArtifactV1 { validated, binding } = token;
    let _ = (validated, binding);
}

fn main() {}
