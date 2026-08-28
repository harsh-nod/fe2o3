use std::fmt;
use std::marker::PhantomData;
use std::ptr::NonNull;

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_artifact_transaction::DurableCurrentLinkPublicationTokenV1;
use fe2o3_kfd::{Gfx942KfdDispatchPointerFixupV1, Gfx942KfdDispatchRequestErrorV1};
use fe2o3_runtime::{
    Gfx942AuthorizedRuntimeDispatchResultV1, Gfx942RuntimeBufferAccessV1,
    Gfx942RuntimeDispatchBufferV1, Gfx942RuntimeDispatchInputsV1,
};

use crate::KernelId;
use crate::argument_alias::GeneratedArgumentBorrowV1;
use crate::generated_argument_plan::{
    CompilerGeneratedArgumentLayoutV1, GeneratedArgumentInputV1, GeneratedArgumentLayoutError,
    GeneratedArgumentPackError, GeneratedArgumentPackingError, GeneratedArgumentPackingPlanV1,
    GeneratedDeviceScalarV1, validate_worker_v3_argument_packing,
};
use crate::{
    AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedKernelExpectationV1,
    RecoveredWorkerV3AdmissionErrorV1,
};
use fe2o3_artifacts::RustDisjointIndexSpaceV1;

/// Compiler-generated address-free argument bridge for one exact kernel signature.
///
/// # Safety
///
/// Implementations must be emitted from the same compiler-authenticated Rust signature, canonical
/// ABI, and effect contract as `K`. Every source argument must be bound exactly once. Slice buffers,
/// access classes, pointer fixups, and retained host borrows must all originate from the same typed
/// capability. A false implementation can substitute invocation memory after verification.
#[doc(hidden)]
pub unsafe trait CompilerGeneratedKfdArguments<'allocation, K: CompilerGeneratedKernelExpectationV1>
{
    fn generated_argument_layout()
    -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError>;

    fn bind_kfd_arguments(
        self,
        plan: &GeneratedArgumentPackingPlanV1,
    ) -> Result<GeneratedKfdArgumentBinding<'allocation>, GeneratedKfdArgumentError>;
}

impl<K: CompilerGeneratedKernelExpectationV1> AuthenticatedWorkerV3ExecutableV1<K> {
    /// Packs one generated address-free invocation against the exact authenticated descriptor.
    ///
    /// This transition retains publication currentness around generated layout validation, value
    /// binding, and packing. It grants no execution authority; a later Worker V3 join must still
    /// authenticate the complete invocation digest and checked KFD device.
    pub fn prepare_generated_kfd_arguments<'allocation, Arguments>(
        &self,
        arguments: Arguments,
    ) -> Result<GeneratedKfdPackedArguments<'allocation>, GeneratedKfdPrepareError>
    where
        Arguments: CompilerGeneratedKfdArguments<'allocation, K>,
    {
        let admission = self.admission();
        let current = admission
            .acquire_retained_currentness_token()
            .map_err(GeneratedKfdPrepareError::CurrentPublication)?;
        let prepared = self.prepare_generated_kfd_arguments_with_current(&current, arguments);
        admission
            .revalidate_retained_currentness_token(&current)
            .map_err(GeneratedKfdPrepareError::CurrentPublication)?;
        prepared
    }

    pub(crate) fn prepare_generated_kfd_arguments_with_current<'allocation, Arguments>(
        &self,
        current: &DurableCurrentLinkPublicationTokenV1,
        arguments: Arguments,
    ) -> Result<GeneratedKfdPackedArguments<'allocation>, GeneratedKfdPrepareError>
    where
        Arguments: CompilerGeneratedKfdArguments<'allocation, K>,
    {
        let admission = self.admission();
        admission
            .revalidate_retained_currentness_token(current)
            .map_err(GeneratedKfdPrepareError::CurrentPublication)?;
        let generated = Arguments::generated_argument_layout()
            .map_err(GeneratedKfdPrepareError::GeneratedLayout)?;
        let plan = validate_worker_v3_argument_packing(
            admission.descriptor_table(),
            admission.descriptor(),
            &generated,
        )
        .map_err(GeneratedKfdPrepareError::PackingPlan)?;
        let binding = arguments
            .bind_kfd_arguments(&plan)
            .map_err(GeneratedKfdPrepareError::Bind)?;
        let packed = binding
            .pack(&plan)
            .map_err(GeneratedKfdPrepareError::Bind)?;
        if packed.kernel_id() != admission.descriptor().kernel_id()
            || packed.explicit_kernarg().len()
                != usize::try_from(admission.descriptor().abi_layout().explicit_argument_size())
                    .unwrap_or(usize::MAX)
            || packed.alignment()
                != admission
                    .descriptor()
                    .abi_layout()
                    .kernarg_segment_alignment()
        {
            return Err(GeneratedKfdPrepareError::PackedSubstitution);
        }
        Ok(packed)
    }
}

