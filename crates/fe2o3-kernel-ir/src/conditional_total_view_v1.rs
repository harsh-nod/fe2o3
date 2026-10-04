//! Bounded conditional identity coverage over a borrowed, verified canonical Module.
//!
//! Establishes one write for `global.x < output.len()` and none otherwise,
//! with normal CFG exits, conditional on caller-bound memory validity and
//! representable address arithmetic over the retained address domain. Total
//! coverage additionally requires `output.len() <= global launch extent x`
//! and a valid D1 launch. Whole-buffer validity, pointer targets, and target
//! index/address representability remain caller obligations, never discharged
//! here. This module defines no runtime condition schema, value-refinement
//! proof, receipt, or execution authority.

use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

use crate::{
    AccessMode, AddressSpace, Axis, BinaryOp, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError, CastKind, ComparePredicate,
    Constant, ControlFlowError, ControlFlowLimits, Function, FunctionOperationLocation, IndexKind,
    IndexedControlFlow, IntrinsicKind, Kernel, KernelId, LaunchDomain, LaunchExtent, MemoryAccess,
    Module, Operation, OperationKind, ScalarType, Terminator, Type, ValueId,
    VerifiedKernelIrModuleV1,
    control_flow::{MeteredControlFlowErrorV1, analyze_control_flow_with_verification_budget_v1},
    verification_index_v1::{
        verification_bounded_sort_by_v1, verification_ceil_log2_v1, verification_find_last_by_v1,
    },
};

#[path = "conditional_total_view_paths_v1.rs"]
mod paths;
use paths::check_paths;

/// A closed-fragment refusal, never evidence that the program is incorrect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionalTotalViewUnsupportedV1 {
    Kernel,
    Function,
    LaunchDomain,
    Signature,
    UnreachableBlock {
        block: BlockId,
    },
    BlockArguments {
        block: BlockId,
    },
    StoreCount {
        actual: usize,
    },
    StoreAccess {
        location: FunctionOperationLocation,
    },
    OutputParameter,
    Definition {
        value: ValueId,
    },
    Address {
        value: ValueId,
    },
    Index {
        value: ValueId,
    },
    Predicate {
        value: ValueId,
    },
    MissingPredicate,
    MultiplePredicates,
    Call {
        location: FunctionOperationLocation,
    },
    Operation {
        location: FunctionOperationLocation,
    },
    Terminator {
        block: BlockId,
    },
    Cycle,
    AbnormalExit {
        block: BlockId,
    },
    WriteCount {
        block: BlockId,
        predicate_true: bool,
    },
}

/// Descriptive address-arithmetic domain, not a discharged runtime premise.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConditionalTotalViewAddressDomainV1 {
    /// Address formation uses an output-bounded index or a selected zero offset.
    /// The caller must admit output-span arithmetic, including the zero-offset
    /// base on a tail invocation even when the logical output length is zero.
    GuardedOutput,
    /// A direct global index may form an address when `global.x >= output.len()`.
    /// For global extent `G > 0`, the caller must establish representability of
    /// `base + (G - 1) * element_bytes`, including intermediate arithmetic, over
    /// the whole launch even outside the logical output and when its length is
    /// zero. For `G == 0`, this domain is empty; `G - 1` is not evaluated.
    /// This requires no tail memory access or allocation of `G` elements.
    GlobalLaunch,
}

/// An exact input read and the two distinct domains required by its execution.
/// `access_domain` requires readable elements; `address_domain` only requires
/// representable address formation. Neither implies allocation of a launch tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalTotalViewReadV1 {
    parameter: u32,
    slice: ValueId,
    pointer: ValueId,
    index: ValueId,
    value: ValueId,
    location: FunctionOperationLocation,
    access_domain: ConditionalTotalViewAddressDomainV1,
    address_domain: ConditionalTotalViewAddressDomainV1,
    element_bytes: u64,
    alignment: u32,
}

