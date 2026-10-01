//! Existing-owner heap walk for the ownership grammar reachable from V11.
//! This is not canonical encoding, semantic verification, or physical memory
//! accounting. Post-V11 owner families fail closed instead of disappearing.

use crate::*;
use std::{collections::BTreeSet, mem::size_of};

type Counter = LogicalStorageCounterV1;
type Result = std::result::Result<(), LogicalStorageErrorV1>;

// Compile-time guard for payloads inspected as fixed, non-owning values.
fn fixed<T: Copy>(_: &T) {}

impl Module {
    /// Adds every retained heap owner reachable in the V11 ownership grammar.
    /// Excludes the Module header; an embedding owner charges its header once.
    /// Walks only this instance (including spare capacities), allocates nothing,
    /// and never decodes/re-encodes or computes derived capability sets.
    /// An error means the ledger is incomplete and must not be reported as a
    /// complete extent. Newer ownership families are explicitly unsupported.
    pub fn charge_retained_heap_v11(
        &self,
        counter: &mut LogicalStorageCounterV1,
    ) -> std::result::Result<(), LogicalStorageErrorV1> {
        let Self {
            storage_layouts,
            id,
            functions,
            kernels,
            required_capabilities,
        } = self;
        counter.charge(0, 1)?;
        counter.vector(storage_layouts)?;
        if !storage_layouts.is_empty() {
            return Err(LogicalStorageErrorV1::UnsupportedV11Owner);
        }
        counter.charge(id.retained_capacity_bytes(), 1)?;
        counter.vector(functions)?;
        for function in functions {
            function_heap(function, counter)?;
        }
        counter.vector(kernels)?;
        for kernel in kernels {
            let Kernel {
                id,
                entry,
                domain,
                workgroup_size,
                required_capabilities,
            } = kernel;
            counter.charge(0, 1)?;
            counter.charge(id.retained_capacity_bytes(), 1)?;
            counter.charge(entry.retained_capacity_bytes(), 1)?;
            match domain {
                LaunchDomain::D1 { x } => fixed(x),
                LaunchDomain::D2 { x, y } => {
                    fixed(x);
                    fixed(y);
                }
                LaunchDomain::D3 { x, y, z } => {
                    fixed(x);
                    fixed(y);
                    fixed(z);
                }
            }
            fixed(workgroup_size);
            capabilities(required_capabilities, counter)?;
        }
        capabilities(required_capabilities, counter)
    }
}

fn capabilities(values: &BTreeSet<TargetCapability>, c: &mut Counter) -> Result {
    c.set(values)?;
    for value in values {
        c.charge(0, 1)?;
        match value {
            TargetCapability::Extension { namespace, name } => {
                c.string(namespace)?;
                c.string(name)?;
            }
            TargetCapability::Float16
            | TargetCapability::BFloat16
            | TargetCapability::Float64
            | TargetCapability::Int64
            | TargetCapability::Subgroups
            | TargetCapability::WorkgroupMemory
            | TargetCapability::WorkgroupBarrier
            | TargetCapability::DynamicWorkgroupMemory => {}
            TargetCapability::SubgroupSize(size) => fixed(size),
            TargetCapability::WaveWidth(width) => fixed(width),
            TargetCapability::Atomic {
                width_bits,
                address_space,
                max_scope,
            } => {
                fixed(width_bits);
                fixed(address_space);
                fixed(max_scope);
            }
        }
    }
    Ok(())
}

fn type_heap(mut ty: &Type, c: &mut Counter) -> Result {
    loop {
        c.charge(0, 1)?;
        match ty {
            Type::Unit => return Ok(()),
            Type::Scalar(scalar) => {
                fixed(scalar);
                return Ok(());
            }
            Type::Pointer(PointerType {
                pointee,
                address_space,
                access,
            }) => {
                fixed(address_space);
                fixed(access);
                c.charge(size_of::<Type>(), 0)?;
                ty = pointee;
            }
            Type::Slice(SliceType {
                element,
                address_space,
                access,
            }) => {
                fixed(address_space);
                fixed(access);
                c.charge(size_of::<Type>(), 0)?;
                ty = element;
            }
            Type::Vector(_) | Type::Execution(_) | Type::StorageObject(_) => {
                return Err(LogicalStorageErrorV1::UnsupportedV11Owner);
            }
        }
    }
}