/// Borrowed initialized host input for the permanent address-free KFD argument path.
#[doc(hidden)]
pub struct GeneratedKfdReadSlice<'allocation, T: GeneratedDeviceScalarV1> {
    values: &'allocation [T],
}

impl<'allocation, T: GeneratedDeviceScalarV1> GeneratedKfdReadSlice<'allocation, T> {
    pub const fn new(values: &'allocation [T]) -> Self {
        Self { values }
    }

    pub const fn len(&self) -> usize {
        self.values.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn bind_argument(
        self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
    ) -> Result<GeneratedKfdSliceBinding<'allocation>, GeneratedKfdArgumentError> {
        let input = plan
            .bind_generated_address_free_read_slice_v1::<T>(
                argument_index,
                self.values.len(),
                GeneratedArgumentBorrowV1::new(),
            )
            .map_err(GeneratedKfdArgumentError::Pack)?;
        let buffer = (!self.values.is_empty())
            .then(|| {
                Gfx942RuntimeDispatchBufferV1::new(
                    encode_values(self.values),
                    Gfx942RuntimeBufferAccessV1::ReadOnly,
                )
            })
            .transpose()
            .map_err(GeneratedKfdArgumentError::Buffer)?;
        Ok(GeneratedKfdSliceBinding {
            argument_index,
            input,
            buffer,
            required_alignment: T::RUST_SCALAR_TYPE.size_bytes(),
            writeback: None,
        })
    }
}

/// Exclusively borrowed initialized host input/output for the permanent KFD path.
#[doc(hidden)]
pub struct GeneratedKfdReadWriteSlice<'allocation, T: GeneratedDeviceScalarV1> {
    values: &'allocation mut [T],
}

impl<'allocation, T: GeneratedDeviceScalarV1> GeneratedKfdReadWriteSlice<'allocation, T> {
    pub fn new(values: &'allocation mut [T]) -> Self {
        Self { values }
    }

    pub const fn len(&self) -> usize {
        self.values.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn bind_argument(
        self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
    ) -> Result<GeneratedKfdSliceBinding<'allocation>, GeneratedKfdArgumentError> {
        self.bind_argument_with_index_space(plan, argument_index, None)
    }

