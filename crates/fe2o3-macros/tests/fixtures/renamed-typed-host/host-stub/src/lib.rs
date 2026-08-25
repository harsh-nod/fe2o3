pub mod __generated {
    use core::marker::PhantomData;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum CompilerGeneratedKernelProfileV1 {
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

    pub struct ObservedContext;

    pub struct DeviceBuffer<T>(PhantomData<T>);

    #[derive(Debug)]
    pub struct RegionError;

    #[derive(Clone, Copy)]
    pub struct TypeIdentity;

    pub trait GeneratedDeviceScalarV1: Copy {
        fn scalar_type_identity_v1(_width: PointerWidth) -> TypeIdentity {
            TypeIdentity
        }

        fn shared_slice_type_identity_v1(_width: PointerWidth) -> TypeIdentity {
            TypeIdentity
        }

        fn disjoint_slice_type_identity_v1(_width: PointerWidth) -> TypeIdentity {
            TypeIdentity
        }
    }

    impl GeneratedDeviceScalarV1 for u32 {}
    impl GeneratedDeviceScalarV1 for f32 {}

    pub struct GeneratedReadDeviceSlice<'allocation, T> {
        _buffer: &'allocation DeviceBuffer<T>,
    }

    impl<'allocation, T> GeneratedReadDeviceSlice<'allocation, T> {
        pub fn new(
            _observed: &ObservedContext,
            buffer: &'allocation DeviceBuffer<T>,
        ) -> Result<Self, RegionError> {
            Ok(Self { _buffer: buffer })
        }

        pub fn bind_argument_pair(
            &self,
            _plan: &GeneratedArgumentPackingPlanV1,
            _argument_index: usize,
        ) -> Result<GeneratedSliceArgumentPairV1<'allocation>, GeneratedArgumentPackError> {
            Ok(GeneratedSliceArgumentPairV1(PhantomData))
        }
    }

    pub struct GeneratedReadWriteDeviceSlice<'allocation, T> {
        _buffer: &'allocation mut DeviceBuffer<T>,
    }

    impl<'allocation, T> GeneratedReadWriteDeviceSlice<'allocation, T> {
        pub fn new(
            _observed: &ObservedContext,
            buffer: &'allocation mut DeviceBuffer<T>,
        ) -> Result<Self, RegionError> {
            Ok(Self { _buffer: buffer })
        }

        pub fn len(&self) -> usize {
            1
        }

        pub fn bind_argument_pair(
            &self,
            _plan: &GeneratedArgumentPackingPlanV1,
            _argument_index: usize,
        ) -> Result<GeneratedSliceArgumentPairV1<'allocation>, GeneratedArgumentPackError> {
            Ok(GeneratedSliceArgumentPairV1(PhantomData))
        }
    }

    #[derive(Clone, Copy)]
    pub enum PointerWidth {
        Bits64,
    }

    pub enum ScalarType {
        I8,
        U8,
        I16,
        U16,
        I32,
        U32,
        I64,
        U64,
        F32,
        F64,
    }

    pub enum AbiKind {
        Scalar(ScalarType),
        Slice {
            element_size: u64,
            element_alignment: u32,
        },
    }

    pub enum Mutability {
        Immutable,
        Mutable,
    }

    pub enum Access {
        ByValue,
        ReadOnly,
        ReadWrite,
    }

    pub enum AddressSpace {
        Value,
        Global,
    }

    pub enum ArgumentOwnership {
        ByValue,
        SharedBorrow,
        UniqueBorrow,
    }

    pub enum AliasClass {
        Value,
        SharedReadOnly,
        Exclusive,
    }

    pub struct Name;

    impl Name {
        pub fn new(_value: &str) -> Result<Self, GeneratedArgumentLayoutError> {
            Ok(Self)
        }
    }

    pub struct AbiField;

    impl AbiField {
        #[allow(clippy::too_many_arguments)]
        pub fn new(
            _name: Name,
            _offset: u64,
            _size: u64,
            _alignment: u32,
            _kind: AbiKind,
            _mutability: Mutability,
            _access: Access,
            _address_space: AddressSpace,
            _type_identity: TypeIdentity,
            _ownership: ArgumentOwnership,
            _alias: AliasClass,
        ) -> Result<Self, GeneratedArgumentLayoutError> {
            Ok(Self)
        }
    }

    #[derive(Debug)]
    pub struct GeneratedArgumentLayoutError;

    #[derive(Debug)]
    pub struct GeneratedArgumentPackError;

    #[derive(Debug)]
    pub struct GeneratedWorkerV3PrepareErrorV1;

    pub struct CompilerGeneratedArgumentLayoutV1;

    impl CompilerGeneratedArgumentLayoutV1 {
        pub fn new(
            _size: u64,
            _alignment: u32,
            _pointer_width: PointerWidth,
            _fields: Vec<AbiField>,
        ) -> Result<Self, GeneratedArgumentLayoutError> {
            Ok(Self)
        }
    }

    pub struct GeneratedArgumentInputV1<'allocation>(PhantomData<&'allocation ()>);
    pub struct GeneratedSliceArgumentPairV1<'allocation>(PhantomData<&'allocation ()>);
    pub struct GeneratedArgumentPackingPlanV1;

    impl GeneratedArgumentPackingPlanV1 {
        pub fn scalar<T: GeneratedDeviceScalarV1>(
            &self,
            _argument_index: usize,
            _value: T,
        ) -> Result<GeneratedArgumentInputV1<'static>, GeneratedArgumentPackError> {
            Ok(GeneratedArgumentInputV1(PhantomData))
        }
    }

    pub struct GeneratedWorkerV3ArgumentBindingV1<'allocation>(PhantomData<&'allocation ()>);

    impl<'allocation> GeneratedWorkerV3ArgumentBindingV1<'allocation> {
        pub fn from_compiler_generated_parts_v1(
            _scalars: Vec<GeneratedArgumentInputV1<'static>>,
            _memory: Vec<GeneratedSliceArgumentPairV1<'allocation>>,
        ) -> Self {
            Self(PhantomData)
        }
    }

    /// Minimal fixture copy of the compiler-only generated argument bridge.
    ///
    /// # Safety
    ///
    /// Implementations must describe the exact marker signature.
    pub unsafe trait CompilerGeneratedWorkerV3ArgumentsV1<
        'allocation,
        K: CompilerGeneratedKernelExpectationV1,
    > {
        fn generated_argument_layout_v1(
        ) -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError>;

        fn bind_arguments_v1(
            &self,
            plan: &GeneratedArgumentPackingPlanV1,
        ) -> Result<GeneratedWorkerV3ArgumentBindingV1<'allocation>, GeneratedArgumentPackError>;
    }

    pub struct HsaLaunchGeometryV1;

    pub trait ReviewedHsaImplicitKernargAdapterV1 {}

    pub struct LoadedWorkerV3HsaExecutableV1<K, A>(PhantomData<(K, A)>);

    pub struct GeneratedWorkerV3PreparedInvocationV1<
        'loaded,
        'allocation,
        K,
        A,
        Arguments,
    >(PhantomData<(&'loaded K, &'allocation K, A, Arguments)>);

    impl<K: CompilerGeneratedKernelExpectationV1, A: ReviewedHsaImplicitKernargAdapterV1>
        LoadedWorkerV3HsaExecutableV1<K, A>
    {
        pub fn prepare_generated_worker_v3_v1<'loaded, 'allocation, Arguments>(
            &'loaded mut self,
            _observed: &ObservedContext,
            _geometry: HsaLaunchGeometryV1,
            _arguments: Arguments,
        ) -> Result<
            GeneratedWorkerV3PreparedInvocationV1<'loaded, 'allocation, K, A, Arguments>,
            GeneratedWorkerV3PrepareErrorV1,
        >
        where
            Arguments: CompilerGeneratedWorkerV3ArgumentsV1<'allocation, K>,
        {
            Ok(GeneratedWorkerV3PreparedInvocationV1(PhantomData))
        }
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

pub use __generated::{
    HsaLaunchGeometryV1, LoadedWorkerV3HsaExecutableV1, ObservedContext,
    ReviewedHsaImplicitKernargAdapterV1,
};