fn types(values: &Vec<Type>, c: &mut Counter) -> Result {
    c.vector(values)?;
    for value in values {
        type_heap(value, c)?;
    }
    Ok(())
}
fn definitions(values: &Vec<ValueDef>, c: &mut Counter) -> Result {
    c.vector(values)?;
    for ValueDef { id, ty } in values {
        c.charge(0, 1)?;
        fixed(id);
        type_heap(ty, c)?;
    }
    Ok(())
}
fn function_heap(function: &Function, c: &mut Counter) -> Result {
    let Function {
        id,
        signature,
        role,
        body,
        required_capabilities,
    } = function;
    let Signature {
        parameters,
        results,
    } = signature;
    c.charge(0, 1)?;
    c.charge(id.retained_capacity_bytes(), 1)?;
    fixed(role);
    types(parameters, c)?;
    types(results, c)?;
    capabilities(required_capabilities, c)?;
    if let Some(FunctionBody { parameters, blocks }) = body {
        c.charge(0, 1)?;
        c.vector(parameters)?;
        c.vector(blocks)?;
        for BasicBlock {
            id,
            parameters,
            operations,
            terminator,
        } in blocks
        {
            c.charge(0, 1)?;
            fixed(id);
            definitions(parameters, c)?;
            c.vector(operations)?;
            for Operation { results, kind } in operations {
                c.charge(0, 1)?;
                definitions(results, c)?;
                operation_heap(kind, c)?;
            }
            if let Some(terminator) = terminator {
                terminator_heap(terminator, c)?;
            }
        }
    }
    Ok(())
}

