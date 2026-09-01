use std::collections::{BTreeSet, HashMap};
use std::hash::Hash;
use std::mem::{align_of, size_of};

use fe2o3_kernel_ir::{
    AddressSpace, AssemblyEffect, AssemblyOption, BarrierSemantics, BasicBlock, Function,
    FunctionBody, InlineAssembly, IntegerSwitchCase, Kernel, MatrixFrontendBindingV2,
    MatrixOperation, Module, Operation, OperationKind, Signature, SwitchCase, TargetCapability,
    Terminator, Type, ValueDef,
};

/// Checked accounting for retained allocation payloads.
///
/// The contract counts capacities reported by standard containers. Allocator
/// bookkeeping and page rounding are outside `max_resident_bytes`; no stable
/// Rust API exposes them. B-tree storage uses a deliberately conservative
/// upper bound for the pinned standard-library implementation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ResidentLedger {
    bytes: usize,
}

impl ResidentLedger {
    pub(crate) const fn new(bytes: usize) -> Self {
        Self { bytes }
    }

    pub(crate) const fn bytes(self) -> usize {
        self.bytes
    }

    pub(crate) fn add_bytes(&mut self, bytes: usize) -> Option<()> {
        self.bytes = self.bytes.checked_add(bytes)?;
        Some(())
    }

    pub(crate) fn add_product(&mut self, count: usize, width: usize) -> Option<()> {
        self.add_bytes(count.checked_mul(width)?)
    }

    pub(crate) fn add_vec<T>(&mut self, capacity: usize) -> Option<()> {
        self.add_product(capacity, size_of::<T>())
    }

    pub(crate) fn add_box<T>(&mut self) -> Option<()> {
        self.add_bytes(size_of::<T>())
    }

    pub(crate) fn add_btree_set<T>(&mut self, entries: usize) -> Option<()> {
        if entries == 0 {
            return Some(());
        }
        // Rust's pinned B-tree node uses 11 key slots and 12 edge slots. Assume
        // one complete maximum-sized node allocation per retained entry, plus
        // ample header, parent, length, and alignment storage. This intentionally
        // overcounts sparse/root nodes and avoids relying on minimum occupancy.
        const KEY_SLOTS: usize = 11;
        const POINTER_AND_HEADER_WORDS: usize = 20;
        let one_node = size_of::<T>()
            .checked_mul(KEY_SLOTS)?
            .checked_add(size_of::<usize>().checked_mul(POINTER_AND_HEADER_WORDS)?)?
            .checked_add(align_of::<T>())?;
        self.add_product(entries, one_node)
    }
}

pub(crate) fn reserved_vec_bytes<T>(elements: usize) -> Option<usize> {
    let mut values = Vec::<T>::new();
    values.try_reserve_exact(elements).ok()?;
    values.capacity().checked_mul(size_of::<T>())
}

pub(crate) fn bool_vec_storage_bytes(capacity_bits: usize) -> Option<usize> {
    capacity_bits.checked_add(7)?.checked_div(8)
}

pub(crate) fn partitioned_bool_vec_storage_bytes(
    total_bits: usize,
    partitions: usize,
) -> Option<usize> {
    let nonempty = total_bits.min(partitions);
    bool_vec_storage_bytes(total_bits)?.checked_add(nonempty.checked_mul(size_of::<usize>())?)
}

pub(crate) fn reserved_bool_vec_bytes(elements: usize) -> Option<usize> {
    let mut values = Vec::<bool>::new();
    values.try_reserve_exact(elements).ok()?;
    bool_vec_storage_bytes(values.capacity())
}

/// Upper-bounds retained capacity for one lazily grown standard `Vec` on the
/// pinned toolchain (minimum non-zero capacity four, then geometric growth).
pub(crate) fn geometric_vec_bytes<T>(maximum_len: usize) -> Option<usize> {
    if maximum_len == 0 {
        return Some(0);
    }
    maximum_len
        .checked_next_power_of_two()?
        .max(4)
        .checked_mul(size_of::<T>())
}

