//! Owned, address-free generated data. Nothing in this module grants launch authority.

use std::fmt;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

use fe2o3_aql::AqlDispatchGeometryV1;
use fe2o3_artifacts::{RustDisjointIndexSpaceV1, RustScalarElementTypeV1};
use fe2o3_runtime::{
    GeneratedGfx942PersistentStorageV1, Gfx942RuntimeBufferAccessV1, Gfx942RuntimeDispatchBufferV1,
    Gfx942RuntimeDispatchInputsV1, Gfx942RuntimePreparationErrorV1, Gfx942RuntimeProjectionErrorV1,
    PreparedGfx942RuntimeDispatchV1, prepare_gfx942_runtime_dispatch_v1,
};

use crate::generated_argument_borrow::GeneratedArgumentBorrowV1;
use crate::generated_argument_plan::{
    CompilerGeneratedArgumentLayoutV1, GeneratedArgumentInputV1, GeneratedArgumentLayoutError,
    GeneratedArgumentPackError, GeneratedArgumentPackingPlanV1, GeneratedDeviceScalarV1,
    validate_worker_v3_argument_packing,
};
use crate::generated_kfd_arguments::{
    GeneratedKfdArgumentBinding, GeneratedKfdArgumentError, GeneratedKfdOwnedPackedParts,
    GeneratedKfdPackingObservationV1, GeneratedKfdPrepareError, GeneratedKfdSliceBinding,
};
use crate::generated_runtime_results::{
    ChargedOutputCustodyV1, ChargedTypedResultV1, GeneratedRuntimeChargedResultV1,
    GeneratedRuntimeResultBudgetV1, ReadResultCreditV1, ResultBindingBudgetV1, ResultDescriptorV1,
    ResultMemberV1, ResultPreflightV1, ResultReadyGateV1,
};
use crate::{AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedKernelExpectationV1, KernelId};

mod charged_decode;
#[cfg(test)]
mod charged_tests;
mod readback;
mod registry;
pub(crate) use readback::GeneratedRuntimeReadbackOwnerV1;
pub(crate) use registry::{GeneratedRegistryRepeatFrameV1, GeneratedRegistryStorageV1};

/// Compiler-generated owned counterpart of the borrowed KFD argument bridge.
///
/// # Safety
/// Implementations must use the same authenticated signature, ABI, effects and index mappings as
/// `K`, bind every argument exactly once, and charge all storage through the supplied budget.
/// Implementations must not relabel borrowed capabilities or introduce address-bearing inputs.
/// The accounting pass must inspect the complete invocation without encoding or changing custody.
#[doc(hidden)]
pub unsafe trait CompilerGeneratedRuntimeArguments<K: CompilerGeneratedKernelExpectationV1>:
    Send + 'static
{
    fn generated_argument_layout()
    -> Result<CompilerGeneratedArgumentLayoutV1, GeneratedArgumentLayoutError>;

    fn account_runtime_arguments(
        &self,
        budget: &mut GeneratedRuntimeArgumentBudgetV1,
    ) -> Result<(), GeneratedRuntimeArgumentErrorV1>;

    fn bind_runtime_arguments(
        self,
        plan: &GeneratedArgumentPackingPlanV1,
        budget: &mut GeneratedRuntimeArgumentBudgetV1,
    ) -> Result<GeneratedRuntimeArgumentBindingV1, GeneratedRuntimeArgumentErrorV1>;
}

/// Per-invocation byte and metadata-count bounds, not a global or native memory reservation.
///
/// Payload bytes cover two kernarg copies and both the owned typed seed and its encoded copy.
/// Result bytes cover all returned buffer bytes plus a decoded typed copy of every output. The
/// latter remains bounded when an observer retains a result after the input snapshot is dropped.
/// Binding counts bound metadata cardinality; allocator overhead is not claimed as byte-accounted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneratedRuntimeArgumentLimitsV1 {
    max_payload_bytes: usize,
    max_result_bytes: usize,
    max_bindings: usize,
}

impl GeneratedRuntimeArgumentLimitsV1 {
    pub const fn new(
        max_payload_bytes: usize,
        max_result_bytes: usize,
        max_bindings: usize,
    ) -> Self {
        Self {
            max_payload_bytes,
            max_result_bytes,
            max_bindings,
        }
    }
}

/// Measured logical storage bounds for the resource-credit integration boundary.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GeneratedRuntimeArgumentFootprintV1 {
    pub kernarg_bytes: usize,
    pub input_bytes: usize,
    pub output_bytes: usize,
    pub payload_bytes: usize,
    pub result_bytes: usize,
    pub bindings: usize,
    pub output_observers: usize,
}

#[doc(hidden)]
pub struct GeneratedRuntimeArgumentBudgetV1 {
    limits: GeneratedRuntimeArgumentLimitsV1,
    footprint: GeneratedRuntimeArgumentFootprintV1,
    result_mode: ResultMode,
}

enum ResultMode {
    Legacy,
    Preflight(ResultPreflightV1),
    Binding(ResultBindingBudgetV1),
}

impl GeneratedRuntimeArgumentBudgetV1 {
    pub fn new(
        plan: &GeneratedArgumentPackingPlanV1,
        limits: GeneratedRuntimeArgumentLimitsV1,
    ) -> Result<Self, GeneratedRuntimeArgumentErrorV1> {
        let kernarg_bytes = usize::try_from(plan.kernarg_size())
            .map_err(|_| GeneratedRuntimeArgumentErrorV1::ByteLength)?;
        let payload_bytes = kernarg_bytes
            .checked_mul(2)
            .ok_or(GeneratedRuntimeArgumentErrorV1::ByteLength)?;
        if plan.argument_count() > limits.max_bindings || payload_bytes > limits.max_payload_bytes {
            return Err(GeneratedRuntimeArgumentErrorV1::PayloadLimit);
        }
        Ok(Self {
            limits,
            result_mode: ResultMode::Legacy,
            footprint: GeneratedRuntimeArgumentFootprintV1 {
                kernarg_bytes,
                payload_bytes,
                bindings: plan.argument_count(),
                ..GeneratedRuntimeArgumentFootprintV1::default()
            },
        })
    }