    /// Binds this address-free capability with the exact compiler-retained disjoint mapping.
    #[doc(hidden)]
    pub fn bind_mapped_argument(
        self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
        index_space: RustDisjointIndexSpaceV1,
    ) -> Result<GeneratedKfdSliceBinding<'allocation>, GeneratedKfdArgumentError> {
        self.bind_argument_with_index_space(plan, argument_index, Some(index_space))
    }

    fn bind_argument_with_index_space(
        self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
        index_space: Option<RustDisjointIndexSpaceV1>,
    ) -> Result<GeneratedKfdSliceBinding<'allocation>, GeneratedKfdArgumentError> {
        let input = match index_space {
            Some(index_space) => plan.bind_generated_address_free_mapped_read_write_slice_v1::<T>(
                argument_index,
                self.values.len(),
                index_space,
                GeneratedArgumentBorrowV1::new(),
            ),
            None => plan.bind_generated_address_free_read_write_slice_v1::<T>(
                argument_index,
                self.values.len(),
                GeneratedArgumentBorrowV1::new(),
            ),
        }
        .map_err(GeneratedKfdArgumentError::Pack)?;
        if self.values.is_empty() {
            return Ok(GeneratedKfdSliceBinding {
                argument_index,
                input,
                buffer: None,
                required_alignment: T::RUST_SCALAR_TYPE.size_bytes(),
                writeback: None,
            });
        }
        let initial_bytes = encode_values(self.values);
        let writeback = GeneratedKfdWriteback::new(self.values);
        let buffer = Gfx942RuntimeDispatchBufferV1::new(
            initial_bytes,
            Gfx942RuntimeBufferAccessV1::ReadWrite,
        )
        .map_err(GeneratedKfdArgumentError::Buffer)?;
        Ok(GeneratedKfdSliceBinding {
            argument_index,
            input,
            buffer: Some(buffer),
            required_alignment: T::RUST_SCALAR_TYPE.size_bytes(),
            writeback: Some(writeback),
        })
    }
}

/// One compiler-indexed address-free slice input, runtime buffer, and optional writeback.
#[doc(hidden)]
pub struct GeneratedKfdSliceBinding<'allocation> {
    argument_index: usize,
    input: GeneratedArgumentInputV1<'allocation>,
    buffer: Option<Gfx942RuntimeDispatchBufferV1>,
    required_alignment: u64,
    writeback: Option<GeneratedKfdWriteback<'allocation>>,
}

/// Complete generated scalar/slice binding before deterministic ABI packing.
#[doc(hidden)]
pub struct GeneratedKfdArgumentBinding<'allocation> {
    scalar_inputs: Vec<GeneratedArgumentInputV1<'static>>,
    memory_arguments: Vec<GeneratedKfdSliceBinding<'allocation>>,
}

impl<'allocation> GeneratedKfdArgumentBinding<'allocation> {
    pub fn from_compiler_generated_parts(
        scalar_inputs: Vec<GeneratedArgumentInputV1<'static>>,
        memory_arguments: Vec<GeneratedKfdSliceBinding<'allocation>>,
    ) -> Self {
        Self {
            scalar_inputs,
            memory_arguments,
        }
    }

    pub fn pack(
        self,
        plan: &GeneratedArgumentPackingPlanV1,
    ) -> Result<GeneratedKfdPackedArguments<'allocation>, GeneratedKfdArgumentError> {
        let mut inputs = Vec::with_capacity(
            self.scalar_inputs
                .len()
                .saturating_add(self.memory_arguments.len()),
        );
        inputs.extend(self.scalar_inputs);
        let mut buffers = Vec::new();
        let mut pointer_fixups = Vec::new();
        let mut completion = Vec::new();
        for memory in self.memory_arguments {
            if !memory.input.is_address_free_slice_v1() {
                return Err(GeneratedKfdArgumentError::AddressBearingInput {
                    argument_index: memory.argument_index,
                });
            }
            inputs.push(memory.input);
            let Some(buffer) = memory.buffer else {
                if memory.writeback.is_some() {
                    return Err(GeneratedKfdArgumentError::WritebackWithoutBuffer {
                        argument_index: memory.argument_index,
                    });
                }
                continue;
            };
            let component = plan
                .address_free_slice_pointer_component_v1(memory.argument_index)
                .map_err(GeneratedKfdArgumentError::Pack)?;
            let kernarg_offset = usize::try_from(component.offset()).map_err(|_| {
                GeneratedKfdArgumentError::PointerOffset {
                    argument_index: memory.argument_index,
                    offset: component.offset(),
                }
            })?;
            let buffer_index = buffers.len();
            let access = buffer.access();
            let byte_len = buffer.bytes().len();
            pointer_fixups.push(Gfx942KfdDispatchPointerFixupV1::new(
                kernarg_offset,
                buffer_index,
                0,
                memory.required_alignment,
            ));
            completion.push(GeneratedKfdCompletedBufferExpectation {
                access,
                byte_len,
                writeback: memory.writeback,
            });
            buffers.push(buffer);
        }
        let packed = plan.pack(inputs).map_err(GeneratedKfdArgumentError::Pack)?;
        Ok(GeneratedKfdPackedArguments {
            kernel_id: packed.kernel_id(),
            alignment: packed.alignment(),
            explicit_kernarg: packed.bytes().to_vec(),
            buffers,
            pointer_fixups,
            completion: GeneratedKfdCompletion {
                buffers: completion,
            },
        })
    }
}