impl ConditionalTotalViewReadV1 {
    pub const fn parameter(self) -> u32 {
        self.parameter
    }
    pub const fn slice(self) -> ValueId {
        self.slice
    }
    pub const fn pointer(self) -> ValueId {
        self.pointer
    }
    pub const fn index(self) -> ValueId {
        self.index
    }
    pub const fn value(self) -> ValueId {
        self.value
    }
    pub const fn location(self) -> FunctionOperationLocation {
        self.location
    }
    pub const fn access_domain(self) -> ConditionalTotalViewAddressDomainV1 {
        self.access_domain
    }
    pub const fn address_domain(self) -> ConditionalTotalViewAddressDomainV1 {
        self.address_domain
    }
    pub const fn element_bytes(self) -> u64 {
        self.element_bytes
    }
    pub const fn alignment(self) -> u32 {
        self.alignment
    }
}

/// Typed failures from the caller's shared ledger or the existing bounded CFG.
#[derive(Debug, Eq, PartialEq)]
pub enum ConditionalTotalViewErrorV1 {
    Resource(ResourceError),
    ControlFlow(ControlFlowError),
}

impl From<ResourceError> for ConditionalTotalViewErrorV1 {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

impl fmt::Display for ConditionalTotalViewErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(out),
            Self::ControlFlow(error) => error.fmt(out),
        }
    }
}

impl std::error::Error for ConditionalTotalViewErrorV1 {}

/// Fixed-size observations tied to the exact Module borrowed at derivation.
///
/// Private fields prevent caller-authored premises. Coordinates and metadata
/// remain inert outside this subject; the Module borrow authenticates neither
/// a production owner nor a runtime launch. Whole-buffer validity, pointer
/// targets, and representable arithmetic for `address_domain` are obligations
/// of the integrating caller, including when no output element is written.
pub struct ConditionalTotalViewFactsV1<'module> {
    module: &'module Module,
    kernel_ordinal: usize,
    function_ordinal: usize,
    output_parameter_index: u32,
    output_value: ValueId,
    index: ValueId,
    length: ValueId,
    predicate: ValueId,
    pointer: ValueId,
    offset: ValueId,
    address_domain: ConditionalTotalViewAddressDomainV1,
    store_value: ValueId,
    store_predicate: Option<ValueId>,
    store_location: FunctionOperationLocation,
    element_bytes: u64,
    alignment: u32,
    reads: usize,
}

impl fmt::Debug for ConditionalTotalViewFactsV1<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("ConditionalTotalViewFactsV1")
            .field("kernel_ordinal", &self.kernel_ordinal)
            .field("function_ordinal", &self.function_ordinal)
            .field("output_parameter_index", &self.output_parameter_index)
            .field("output_value", &self.output_value)
            .field("index", &self.index)
            .field("length", &self.length)
            .field("predicate", &self.predicate)
            .field("pointer", &self.pointer)
            .field("offset", &self.offset)
            .field("address_domain", &self.address_domain)
            .field("store_value", &self.store_value)
            .field("store_predicate", &self.store_predicate)
            .field("store_location", &self.store_location)
            .field("element_bytes", &self.element_bytes)
            .field("alignment", &self.alignment)
            .finish()
    }
}