    fn charge(
        &mut self,
        bytes: usize,
        output: bool,
    ) -> Result<(), GeneratedRuntimeArgumentErrorV1> {
        let mut next = self.footprint;
        next.input_bytes = next
            .input_bytes
            .checked_add(bytes)
            .ok_or(GeneratedRuntimeArgumentErrorV1::ByteLength)?;
        next.payload_bytes = next
            .payload_bytes
            .checked_add(
                bytes
                    .checked_mul(2)
                    .ok_or(GeneratedRuntimeArgumentErrorV1::ByteLength)?,
            )
            .ok_or(GeneratedRuntimeArgumentErrorV1::ByteLength)?;
        next.result_bytes = next
            .result_bytes
            .checked_add(bytes)
            .ok_or(GeneratedRuntimeArgumentErrorV1::ByteLength)?;
        if output {
            next.output_bytes = next
                .output_bytes
                .checked_add(bytes)
                .ok_or(GeneratedRuntimeArgumentErrorV1::ByteLength)?;
            next.result_bytes = next
                .result_bytes
                .checked_add(bytes)
                .ok_or(GeneratedRuntimeArgumentErrorV1::ByteLength)?;
            next.output_observers = next
                .output_observers
                .checked_add(1)
                .ok_or(GeneratedRuntimeArgumentErrorV1::ByteLength)?;
        }
        if next.payload_bytes > self.limits.max_payload_bytes
            || next.result_bytes > self.limits.max_result_bytes
        {
            return Err(GeneratedRuntimeArgumentErrorV1::PayloadLimit);
        }
        self.footprint = next;
        Ok(())
    }

    pub const fn footprint(&self) -> GeneratedRuntimeArgumentFootprintV1 {
        self.footprint
    }

    fn account_slice<T: GeneratedDeviceScalarV1>(
        &mut self,
        elements: usize,
        access: Gfx942RuntimeBufferAccessV1,
        custody: Option<&OutputCustody>,
    ) -> Result<(), GeneratedRuntimeArgumentErrorV1> {
        let bytes = elements
            .checked_mul(size_of::<T>())
            .ok_or(GeneratedRuntimeArgumentErrorV1::ByteLength)?;
        let descriptor = self.result_descriptor::<T>(elements, access, custody)?;
        let before = self.footprint;
        self.charge(bytes, custody.is_some())?;
        match (&mut self.result_mode, descriptor) {
            (ResultMode::Legacy, None) => Ok(()),
            (ResultMode::Preflight(roster), Some(descriptor)) => {
                if let Err(error) = roster.push(descriptor) {
                    self.footprint = before;
                    return Err(error);
                }
                Ok(())
            }
            _ => {
                self.footprint = before;
                Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch)
            }
        }
    }

    fn result_descriptor<T: GeneratedDeviceScalarV1>(
        &self,
        elements: usize,
        access: Gfx942RuntimeBufferAccessV1,
        custody: Option<&OutputCustody>,
    ) -> Result<Option<ResultDescriptorV1>, GeneratedRuntimeArgumentErrorV1> {
        match (&self.result_mode, custody) {
            (ResultMode::Legacy, Some(OutputCustody::Charged(_)))
            | (ResultMode::Preflight(_) | ResultMode::Binding(_), Some(OutputCustody::Legacy(_))) => {
                Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch)
            }
            (ResultMode::Legacy, _) => Ok(None),
            (_, custody) => Ok(Some(ResultDescriptorV1::new::<T>(
                elements,
                access,
                custody.and_then(|custody| match custody {
                    OutputCustody::Charged(charged) => Some(charged),
                    _ => None,
                }),
            )?)),
        }
    }

    fn bind_slice<T: GeneratedDeviceScalarV1>(
        &mut self,
        elements: usize,
        access: Gfx942RuntimeBufferAccessV1,
        custody: Option<&OutputCustody>,
    ) -> Result<Option<ResultMemberV1>, GeneratedRuntimeArgumentErrorV1> {
        let descriptor = self.result_descriptor::<T>(elements, access, custody)?;
        let bytes = elements
            .checked_mul(size_of::<T>())
            .ok_or(GeneratedRuntimeArgumentErrorV1::ByteLength)?;
        let before = self.footprint;
        self.charge(bytes, custody.is_some())?;
        let result = match (&mut self.result_mode, descriptor) {
            (ResultMode::Legacy, None) => Ok(None),
            (ResultMode::Binding(roster), Some(descriptor)) => roster.take(&descriptor).map(Some),
            _ => Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch),
        };
        if result.is_err() {
            self.footprint = before;
        }
        result
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> AuthenticatedWorkerV3ExecutableV1<K> {
    /// Preflights and reserves the complete result roster before encoding owned inputs.
    ///
    /// This returns charged data custody only. It does not admit a Context operation,
    /// authorize publication, or expose a decoder accepting caller-supplied results.
    pub fn prepare_generated_runtime_arguments_charged<Arguments>(
        &self,
        arguments: Arguments,
        limits: GeneratedRuntimeArgumentLimitsV1,
        result_budget: &GeneratedRuntimeResultBudgetV1,
    ) -> Result<GeneratedRuntimeChargedArgumentsV1, GeneratedRuntimeArgumentErrorV1>
    where
        Arguments: CompilerGeneratedRuntimeArguments<K>,
    {
        let current = self.current_publication_token();
        let admission = self.admission();
        let check_current = || {
            admission
                .revalidate_retained_currentness_token(current)
                .map_err(GeneratedKfdPrepareError::CurrentPublication)
                .map_err(GeneratedRuntimeArgumentErrorV1::Prepare)
        };
        check_current()?;
        let packed = (|| {
            let generated = Arguments::generated_argument_layout()
                .map_err(GeneratedKfdPrepareError::GeneratedLayout)
                .map_err(GeneratedRuntimeArgumentErrorV1::Prepare)?;
            let plan = validate_worker_v3_argument_packing(
                admission.descriptor_table(),
                admission.descriptor(),
                &generated,
            )
            .map_err(GeneratedKfdPrepareError::PackingPlan)
            .map_err(GeneratedRuntimeArgumentErrorV1::Prepare)?;
            prepare_charged_with_plan(
                arguments,
                &plan,
                limits,
                result_budget,
                Arguments::account_runtime_arguments,
                |arguments, budget| arguments.bind_runtime_arguments(&plan, budget),
            )
        })();
        check_current()?;
        packed
    }

    /// Validates current compiler publication and packs owned data for one exact descriptor.
    ///
    /// This does not admit a Context allocation, prove its freshness, or authorize execution.
    /// GEN-2 must pair these data with exact invocation authority and retain both through quiescence.
    pub fn prepare_generated_runtime_arguments<Arguments>(
        &self,
        arguments: Arguments,
        limits: GeneratedRuntimeArgumentLimitsV1,
    ) -> Result<GeneratedRuntimePackedArgumentsV1, GeneratedRuntimeArgumentErrorV1>
    where
        Arguments: CompilerGeneratedRuntimeArguments<K>,
    {
        let current = self.current_publication_token();
        let admission = self.admission();
        let check_current = || {
            admission
                .revalidate_retained_currentness_token(current)
                .map_err(GeneratedKfdPrepareError::CurrentPublication)
                .map_err(GeneratedRuntimeArgumentErrorV1::Prepare)
        };
        check_current()?;
        let packed = (|| {
            let generated = Arguments::generated_argument_layout()
                .map_err(GeneratedKfdPrepareError::GeneratedLayout)
                .map_err(GeneratedRuntimeArgumentErrorV1::Prepare)?;
            let plan = validate_worker_v3_argument_packing(
                admission.descriptor_table(),
                admission.descriptor(),
                &generated,
            )
            .map_err(GeneratedKfdPrepareError::PackingPlan)
            .map_err(GeneratedRuntimeArgumentErrorV1::Prepare)?;
            let mut preflight = GeneratedRuntimeArgumentBudgetV1::new(&plan, limits)?;
            arguments.account_runtime_arguments(&mut preflight)?;
            let mut budget = GeneratedRuntimeArgumentBudgetV1::new(&plan, limits)?;
            let packed = arguments
                .bind_runtime_arguments(&plan, &mut budget)?
                .pack(&plan, budget)?;
            if packed.footprint() != preflight.footprint() {
                return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
            }
            Ok(packed)
        })();
        check_current()?;
        packed
    }
}