/// Address-free generated kernarg bytes and owned KFD buffers for one invocation.
#[doc(hidden)]
pub struct GeneratedKfdPackedArguments<'allocation> {
    kernel_id: KernelId,
    alignment: u32,
    explicit_kernarg: Vec<u8>,
    buffers: Vec<Gfx942RuntimeDispatchBufferV1>,
    pointer_fixups: Vec<Gfx942KfdDispatchPointerFixupV1>,
    completion: GeneratedKfdCompletion<'allocation>,
}

impl<'allocation> GeneratedKfdPackedArguments<'allocation> {
    pub const fn kernel_id(&self) -> KernelId {
        self.kernel_id
    }

    pub const fn alignment(&self) -> u32 {
        self.alignment
    }

    pub fn explicit_kernarg(&self) -> &[u8] {
        &self.explicit_kernarg
    }

    pub fn buffers(&self) -> &[Gfx942RuntimeDispatchBufferV1] {
        &self.buffers
    }

    pub fn pointer_fixups(&self) -> &[Gfx942KfdDispatchPointerFixupV1] {
        &self.pointer_fixups
    }

    pub fn into_runtime_inputs(
        self,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
    ) -> (
        Gfx942RuntimeDispatchInputsV1,
        GeneratedKfdCompletion<'allocation>,
    ) {
        (
            Gfx942RuntimeDispatchInputsV1::new(
                self.explicit_kernarg,
                self.buffers,
                self.pointer_fixups,
                geometry,
                dynamic_group_segment_bytes,
                timeout_milliseconds,
            ),
            self.completion,
        )
    }
}

/// Retained exclusive host borrows applied only to a checked authorized runtime result.
#[must_use = "KFD output borrows remain unavailable until completion is applied"]
#[doc(hidden)]
pub struct GeneratedKfdCompletion<'allocation> {
    buffers: Vec<GeneratedKfdCompletedBufferExpectation<'allocation>>,
}

impl GeneratedKfdCompletion<'_> {
    pub fn apply(
        self,
        result: Gfx942AuthorizedRuntimeDispatchResultV1,
    ) -> Result<Gfx942AuthorizedRuntimeDispatchResultV1, GeneratedKfdCompletionError> {
        if self.buffers.len() != result.buffers().len() {
            return Err(GeneratedKfdCompletionError::BufferCount {
                expected: self.buffers.len(),
                actual: result.buffers().len(),
            });
        }
        for (index, (expected, completed)) in self.buffers.iter().zip(result.buffers()).enumerate()
        {
            if expected.access != completed.access() {
                return Err(GeneratedKfdCompletionError::Access { index });
            }
            if expected.byte_len != completed.bytes().len() {
                return Err(GeneratedKfdCompletionError::ByteLength {
                    index,
                    expected: expected.byte_len,
                    actual: completed.bytes().len(),
                });
            }
        }
        for (expected, completed) in self.buffers.into_iter().zip(result.buffers()) {
            if let Some(writeback) = expected.writeback {
                writeback.apply(completed.bytes());
            }
        }
        Ok(result)
    }
}

struct GeneratedKfdCompletedBufferExpectation<'allocation> {
    access: Gfx942RuntimeBufferAccessV1,
    byte_len: usize,
    writeback: Option<GeneratedKfdWriteback<'allocation>>,
}