impl<'module> ConditionalTotalViewFactsV1<'module> {
    pub const fn module(&self) -> &'module Module {
        self.module
    }
    pub fn kernel(&self) -> &'module Kernel {
        &self.module.kernels[self.kernel_ordinal]
    }
    pub fn function(&self) -> &'module Function {
        &self.module.functions[self.function_ordinal]
    }
    pub const fn kernel_ordinal(&self) -> usize {
        self.kernel_ordinal
    }
    pub const fn function_ordinal(&self) -> usize {
        self.function_ordinal
    }
    pub const fn output_parameter_index(&self) -> u32 {
        self.output_parameter_index
    }
    pub const fn output_value(&self) -> ValueId {
        self.output_value
    }
    pub const fn index(&self) -> ValueId {
        self.index
    }
    pub const fn length(&self) -> ValueId {
        self.length
    }
    pub const fn predicate(&self) -> ValueId {
        self.predicate
    }
    pub const fn pointer(&self) -> ValueId {
        self.pointer
    }
    pub const fn offset(&self) -> ValueId {
        self.offset
    }
    /// Address arithmetic required as a premise, not proved runtime validity.
    pub const fn address_domain(&self) -> ConditionalTotalViewAddressDomainV1 {
        self.address_domain
    }
    pub const fn store_value(&self) -> ValueId {
        self.store_value
    }
    pub const fn store_predicate(&self) -> Option<ValueId> {
        self.store_predicate
    }
    pub const fn store_location(&self) -> FunctionOperationLocation {
        self.store_location
    }
    pub const fn element_bytes(&self) -> u64 {
        self.element_bytes
    }
    pub const fn alignment(&self) -> u32 {
        self.alignment
    }

    pub const fn read_count(&self) -> usize {
        self.reads
    }

    /// Replays the same borrowed graph and visits each derived input obligation.
    /// No caller-provided read roster or second graph is accepted. A visitor's
    /// retained allocations must be admitted separately on this same ledger.
    pub fn visit_reads_v1(
        &self,
        budget: &mut Budget<'_>,
        mut visit: impl FnMut(ConditionalTotalViewReadV1) -> Result<(), ResourceError>,
    ) -> Result<(), ConditionalTotalViewErrorV1> {
        let floor = budget.storage_checkpoint();
        let result = catch_unwind(AssertUnwindSafe(|| {
            derive(self.module, &self.kernel().id, budget, &mut visit)
        }));
        budget.rollback_storage(floor)?;
        match result {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(Failure::Error(error))) => Err(error),
            // An immutable successfully derived subject cannot change fragments.
            Ok(Err(Failure::Unsupported(_))) => Err(ResourceError::Accounting.into()),
            Err(payload) => resume_unwind(payload),
        }
    }
}

#[derive(Debug)]
pub enum ConditionalTotalViewAnalysisV1<'module> {
    Established(ConditionalTotalViewFactsV1<'module>),
    Unsupported(ConditionalTotalViewUnsupportedV1),
}

impl<'module> ConditionalTotalViewAnalysisV1<'module> {
    pub const fn facts(&self) -> Option<&ConditionalTotalViewFactsV1<'module>> {
        match self {
            Self::Established(facts) => Some(facts),
            Self::Unsupported(_) => None,
        }
    }

    pub const fn unsupported_reason(&self) -> Option<ConditionalTotalViewUnsupportedV1> {
        match self {
            Self::Established(_) => None,
            Self::Unsupported(reason) => Some(*reason),
        }
    }
}

enum Failure {
    Unsupported(ConditionalTotalViewUnsupportedV1),
    Error(ConditionalTotalViewErrorV1),
}

impl From<ResourceError> for Failure {
    fn from(error: ResourceError) -> Self {
        Self::Error(error.into())
    }
}

impl From<MeteredControlFlowErrorV1> for Failure {
    fn from(error: MeteredControlFlowErrorV1) -> Self {
        Self::Error(match error {
            MeteredControlFlowErrorV1::Resource(error) => error.into(),
            MeteredControlFlowErrorV1::ControlFlow(error) => {
                ConditionalTotalViewErrorV1::ControlFlow(error)
            }
        })
    }
}

type Derived<T> = Result<T, Failure>;
use ConditionalTotalViewUnsupportedV1 as Unsupported;

fn refuse<T>(reason: Unsupported) -> Derived<T> {
    Err(Failure::Unsupported(reason))
}

#[derive(Clone, Copy)]
struct Definition<'module> {
    value: ValueId,
    operation: &'module Operation,
    location: FunctionOperationLocation,
}

#[derive(Clone, Copy)]
struct Store {
    location: FunctionOperationLocation,
    pointer: ValueId,
    value: ValueId,
    predicate: Option<ValueId>,
    access: MemoryAccess,
}

#[derive(Clone, Copy, Default)]
struct BlockState {
    reachable: bool,
    indegree: usize,
    // Bits 0, 1 and 2 mean zero, one and two-or-more writes. Cases: false, true.
    counts: [u8; 2],
}

