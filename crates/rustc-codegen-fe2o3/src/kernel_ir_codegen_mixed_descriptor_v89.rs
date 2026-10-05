//! Exact final V18 LLVM plus mandatory mixed descriptor, without authority.
use super::mixed_descriptor_family::mixed_descriptor_family;

mixed_descriptor_family!(
    MIXED_DESCRIPTOR_READER_STORAGE_V89,
    MixedDescriptorErrorV89,
    MixedDescriptorTableV89,
    decode_mixed_descriptor_v89,
    MixedModuleErrorV89,
    Mixed89,
    "\nmodule asm \".section .fe2o3.kd.v89,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n",
    b"FE2O3/COMPILER-MIXED-DESCRIPTOR/V89\0"
);

#[cfg(test)]
#[path = "kernel_ir_codegen_predicated_layout_v89_tests.rs"]
mod layout_tests;
