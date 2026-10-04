//! Exact final V18 LLVM plus mandatory mixed descriptor, without authority.
use super::mixed_descriptor_family::mixed_descriptor_family;

mixed_descriptor_family!(
    MIXED_DESCRIPTOR_READER_STORAGE_V53,
    MixedDescriptorErrorV53,
    MixedDescriptorTableV53,
    decode_mixed_descriptor_v53,
    MixedModuleErrorV53,
    Mixed53,
    "\nmodule asm \".section .fe2o3.kd.v53,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n",
    b"FE2O3/COMPILER-MIXED-DESCRIPTOR/V53\0"
);

#[cfg(test)]
#[path = "kernel_ir_codegen_mixed_layout_v60_tests.rs"]
mod layout_tests;