fn allocate<T>(count: usize, budget: &mut Budget<'_>) -> Derived<Vec<T>> {
    budget.charge_work(2)?;
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(ResourceError::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| ResourceError::Allocation)?;
    let actual = rows
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(ResourceError::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(bytes).ok_or(ResourceError::Accounting)?)?;
    Ok(rows)
}

fn definition<'module>(
    rows: &[Definition<'module>],
    value: ValueId,
    budget: &mut Budget<'_>,
) -> Derived<Definition<'module>> {
    verification_find_last_by_v1(rows, 1, budget, |row| row.value.cmp(&value))?
        .map(|index| rows[index])
        .ok_or(Failure::Unsupported(Unsupported::Definition { value }))
}

fn position(flow: &IndexedControlFlow, block: BlockId, budget: &mut Budget<'_>) -> Derived<usize> {
    // Also prepays one subsequent reachability/outgoing-edge lookup at this block.
    budget.charge_work(
        verification_ceil_log2_v1(flow.block_count())
            .checked_add(4)
            .and_then(|work| work.checked_mul(2))
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    flow.block_position(block)
        .ok_or(ResourceError::Accounting.into())
}

fn matching_predicate(
    rows: &[Definition<'_>],
    predicate: ValueId,
    index: ValueId,
    output: ValueId,
    budget: &mut Budget<'_>,
) -> Derived<Option<ValueId>> {
    let compare = definition(rows, predicate, budget)?;
    budget.charge_work(4)?;
    let OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs,
        rhs,
    } = compare.operation.kind
    else {
        return Ok(None);
    };
    if lhs != index {
        return Ok(None);
    }
    let length = definition(rows, rhs, budget)?;
    Ok(
        matches!(length.operation.kind, OperationKind::SliceLength { slice } if slice == output)
            .then_some(rhs),
    )
}

fn derive<'module>(
    module: &'module Module,
    kernel_id: &KernelId,
    budget: &mut Budget<'_>,
    visit_read: &mut dyn FnMut(ConditionalTotalViewReadV1) -> Result<(), ResourceError>,
) -> Derived<ConditionalTotalViewFactsV1<'module>> {
    let mut kernel_ordinal = None;
    for (ordinal, kernel) in module.kernels.iter().enumerate() {
        budget.charge_work(
            kernel
                .id
                .as_str()
                .len()
                .min(kernel_id.as_str().len())
                .checked_add(2)
                .ok_or(ResourceError::Arithmetic)?,
        )?;
        if kernel.id == *kernel_id {
            kernel_ordinal = Some(ordinal);
            break;
        }
    }
    let kernel_ordinal = kernel_ordinal.ok_or(Failure::Unsupported(Unsupported::Kernel))?;
    let kernel = &module.kernels[kernel_ordinal];
    if kernel.domain
        != (LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        })
    {
        return refuse(Unsupported::LaunchDomain);
    }
    let mut function_ordinal = None;
    for (ordinal, function) in module.functions.iter().enumerate() {
        budget.charge_work(
            function
                .id
                .as_str()
                .len()
                .min(kernel.entry.as_str().len())
                .checked_add(2)
                .ok_or(ResourceError::Arithmetic)?,
        )?;
        if function.id == kernel.entry {
            function_ordinal = Some(ordinal);
            break;
        }
    }
    let function_ordinal = function_ordinal.ok_or(Failure::Unsupported(Unsupported::Function))?;
    let function = &module.functions[function_ordinal];
    let body = function
        .body
        .as_ref()
        .ok_or(Failure::Unsupported(Unsupported::Function))?;
    budget.charge_work(4)?;
    if !function.signature.results.is_empty() {
        return refuse(Unsupported::Signature);
    }
    let control = analyze_control_flow_with_verification_budget_v1(
        function,
        ControlFlowLimits::DEFAULT,
        budget,
    )?;
    let flow = control.indexed_v15();
    let mut states = allocate::<BlockState>(body.blocks.len(), budget)?;
    let mut definition_count = 0_usize;
    let mut store = None;
    for block in &body.blocks {
        position(flow, block.id, budget)?;
        let reachable = flow.is_reachable(block.id);
        states.push(BlockState {
            reachable,
            ..BlockState::default()
        });
        if !reachable {
            return refuse(Unsupported::UnreachableBlock { block: block.id });
        }
        if !block.parameters.is_empty() {
            return refuse(Unsupported::BlockArguments { block: block.id });
        }
        for (ordinal, operation) in block.operations.iter().enumerate() {
            budget.charge_work(4)?;
            definition_count = definition_count
                .checked_add(operation.results.len())
                .ok_or(ResourceError::Arithmetic)?;
            let location = FunctionOperationLocation::new(block.id, ordinal);
            match operation.kind {
                // Calls are rejected after input premises establish path reachability.
                OperationKind::Call { .. }
                | OperationKind::Constant(_)
                | OperationKind::Intrinsic(_)
                | OperationKind::Compare { .. }
                | OperationKind::Cast { .. }
                | OperationKind::Select { .. }
                | OperationKind::SliceLength { .. }
                | OperationKind::SliceData { .. }
                | OperationKind::GetElementPointer { .. }
                | OperationKind::Load { .. }
                | OperationKind::GuardedLoad { .. }
                | OperationKind::Binary { .. }
                | OperationKind::Store { .. }
                | OperationKind::GuardedStore { .. } => {}
                _ => return refuse(Unsupported::Operation { location }),
            }
            let candidate = match operation.kind {
                OperationKind::Store {
                    pointer,
                    value,
                    access,
                } => Some(Store {
                    location,
                    pointer,
                    value,
                    access,
                    predicate: None,
                }),
                OperationKind::GuardedStore {
                    pointer,
                    value,
                    access,
                    predicate,
                } => Some(Store {
                    location,
                    pointer,
                    value,
                    access,
                    predicate: Some(predicate),
                }),
                _ => None,
            };
            if let Some(candidate) = candidate {
                if store.is_some() {
                    return refuse(Unsupported::StoreCount { actual: 2 });
                }
                if candidate.access.volatile
                    || candidate.access.address_space != AddressSpace::Global
                {
                    return refuse(Unsupported::StoreAccess { location });
                }
                store = Some(candidate);
            }
        }
    }
    let store = store.ok_or(Failure::Unsupported(Unsupported::StoreCount { actual: 0 }))?;
    let mut definitions = allocate::<Definition<'_>>(definition_count, budget)?;
    for (block_index, block) in body.blocks.iter().enumerate() {
        budget.charge_work(1)?;
        if !states[block_index].reachable {
            continue;
        }
        for (ordinal, operation) in block.operations.iter().enumerate() {
            budget.charge_work(
                operation
                    .results
                    .len()
                    .checked_add(1)
                    .ok_or(ResourceError::Arithmetic)?,
            )?;
            for result in &operation.results {
                definitions.push(Definition {
                    value: result.id,
                    operation,
                    location: FunctionOperationLocation::new(block.id, ordinal),
                });
            }
        }
    }
    verification_bounded_sort_by_v1(&mut definitions, 1, budget, |a, b| a.value.cmp(&b.value))?;
    let pointer = definition(&definitions, store.pointer, budget)?;
    budget.charge_work(8)?;
    let OperationKind::GetElementPointer { base, offset } = pointer.operation.kind else {
        return refuse(Unsupported::Address {
            value: store.pointer,
        });
    };
    let data = definition(&definitions, base, budget)?;
    let OperationKind::SliceData { slice: output } = data.operation.kind else {
        return refuse(Unsupported::Address { value: base });
    };
    let mut output_parameter_index = None;
    for (ordinal, parameter) in body.parameters.iter().enumerate() {
        budget.charge_work(2)?;
        if *parameter == output {
            output_parameter_index = Some(ordinal);
            break;
        }
    }
    let output_parameter_index =
        output_parameter_index.ok_or(Failure::Unsupported(Unsupported::OutputParameter))?;
    let Some(Type::Slice(slice)) = function.signature.parameters.get(output_parameter_index) else {
        return refuse(Unsupported::OutputParameter);
    };
    let Type::Scalar(element) = &*slice.element else {
        return refuse(Unsupported::OutputParameter);
    };
    let Some(bits) = element.bit_width() else {
        return refuse(Unsupported::OutputParameter);
    };
    if bits == 0
        || bits % 8 != 0
        || slice.address_space != AddressSpace::Global
        || !matches!(slice.access, AccessMode::ReadWrite | AccessMode::WriteOnly)
    {
        return refuse(Unsupported::OutputParameter);
    }
    let offset_definition = definition(&definitions, offset, budget)?;
    budget.charge_work(6)?;
    let (index, selected_predicate) = match offset_definition.operation.kind {
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } => {
            let zero = definition(&definitions, false_value, budget)?;
            if !matches!(
                zero.operation.kind,
                OperationKind::Constant(Constant::Index(0))
            ) {
                return refuse(Unsupported::Address { value: offset });
            }
            (true_value, Some(condition))
        }
        _ => (offset, None),
    };
    let index_definition = definition(&definitions, index, budget)?;
    if !matches!(&index_definition.operation.kind, OperationKind::Intrinsic(intrinsic)
        if intrinsic.kind == (IntrinsicKind::InvocationIndex { kind: IndexKind::Global, axis: Axis::X }))
    {
        return refuse(Unsupported::Index { value: index });
    }
    let mut chosen = None;
    if let Some(predicate) = selected_predicate.or(store.predicate) {
        let length = matching_predicate(&definitions, predicate, index, output, budget)?.ok_or(
            Failure::Unsupported(Unsupported::Predicate { value: predicate }),
        )?;
        chosen = Some((predicate, length));
    } else {
        for row in &definitions {
            budget.charge_work(2)?;
            if !matches!(
                row.operation.kind,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    ..
                }
            ) {
                continue;
            }
            if let Some(length) =
                matching_predicate(&definitions, row.value, index, output, budget)?
            {
                if chosen.is_some() {
                    return refuse(Unsupported::MultiplePredicates);
                }
                chosen = Some((row.value, length));
            }
        }
    }
    let (predicate, length) = chosen.ok_or(Failure::Unsupported(Unsupported::MissingPredicate))?;
    if store.predicate.is_some_and(|actual| actual != predicate) {
        return refuse(Unsupported::Predicate {
            value: store.predicate.unwrap(),
        });
    }
    let mut facts = ConditionalTotalViewFactsV1 {
        module,
        kernel_ordinal,
        function_ordinal,
        output_parameter_index: u32::try_from(output_parameter_index)
            .map_err(|_| ResourceError::Arithmetic)?,
        output_value: output,
        index,
        length,
        predicate,
        pointer: store.pointer,
        offset,
        address_domain: ConditionalTotalViewAddressDomainV1::GuardedOutput,
        store_value: store.value,
        store_predicate: store.predicate,
        store_location: store.location,
        element_bytes: u64::from(bits / 8),
        alignment: store.access.alignment,
        reads: 0,
    };
    // Discover domains without using input guards as assumptions. Otherwise an
    // input guard could circularly justify its own readable-span premise.
    check_paths(
        function,
        flow,
        &definitions,
        &mut states,
        &facts,
        pointer.location,
        selected_predicate.is_some(),
        None,
        budget,
    )?;
    let mut reads = allocate::<ConditionalTotalViewReadV1>(definitions.len(), budget)?;
    for (block_index, block) in body.blocks.iter().enumerate() {
        for (ordinal, operation) in block.operations.iter().enumerate() {
            budget.charge_work(2)?;
            if matches!(operation.kind, OperationKind::GetElementPointer { .. })
                && !operation
                    .results
                    .iter()
                    .any(|result| result.id == facts.pointer)
            {
                let [pointer] = operation.results.as_slice() else {
                    return refuse(Unsupported::Operation {
                        location: FunctionOperationLocation::new(block.id, ordinal),
                    });
                };
                let mut used_by_read = false;
                for row in &definitions {
                    budget.charge_work(2)?;
                    used_by_read |= matches!(row.operation.kind,
                        OperationKind::Load { pointer: value, .. }
                        | OperationKind::GuardedLoad { pointer: value, .. } if value == pointer.id);
                }
                if !used_by_read {
                    return refuse(Unsupported::Address { value: pointer.id });
                }
            }
            if matches!(
                operation.kind,
                OperationKind::Load { .. } | OperationKind::GuardedLoad { .. }
            ) {
                let read = derive_read(
                    operation,
                    FunctionOperationLocation::new(block.id, ordinal),
                    &definitions,
                    &facts,
                    flow,
                    &states,
                    block_index,
                    budget,
                )?;
                reads.push(read);
                facts.reads = facts
                    .reads
                    .checked_add(1)
                    .ok_or(ResourceError::Arithmetic)?;
            }
        }
    }
    for state in &mut states {
        budget.charge_work(2)?;
        state.counts = [0, 0];
        state.indegree = 0;
    }
    facts.address_domain = check_paths(
        function,
        flow,
        &definitions,
        &mut states,
        &facts,
        pointer.location,
        selected_predicate.is_some(),
        Some(&reads),
        budget,
    )?;
    for read in reads {
        budget.charge_work(1)?;
        visit_read(read)?;
    }
    // All scratch owners drop before the outer scope restores the storage floor.
    Ok(facts)
}