struct GeneratedKfdWriteback<'allocation> {
    destination: NonNull<u8>,
    byte_len: usize,
    apply_values: unsafe fn(NonNull<u8>, &[u8]),
    _borrow: PhantomData<&'allocation mut [u8]>,
}

impl<'allocation> GeneratedKfdWriteback<'allocation> {
    fn new<T: GeneratedDeviceScalarV1>(destination: &'allocation mut [T]) -> Self {
        let byte_len = destination
            .len()
            .checked_mul(size_of::<T>())
            .expect("generated slice byte length was validated by Vec allocation");
        Self {
            destination: NonNull::new(destination.as_mut_ptr().cast())
                .expect("nonempty generated slice has a non-null pointer"),
            byte_len,
            apply_values: apply_values::<T>,
            _borrow: PhantomData,
        }
    }

    fn apply(self, bytes: &[u8]) {
        assert_eq!(bytes.len(), self.byte_len);
        // SAFETY: construction selected this function for the exact retained destination type and
        // consumed an exclusive borrow covering `byte_len` bytes. The completion object retains
        // that borrow until this call.
        unsafe { (self.apply_values)(self.destination, bytes) };
    }
}

fn encode_values<T: GeneratedDeviceScalarV1>(values: &[T]) -> Vec<u8> {
    let element_bytes = usize::try_from(T::RUST_SCALAR_TYPE.size_bytes())
        .expect("generated scalar width is bounded");
    let mut encoded = Vec::with_capacity(
        values
            .len()
            .checked_mul(element_bytes)
            .expect("generated slice byte length fits address space"),
    );
    for value in values {
        let (bytes, byte_len) = value.encode_le_bytes_v1();
        assert_eq!(usize::from(byte_len), element_bytes);
        encoded.extend_from_slice(&bytes[..element_bytes]);
    }
    encoded
}

unsafe fn apply_values<T: GeneratedDeviceScalarV1>(destination: NonNull<u8>, bytes: &[u8]) {
    let element_bytes = usize::try_from(T::RUST_SCALAR_TYPE.size_bytes())
        .expect("generated scalar width is bounded");
    assert!(bytes.len().is_multiple_of(element_bytes));
    let element_count = bytes.len() / element_bytes;
    // SAFETY: upheld by `GeneratedKfdWriteback::apply`; this function was retained with the exact
    // `T` used to create `destination`, and the exclusive borrow covers `element_count` elements.
    let destination =
        unsafe { std::slice::from_raw_parts_mut(destination.cast::<T>().as_ptr(), element_count) };
    for (slot, encoded) in destination
        .iter_mut()
        .zip(bytes.chunks_exact(element_bytes))
    {
        *slot = T::decode_le_bytes_v1(encoded)
            .expect("generated scalar decoder accepts its exact canonical width");
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedKfdArgumentError {
    Pack(GeneratedArgumentPackError),
    Buffer(Gfx942KfdDispatchRequestErrorV1),
    AddressBearingInput { argument_index: usize },
    WritebackWithoutBuffer { argument_index: usize },
    PointerOffset { argument_index: usize, offset: u64 },
}

impl fmt::Display for GeneratedKfdArgumentError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pack(error) => write!(formatter, "generated argument packing failed: {error}"),
            Self::Buffer(error) => write!(formatter, "generated KFD buffer failed: {error:?}"),
            Self::AddressBearingInput { argument_index } => write!(
                formatter,
                "generated KFD argument {argument_index} contains a host or device address"
            ),
            Self::WritebackWithoutBuffer { argument_index } => write!(
                formatter,
                "generated KFD argument {argument_index} has writeback without a runtime buffer"
            ),
            Self::PointerOffset {
                argument_index,
                offset,
            } => write!(
                formatter,
                "generated KFD argument {argument_index} pointer offset {offset} is not representable"
            ),
        }
    }
}