pub(crate) fn prepare_charged_with_plan<A>(
    arguments: A,
    plan: &GeneratedArgumentPackingPlanV1,
    limits: GeneratedRuntimeArgumentLimitsV1,
    result_budget: &GeneratedRuntimeResultBudgetV1,
    account: impl FnOnce(
        &A,
        &mut GeneratedRuntimeArgumentBudgetV1,
    ) -> Result<(), GeneratedRuntimeArgumentErrorV1>,
    bind: impl FnOnce(
        A,
        &mut GeneratedRuntimeArgumentBudgetV1,
    ) -> Result<GeneratedRuntimeArgumentBindingV1, GeneratedRuntimeArgumentErrorV1>,
) -> Result<GeneratedRuntimeChargedArgumentsV1, GeneratedRuntimeArgumentErrorV1> {
    let mut preflight = GeneratedRuntimeArgumentBudgetV1::new(plan, limits)?;
    preflight.result_mode = ResultMode::Preflight(ResultPreflightV1::new()?);
    account(&arguments, &mut preflight)?;
    let expected = preflight.footprint;
    let ResultMode::Preflight(roster) = preflight.result_mode else {
        unreachable!()
    };
    let mut budget = GeneratedRuntimeArgumentBudgetV1::new(plan, limits)?;
    budget.result_mode = ResultMode::Binding(roster.reserve(result_budget)?);
    let packed = bind(arguments, &mut budget)?.pack_inner(plan, budget, true)?;
    if packed.footprint() != expected {
        return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
    }
    Ok(GeneratedRuntimeChargedArgumentsV1 { packed })
}

/// Owned immutable input. Safe construction cannot alias another owned slice or retain a borrow.
///
/// ```compile_fail
/// use fe2o3_host::GeneratedRuntimeReadSlice;
/// let values = [1_u32, 2, 3];
/// let borrowed = GeneratedRuntimeReadSlice::new(&values[..]);
/// ```
pub struct GeneratedRuntimeReadSlice<T: GeneratedDeviceScalarV1> {
    values: OwnedRuntimeSliceV1<T>,
}

impl<T: GeneratedDeviceScalarV1> GeneratedRuntimeReadSlice<T> {
    pub fn new(values: Box<[T]>) -> Self {
        Self {
            values: OwnedRuntimeSliceV1::Seed(values),
        }
    }