/// Upper-bounds the summed capacities of lazily grown vectors sharing one
/// total element budget. At most `min(total, partitions)` vectors are nonempty;
/// the pinned minimum-four/geometric policy is bounded by two elements of
/// slack per nonempty vector plus twice the total retained elements.
pub(crate) fn partitioned_geometric_vec_bytes<T>(
    total_elements: usize,
    partitions: usize,
) -> Option<usize> {
    let nonempty = total_elements.min(partitions);
    total_elements
        .checked_mul(2)?
        .checked_add(nonempty.checked_mul(2)?)?
        .checked_mul(size_of::<T>())
}

pub(crate) fn reserved_hash_map_bytes<K: Eq + Hash, V>(entries: usize) -> Option<usize> {
    let mut values = HashMap::<K, V>::new();
    values.try_reserve(entries).ok()?;
    hash_map_capacity_bytes::<K, V>(values.capacity())
}

pub(crate) fn conservative_hash_map_bytes_for_entries<K, V>(entries: usize) -> Option<usize> {
    if entries == 0 {
        return Some(0);
    }
    let buckets = entries
        .checked_mul(8)?
        .checked_add(6)?
        .checked_div(7)?
        .checked_next_power_of_two()?
        .max(4);
    hash_map_bucket_bytes::<K, V>(buckets)
}

pub(crate) fn hash_map_capacity_bytes<K, V>(capacity: usize) -> Option<usize> {
    if capacity == 0 {
        return Some(0);
    }
    // HashMap::capacity is the effective 7/8-load capacity. Reconstruct a
    // conservative bucket count from that actual post-reservation capacity.
    let required = capacity.checked_mul(8)?.checked_add(6)?.checked_div(7)?;
    let buckets = required.checked_next_power_of_two()?.max(4);
    hash_map_bucket_bytes::<K, V>(buckets)
}

fn hash_map_bucket_bytes<K, V>(buckets: usize) -> Option<usize> {
    buckets
        .checked_mul(size_of::<(K, V)>())?
        .checked_add(buckets.checked_add(16)?)?
        .checked_add(align_of::<(K, V)>())
}

/// Returns heap bytes retained by a decoded module, excluding the inline
/// `Module` value itself.
pub(crate) fn module_retained_heap_bytes(module: &Module) -> Option<usize> {
    let mut resident = ResidentLedger::new(0);
    resident.add_bytes(module.id.retained_capacity_bytes())?;
    resident.add_vec::<Function>(module.functions.capacity())?;
    for function in &module.functions {
        add_function(&mut resident, function)?;
    }
    resident.add_vec::<Kernel>(module.kernels.capacity())?;
    for kernel in &module.kernels {
        add_kernel(&mut resident, kernel)?;
    }
    add_capabilities(&mut resident, &module.required_capabilities)?;
    Some(resident.bytes())
}

pub(crate) fn type_retained_heap_bytes(ty: &Type) -> Option<usize> {
    let mut resident = ResidentLedger::new(0);
    add_type_boxes(&mut resident, ty)?;
    Some(resident.bytes())
}

fn add_function(resident: &mut ResidentLedger, function: &Function) -> Option<()> {
    resident.add_bytes(function.id.retained_capacity_bytes())?;
    add_signature(resident, &function.signature)?;
    if let Some(body) = &function.body {
        add_function_body(resident, body)?;
    }
    add_capabilities(resident, &function.required_capabilities)
}

fn add_signature(resident: &mut ResidentLedger, signature: &Signature) -> Option<()> {
    resident.add_vec::<Type>(signature.parameters.capacity())?;
    for ty in &signature.parameters {
        add_type_boxes(resident, ty)?;
    }
    resident.add_vec::<Type>(signature.results.capacity())?;
    for ty in &signature.results {
        add_type_boxes(resident, ty)?;
    }
    Some(())
}