fn allowed_operation(operation: &Operation) -> bool {
    match &operation.kind {
        OperationKind::Constant(_) | OperationKind::Compare { .. } => true,
        OperationKind::Intrinsic(intrinsic) => {
            intrinsic.kind
                == (IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis: Axis::X,
                })
        }
        OperationKind::Select { .. } => {
            matches!(operation.results.as_slice(), [result] if matches!(result.ty, Type::Scalar(_)))
        }
        OperationKind::Cast {
            kind: CastKind::ZeroExtend,
            to: Type::Scalar(to),
            ..
        } => matches!(
            to,
            ScalarType::I8
                | ScalarType::I16
                | ScalarType::I32
                | ScalarType::I64
                | ScalarType::U8
                | ScalarType::U16
                | ScalarType::U32
                | ScalarType::U64
        ),
        OperationKind::SliceLength { .. } | OperationKind::SliceData { .. } => true,
        OperationKind::GetElementPointer { .. } => {
            matches!(operation.results.as_slice(), [_])
        }
        OperationKind::Load { .. } | OperationKind::GuardedLoad { .. } => true,
        OperationKind::Binary { op, .. } => {
            matches!(
                op,
                BinaryOp::Add
                    | BinaryOp::Subtract
                    | BinaryOp::Multiply
                    | BinaryOp::BitAnd
                    | BinaryOp::BitOr
                    | BinaryOp::BitXor
            ) && matches!(operation.results.as_slice(), [result] if
                matches!(result.ty, Type::Scalar(scalar) if scalar != ScalarType::Index))
        }
        OperationKind::Store { .. } | OperationKind::GuardedStore { .. } => true,
        // Even pure calls need completion evidence; arithmetic needs totality/no-wrap facts.
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn derive_read(
    operation: &Operation,
    location: FunctionOperationLocation,
    definitions: &[Definition<'_>],
    facts: &ConditionalTotalViewFactsV1<'_>,
    flow: &IndexedControlFlow,
    states: &[BlockState],
    block: usize,
    budget: &mut Budget<'_>,
) -> Derived<ConditionalTotalViewReadV1> {
    budget.charge_work(16)?;
    let (pointer, access, guarded) = match operation.kind {
        OperationKind::Load { pointer, access } => (pointer, access, false),
        OperationKind::GuardedLoad {
            pointer,
            access,
            predicate,
            ..
        } if predicate == facts.predicate => (pointer, access, true),
        _ => return refuse(Unsupported::Operation { location }),
    };
    if access.volatile || access.address_space != AddressSpace::Global {
        return refuse(Unsupported::Operation { location });
    }
    let address = definition(definitions, pointer, budget)?;
    let OperationKind::GetElementPointer { base, offset } = address.operation.kind else {
        return refuse(Unsupported::Address { value: pointer });
    };
    if offset != facts.index {
        return refuse(Unsupported::Index { value: offset });
    }
    let data = definition(definitions, base, budget)?;
    let OperationKind::SliceData { slice } = data.operation.kind else {
        return refuse(Unsupported::Address { value: base });
    };
    let body = facts
        .function()
        .body
        .as_ref()
        .ok_or(ResourceError::Accounting)?;
    let mut parameter = None;
    for (ordinal, value) in body.parameters.iter().enumerate() {
        budget.charge_work(2)?;
        if *value == slice {
            parameter = Some(ordinal);
        }
    }
    let parameter = parameter.ok_or(Failure::Unsupported(Unsupported::Signature))?;
    let Some(Type::Slice(ty)) = facts.function().signature.parameters.get(parameter) else {
        return refuse(Unsupported::Signature);
    };
    let [result] = operation.results.as_slice() else {
        return refuse(Unsupported::Operation { location });
    };
    let Type::Scalar(scalar) = result.ty else {
        return refuse(Unsupported::Operation { location });
    };
    let bits = scalar
        .bit_width()
        .ok_or(Failure::Unsupported(Unsupported::Signature))?;
    if ty.address_space != AddressSpace::Global
        || ty.access != AccessMode::ReadOnly
        || *ty.element != result.ty
        || bits == 0
        || bits % 8 != 0
    {
        return refuse(Unsupported::Operation { location });
    }
    let address_block = position(flow, address.location.block, budget)?;
    Ok(ConditionalTotalViewReadV1 {
        parameter: u32::try_from(parameter).map_err(|_| ResourceError::Arithmetic)?,
        slice,
        pointer,
        index: offset,
        value: result.id,
        location,
        access_domain: if !guarded && states[block].counts[0] != 0 {
            ConditionalTotalViewAddressDomainV1::GlobalLaunch
        } else {
            ConditionalTotalViewAddressDomainV1::GuardedOutput
        },
        address_domain: if states[address_block].counts[0] != 0 {
            ConditionalTotalViewAddressDomainV1::GlobalLaunch
        } else {
            ConditionalTotalViewAddressDomainV1::GuardedOutput
        },
        element_bytes: u64::from(bits / 8),
        alignment: access.alignment,
    })
}

/// Derives a conditional, single-output identity-write theorem without a launch sample.
///
/// The caller keeps its Module-owner reservation. All work uses the supplied
/// ledger. The existing CFG retains its logical-cell accounting; local scratch
/// headers and actual Vec capacities are charged in bytes. Scratch is dropped
/// and storage restored on success, refusal, resource failure and unwinding.
/// The returned fixed-size facts borrow the Module and retain no heap payload.
pub fn derive_conditional_total_view_from_verified_v1<'module>(
    verified: VerifiedKernelIrModuleV1<'module>,
    kernel: &KernelId,
    budget: &mut Budget<'_>,
) -> Result<ConditionalTotalViewAnalysisV1<'module>, ConditionalTotalViewErrorV1> {
    let floor = budget.storage_checkpoint();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        budget.charge_work(4)?;
        budget.reserve_storage(
            size_of::<ConditionalTotalViewFactsV1<'_>>() + 4 * size_of::<Vec<()>>(),
        )?;
        derive(verified.module(), kernel, budget, &mut |_| Ok(()))
    }));
    budget.rollback_storage(floor)?;
    match outcome {
        Ok(Ok(facts)) => Ok(ConditionalTotalViewAnalysisV1::Established(facts)),
        Ok(Err(Failure::Unsupported(reason))) => {
            Ok(ConditionalTotalViewAnalysisV1::Unsupported(reason))
        }
        Ok(Err(Failure::Error(error))) => Err(error),
        Err(payload) => resume_unwind(payload),
    }
}

#[cfg(test)]
#[path = "conditional_total_view_v1_tests.rs"]
mod tests;