impl std::error::Error for GeneratedKfdArgumentError {}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedKfdPrepareError {
    CurrentPublication(RecoveredWorkerV3AdmissionErrorV1),
    GeneratedLayout(GeneratedArgumentLayoutError),
    PackingPlan(GeneratedArgumentPackingError),
    Bind(GeneratedKfdArgumentError),
    PackedSubstitution,
}

impl fmt::Display for GeneratedKfdPrepareError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentPublication(error) => {
                write!(
                    formatter,
                    "generated KFD publication is not current: {error}"
                )
            }
            Self::GeneratedLayout(error) => {
                write!(formatter, "generated KFD layout is invalid: {error}")
            }
            Self::PackingPlan(error) => {
                write!(
                    formatter,
                    "generated KFD layout differs from the descriptor: {error}"
                )
            }
            Self::Bind(error) => write!(formatter, "generated KFD binding failed: {error}"),
            Self::PackedSubstitution => {
                formatter.write_str("generated KFD packed arguments differ from the descriptor")
            }
        }
    }
}

impl std::error::Error for GeneratedKfdPrepareError {}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedKfdCompletionError {
    BufferCount {
        expected: usize,
        actual: usize,
    },
    Access {
        index: usize,
    },
    ByteLength {
        index: usize,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for GeneratedKfdCompletionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferCount { expected, actual } => {
                write!(
                    formatter,
                    "KFD completed {actual} buffers; expected {expected}"
                )
            }
            Self::Access { index } => {
                write!(
                    formatter,
                    "KFD completed buffer {index} changed access class"
                )
            }
            Self::ByteLength {
                index,
                expected,
                actual,
            } => write!(
                formatter,
                "KFD completed buffer {index} has {actual} bytes; expected {expected}"
            ),
        }
    }
}