fn add_function_body(resident: &mut ResidentLedger, body: &FunctionBody) -> Option<()> {
    resident.add_vec::<fe2o3_kernel_ir::ValueId>(body.parameters.capacity())?;
    resident.add_vec::<BasicBlock>(body.blocks.capacity())?;
    for block in &body.blocks {
        add_block(resident, block)?;
    }
    Some(())
}

fn add_kernel(resident: &mut ResidentLedger, kernel: &Kernel) -> Option<()> {
    resident.add_bytes(kernel.id.retained_capacity_bytes())?;
    resident.add_bytes(kernel.entry.retained_capacity_bytes())?;
    add_capabilities(resident, &kernel.required_capabilities)
}

fn add_block(resident: &mut ResidentLedger, block: &BasicBlock) -> Option<()> {
    resident.add_vec::<ValueDef>(block.parameters.capacity())?;
    for parameter in &block.parameters {
        add_value_def(resident, parameter)?;
    }
    resident.add_vec::<Operation>(block.operations.capacity())?;
    for operation in &block.operations {
        add_operation(resident, operation)?;
    }
    if let Some(terminator) = &block.terminator {
        add_terminator(resident, terminator)?;
    }
    Some(())
}

fn add_value_def(resident: &mut ResidentLedger, value: &ValueDef) -> Option<()> {
    add_type_boxes(resident, &value.ty)
}

