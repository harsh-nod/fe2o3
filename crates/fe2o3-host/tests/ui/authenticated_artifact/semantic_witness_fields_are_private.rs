use fe2o3_host::__generated::{
    CompilerGeneratedKernelProfileV1, ValidatedCompilerGeneratedSemanticWitnessV1,
};

fn main() {
    let _forged = ValidatedCompilerGeneratedSemanticWitnessV1 {
        profile: CompilerGeneratedKernelProfileV1::new([2; 32]),
        kernel_binding: [1; 32],
        generated_host_contract: [2; 32],
    };
}