impl std::error::Error for GeneratedKfdCompletionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generated_argument_plan::{
        CompilerGeneratedArgumentLayoutV1, validate_argument_packing,
    };
    use fe2o3_artifacts::{
        AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
        Mutability, Name, PointerWidth,
    };

    fn slice_field<T: GeneratedDeviceScalarV1>(
        name: &str,
        offset: u64,
        read_write: bool,
    ) -> AbiField {
        AbiField::new(
            Name::new(name).unwrap(),
            offset,
            16,
            8,
            AbiKind::Slice {
                element_size: T::RUST_SCALAR_TYPE.size_bytes(),
                element_alignment: u32::try_from(T::RUST_SCALAR_TYPE.size_bytes()).unwrap(),
            },
            if read_write {
                Mutability::Mutable
            } else {
                Mutability::Immutable
            },
            if read_write {
                Access::ReadWrite
            } else {
                Access::ReadOnly
            },
            AddressSpace::Global,
            if read_write {
                T::disjoint_slice_type_identity_v1(PointerWidth::Bits64)
            } else {
                T::shared_slice_type_identity_v1(PointerWidth::Bits64)
            },
            if read_write {
                ArgumentOwnership::UniqueBorrow
            } else {
                ArgumentOwnership::SharedBorrow
            },
            if read_write {
                AliasClass::Exclusive
            } else {
                AliasClass::SharedReadOnly
            },
        )
        .unwrap()
    }

    fn scalar_field<T: GeneratedDeviceScalarV1>(name: &str, offset: u64) -> AbiField {
        let size = T::RUST_SCALAR_TYPE.size_bytes();
        AbiField::new(
            Name::new(name).unwrap(),
            offset,
            size,
            u32::try_from(size).unwrap(),
            AbiKind::Scalar(T::ABI_SCALAR_TYPE),
            Mutability::Immutable,
            Access::ByValue,
            AddressSpace::Value,
            T::scalar_type_identity_v1(PointerWidth::Bits64),
            ArgumentOwnership::ByValue,
            AliasClass::Value,
        )
        .unwrap()
    }

    fn plan() -> GeneratedArgumentPackingPlanV1 {
        let fields = vec![
            slice_field::<i32>("input", 0, false),
            slice_field::<i32>("output", 16, true),
            scalar_field::<u32>("count", 32),
        ];
        let manifest = AbiLayout::new(40, 8, PointerWidth::Bits64, fields.clone()).unwrap();
        let generated =
            CompilerGeneratedArgumentLayoutV1::new(40, 8, PointerWidth::Bits64, fields).unwrap();
        validate_argument_packing(KernelId::from_bytes([0x42; 32]), &manifest, &generated).unwrap()
    }

    #[test]
    fn packing_is_address_free_and_binds_exact_buffers_and_fixups() {
        let plan = plan();
        let input = [1_i32, 2, 3];
        let mut output = [i32::MIN, i32::MIN];
        let scalar = plan.scalar(2, 7_u32).unwrap();
        let input = GeneratedKfdReadSlice::new(&input)
            .bind_argument(&plan, 0)
            .unwrap();
        let output_binding = GeneratedKfdReadWriteSlice::new(&mut output)
            .bind_argument(&plan, 1)
            .unwrap();
        let packed = GeneratedKfdArgumentBinding::from_compiler_generated_parts(
            vec![scalar],
            vec![input, output_binding],
        )
        .pack(&plan)
        .unwrap();

        assert_eq!(packed.kernel_id(), KernelId::from_bytes([0x42; 32]));
        assert_eq!(packed.alignment(), 8);
        assert_eq!(&packed.explicit_kernarg()[0..8], &[0; 8]);
        assert_eq!(&packed.explicit_kernarg()[8..16], &3_u64.to_le_bytes());
        assert_eq!(&packed.explicit_kernarg()[16..24], &[0; 8]);
        assert_eq!(&packed.explicit_kernarg()[24..32], &2_u64.to_le_bytes());
        assert_eq!(&packed.explicit_kernarg()[32..36], &7_u32.to_le_bytes());
        assert_eq!(&packed.explicit_kernarg()[36..40], &[0; 4]);

        assert_eq!(packed.buffers().len(), 2);
        assert_eq!(
            packed.buffers()[0].access(),
            Gfx942RuntimeBufferAccessV1::ReadOnly
        );
        assert_eq!(packed.buffers()[0].bytes(), encode_values(&[1_i32, 2, 3]));
        assert_eq!(
            packed.buffers()[1].access(),
            Gfx942RuntimeBufferAccessV1::ReadWrite
        );
        assert_eq!(
            packed.buffers()[1].bytes(),
            encode_values(&[i32::MIN, i32::MIN])
        );
        assert_eq!(
            packed.pointer_fixups(),
            [
                Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 4),
                Gfx942KfdDispatchPointerFixupV1::new(16, 1, 0, 4),
            ]
        );
        drop(packed);
        assert_eq!(output, [i32::MIN, i32::MIN]);
    }

    #[test]
    fn empty_slices_use_null_placeholders_without_runtime_buffers() {
        let plan = plan();
        let input = [];
        let mut output = [];
        let scalar = plan.scalar(2, 0_u32).unwrap();
        let input = GeneratedKfdReadSlice::<i32>::new(&input)
            .bind_argument(&plan, 0)
            .unwrap();
        let output_binding = GeneratedKfdReadWriteSlice::<i32>::new(&mut output)
            .bind_argument(&plan, 1)
            .unwrap();
        let packed = GeneratedKfdArgumentBinding::from_compiler_generated_parts(
            vec![scalar],
            vec![input, output_binding],
        )
        .pack(&plan)
        .unwrap();

        assert!(packed.buffers().is_empty());
        assert!(packed.pointer_fixups().is_empty());
        assert_eq!(&packed.explicit_kernarg()[..32], &[0; 32]);
    }

    #[test]
    fn writeback_retains_and_updates_exact_typed_storage() {
        let mut values = [1_u32, 2, 3];
        let writeback = GeneratedKfdWriteback::new(&mut values);
        writeback.apply(&[9, 0, 0, 0, 10, 0, 0, 0, 11, 0, 0, 0]);
        assert_eq!(values, [9, 10, 11]);
    }

    #[test]
    fn value_bytes_preserve_exact_little_endian_host_representation() {
        assert_eq!(encode_values(&[0x0102_u16, 0x0304]), [2, 1, 4, 3]);
    }

    #[test]
    fn mapped_read_write_binding_preserves_exact_identity_fixup_and_writeback() {
        let blocked = RustDisjointIndexSpaceV1::blocked_index_1d(1, 8).unwrap();
        let field = AbiField::new(
            Name::new("output").unwrap(),
            0,
            16,
            8,
            AbiKind::Slice {
                element_size: 2,
                element_alignment: 2,
            },
            Mutability::Mutable,
            Access::ReadWrite,
            AddressSpace::Global,
            u16::disjoint_slice_type_identity_for_index_space_v1(PointerWidth::Bits64, blocked),
            ArgumentOwnership::UniqueBorrow,
            AliasClass::Exclusive,
        )
        .unwrap();
        let manifest = AbiLayout::new(16, 8, PointerWidth::Bits64, vec![field.clone()]).unwrap();
        let generated = CompilerGeneratedArgumentLayoutV1::new_with_disjoint_index_spaces_v1(
            16,
            8,
            PointerWidth::Bits64,
            vec![field],
            vec![Some(blocked)],
        )
        .unwrap();
        let plan =
            validate_argument_packing(KernelId::from_bytes([0x43; 32]), &manifest, &generated)
                .unwrap();

        for substituted in [
            RustDisjointIndexSpaceV1::Index1D,
            RustDisjointIndexSpaceV1::blocked_index_1d(1, 4).unwrap(),
        ] {
            let mut rejected = [0_u16; 3];
            assert!(matches!(
                GeneratedKfdReadWriteSlice::new(&mut rejected).bind_mapped_argument(
                    &plan,
                    0,
                    substituted,
                ),
                Err(GeneratedKfdArgumentError::Pack(
                    GeneratedArgumentPackError::FieldMismatch {
                        argument_index: 0,
                        property: crate::GeneratedArgumentFieldProperty::TypeIdentity,
                    }
                ))
            ));
        }

        let mut output = [0xaaaa_u16, 0xbbbb, 0xcccc];
        let binding = GeneratedKfdReadWriteSlice::new(&mut output)
            .bind_mapped_argument(&plan, 0, blocked)
            .unwrap();
        let packed =
            GeneratedKfdArgumentBinding::from_compiler_generated_parts(Vec::new(), vec![binding])
                .pack(&plan)
                .unwrap();
        assert_eq!(&packed.explicit_kernarg()[0..8], &[0; 8]);
        assert_eq!(&packed.explicit_kernarg()[8..16], &3_u64.to_le_bytes());
        assert_eq!(packed.buffers().len(), 1);
        assert_eq!(
            packed.buffers()[0].access(),
            Gfx942RuntimeBufferAccessV1::ReadWrite
        );
        assert_eq!(
            packed.pointer_fixups(),
            [Gfx942KfdDispatchPointerFixupV1::new(0, 0, 0, 2)]
        );
        drop(packed);
        assert_eq!(output, [0xaaaa, 0xbbbb, 0xcccc]);

        let mut binding = GeneratedKfdReadWriteSlice::new(&mut output)
            .bind_mapped_argument(&plan, 0, blocked)
            .unwrap();
        binding.writeback.take().unwrap().apply(&[1, 0, 2, 0, 3, 0]);
        drop(binding);
        assert_eq!(output, [1, 2, 3]);

        let mut empty = [];
        let binding = GeneratedKfdReadWriteSlice::<u16>::new(&mut empty)
            .bind_mapped_argument(&plan, 0, blocked)
            .unwrap();
        let packed =
            GeneratedKfdArgumentBinding::from_compiler_generated_parts(Vec::new(), vec![binding])
                .pack(&plan)
                .unwrap();
        assert!(packed.buffers().is_empty());
        assert!(packed.pointer_fixups().is_empty());
        assert_eq!(packed.explicit_kernarg(), &[0; 16]);
    }
}