fn add_operation(resident: &mut ResidentLedger, operation: &Operation) -> Option<()> {
    resident.add_vec::<ValueDef>(operation.results.capacity())?;
    for result in &operation.results {
        add_value_def(resident, result)?;
    }
    match &operation.kind {
        OperationKind::Intrinsic(intrinsic) => add_type_boxes(resident, &intrinsic.result_type),
        OperationKind::Cast { to, .. } => add_type_boxes(resident, to),
        OperationKind::Call { callee, arguments } => {
            resident.add_bytes(callee.retained_capacity_bytes())?;
            resident.add_vec::<fe2o3_kernel_ir::ValueId>(arguments.capacity())
        }
        OperationKind::Alloca { element, .. } => add_type_boxes(resident, element),
        OperationKind::Barrier(barrier) => add_barrier_semantics(resident, &barrier.semantics),
        OperationKind::Fence(fence) => add_barrier_semantics(resident, &fence.semantics),
        OperationKind::WorkgroupBarrier(barrier) => {
            add_barrier_semantics(resident, &barrier.semantics)
        }
        OperationKind::WorkgroupMemory(memory) => add_type_boxes(resident, &memory.element),
        OperationKind::Matrix(matrix) => add_matrix(resident, matrix),
        OperationKind::InlineAssembly(assembly) => add_inline_assembly(resident, assembly),
        OperationKind::Constant(_)
        | OperationKind::MemoryIntrinsic(_)
        | OperationKind::Unary { .. }
        | OperationKind::Binary { .. }
        | OperationKind::Compare { .. }
        | OperationKind::Select { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::SliceData { .. }
        | OperationKind::GetElementPointer { .. }
        | OperationKind::Load { .. }
        | OperationKind::GuardedLoad { .. }
        | OperationKind::Store { .. }
        | OperationKind::GuardedStore { .. }
        | OperationKind::Atomic(_)
        | OperationKind::TargetExtension(_)
        | OperationKind::Wave(_) => Some(()),
    }
}

fn add_type_boxes(resident: &mut ResidentLedger, ty: &Type) -> Option<()> {
    let mut current = ty;
    loop {
        current = match current {
            Type::Pointer(pointer) => {
                resident.add_box::<Type>()?;
                &pointer.pointee
            }
            Type::Slice(slice) => {
                resident.add_box::<Type>()?;
                &slice.element
            }
            Type::Unit | Type::Scalar(_) => return Some(()),
        };
    }
}

fn add_barrier_semantics(
    resident: &mut ResidentLedger,
    semantics: &BarrierSemantics,
) -> Option<()> {
    add_plain_btree_set::<AddressSpace>(resident, &semantics.address_spaces)
}

fn add_inline_assembly(resident: &mut ResidentLedger, assembly: &InlineAssembly) -> Option<()> {
    resident.add_bytes(assembly.mnemonic.capacity())?;
    resident.add_vec::<fe2o3_kernel_ir::AssemblyOperand>(assembly.operands.capacity())?;
    add_plain_btree_set::<AssemblyOption>(resident, &assembly.options)?;
    add_plain_btree_set::<AssemblyEffect>(resident, &assembly.declared_effects)
}

fn add_matrix(resident: &mut ResidentLedger, matrix: &MatrixOperation) -> Option<()> {
    let Some(binding) = &matrix.frontend_binding else {
        return Some(());
    };
    add_matrix_binding(resident, binding)
}

fn add_matrix_binding(
    resident: &mut ResidentLedger,
    binding: &MatrixFrontendBindingV2,
) -> Option<()> {
    let observation = &binding.observed_source;
    resident.add_bytes(observation.provider.crate_name.capacity())?;
    resident.add_vec::<[u8; 16]>(observation.provider.definition_identities.capacity())?;
    resident.add_vec::<u8>(observation.canonical_record.capacity())
}

fn add_terminator(resident: &mut ResidentLedger, terminator: &Terminator) -> Option<()> {
    match terminator {
        Terminator::Branch { arguments, .. } => {
            resident.add_vec::<fe2o3_kernel_ir::ValueId>(arguments.capacity())
        }
        Terminator::ConditionalBranch {
            then_arguments,
            else_arguments,
            ..
        } => {
            resident.add_vec::<fe2o3_kernel_ir::ValueId>(then_arguments.capacity())?;
            resident.add_vec::<fe2o3_kernel_ir::ValueId>(else_arguments.capacity())
        }
        Terminator::Switch {
            cases,
            default_arguments,
            ..
        } => {
            resident.add_vec::<SwitchCase>(cases.capacity())?;
            for case in cases {
                resident.add_vec::<fe2o3_kernel_ir::ValueId>(case.arguments.capacity())?;
            }
            resident.add_vec::<fe2o3_kernel_ir::ValueId>(default_arguments.capacity())
        }
        Terminator::IntegerSwitch {
            cases,
            default_arguments,
            ..
        } => {
            resident.add_vec::<IntegerSwitchCase>(cases.capacity())?;
            for case in cases {
                resident.add_vec::<fe2o3_kernel_ir::ValueId>(case.arguments.capacity())?;
            }
            resident.add_vec::<fe2o3_kernel_ir::ValueId>(default_arguments.capacity())
        }
        Terminator::Return { values } => {
            resident.add_vec::<fe2o3_kernel_ir::ValueId>(values.capacity())
        }
        Terminator::Unreachable => Some(()),
    }
}

fn add_capabilities(
    resident: &mut ResidentLedger,
    capabilities: &BTreeSet<TargetCapability>,
) -> Option<()> {
    resident.add_btree_set::<TargetCapability>(capabilities.len())?;
    for capability in capabilities {
        if let TargetCapability::Extension { namespace, name } = capability {
            resident.add_bytes(namespace.capacity())?;
            resident.add_bytes(name.capacity())?;
        }
    }
    Some(())
}

fn add_plain_btree_set<T: Ord>(resident: &mut ResidentLedger, values: &BTreeSet<T>) -> Option<()> {
    resident.add_btree_set::<T>(values.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        AccessMode, BasicBlock, BlockId, Function, FunctionId, Gfx950LdsTransposeFormatV1,
        Gfx950LdsTransposeOperationKindV1, Gfx950LdsTransposeOperationV1, Kernel, LaunchDomain,
        LaunchExtent, Module, Operation, Signature, Terminator, Type, ValueId,
    };

    fn spare_string(value: &str, capacity: usize) -> String {
        let mut string = String::with_capacity(capacity);
        string.push_str(value);
        string
    }

    #[test]
    fn module_accounting_includes_spare_ids_nested_types_and_unreachable_storage() {
        let mut module = Module::new(spare_string("module", 8_192));
        let nested = Type::pointer(
            Type::slice(
                Type::pointer(
                    Type::Scalar(fe2o3_kernel_ir::ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        );
        let mut block = BasicBlock::new(BlockId(0));
        let mut arguments = Vec::with_capacity(1_024);
        arguments.push(ValueId(0));
        block.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Call {
                callee: FunctionId::new(spare_string("callee", 4_096)),
                arguments,
            },
        ));
        let mut cases = Vec::with_capacity(512);
        let mut case_arguments = Vec::with_capacity(256);
        case_arguments.push(ValueId(0));
        cases.push(SwitchCase {
            value: 0,
            target: BlockId(0),
            arguments: case_arguments,
        });
        block.terminator = Some(Terminator::Switch {
            selector: ValueId(0),
            cases,
            default_target: BlockId(0),
            default_arguments: Vec::with_capacity(128),
        });
        module.functions.push(Function::internal_helper(
            spare_string("unreachable", 16_384),
            Signature::new(vec![nested], Vec::new()),
            vec![ValueId(0)],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "unused",
            "unreachable",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));

        let retained = module_retained_heap_bytes(&module).unwrap();
        assert!(retained >= 8_192 + 4_096 + 16_384);
        assert!(retained >= 1_024 * size_of::<ValueId>());
        assert!(retained >= 512 * size_of::<SwitchCase>());
        assert!(retained >= 3 * size_of::<Type>());
    }

    #[test]
    fn module_accounting_rejects_checked_overflow() {
        let mut resident = ResidentLedger::new(usize::MAX);
        assert_eq!(resident.add_bytes(1), None);
        assert_eq!(resident.add_product(usize::MAX, 2), None);
    }

    #[test]
    fn module_accounting_includes_empty_container_spare_capacity() {
        let mut module = Module::new("empty-capacity");
        module.functions = Vec::with_capacity(2_048);
        module.kernels = Vec::with_capacity(1_024);

        let retained = module_retained_heap_bytes(&module).unwrap();
        assert!(retained >= 2_048 * size_of::<Function>());
        assert!(retained >= 1_024 * size_of::<Kernel>());
    }

    #[test]
    fn boolean_vector_capacity_is_charged_as_packed_bits() {
        let bytes = reserved_bool_vec_bytes(1_025).unwrap();
        assert!(bytes >= 1_025_usize.div_ceil(8));
        assert!(bytes < 1_025);
        let partitioned = partitioned_bool_vec_storage_bytes(17, 17).unwrap();
        let mut actual = 0usize;
        for _ in 0..17 {
            actual += reserved_bool_vec_bytes(1).unwrap();
        }
        assert!(partitioned >= actual);
    }

    #[test]
    fn gfx950_lds_transpose_kinds_retain_no_operation_owned_heap() {
        let format = Gfx950LdsTransposeFormatV1::Fp8E4M3;
        let kinds = [
            Gfx950LdsTransposeOperationKindV1::Current { format },
            Gfx950LdsTransposeOperationKindV1::Stage {
                format,
                storage: ValueId(0),
                source_slice: ValueId(1),
                offset: ValueId(2),
                rows: ValueId(3),
                columns: ValueId(4),
                stride: ValueId(5),
                token_base: ValueId(6),
                reduction_base: ValueId(7),
            },
            Gfx950LdsTransposeOperationKindV1::Publish {
                format,
                storage: ValueId(0),
            },
            Gfx950LdsTransposeOperationKindV1::Read {
                format,
                storage: ValueId(0),
            },
        ];

        for kind in kinds {
            let operation = Operation::new(
                Vec::new(),
                OperationKind::TargetExtension(
                    fe2o3_kernel_ir::TargetExtensionOperation::amdgcn_gfx950_lds_transpose(
                        Gfx950LdsTransposeOperationV1::full(kind),
                    ),
                ),
            );
            let mut resident = ResidentLedger::new(0);
            add_operation(&mut resident, &operation).unwrap();
            assert_eq!(resident.bytes(), 0, "{kind:?}");
        }
    }
}
