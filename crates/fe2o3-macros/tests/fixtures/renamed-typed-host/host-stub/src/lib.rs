pub mod __generated {
    use core::marker::PhantomData;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CompilerGeneratedKernelProfileV1 {
        generated_host_contract_identity: [u8; 32],
    }

    impl CompilerGeneratedKernelProfileV1 {
        pub const fn new(generated_host_contract_identity: [u8; 32]) -> Self {
            Self {
                generated_host_contract_identity,
            }
        }

        pub const fn generated_host_contract_identity(self) -> [u8; 32] {
            self.generated_host_contract_identity
        }
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

    #[derive(Debug)]
    pub struct RegionError;

    #[derive(Clone, Copy)]
    pub struct TypeIdentity;

    pub trait GeneratedDeviceScalarV1: Copy + Send + Sync + 'static {
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
    pub enum GeneratedKfdArgumentError {
        Pack(GeneratedArgumentPackError),
    }

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

    pub struct GeneratedKfdSliceBinding<'allocation>(PhantomData<&'allocation ()>);

    pub struct GeneratedKfdReadSlice<'allocation, T> {
        _values: &'allocation [T],
    }

    impl<'allocation, T> GeneratedKfdReadSlice<'allocation, T> {
        pub fn new(values: &'allocation [T]) -> Self {
            Self { _values: values }
        }

        pub fn bind_argument(
            self,
            _plan: &GeneratedArgumentPackingPlanV1,
            _argument_index: usize,
        ) -> Result<GeneratedKfdSliceBinding<'allocation>, GeneratedKfdArgumentError> {
            Ok(GeneratedKfdSliceBinding(PhantomData))
        }
    }

    pub struct GeneratedKfdReadWriteSlice<'allocation, T> {
        _values: &'allocation mut [T],
    }

    impl<'allocation, T> GeneratedKfdReadWriteSlice<'allocation, T> {
        pub fn new(values: &'allocation mut [T]) -> Self {
            Self { _values: values }
        }

        pub fn len(&self) -> usize {
            self._values.len()
        }

        pub fn bind_argument(
            self,
            _plan: &GeneratedArgumentPackingPlanV1,
            _argument_index: usize,
        ) -> Result<GeneratedKfdSliceBinding<'allocation>, GeneratedKfdArgumentError> {
            Ok(GeneratedKfdSliceBinding(PhantomData))
        }
    }

    pub struct GeneratedKfdWriteSlice<'allocation, T> {
        _values: &'allocation mut [T],
    }

    impl<'allocation, T> GeneratedKfdWriteSlice<'allocation, T> {
        pub fn new(values: &'allocation mut [T]) -> Self {
            Self { _values: values }
        }

        pub fn bind_argument(
            self,
            _plan: &GeneratedArgumentPackingPlanV1,
            _argument_index: usize,
        ) -> Result<GeneratedKfdSliceBinding<'allocation>, GeneratedKfdArgumentError> {
            Ok(GeneratedKfdSliceBinding(PhantomData))
        }
    }

    pub struct GeneratedKfdArgumentBinding<'allocation>(PhantomData<&'allocation ()>);

    impl<'allocation> GeneratedKfdArgumentBinding<'allocation> {
        pub fn from_compiler_generated_parts(
            _scalars: Vec<GeneratedArgumentInputV1<'static>>,
            _memory: Vec<GeneratedKfdSliceBinding<'allocation>>,
        ) -> Self {
            Self(PhantomData)
        }
    }

    /// Minimal fixture copy of the address-free generated KFD argument bridge.
    ///
    /// # Safety
    ///
    /// Implementations must describe the exact marker signature and effects.
    pub unsafe trait CompilerGeneratedKfdArguments<
        'allocation,
        K: CompilerGeneratedKernelExpectationV1,
    > {
        fn generated_argument_layout(
        ) -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError>;

        fn bind_kfd_arguments(
            self,
            plan: &GeneratedArgumentPackingPlanV1,
        ) -> Result<GeneratedKfdArgumentBinding<'allocation>, GeneratedKfdArgumentError>;
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