    /// Moves completed host data into a new immutable input without cloning its typed storage.
    ///
    /// The original result credit stays charged until that storage is disposed,
    /// after encoding or on rejection/drop. Charged preparation independently
    /// reserves the next invocation's complete result roster before encoding;
    /// a shared budget therefore needs room for both reservations at that peak.
    /// This neither forwards a native buffer nor grants completion/launch authority.
    ///
    /// ```compile_fail
    /// use fe2o3_host::{ChargedTypedResultV1, GeneratedRuntimeReadSlice};
    /// fn reuse(result: ChargedTypedResultV1<u32>) {
    ///     let input = GeneratedRuntimeReadSlice::from_charged_result(result);
    ///     let second = GeneratedRuntimeReadSlice::from_charged_result(result);
    /// }
    /// ```
    pub fn from_charged_result(result: ChargedTypedResultV1<T>) -> Self {
        Self {
            values: OwnedRuntimeSliceV1::Completed(result),
        }
    }

    pub fn len(&self) -> usize {
        self.values.as_slice().len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.as_slice().is_empty()
    }

    #[doc(hidden)]
    pub fn account_storage(
        &self,
        budget: &mut GeneratedRuntimeArgumentBudgetV1,
    ) -> Result<(), GeneratedRuntimeArgumentErrorV1> {
        budget.account_slice::<T>(self.len(), Gfx942RuntimeBufferAccessV1::ReadOnly, None)
    }

    #[doc(hidden)]
    pub fn bind_argument(
        self,
        plan: &GeneratedArgumentPackingPlanV1,
        argument_index: usize,
        budget: &mut GeneratedRuntimeArgumentBudgetV1,
    ) -> Result<GeneratedRuntimeSliceBindingV1, GeneratedRuntimeArgumentErrorV1> {
        bind_owned_slice(
            self.values,
            None,
            plan,
            argument_index,
            Gfx942RuntimeBufferAccessV1::ReadOnly,
            None,
            budget,
        )
    }
}

enum OwnedRuntimeSliceV1<T: GeneratedDeviceScalarV1> {
    Seed(Box<[T]>),
    Completed(ChargedTypedResultV1<T>),
}

impl<T: GeneratedDeviceScalarV1> OwnedRuntimeSliceV1<T> {
    fn as_slice(&self) -> &[T] {
        match self {
            Self::Seed(values) => values,
            Self::Completed(result) => result.as_slice(),
        }
    }
}

macro_rules! owned_output_slice {
    ($name:ident, $access:ident) => {
        /// Owned initialized output seed. Untouched elements retain their original values.
        pub struct $name<T: GeneratedDeviceScalarV1> {
            values: Box<[T]>,
            custody: OutputCustody,
        }

        impl<T: GeneratedDeviceScalarV1> $name<T> {
            pub fn new(values: Box<[T]>) -> (Self, GeneratedRuntimeResultV1<T>) {
                let state = Arc::new(Mutex::new(OutputState::Unbound));
                let result = GeneratedRuntimeResultV1 {
                    state: Arc::clone(&state),
                    scalar: PhantomData,
                };
                (
                    Self {
                        values,
                        custody: OutputCustody::Legacy(LegacyOutputCustody {
                            state,
                            scalar: T::RUST_SCALAR_TYPE,
                        }),
                    },
                    result,
                )
            }

            /// Retains a caller-owned typed seed for the distinct charged preparation path.
            /// No result credit or invocation authority exists until preparation succeeds.
            pub fn new_charged(values: Box<[T]>) -> (Self, GeneratedRuntimeChargedResultV1<T>) {
                let (custody, observer) = ChargedOutputCustodyV1::new(values.len());
                (
                    Self {
                        values,
                        custody: OutputCustody::Charged(custody),
                    },
                    observer,
                )
            }

            pub fn len(&self) -> usize {
                self.values.len()
            }

            pub fn is_empty(&self) -> bool {
                self.values.is_empty()
            }

            #[doc(hidden)]
            pub fn account_storage(
                &self,
                budget: &mut GeneratedRuntimeArgumentBudgetV1,
            ) -> Result<(), GeneratedRuntimeArgumentErrorV1> {
                budget.account_slice::<T>(
                    self.values.len(),
                    Gfx942RuntimeBufferAccessV1::$access,
                    Some(&self.custody),
                )
            }

            #[doc(hidden)]
            pub fn bind_argument(
                self,
                plan: &GeneratedArgumentPackingPlanV1,
                argument_index: usize,
                budget: &mut GeneratedRuntimeArgumentBudgetV1,
            ) -> Result<GeneratedRuntimeSliceBindingV1, GeneratedRuntimeArgumentErrorV1> {
                bind_owned_slice(
                    OwnedRuntimeSliceV1::Seed(self.values),
                    Some(self.custody),
                    plan,
                    argument_index,
                    Gfx942RuntimeBufferAccessV1::$access,
                    None,
                    budget,
                )
            }

            #[doc(hidden)]
            pub fn bind_mapped_argument(
                self,
                plan: &GeneratedArgumentPackingPlanV1,
                argument_index: usize,
                index_space: RustDisjointIndexSpaceV1,
                budget: &mut GeneratedRuntimeArgumentBudgetV1,
            ) -> Result<GeneratedRuntimeSliceBindingV1, GeneratedRuntimeArgumentErrorV1> {
                bind_owned_slice(
                    OwnedRuntimeSliceV1::Seed(self.values),
                    Some(self.custody),
                    plan,
                    argument_index,
                    Gfx942RuntimeBufferAccessV1::$access,
                    Some(index_space),
                    budget,
                )
            }
        }
    };
}