fn barrier_heap(semantics: &BarrierSemantics, c: &mut Counter) -> Result {
    let BarrierSemantics {
        ordering,
        address_spaces,
    } = semantics;
    fixed(ordering);
    c.set(address_spaces)
}
fn constant(value: &Constant) {
    match value {
        Constant::Bool(v) => fixed(v),
        Constant::I8(v) => fixed(v),
        Constant::I16(v) => fixed(v),
        Constant::I32(v) => fixed(v),
        Constant::I64(v) => fixed(v),
        Constant::U8(v) => fixed(v),
        Constant::U16(v) => fixed(v),
        Constant::U32(v) => fixed(v),
        Constant::U64(v) | Constant::Index(v) => fixed(v),
        Constant::F16Bits(v) | Constant::Bf16Bits(v) => fixed(v),
        Constant::F32Bits(v) => fixed(v),
        Constant::F64Bits(v) => fixed(v),
    }
}
fn operation_heap(kind: &OperationKind, c: &mut Counter) -> Result {
    c.charge(0, 1)?;
    match kind {
        OperationKind::Constant(value) => constant(value),
        OperationKind::Intrinsic(IntrinsicOperation { kind, result_type }) => {
            fixed(kind);
            type_heap(result_type, c)?;
        }
        OperationKind::MemoryIntrinsic(value) => fixed(value),
        OperationKind::Unary { op, operand } => {
            fixed(op);
            fixed(operand);
        }
        OperationKind::Binary { op, lhs, rhs } => {
            fixed(op);
            fixed(lhs);
            fixed(rhs);
        }
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } => {
            fixed(predicate);
            fixed(lhs);
            fixed(rhs);
        }
        OperationKind::Cast { kind, value, to } => {
            fixed(kind);
            fixed(value);
            type_heap(to, c)?;
        }
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } => {
            fixed(condition);
            fixed(true_value);
            fixed(false_value);
        }
        OperationKind::Call { callee, arguments } => {
            c.charge(callee.retained_capacity_bytes(), 1)?;
            c.vector(arguments)?;
        }
        OperationKind::Alloca {
            element,
            count,
            address_space,
            alignment,
        } => {
            fixed(count);
            fixed(address_space);
            fixed(alignment);
            type_heap(element, c)?;
        }
        OperationKind::SliceLength { slice } | OperationKind::SliceData { slice } => fixed(slice),
        OperationKind::GetElementPointer { base, offset } => {
            fixed(base);
            fixed(offset);
        }
        OperationKind::Load { pointer, access } => {
            fixed(pointer);
            fixed(access);
        }
        OperationKind::GuardedLoad {
            pointer,
            predicate,
            fallback,
            access,
        } => {
            fixed(pointer);
            fixed(predicate);
            fixed(fallback);
            fixed(access);
        }
        OperationKind::Store {
            pointer,
            value,
            access,
        } => {
            fixed(pointer);
            fixed(value);
            fixed(access);
        }
        OperationKind::GuardedStore {
            pointer,
            predicate,
            value,
            access,
        } => {
            fixed(pointer);
            fixed(predicate);
            fixed(value);
            fixed(access);
        }
        OperationKind::Barrier(Barrier {
            execution_scope,
            memory_scope,
            semantics,
        }) => {
            fixed(execution_scope);
            fixed(memory_scope);
            barrier_heap(semantics, c)?;
        }
        OperationKind::Fence(Fence {
            memory_scope,
            semantics,
        }) => {
            fixed(memory_scope);
            barrier_heap(semantics, c)?;
        }
        OperationKind::WorkgroupBarrier(WorkgroupBarrier {
            memory_scope,
            semantics,
            convergence,
        }) => {
            fixed(memory_scope);
            fixed(convergence);
            barrier_heap(semantics, c)?;
        }
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element,
            extent,
            alignment,
        }) => {
            fixed(extent);
            fixed(alignment);
            type_heap(element, c)?;
        }
        OperationKind::Atomic(Atomic {
            kind,
            pointer,
            value,
            compare,
            access,
            scope,
            ordering,
            failure_ordering,
        }) => {
            fixed(kind);
            fixed(pointer);
            fixed(value);
            fixed(compare);
            fixed(access);
            fixed(scope);
            fixed(ordering);
            fixed(failure_ordering);
        }
        OperationKind::Wave(WaveOperation {
            kind,
            width,
            active_lanes,
            convergence,
        }) => {
            fixed(kind);
            fixed(width);
            fixed(active_lanes);
            fixed(convergence);
        }
        OperationKind::Gfx950LdsTranspose(value) => fixed(value),
        OperationKind::InlineAssembly(InlineAssembly {
            target,
            source,
            mnemonic,
            operands,
            options,
            declared_effects,
        }) => {
            fixed(target);
            fixed(source);
            c.string(mnemonic)?;
            c.vector(operands)?;
            for AssemblyOperand { kind, constraint } in operands {
                c.charge(0, 1)?;
                fixed(constraint);
                match kind {
                    AssemblyOperandKind::Input(value) => fixed(value),
                    AssemblyOperandKind::Output { result_index } => fixed(result_index),
                    AssemblyOperandKind::InOut {
                        input,
                        result_index,
                    } => {
                        fixed(input);
                        fixed(result_index);
                    }
                    AssemblyOperandKind::ImmediateI32(value) => fixed(value),
                }
            }
            // These two BTree sets retain only fixed-size enum values.
            c.set(options)?;
            c.set(declared_effects)?;
        }
        OperationKind::Matrix(value) => matrix_heap(value, c)?,
        OperationKind::Storage(_)
        | OperationKind::Execution(_)
        | OperationKind::VerificationContract(_)
        | OperationKind::VectorLoad(_)
        | OperationKind::VectorStore(_)
        | OperationKind::VectorLayoutConvert(_)
        | OperationKind::Gfx942OrderedRegion(_)
        | OperationKind::Gfx942OrderedProgram(_)
        | OperationKind::Gfx942CompleteBodyDeclaration(_)
        | OperationKind::Gfx942CompleteBodyStep(_)
        | OperationKind::Gfx942PhysicalEntryDeclaration(_)
        | OperationKind::Gfx942PhysicalEntryStep(_)
        | OperationKind::Gfx942PhysicalGlobalCopyDeclaration(_)
        | OperationKind::Gfx942PhysicalGlobalCopyStep(_)
        | OperationKind::Gfx942PhysicalLdsExchangeDeclaration(_)
        | OperationKind::Gfx942PhysicalLdsExchangeStep(_) => {
            return Err(LogicalStorageErrorV1::UnsupportedV11Owner);
        }
    }
    Ok(())
}

