pub mod __generated {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilerGeneratedKernelProfileV1 {
        TypedVecAddF32V1,
    }

    /// Minimal fixture copy of the host contract consumed by generated bindings.
    ///
    /// # Safety
    ///
    /// Implementors must return the exact compiler-generated artifact for `Self`.
    pub unsafe trait CompilerGeneratedKernelContractV1 {
        const PROFILE: CompilerGeneratedKernelProfileV1;

        fn artifact_container_bytes() -> &'static [u8];
    }
}