owned_output_slice!(GeneratedRuntimeWriteSlice, WriteOnly);
owned_output_slice!(GeneratedRuntimeReadWriteSlice, ReadWrite);

#[allow(clippy::too_many_arguments)]
fn bind_owned_slice<T: GeneratedDeviceScalarV1>(
    values: OwnedRuntimeSliceV1<T>,
    custody: Option<OutputCustody>,
    plan: &GeneratedArgumentPackingPlanV1,
    argument_index: usize,
    access: Gfx942RuntimeBufferAccessV1,
    index_space: Option<RustDisjointIndexSpaceV1>,
    budget: &mut GeneratedRuntimeArgumentBudgetV1,
) -> Result<GeneratedRuntimeSliceBindingV1, GeneratedRuntimeArgumentErrorV1> {
    if matches!(&values, OwnedRuntimeSliceV1::Completed(_))
        && (access != Gfx942RuntimeBufferAccessV1::ReadOnly
            || custody.is_some()
            || index_space.is_some())
    {
        return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
    }
    let elements = values.as_slice().len();
    let byte_len = elements
        .checked_mul(size_of::<T>())
        .ok_or(GeneratedRuntimeArgumentErrorV1::ByteLength)?;
    let borrow = GeneratedArgumentBorrowV1::new();
    let input = match (access, index_space) {
        (Gfx942RuntimeBufferAccessV1::ReadOnly, None) => {
            plan.bind_generated_address_free_read_slice_v1::<T>(argument_index, elements, borrow)
        }
        (Gfx942RuntimeBufferAccessV1::WriteOnly, None) => {
            plan.bind_generated_address_free_write_slice_v1::<T>(argument_index, elements, borrow)
        }
        (Gfx942RuntimeBufferAccessV1::ReadWrite, None) => plan
            .bind_generated_address_free_read_write_slice_v1::<T>(argument_index, elements, borrow),
        (Gfx942RuntimeBufferAccessV1::WriteOnly, Some(mapping)) => plan
            .bind_generated_address_free_mapped_write_slice_v1::<T>(
                argument_index,
                elements,
                mapping,
                borrow,
            ),
        (Gfx942RuntimeBufferAccessV1::ReadWrite, Some(mapping)) => plan
            .bind_generated_address_free_mapped_read_write_slice_v1::<T>(
                argument_index,
                elements,
                mapping,
                borrow,
            ),
        _ => return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch),
    }
    .map_err(GeneratedRuntimeArgumentErrorV1::Pack)?;
    let member = budget.bind_slice::<T>(elements, access, custody.as_ref())?;
    if let Some(OutputCustody::Legacy(custody)) = &custody {
        let mut state = custody
            .state
            .lock()
            .map_err(|_| GeneratedRuntimeArgumentErrorV1::Custody)?;
        if custody.scalar != T::RUST_SCALAR_TYPE || !matches!(*state, OutputState::Unbound) {
            return Err(GeneratedRuntimeArgumentErrorV1::StaleOrAliasedOutput);
        }
        *state = OutputState::Bound;
    }
    let mut read_credit = None;
    let buffer = if let Some(OutputCustody::Charged(custody)) = &custody {
        let OwnedRuntimeSliceV1::Seed(values) = values else {
            return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
        };
        custody.bind_seed(
            values,
            member.ok_or(GeneratedRuntimeArgumentErrorV1::BindingMismatch)?,
        )?;
        custody.with_seed::<T, _>(|values| encode_owned_slice(values, byte_len, access))?
    } else {
        read_credit = member.map(ResultMemberV1::retain_read);
        encode_owned_slice(values.as_slice(), byte_len, access)?
    };
    Ok(GeneratedRuntimeSliceBindingV1 {
        binding: GeneratedKfdSliceBinding::from_owned_buffer(
            argument_index,
            input,
            buffer,
            T::RUST_SCALAR_TYPE.size_bytes(),
        ),
        byte_len,
        access,
        custody,
        read_credit,
    })
}

fn encode_owned_slice<T: GeneratedDeviceScalarV1>(
    values: &[T],
    byte_len: usize,
    access: Gfx942RuntimeBufferAccessV1,
) -> Result<Option<Gfx942RuntimeDispatchBufferV1>, GeneratedRuntimeArgumentErrorV1> {
    Ok(if values.is_empty() {
        None
    } else {
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(byte_len)
            .map_err(|_| GeneratedRuntimeArgumentErrorV1::Allocation)?;
        for value in values {
            let (encoded, width) = value.encode_le_bytes_v1();
            bytes.extend_from_slice(&encoded[..usize::from(width)]);
        }
        Some(
            Gfx942RuntimeDispatchBufferV1::new(bytes, access)
                .map_err(GeneratedKfdArgumentError::Buffer)
                .map_err(GeneratedRuntimeArgumentErrorV1::Binding)?,
        )
    })
}

#[doc(hidden)]
pub struct GeneratedRuntimeSliceBindingV1 {
    binding: GeneratedKfdSliceBinding<'static>,
    byte_len: usize,
    access: Gfx942RuntimeBufferAccessV1,
    custody: Option<OutputCustody>,
    read_credit: Option<ReadResultCreditV1>,
}

#[doc(hidden)]
pub struct GeneratedRuntimeArgumentBindingV1 {
    scalar_inputs: Vec<GeneratedArgumentInputV1<'static>>,
    memory_arguments: Vec<GeneratedRuntimeSliceBindingV1>,
}

impl GeneratedRuntimeArgumentBindingV1 {
    pub fn from_compiler_generated_parts(
        scalar_inputs: Vec<GeneratedArgumentInputV1<'static>>,
        memory_arguments: Vec<GeneratedRuntimeSliceBindingV1>,
    ) -> Self {
        Self {
            scalar_inputs,
            memory_arguments,
        }
    }

