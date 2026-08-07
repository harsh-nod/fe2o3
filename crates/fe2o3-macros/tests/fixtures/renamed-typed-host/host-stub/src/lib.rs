pub mod __generated {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilerGeneratedKernelProfileV1 {
        TypedVecAddF32RustcLayoutV2,
        ManifestDerivedScalarSliceV1 {
            generated_host_contract_identity: [u8; 32],
        },
    }

    pub struct ValidatedCompilerGeneratedSemanticWitnessV1;

    #[derive(Debug)]
    pub enum CompilerGeneratedSemanticWitnessErrorV1 {
        MissingBackendWitness,
    }

    /// Minimal fixture copy of the shared-bundle expectation contract.
    ///
    /// # Safety
    ///
    /// Implementors must describe the exact compiler-generated kernel marker.
    pub unsafe trait CompilerGeneratedKernelExpectationV1 {
        const PROFILE: CompilerGeneratedKernelProfileV1;
        const KERNEL_BINDING_ID_V1: [u8; 32];

        fn semantic_witness_v1() -> Result<
            ValidatedCompilerGeneratedSemanticWitnessV1,
            CompilerGeneratedSemanticWitnessErrorV1,
        > {
            Err(CompilerGeneratedSemanticWitnessErrorV1::MissingBackendWitness)
        }
    }

    /// Minimal fixture copy of the host contract consumed by generated bindings.
    ///
    /// # Safety
    ///
    /// Implementors must return the exact compiler-generated artifact for `Self`.
    pub unsafe trait CompilerGeneratedKernelContractV1 {
        const PROFILE: CompilerGeneratedKernelProfileV1;
        const KERNEL_BINDING_ID_V1: [u8; 32];

        fn artifact_container_bytes() -> &'static [u8];
    }

    pub struct GeneratedVecAddKernelV1<K: CompilerGeneratedKernelContractV1>(
        core::marker::PhantomData<K>,
    );

    pub struct GeneratedVecAddPreparedV1<'loaded, 'allocation, K: CompilerGeneratedKernelContractV1>(
        core::marker::PhantomData<(&'loaded K, &'allocation K)>,
    );

    /// Minimal fixture copy of the host accessor shim.
    ///
    /// # Safety
    ///
    /// The pointer and length must describe one immutable static allocation.
    pub unsafe fn artifact_bytes_from_backend_v1(
        _pointer: *const u8,
        _length: usize,
    ) -> &'static [u8] {
        &[]
    }

    /// Minimal fixture copy of the V3 semantic-witness parser SPI.
    ///
    /// # Safety
    ///
    /// The pointer and length must describe one immutable backend allocation.
    pub unsafe fn semantic_witness_from_backend_v1(
        _pointer: *const u8,
        _length: usize,
        _kernel_binding: [u8; 32],
        _generated_host_contract: [u8; 32],
    ) -> Result<ValidatedCompilerGeneratedSemanticWitnessV1, CompilerGeneratedSemanticWitnessErrorV1>
    {
        Err(CompilerGeneratedSemanticWitnessErrorV1::MissingBackendWitness)
    }
}
