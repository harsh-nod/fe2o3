//! Exact native subjects. Address uncertainty is distinct from event omission.
use super::native_input_v1::{NativeTraceRefusalV1, failure, native_resource};
use super::native_values_v1::NativeTraceStateV1;
use super::*;
use crate::kir_bridge_v1::canonical_trace_v1::NativeSubjectV1;
use fe2o3_kernel_ir::{AccessMode, AddressSpace, OperationKind, Type, ValueId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeEventKindV1 {
    Load,
    Store,
    Atomic,
    Alloca,
    Return,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeAddressV1 {
    pub base_subject: ValueId,
    pub byte_offset: Option<i128>,
    pub pointee_bytes: Option<u32>,
    pub address_space: AddressSpace,
    pub access: AccessMode,
}

fn type_bytes(ty: &Type) -> Option<u32> {
    match ty {
        Type::Scalar(scalar) => scalar.bit_width().map(|v| u32::from(v).div_ceil(8)),
        _ => None,
    }
}

impl NativeTraceStateV1<'_, '_, '_, '_> {
    pub(crate) fn push_event(
        &mut self,
        events: &mut Vec<PlironTraceEventV1>,
        event: PlironTraceEventV1,
    ) -> Result<(), PlironTraceFailureV1> {
        self.budget.charge_work(1).map_err(native_resource)?;
        if events.len() == events.capacity() {
            let old = events.capacity();
            let extra = old.max(1);
            self.budget
                .reserve_storage(
                    extra
                        .checked_mul(std::mem::size_of::<PlironTraceEventV1>())
                        .ok_or(PlironTraceFailureV1::ResourceLimit)?,
                )
                .map_err(native_resource)?;
            events
                .try_reserve_exact(extra)
                .map_err(|_| PlironTraceFailureV1::ResourceLimit)?;
            let additional = events
                .capacity()
                .checked_sub(old + extra)
                .ok_or(PlironTraceFailureV1::ResourceLimit)?;
            self.budget
                .reserve_storage(
                    additional
                        .checked_mul(std::mem::size_of::<PlironTraceEventV1>())
                        .ok_or(PlironTraceFailureV1::ResourceLimit)?,
                )
                .map_err(native_resource)?;
        }
        events.push(event);
        Ok(())
    }

    pub(crate) fn event(
        &mut self,
        context: &Context,
        pointer: Ptr<Operation>,
        total: &mut usize,
    ) -> Result<Option<PlironTraceEventV1>, PlironTraceFailureV1> {
        self.budget.charge_work(1).map_err(native_resource)?;
        let input = self.input;
        let ordinal = *input
            .function
            .occurrence_index
            .get(&pointer)
            .ok_or_else(|| failure(0, 0, NativeTraceRefusalV1::Correspondence))?;
        let occurrence = &input.function.occurrences[ordinal];
        let location = PlironTraceLocationV1 {
            block: occurrence.block,
            operation: occurrence.operation,
        };
        let NativeSubjectV1::Operation(operation) = occurrence.subject else {
            return Ok(None);
        };
        let kind = match &operation.kind {
            OperationKind::Constant(_)
            | OperationKind::Unary { .. }
            | OperationKind::Binary { .. }
            | OperationKind::Compare { .. }
            | OperationKind::Cast { .. }
            | OperationKind::Select { .. } => {
                let raw = pointer.deref(context);
                for result in raw.results() {
                    if self.evaluate(context, result, total)?.is_none() {
                        return Err(failure(
                            location.block,
                            location.operation,
                            NativeTraceRefusalV1::UnsupportedValue,
                        ));
                    }
                }
                return Ok(None);
            }
            OperationKind::Load { .. } => NativeEventKindV1::Load,
            OperationKind::Store { .. } => NativeEventKindV1::Store,
            OperationKind::Atomic(_) => NativeEventKindV1::Atomic,
            OperationKind::Alloca { .. } => NativeEventKindV1::Alloca,
            OperationKind::Barrier(barrier) => {
                self.budget
                    .charge_work(barrier.semantics.address_spaces.len() + 4)
                    .map_err(native_resource)?;
                return Ok(Some(PlironTraceEventV1::NativeBarrier {
                    location,
                    occurrence: ordinal,
                    execution_scope: hierarchy(barrier.execution_scope).ok_or_else(|| {
                        failure(
                            location.block,
                            location.operation,
                            NativeTraceRefusalV1::MissingHierarchy,
                        )
                    })?,
                    address_spaces: address_spaces(&barrier.semantics.address_spaces),
                }));
            }
            OperationKind::Fence(fence) => {
                self.budget
                    .charge_work(fence.semantics.address_spaces.len() + 3)
                    .map_err(native_resource)?;
                return Ok(Some(PlironTraceEventV1::NativeFence {
                    location,
                    occurrence: ordinal,
                    address_spaces: address_spaces(&fence.semantics.address_spaces),
                }));
            }
            _ => return Ok(None),
        };
        self.budget
            .charge_work(
                pointer.deref(context).get_num_operands()
                    + pointer.deref(context).get_num_results()
                    + 1,
            )
            .map_err(native_resource)?;
        let address = if kind == NativeEventKindV1::Alloca {
            None
        } else {
            self.address(context, pointer.deref(context).get_operand(0), total, 0)?
        };
        Ok(Some(PlironTraceEventV1::NativeSubject {
            location,
            occurrence: ordinal,
            kind,
            address,
        }))
    }

    fn address(
        &mut self,
        context: &Context,
        value: Value,
        total: &mut usize,
        depth: usize,
    ) -> Result<Option<NativeAddressV1>, PlironTraceFailureV1> {
        charge_trace_work_v1(total, 1)?;
        self.budget.charge_work(1).map_err(native_resource)?;
        if depth >= MAX_PLIRON_TRACE_VALUE_VISITS_PER_QUERY_V1 {
            return Err(PlironTraceFailureV1::ResourceLimit);
        }
        let input = self.input;
        let Some((id, Type::Pointer(pointer_type))) = input.function.values.get(&value) else {
            return Ok(None);
        };
        let mut address = NativeAddressV1 {
            base_subject: *id,
            byte_offset: None,
            pointee_bytes: type_bytes(&pointer_type.pointee),
            address_space: pointer_type.address_space,
            access: pointer_type.access,
        };
        let Some(pointer) = value.defining_op() else {
            address.byte_offset = Some(0);
            return Ok(Some(address));
        };
        let ordinal = *input
            .function
            .occurrence_index
            .get(&pointer)
            .ok_or_else(|| failure(0, 0, NativeTraceRefusalV1::Correspondence))?;
        let NativeSubjectV1::Operation(operation) = input.function.occurrences[ordinal].subject
        else {
            return Ok(Some(address));
        };
        match &operation.kind {
            OperationKind::Alloca { .. } => address.byte_offset = Some(0),
            OperationKind::SliceData { slice } => {
                address.base_subject = *slice;
                address.byte_offset = Some(0);
            }
            OperationKind::GetElementPointer { .. } => {
                let raw = pointer.deref(context);
                let base = self.address(context, raw.get_operand(0), total, depth + 1)?;
                let offset = self.evaluate(context, raw.get_operand(1), total)?;
                if let Some(base) = base {
                    address.base_subject = base.base_subject;
                    address.byte_offset = base
                        .byte_offset
                        .zip(offset.and_then(|v| v.signed_offset()))
                        .zip(base.pointee_bytes)
                        .and_then(|((base, offset), width)| {
                            offset
                                .checked_mul(i128::from(width))
                                .and_then(|v| base.checked_add(v))
                        });
                }
            }
            _ => {}
        }
        Ok(Some(address))
    }

    pub(crate) fn scoped_identity(
        &self,
        invocation: &[u64],
        linear: u64,
    ) -> Result<(u64, u64, u64, u64), PlironTraceFailureV1> {
        let Some(workgroup) = self.input.geometry.workgroup_extents else {
            return Ok((0, 0, 0, linear));
        };
        let mut group = [0; 3];
        let mut local = [0; 3];
        let mut counts = [0; 3];
        for dimension in 0..3 {
            if workgroup[dimension] == 0 {
                return Err(PlironTraceFailureV1::InvalidExecutionLayout);
            }
            group[dimension] = invocation[dimension] / workgroup[dimension];
            local[dimension] = invocation[dimension] % workgroup[dimension];
            counts[dimension] =
                self.input.geometry.global_extents[dimension].div_ceil(workgroup[dimension]);
        }
        let flatten = |v: [u64; 3], extents: [u64; 3]| -> Option<u64> {
            v[0].checked_add(
                extents[0].checked_mul(v[1].checked_add(extents[1].checked_mul(v[2])?)?)?,
            )
        };
        let group = flatten(group, counts).ok_or(PlironTraceFailureV1::InvalidExecutionLayout)?;
        let local =
            flatten(local, workgroup).ok_or(PlironTraceFailureV1::InvalidExecutionLayout)?;
        // These slots are not subgroup facts. The sealed geometry says None,
        // and the census refuses every consumer operation requiring a subgroup.
        Ok((0, group, 0, local))
    }
}

fn hierarchy(scope: fe2o3_kernel_ir::SynchronizationScope) -> Option<HierarchyAttr> {
    use fe2o3_kernel_ir::SynchronizationScope as Scope;
    match scope {
        Scope::Workgroup => Some(HierarchyAttr::Workgroup),
        Scope::Subgroup => Some(HierarchyAttr::Subgroup),
        _ => None,
    }
}

fn address_spaces(spaces: &std::collections::BTreeSet<AddressSpace>) -> u8 {
    spaces.iter().fold(0, |mask, space| {
        mask | match space {
            AddressSpace::Private => 1,
            AddressSpace::Workgroup => 2,
            AddressSpace::Global => 4,
            AddressSpace::Constant => 8,
            AddressSpace::Generic => 16,
        }
    })
}