    pub fn pack(
        self,
        plan: &GeneratedArgumentPackingPlanV1,
        budget: GeneratedRuntimeArgumentBudgetV1,
    ) -> Result<GeneratedRuntimePackedArgumentsV1, GeneratedRuntimeArgumentErrorV1> {
        self.pack_inner(plan, budget, false)
    }

    fn pack_inner(
        self,
        plan: &GeneratedArgumentPackingPlanV1,
        budget: GeneratedRuntimeArgumentBudgetV1,
        charged: bool,
    ) -> Result<GeneratedRuntimePackedArgumentsV1, GeneratedRuntimeArgumentErrorV1> {
        let result_gate = match (&budget.result_mode, charged) {
            (ResultMode::Legacy, false) => None,
            (ResultMode::Binding(roster), true) if roster.complete() => Some(roster.gate.clone()),
            _ => return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch),
        };
        let count = self
            .scalar_inputs
            .len()
            .checked_add(self.memory_arguments.len())
            .ok_or(GeneratedRuntimeArgumentErrorV1::ByteLength)?;
        if count != plan.argument_count() || count != budget.footprint.bindings {
            return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
        }
        let mut measured = GeneratedRuntimeArgumentBudgetV1::new(plan, budget.limits)?;
        for (index, memory) in self.memory_arguments.iter().enumerate() {
            measured.charge(memory.byte_len, memory.custody.is_some())?;
            if let Some(custody) = &memory.custody {
                if self.memory_arguments[..index].iter().any(|earlier| {
                    earlier
                        .custody
                        .as_ref()
                        .is_some_and(|other| custody.same(other))
                }) {
                    return Err(GeneratedRuntimeArgumentErrorV1::StaleOrAliasedOutput);
                }
                custody.bound_to(result_gate.as_ref())?;
            } else if !match (&memory.read_credit, &result_gate) {
                (None, None) => true,
                (Some(credit), Some(gate)) => credit.bound_to(gate),
                _ => false,
            } {
                return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
            }
        }
        if measured.footprint != budget.footprint {
            return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
        }
        let mut bindings = Vec::new();
        let mut expectations = Vec::new();
        bindings
            .try_reserve_exact(self.memory_arguments.len())
            .map_err(|_| GeneratedRuntimeArgumentErrorV1::Allocation)?;
        expectations
            .try_reserve_exact(self.memory_arguments.len())
            .map_err(|_| GeneratedRuntimeArgumentErrorV1::Allocation)?;
        for memory in self.memory_arguments {
            bindings.push(memory.binding);
            expectations.push(OwnedBufferExpectation {
                byte_len: memory.byte_len,
                access: memory.access,
                custody: memory.custody,
                read_credit: memory.read_credit,
            });
        }
        let packed = GeneratedKfdArgumentBinding::from_compiler_generated_parts(
            self.scalar_inputs,
            bindings,
        )
        .pack(plan)
        .map_err(GeneratedRuntimeArgumentErrorV1::Binding)?
        .into_owned_parts()
        .map_err(GeneratedRuntimeArgumentErrorV1::Binding)?;
        Ok(GeneratedRuntimePackedArgumentsV1 {
            packed,
            footprint: measured.footprint,
            decoder: GeneratedRuntimeOutputDecoderV1 {
                expectations,
                result_gate,
            },
        })
    }
}

/// Owned, Send + 'static packed data and output custody. This is not an execution permit.
#[must_use]
pub struct GeneratedRuntimePackedArgumentsV1 {
    packed: GeneratedKfdOwnedPackedParts,
    footprint: GeneratedRuntimeArgumentFootprintV1,
    decoder: GeneratedRuntimeOutputDecoderV1,
}

/// Charged owned arguments for the private generated-invocation adapter.
/// This value neither launches work nor grants completion authority.
///
/// ```compile_fail
/// use fe2o3_host::GeneratedRuntimeChargedArgumentsV1;
/// fn expose_decoder(packed: GeneratedRuntimeChargedArgumentsV1) {
///     packed.into_runtime_inputs(todo!(), 0, 1000);
/// }
/// ```
#[must_use]
pub struct GeneratedRuntimeChargedArgumentsV1 {
    packed: GeneratedRuntimePackedArgumentsV1,
}

impl GeneratedRuntimeChargedArgumentsV1 {
    pub(crate) fn packed_view_v1(
        &self,
    ) -> crate::generated_kfd_arguments::GeneratedPackedArgumentsViewV1<'_> {
        self.packed.packed.packed_view_v1()
    }

    pub fn kernel_id(&self) -> KernelId {
        self.packed.kernel_id()
    }
    pub fn footprint(&self) -> GeneratedRuntimeArgumentFootprintV1 {
        self.packed.footprint()
    }
    pub fn packing_observation(&self) -> &GeneratedKfdPackingObservationV1 {
        self.packed.packing_observation()
    }

    pub(crate) fn into_runtime_inputs(
        self,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
    ) -> GeneratedRuntimeInvocationPartsV1 {
        let GeneratedRuntimePackedArgumentsV1 {
            packed,
            footprint,
            decoder,
        } = self.packed;
        GeneratedRuntimeInvocationPartsV1 {
            storage: GeneratedRuntimeStorageV1 {
                payload: Gfx942RuntimeDispatchInputsV1::new(
                    packed.explicit_kernarg,
                    packed.buffers,
                    packed.pointer_fixups,
                    geometry,
                    dynamic_group_segment_bytes,
                    timeout_milliseconds,
                ),
                readback: None,
                decoder,
            },
            kernel_id: packed.kernel_id,
            footprint,
            packing: packed.packing_observation,
        }
    }
}