fn matrix_heap(value: &MatrixOperation, c: &mut Counter) -> Result {
    let MatrixOperation {
        kind,
        active_lanes,
        convergence,
        frontend_binding,
        tensor_layout,
    } = value;
    fixed(active_lanes);
    fixed(convergence);
    fixed(tensor_layout);
    match kind {
        MatrixOperationKind::MultiplyAccumulate {
            lhs,
            rhs,
            accumulator,
            profile,
        } => {
            fixed(lhs);
            fixed(rhs);
            fixed(accumulator);
            fixed(profile);
        }
        MatrixOperationKind::ScaledMultiplyAccumulate {
            lhs,
            rhs,
            accumulator,
            profile,
        } => {
            fixed(lhs);
            fixed(rhs);
            fixed(accumulator);
            fixed(profile);
        }
        MatrixOperationKind::LdsLoad { base, profile } => {
            fixed(base);
            fixed(profile);
        }
        MatrixOperationKind::LdsStore {
            base,
            values,
            profile,
        } => {
            fixed(base);
            fixed(values);
            fixed(profile);
        }
    }
    if let Some(MatrixFrontendBindingV2 {
        observed_source,
        projected_kernarg,
    }) = frontend_binding
    {
        let MatrixSourceAbiObservationV2 {
            provider,
            canonical_record,
            digest,
        } = observed_source;
        let MatrixProviderIdentityV2 {
            crate_name,
            stable_crate_id,
            crate_hash,
            cargo_metadata_build_observation,
            source_identity,
            definition_identities,
        } = provider;
        let MatrixProjectedKernargPolicyV1 {
            parameters,
            explicit_argument_size,
            implicit_argument_bytes,
            kernarg_segment_size,
            kernarg_segment_alignment,
            digest: projected_digest,
        } = projected_kernarg;
        c.charge(0, 1)?;
        c.string(crate_name)?;
        c.vector(definition_identities)?;
        c.vector(canonical_record)?;
        fixed(stable_crate_id);
        fixed(crate_hash);
        fixed(cargo_metadata_build_observation);
        fixed(source_identity);
        fixed(digest);
        fixed(parameters);
        fixed(explicit_argument_size);
        fixed(implicit_argument_bytes);
        fixed(kernarg_segment_size);
        fixed(kernarg_segment_alignment);
        fixed(projected_digest);
    }
    Ok(())
}

fn terminator_heap(value: &Terminator, c: &mut Counter) -> Result {
    c.charge(0, 1)?;
    match value {
        Terminator::Branch { target, arguments } => {
            fixed(target);
            c.vector(arguments)?;
        }
        Terminator::ConditionalBranch {
            condition,
            then_target,
            then_arguments,
            else_target,
            else_arguments,
        } => {
            fixed(condition);
            fixed(then_target);
            fixed(else_target);
            c.vector(then_arguments)?;
            c.vector(else_arguments)?;
        }
        Terminator::Switch {
            selector,
            cases,
            default_target,
            default_arguments,
        } => {
            fixed(selector);
            fixed(default_target);
            c.vector(cases)?;
            for SwitchCase {
                value,
                target,
                arguments,
            } in cases
            {
                c.charge(0, 1)?;
                fixed(value);
                fixed(target);
                c.vector(arguments)?;
            }
            c.vector(default_arguments)?;
        }
        Terminator::IntegerSwitch {
            selector,
            cases,
            default_target,
            default_arguments,
        } => {
            fixed(selector);
            fixed(default_target);
            c.vector(cases)?;
            for IntegerSwitchCase {
                value,
                target,
                arguments,
            } in cases
            {
                c.charge(0, 1)?;
                constant(value);
                fixed(target);
                c.vector(arguments)?;
            }
            c.vector(default_arguments)?;
        }
        Terminator::Return { values } => c.vector(values)?,
        Terminator::Unreachable => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "module_retained_storage_v11_tests.rs"]
mod tests;
