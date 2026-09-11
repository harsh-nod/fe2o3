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
    ChargedOutputCustodyV1, GeneratedRuntimeChargedResultV1, GeneratedRuntimeResultBudgetV1,
    ReadResultCreditV1, ResultBindingBudgetV1, ResultDescriptorV1, ResultMemberV1,
    ResultPreflightV1, ResultReadyGateV1,
};
use crate::{AuthenticatedWorkerV3ExecutableV1, CompilerGeneratedKernelExpectationV1, KernelId};

mod charged_decode;
#[cfg(test)]
mod charged_tests;
mod readback;
pub(crate) use readback::GeneratedRuntimeReadbackOwnerV1;

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

fn prepare_charged_with_plan<A>(
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
    values: Box<[T]>,
}

impl<T: GeneratedDeviceScalarV1> GeneratedRuntimeReadSlice<T> {
    pub fn new(values: Box<[T]>) -> Self {
        Self { values }
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
            Gfx942RuntimeBufferAccessV1::ReadOnly,
            None,
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
                    self.values,
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
                    self.values,
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
    values: Box<[T]>,
    custody: Option<OutputCustody>,
    plan: &GeneratedArgumentPackingPlanV1,
    argument_index: usize,
    access: Gfx942RuntimeBufferAccessV1,
    index_space: Option<RustDisjointIndexSpaceV1>,
    budget: &mut GeneratedRuntimeArgumentBudgetV1,
) -> Result<GeneratedRuntimeSliceBindingV1, GeneratedRuntimeArgumentErrorV1> {
    let byte_len = values
        .len()
        .checked_mul(size_of::<T>())
        .ok_or(GeneratedRuntimeArgumentErrorV1::ByteLength)?;
    let borrow = GeneratedArgumentBorrowV1::new();
    let input = match (access, index_space) {
        (Gfx942RuntimeBufferAccessV1::ReadOnly, None) => plan
            .bind_generated_address_free_read_slice_v1::<T>(argument_index, values.len(), borrow),
        (Gfx942RuntimeBufferAccessV1::WriteOnly, None) => plan
            .bind_generated_address_free_write_slice_v1::<T>(argument_index, values.len(), borrow),
        (Gfx942RuntimeBufferAccessV1::ReadWrite, None) => plan
            .bind_generated_address_free_read_write_slice_v1::<T>(
                argument_index,
                values.len(),
                borrow,
            ),
        (Gfx942RuntimeBufferAccessV1::WriteOnly, Some(mapping)) => plan
            .bind_generated_address_free_mapped_write_slice_v1::<T>(
                argument_index,
                values.len(),
                mapping,
                borrow,
            ),
        (Gfx942RuntimeBufferAccessV1::ReadWrite, Some(mapping)) => plan
            .bind_generated_address_free_mapped_read_write_slice_v1::<T>(
                argument_index,
                values.len(),
                mapping,
                borrow,
            ),
        _ => return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch),
    }
    .map_err(GeneratedRuntimeArgumentErrorV1::Pack)?;
    let member = budget.bind_slice::<T>(values.len(), access, custody.as_ref())?;
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
        custody.bind_seed(
            values,
            member.ok_or(GeneratedRuntimeArgumentErrorV1::BindingMismatch)?,
        )?;
        custody.with_seed::<T, _>(|values| encode_owned_slice(values, byte_len, access))?
    } else {
        read_credit = member.map(ResultMemberV1::retain_read);
        encode_owned_slice(&values, byte_len, access)?
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

    pub(crate) fn project_persistent(
        self,
        hsaco: &[u8],
    ) -> Result<
        GeneratedRuntimeStorageV1<GeneratedGfx942PersistentStorageV1>,
        Gfx942RuntimeProjectionErrorV1,
    > {
        // A closed failure cannot detach consumed storage from its retained decoder.
        let payload = self
            .payload
            .into_persistent_projection_v1(hsaco)?
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

impl GeneratedRuntimeOutputDecoderV1 {
    /// Consumes owned, address-free buffer data after validating the entire shape first.
    /// No native resource is freed, no completion certificate is created, and no work is issued.
    /// Each buffer must have capacity exactly equal to its length; excess backing is rejected.
    pub fn decode_buffers(
        self,
        buffers: Vec<(Gfx942RuntimeBufferAccessV1, Vec<u8>)>,
    ) -> Result<(), GeneratedRuntimeArgumentErrorV1> {
        if self.result_gate.is_some() {
            return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
        }
        let count = self
            .expectations
            .iter()
            .filter(|expected| expected.byte_len != 0)
            .count();
        if count != buffers.len() {
            return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
        }
        let mut data = buffers.iter();
        for expected in &self.expectations {
            if expected.byte_len != 0 {
                let buffer = data
                    .next()
                    .ok_or(GeneratedRuntimeArgumentErrorV1::BindingMismatch)?;
                if buffer.1.len() != expected.byte_len
                    || buffer.1.capacity() != expected.byte_len
                    || buffer.0 != expected.access
                {
                    return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
                }
            }
            if let Some(custody) = &expected.custody {
                custody.bound_to(None)?;
            }
        }
        let mut data = buffers.into_iter();
        for expected in self.expectations {
            let bytes = if expected.byte_len == 0 {
                Vec::new()
            } else {
                data.next()
                    .ok_or(GeneratedRuntimeArgumentErrorV1::BindingMismatch)?
                    .1
            };
            if let Some(OutputCustody::Legacy(custody)) = expected.custody {
                *custody
                    .state
                    .lock()
                    .map_err(|_| GeneratedRuntimeArgumentErrorV1::Custody)? =
                    OutputState::Decoded {
                        scalar: custody.scalar,
                        bytes,
                    };
            }
        }
        Ok(())
    }
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

impl<T: GeneratedDeviceScalarV1> GeneratedRuntimeResultV1<T> {
    pub fn try_take(&mut self) -> Result<Option<Box<[T]>>, GeneratedRuntimeArgumentErrorV1> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| GeneratedRuntimeArgumentErrorV1::Custody)?;
        match &*state {
            OutputState::Unbound | OutputState::Bound => Ok(None),
            OutputState::Taken | OutputState::Unavailable => {
                Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
            }
            OutputState::Decoded { scalar, bytes } => {
                if *scalar != T::RUST_SCALAR_TYPE || !bytes.len().is_multiple_of(size_of::<T>()) {
                    return Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch);
                }
                let mut values = Vec::new();
                values
                    .try_reserve_exact(bytes.len() / size_of::<T>())
                    .map_err(|_| GeneratedRuntimeArgumentErrorV1::Allocation)?;
                for encoded in bytes.chunks_exact(size_of::<T>()) {
                    values.push(
                        T::decode_le_bytes_v1(encoded)
                            .ok_or(GeneratedRuntimeArgumentErrorV1::BindingMismatch)?,
                    );
                }
                *state = OutputState::Taken;
                Ok(Some(values.into_boxed_slice()))
            }
        }
    }
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
mod tests {
    use super::*;
    use crate::generated_argument_plan::validate_argument_packing;
    use crate::generated_kfd_arguments::GeneratedKfdReadWriteSlice;
    use fe2o3_artifacts::{
        AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership,
        Mutability, Name, PointerWidth,
    };

    pub(super) fn plan<T: GeneratedDeviceScalarV1>(
        accesses: &[Access],
        mapping: Option<RustDisjointIndexSpaceV1>,
    ) -> GeneratedArgumentPackingPlanV1 {
        let fields = accesses
            .iter()
            .enumerate()
            .map(|(index, access)| {
                let read_only = *access == Access::ReadOnly;
                let identity = if read_only {
                    T::shared_slice_type_identity_v1(PointerWidth::Bits64)
                } else if let Some(mapping) = mapping {
                    T::disjoint_slice_type_identity_for_index_space_v1(
                        PointerWidth::Bits64,
                        mapping,
                    )
                } else {
                    T::disjoint_slice_type_identity_v1(PointerWidth::Bits64)
                };
                AbiField::new(
                    Name::new(format!("arg_{index}")).unwrap(),
                    (index * 16) as u64,
                    16,
                    8,
                    AbiKind::Slice {
                        element_size: T::RUST_SCALAR_TYPE.size_bytes(),
                        element_alignment: T::RUST_SCALAR_TYPE.size_bytes() as u32,
                    },
                    if read_only {
                        Mutability::Immutable
                    } else {
                        Mutability::Mutable
                    },
                    *access,
                    AddressSpace::Global,
                    identity,
                    if read_only {
                        ArgumentOwnership::SharedBorrow
                    } else {
                        ArgumentOwnership::UniqueBorrow
                    },
                    if read_only {
                        AliasClass::SharedReadOnly
                    } else {
                        AliasClass::Exclusive
                    },
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        let bytes = (fields.len() * 16) as u64;
        let abi = AbiLayout::new(bytes, 8, PointerWidth::Bits64, fields.clone()).unwrap();
        let mappings = accesses
            .iter()
            .map(|access| {
                if *access == Access::ReadOnly {
                    None
                } else {
                    mapping
                }
            })
            .collect();
        let layout = CompilerGeneratedArgumentLayoutV1::new_with_disjoint_index_spaces_v1(
            bytes,
            8,
            PointerWidth::Bits64,
            fields,
            mappings,
        )
        .unwrap();
        validate_argument_packing(KernelId::from_bytes([0x67; 32]), &abi, &layout).unwrap()
    }

    fn budget(plan: &GeneratedArgumentPackingPlanV1) -> GeneratedRuntimeArgumentBudgetV1 {
        GeneratedRuntimeArgumentBudgetV1::new(
            plan,
            GeneratedRuntimeArgumentLimitsV1::new(4096, 4096, 8),
        )
        .unwrap()
    }

    fn output_fixture() -> (
        GeneratedRuntimePackedArgumentsV1,
        GeneratedRuntimeResultV1<u32>,
    ) {
        let plan = plan::<u32>(&[Access::ReadWrite], None);
        let mut budget = budget(&plan);
        let (output, observer) =
            GeneratedRuntimeReadWriteSlice::new(vec![3_u32, 7, 11, 19].into_boxed_slice());
        let binding = output.bind_argument(&plan, 0, &mut budget).unwrap();
        let packed =
            GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(vec![], vec![binding])
                .pack(&plan, budget)
                .unwrap();
        (packed, observer)
    }

    fn words(values: &[u32]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>()
            .into_boxed_slice()
            .into_vec()
    }

    #[test]
    fn owned_packing_reuses_exact_borrowed_abi_bytes_and_observations() {
        fn assert_owned<T: Send + 'static>() {}
        assert_owned::<GeneratedRuntimePackedArgumentsV1>();
        assert_owned::<GeneratedRuntimeOutputDecoderV1>();
        assert_owned::<GeneratedRuntimeResultV1<u32>>();
        let (owned, mut observer) = output_fixture();
        let mut values = [3_u32, 7, 11, 19];
        let plan = plan::<u32>(&[Access::ReadWrite], None);
        let binding = GeneratedKfdReadWriteSlice::new(&mut values)
            .bind_argument(&plan, 0)
            .unwrap();
        let borrowed =
            GeneratedKfdArgumentBinding::from_compiler_generated_parts(vec![], vec![binding])
                .pack(&plan)
                .unwrap();
        assert_eq!(owned.kernel_id(), borrowed.kernel_id());
        assert_eq!(owned.alignment(), borrowed.alignment());
        assert_eq!(owned.explicit_kernarg(), borrowed.explicit_kernarg());
        assert_eq!(owned.buffers(), borrowed.buffers());
        assert_eq!(owned.packing_observation(), borrowed.packing_observation());
        assert_eq!(
            owned.footprint(),
            GeneratedRuntimeArgumentFootprintV1 {
                kernarg_bytes: 16,
                input_bytes: 16,
                output_bytes: 16,
                payload_bytes: 64,
                result_bytes: 32,
                bindings: 1,
                output_observers: 1,
            }
        );
        assert!(observer.try_take().unwrap().is_none());
        drop(owned);
        assert!(matches!(
            observer.try_take(),
            Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
        ));
    }

    #[test]
    fn borrowed_writeback_cannot_be_extracted_as_owned_storage() {
        let mut values = [3_u32];
        let plan = plan::<u32>(&[Access::ReadWrite], None);
        let binding = GeneratedKfdReadWriteSlice::new(&mut values)
            .bind_argument(&plan, 0)
            .unwrap();
        let borrowed =
            GeneratedKfdArgumentBinding::from_compiler_generated_parts(vec![], vec![binding])
                .pack(&plan)
                .unwrap();
        assert!(matches!(
            borrowed.into_owned_parts(),
            Err(GeneratedKfdArgumentError::RetainedWriteback)
        ));
        assert_eq!(values, [3]);
    }

    #[test]
    fn byte_limits_reject_before_encoding_and_checked_charges_do_not_partially_debit() {
        let plan = plan::<u32>(&[Access::ReadWrite], None);
        for limits in [
            GeneratedRuntimeArgumentLimitsV1::new(63, 32, 1),
            GeneratedRuntimeArgumentLimitsV1::new(64, 31, 1),
        ] {
            let mut budget = GeneratedRuntimeArgumentBudgetV1::new(&plan, limits).unwrap();
            let before = budget.footprint();
            let (output, mut observer) =
                GeneratedRuntimeReadWriteSlice::new(vec![1_u32; 4].into_boxed_slice());
            assert!(matches!(
                output.bind_argument(&plan, 0, &mut budget),
                Err(GeneratedRuntimeArgumentErrorV1::PayloadLimit)
            ));
            assert_eq!(budget.footprint(), before);
            assert!(matches!(
                observer.try_take(),
                Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
            ));
        }
        assert!(matches!(
            GeneratedRuntimeArgumentBudgetV1::new(
                &plan,
                GeneratedRuntimeArgumentLimitsV1::new(32, 0, 0)
            ),
            Err(GeneratedRuntimeArgumentErrorV1::PayloadLimit)
        ));
        let mut budget = budget(&plan);
        let before = budget.footprint();
        assert!(matches!(
            budget.charge(usize::MAX, true),
            Err(GeneratedRuntimeArgumentErrorV1::ByteLength)
        ));
        assert_eq!(budget.footprint(), before);
    }

    #[test]
    fn whole_invocation_preflight_rejects_without_binding_or_encoding_earlier_outputs() {
        let plan = plan::<u32>(&[Access::ReadWrite, Access::ReadWrite], None);
        let mut preflight = GeneratedRuntimeArgumentBudgetV1::new(
            &plan,
            GeneratedRuntimeArgumentLimitsV1::new(80, 32, 2),
        )
        .unwrap();
        let (first, mut first_observer) =
            GeneratedRuntimeReadWriteSlice::new(vec![3_u32].into_boxed_slice());
        let (second, mut second_observer) =
            GeneratedRuntimeReadWriteSlice::new(vec![7_u32; 4].into_boxed_slice());
        first.account_storage(&mut preflight).unwrap();
        let before = preflight.footprint();
        assert!(matches!(
            second.account_storage(&mut preflight),
            Err(GeneratedRuntimeArgumentErrorV1::PayloadLimit)
        ));
        assert_eq!(preflight.footprint(), before);
        assert!(matches!(
            *first.custody.legacy().state.lock().unwrap(),
            OutputState::Unbound
        ));
        assert!(matches!(
            *second.custody.legacy().state.lock().unwrap(),
            OutputState::Unbound
        ));
        assert_eq!(&*first.values, &[3]);
        assert!(first_observer.try_take().unwrap().is_none());
        assert!(second_observer.try_take().unwrap().is_none());
        drop((first, second));
        assert!(matches!(
            first_observer.try_take(),
            Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
        ));
        assert!(matches!(
            second_observer.try_take(),
            Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
        ));
    }

    #[test]
    fn owned_type_access_and_index_mapping_substitution_reject() {
        let plan = plan::<u32>(&[Access::ReadWrite], None);
        let (wrong_type, _) = GeneratedRuntimeReadWriteSlice::new(vec![1_f32].into_boxed_slice());
        assert!(matches!(
            wrong_type.bind_argument(&plan, 0, &mut budget(&plan)),
            Err(GeneratedRuntimeArgumentErrorV1::Pack(_))
        ));
        let wrong_access = GeneratedRuntimeReadSlice::new(vec![1_u32].into_boxed_slice());
        assert!(matches!(
            wrong_access.bind_argument(&plan, 0, &mut budget(&plan)),
            Err(GeneratedRuntimeArgumentErrorV1::Pack(_))
        ));
        let (wrong_mapping, _) =
            GeneratedRuntimeReadWriteSlice::new(vec![1_u32].into_boxed_slice());
        assert!(matches!(
            wrong_mapping.bind_mapped_argument(
                &plan,
                0,
                RustDisjointIndexSpaceV1::ShiftedIndex1D { offset: 1 },
                &mut budget(&plan)
            ),
            Err(GeneratedRuntimeArgumentErrorV1::Pack(_))
        ));
    }

    #[test]
    fn stale_private_output_custody_and_duplicate_binding_reject() {
        let plan = plan::<u32>(&[Access::ReadWrite, Access::ReadWrite], None);
        let mut budget = budget(&plan);
        let (first, mut observer) =
            GeneratedRuntimeReadWriteSlice::new(vec![1_u32].into_boxed_slice());
        let duplicate = GeneratedRuntimeReadWriteSlice {
            values: vec![2_u32].into_boxed_slice(),
            custody: OutputCustody::Legacy(LegacyOutputCustody {
                state: Arc::clone(&first.custody.legacy().state),
                scalar: u32::RUST_SCALAR_TYPE,
            }),
        };
        let first = first.bind_argument(&plan, 0, &mut budget).unwrap();
        assert!(matches!(
            duplicate.bind_argument(&plan, 1, &mut budget),
            Err(GeneratedRuntimeArgumentErrorV1::StaleOrAliasedOutput)
        ));
        assert!(matches!(
            observer.try_take(),
            Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
        ));
        drop(first);

        let (stale, _) = GeneratedRuntimeReadWriteSlice::new(vec![1_u32].into_boxed_slice());
        *stale.custody.legacy().state.lock().unwrap() = OutputState::Taken;
        assert!(matches!(
            stale.bind_argument(&plan, 0, &mut budget),
            Err(GeneratedRuntimeArgumentErrorV1::StaleOrAliasedOutput)
        ));
    }

    #[test]
    fn dropped_observer_does_not_withdraw_owned_storage_or_decoder_custody() {
        let (packed, observer) = output_fixture();
        let weak = Arc::downgrade(&observer.state);
        drop(observer);
        assert!(weak.upgrade().is_some());
        std::thread::spawn(move || {
            assert_eq!(packed.buffers()[0].bytes(), words(&[3, 7, 11, 19]));
            let GeneratedRuntimePackedArgumentsV1 {
                packed, decoder, ..
            } = packed;
            drop(packed);
            decoder
                .decode_buffers(vec![(
                    Gfx942RuntimeBufferAccessV1::ReadWrite,
                    words(&[5, 9, 13, 21]),
                )])
                .unwrap();
        })
        .join()
        .unwrap();
        assert!(weak.upgrade().is_none());
    }

    #[test]
    fn owned_result_outlives_input_and_rejects_wrong_type_and_repeated_take() {
        let (packed, mut observer) = output_fixture();
        let GeneratedRuntimePackedArgumentsV1 {
            packed, decoder, ..
        } = packed;
        drop(packed);
        decoder
            .decode_buffers(vec![(
                Gfx942RuntimeBufferAccessV1::ReadWrite,
                words(&[5, 7, 11, 19]),
            )])
            .unwrap();
        let mut forged_type = GeneratedRuntimeResultV1::<f32> {
            state: Arc::clone(&observer.state),
            scalar: PhantomData,
        };
        assert!(matches!(
            forged_type.try_take(),
            Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch)
        ));
        assert_eq!(&*observer.try_take().unwrap().unwrap(), &[5, 7, 11, 19]);
        assert!(matches!(
            observer.try_take(),
            Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
        ));
    }

    #[test]
    fn decoder_checks_every_shape_before_delivering_any_output() {
        let plan = plan::<u32>(&[Access::ReadWrite, Access::ReadWrite], None);
        let mut budget = budget(&plan);
        let (first, mut first_observer) =
            GeneratedRuntimeReadWriteSlice::new(vec![1_u32].into_boxed_slice());
        let (second, mut second_observer) =
            GeneratedRuntimeReadWriteSlice::new(vec![2_u32].into_boxed_slice());
        let bindings = vec![
            first.bind_argument(&plan, 0, &mut budget).unwrap(),
            second.bind_argument(&plan, 1, &mut budget).unwrap(),
        ];
        let packed =
            GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(vec![], bindings)
                .pack(&plan, budget)
                .unwrap();
        assert!(matches!(
            packed.decoder.decode_buffers(vec![
                (Gfx942RuntimeBufferAccessV1::ReadWrite, words(&[10])),
                (Gfx942RuntimeBufferAccessV1::ReadOnly, words(&[20])),
            ]),
            Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch)
        ));
        assert!(matches!(
            first_observer.try_take(),
            Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
        ));
        assert!(matches!(
            second_observer.try_take(),
            Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
        ));
    }

    #[test]
    fn uncharged_binding_and_excess_result_capacity_reject() {
        let plan = plan::<u32>(&[Access::ReadWrite], None);
        let mut charged = budget(&plan);
        let (output, _) = GeneratedRuntimeReadWriteSlice::new(vec![1_u32].into_boxed_slice());
        let binding = output.bind_argument(&plan, 0, &mut charged).unwrap();
        assert!(matches!(
            GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(vec![], vec![binding])
                .pack(&plan, budget(&plan)),
            Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch)
        ));
        let (packed, mut observer) = output_fixture();
        let mut bytes = Vec::with_capacity(4096);
        bytes.extend_from_slice(&words(&[1, 2, 3, 4]));
        assert!(matches!(
            packed
                .decoder
                .decode_buffers(vec![(Gfx942RuntimeBufferAccessV1::ReadWrite, bytes)]),
            Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch)
        ));
        assert!(matches!(
            observer.try_take(),
            Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
        ));
    }

    #[test]
    fn empty_write_only_output_and_mapped_initialized_seed_are_preserved() {
        let mapping = RustDisjointIndexSpaceV1::ShiftedIndex1D { offset: 1 };
        let plan = plan::<u32>(&[Access::WriteOnly, Access::WriteOnly], Some(mapping));
        let mut budget = budget(&plan);
        let (empty, mut empty_observer) =
            GeneratedRuntimeWriteSlice::new(Vec::<u32>::new().into_boxed_slice());
        let (output, mut observer) =
            GeneratedRuntimeWriteSlice::new(vec![17_u32, 23].into_boxed_slice());
        let bindings = vec![
            empty
                .bind_mapped_argument(&plan, 0, mapping, &mut budget)
                .unwrap(),
            output
                .bind_mapped_argument(&plan, 1, mapping, &mut budget)
                .unwrap(),
        ];
        let packed =
            GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(vec![], bindings)
                .pack(&plan, budget)
                .unwrap();
        assert_eq!(packed.buffers().len(), 1);
        assert_eq!(packed.buffers()[0].bytes(), words(&[17, 23]));
        assert_eq!(
            packed.buffers()[0].access(),
            Gfx942RuntimeBufferAccessV1::WriteOnly
        );
        packed
            .decoder
            .decode_buffers(vec![(
                Gfx942RuntimeBufferAccessV1::WriteOnly,
                words(&[17, 29]),
            )])
            .unwrap();
        assert!(empty_observer.try_take().unwrap().unwrap().is_empty());
        assert_eq!(&*observer.try_take().unwrap().unwrap(), &[17, 29]);
    }
}