pub(crate) struct GeneratedRuntimeInvocationPartsV1 {
    pub(crate) storage: GeneratedRuntimeStorageV1<Gfx942RuntimeDispatchInputsV1>,
    pub(crate) kernel_id: KernelId,
    pub(crate) footprint: GeneratedRuntimeArgumentFootprintV1,
    pub(crate) packing: GeneratedKfdPackingObservationV1,
}

// Declaration order is the disposal contract: complete input/prepared storage before credits.
// Keep storage and decoder private; completion must not become an extraction API.
pub(crate) struct GeneratedRuntimeStorageV1<P> {
    payload: P,
    readback: Option<GeneratedRuntimeReadbackOwnerV1>,
    #[allow(
        dead_code,
        reason = "retains charged custody until GEN-2B's completion transition"
    )]
    decoder: GeneratedRuntimeOutputDecoderV1,
}

impl GeneratedRuntimeStorageV1<Gfx942RuntimeDispatchInputsV1> {
    pub(crate) fn prepare(
        self,
        hsaco: &[u8],
        kernel_name: &str,
    ) -> Result<
        GeneratedRuntimeStorageV1<PreparedGfx942RuntimeDispatchV1>,
        Gfx942RuntimePreparationErrorV1,
    > {
        // The callee disposes consumed inputs on Err/unwind before our retained decoder drops.
        // This closed error type cannot return input storage outside its charged owner.
        let payload = prepare_gfx942_runtime_dispatch_v1(hsaco, kernel_name, self.payload)?;
        Ok(GeneratedRuntimeStorageV1 {
            payload,
            readback: self.readback,
            decoder: self.decoder,
        })
    }
}

impl GeneratedRuntimeStorageV1<PreparedGfx942RuntimeDispatchV1> {
    pub(crate) fn prepared(&self) -> &PreparedGfx942RuntimeDispatchV1 {
        &self.payload
    }

    pub(crate) fn project_native_conditional_fill64(
        self,
        hsaco: &[u8],
        premises: fe2o3_kfd::NativeConditionalFill64PremisesV1,
    ) -> Result<
        GeneratedRuntimeStorageV1<GeneratedGfx942PersistentStorageV1>,
        Gfx942RuntimeProjectionErrorV1,
    > {
        let payload = self
            .payload
            .into_native_conditional_fill64_projection_v1(hsaco, premises)?
            .into_generated_storage_v1();
        Ok(GeneratedRuntimeStorageV1 {
            payload,
            readback: self.readback,
            decoder: self.decoder,
        })
    }

    pub(crate) fn project_native_independent_fill64(
        self,
        hsaco: &[u8],
        premises: fe2o3_kfd::NativeConditionalFill64PremisesV1,
    ) -> Result<
        GeneratedRuntimeStorageV1<GeneratedGfx942PersistentStorageV1>,
        Gfx942RuntimeProjectionErrorV1,
    > {
        let payload = self
            .payload
            .into_native_independent_fill64_projection_v1(hsaco, premises)?
            .into_generated_storage_v1();
        Ok(GeneratedRuntimeStorageV1 {
            payload,
            readback: self.readback,
            decoder: self.decoder,
        })
    }

    pub(crate) fn project_persistent(
        self,
        hsaco: &[u8],
    ) -> Result<
        GeneratedRuntimeStorageV1<GeneratedGfx942PersistentStorageV1>,
        Gfx942RuntimeProjectionErrorV1,
    > {
        self.project(hsaco, false)
    }

    pub(crate) fn project_conditional_fill(
        self,
        hsaco: &[u8],
    ) -> Result<
        GeneratedRuntimeStorageV1<GeneratedGfx942PersistentStorageV1>,
        Gfx942RuntimeProjectionErrorV1,
    > {
        self.project(hsaco, true)
    }

    fn project(
        self,
        hsaco: &[u8],
        conditional_fill: bool,
    ) -> Result<
        GeneratedRuntimeStorageV1<GeneratedGfx942PersistentStorageV1>,
        Gfx942RuntimeProjectionErrorV1,
    > {
        // A closed failure cannot detach consumed storage from its retained decoder.
        let projection = self.payload.into_persistent_projection_v1(hsaco)?;
        let payload = if conditional_fill {
            projection.require_conditional_fill_v1()
        } else {
            projection
        }
        .into_generated_storage_v1();
        Ok(GeneratedRuntimeStorageV1 {
            payload,
            readback: self.readback,
            decoder: self.decoder,
        })
    }
}

impl GeneratedRuntimeStorageV1<GeneratedGfx942PersistentStorageV1> {
    pub(crate) fn prepared(&self) -> &GeneratedGfx942PersistentStorageV1 {
        &self.payload
    }

    pub(crate) fn prepared_mut(&mut self) -> &mut GeneratedGfx942PersistentStorageV1 {
        &mut self.payload
    }
}

impl GeneratedRuntimePackedArgumentsV1 {
    pub(crate) fn packed_view_v1(
        &self,
    ) -> crate::generated_kfd_arguments::GeneratedPackedArgumentsViewV1<'_> {
        self.packed.packed_view_v1()
    }

    pub const fn kernel_id(&self) -> KernelId {
        self.packed.kernel_id
    }
    pub const fn alignment(&self) -> u32 {
        self.packed.alignment
    }
    pub fn explicit_kernarg(&self) -> &[u8] {
        &self.packed.explicit_kernarg
    }
    pub fn buffers(&self) -> &[Gfx942RuntimeDispatchBufferV1] {
        &self.packed.buffers
    }
    pub const fn footprint(&self) -> GeneratedRuntimeArgumentFootprintV1 {
        self.footprint
    }
    pub const fn packing_observation(&self) -> &GeneratedKfdPackingObservationV1 {
        &self.packed.packing_observation
    }

    /// Transfers inert inputs and decoder custody, without authorizing publication or completion.
    /// An admitted async adapter must keep these paired with its exact permit through quiescence.
    pub fn into_runtime_inputs(
        self,
        geometry: AqlDispatchGeometryV1,
        dynamic_group_segment_bytes: u32,
        timeout_milliseconds: u32,
    ) -> (
        Gfx942RuntimeDispatchInputsV1,
        GeneratedRuntimeOutputDecoderV1,
    ) {
        (
            Gfx942RuntimeDispatchInputsV1::new(
                self.packed.explicit_kernarg,
                self.packed.buffers,
                self.packed.pointer_fixups,
                geometry,
                dynamic_group_segment_bytes,
                timeout_milliseconds,
            ),
            self.decoder,
        )
    }
}

struct OwnedBufferExpectation {
    byte_len: usize,
    access: Gfx942RuntimeBufferAccessV1,
    custody: Option<OutputCustody>,
    read_credit: Option<ReadResultCreditV1>,
}

/// Data-only decoder. Successfully decoded bytes are not proof that any kernel ran.
///
/// The decoder cannot authenticate their producer or invocation. GEN-2 must retain it privately
/// with exact invocation authority before interpreting decoded values as execution results.
#[must_use]
pub struct GeneratedRuntimeOutputDecoderV1 {
    expectations: Vec<OwnedBufferExpectation>,
    result_gate: Option<Arc<ResultReadyGateV1>>,
}

enum OutputState {
    Unbound,
    Bound,
    Decoded {
        scalar: RustScalarElementTypeV1,
        bytes: Vec<u8>,
    },
    Taken,
    Unavailable,
}

enum OutputCustody {
    Legacy(LegacyOutputCustody),
    Charged(ChargedOutputCustodyV1),
}

impl OutputCustody {
    fn same(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Legacy(left), Self::Legacy(right)) => Arc::ptr_eq(&left.state, &right.state),
            (Self::Charged(left), Self::Charged(right)) => left.same(right),
            _ => false,
        }
    }

    fn bound_to(
        &self,
        gate: Option<&Arc<ResultReadyGateV1>>,
    ) -> Result<(), GeneratedRuntimeArgumentErrorV1> {
        match (self, gate) {
            (Self::Legacy(custody), None)
                if matches!(
                    *custody
                        .state
                        .lock()
                        .map_err(|_| GeneratedRuntimeArgumentErrorV1::Custody)?,
                    OutputState::Bound
                ) =>
            {
                Ok(())
            }
            (Self::Charged(custody), Some(gate)) => custody.bound_to(gate),
            _ => Err(GeneratedRuntimeArgumentErrorV1::StaleOrAliasedOutput),
        }
    }

    #[cfg(test)]
    fn legacy(&self) -> &LegacyOutputCustody {
        match self {
            Self::Legacy(custody) => custody,
            _ => panic!("legacy test custody"),
        }
    }
}

struct LegacyOutputCustody {
    state: Arc<Mutex<OutputState>>,
    scalar: RustScalarElementTypeV1,
}

impl Drop for LegacyOutputCustody {
    fn drop(&mut self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if matches!(*state, OutputState::Unbound | OutputState::Bound) {
            *state = OutputState::Unavailable;
        }
    }
}

/// An observer of owned decoded data. Dropping it does not withdraw the retained packing custody.
/// `Some` means data were decoded, not that a device invocation completed or was even admitted.
#[must_use]
pub struct GeneratedRuntimeResultV1<T: GeneratedDeviceScalarV1> {
    state: Arc<Mutex<OutputState>>,
    scalar: PhantomData<T>,
}

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedRuntimeArgumentErrorV1 {
    Prepare(GeneratedKfdPrepareError),
    Pack(GeneratedArgumentPackError),
    Binding(GeneratedKfdArgumentError),
    ByteLength,
    PayloadLimit,
    BindingMismatch,
    StaleOrAliasedOutput,
    OutputUnavailable,
    Allocation,
    Custody,
    ResultCredit(fe2o3_resource_accounting::ResourceCreditErrorV1),
}

impl fmt::Display for GeneratedRuntimeArgumentErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Prepare(error) => write!(f, "owned generated preparation failed: {error}"),
            Self::Pack(error) => write!(f, "owned generated layout binding failed: {error}"),
            Self::Binding(error) => write!(f, "owned generated packing failed: {error}"),
            Self::ByteLength => f.write_str("owned generated storage byte length overflows"),
            Self::PayloadLimit => {
                f.write_str("owned generated storage exceeds its byte or binding limit")
            }
            Self::BindingMismatch => {
                f.write_str("owned generated binding type, shape or budget differs")
            }
            Self::StaleOrAliasedOutput => {
                f.write_str("owned generated output custody is stale or aliased")
            }
            Self::OutputUnavailable => {
                f.write_str("owned generated output was consumed or discarded")
            }
            Self::Allocation => f.write_str("owned generated storage allocation failed"),
            Self::Custody => f.write_str("owned generated output custody lock is poisoned"),
            Self::ResultCredit(error) => {
                write!(f, "owned generated result reservation failed: {error}")
            }
        }
    }
}

impl std::error::Error for GeneratedRuntimeArgumentErrorV1 {}

#[cfg(test)]
mod tests;

mod decoded_outputs;
