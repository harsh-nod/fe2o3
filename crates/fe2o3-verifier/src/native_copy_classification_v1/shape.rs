//! Closed bounded SSA interpretation, with no machine/ABI or native premises.

use super::NativeCopyProgramClassificationV1 as Classification;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, ComparePredicate, Constant, FunctionRole,
    IntrinsicOperation, LaunchDomain, LaunchExtent, MemoryAccess, Module, OperationKind,
    ScalarType, Terminator, Type, ValueId, WorkgroupSize,
};

const LIMIT: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Side {
    Input,
    Output,
}

impl Side {
    fn access(self) -> AccessMode {
        match self {
            Self::Input => AccessMode::ReadOnly,
            Self::Output => AccessMode::WriteOnly,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fact {
    Slice(Side),
    Index,
    Length(Side),
    InBounds(Side),
    BothBounds,
    ZeroIndex,
    ZeroU32,
    SafeIndex(Side),
    Base(Side),
    Pointer(Side),
    Loaded,
}

impl Fact {
    fn typed(self, ty: &Type) -> bool {
        match (self, ty) {
            (Self::Slice(side), Type::Slice(slice)) => {
                slice.element.as_ref() == &Type::Scalar(ScalarType::U32)
                    && slice.address_space == AddressSpace::Global
                    && slice.access == side.access()
            }
            (Self::Base(side) | Self::Pointer(side), Type::Pointer(pointer)) => {
                pointer.pointee.as_ref() == &Type::Scalar(ScalarType::U32)
                    && pointer.address_space == AddressSpace::Global
                    && pointer.access == side.access()
            }
            (
                Self::Index | Self::Length(_) | Self::ZeroIndex | Self::SafeIndex(_),
                Type::Scalar(ScalarType::Index),
            )
            | (Self::InBounds(_) | Self::BothBounds, Type::Scalar(ScalarType::Bool))
            | (Self::ZeroU32 | Self::Loaded, Type::Scalar(ScalarType::U32)) => true,
            _ => false,
        }
    }
}

struct Facts {
    slots: [Option<(ValueId, Fact)>; LIMIT + 2],
    used: usize,
}

impl Facts {
    fn get(&self, id: ValueId) -> Option<Fact> {
        self.slots[..self.used]
            .iter()
            .flatten()
            .find_map(|(key, value)| (*key == id).then_some(*value))
    }
    fn insert(&mut self, id: ValueId, value: Fact) -> Option<()> {
        if self.used == self.slots.len() || self.get(id).is_some() {
            return None;
        }
        self.slots[self.used] = Some((id, value));
        self.used += 1;
        Some(())
    }
}

pub(super) fn classify(module: &Module) -> Classification {
    match inspect(module) {
        Some((load, store)) => Classification::GuardedU32 { load, store },
        None => Classification::Unsupported,
    }
}

fn inspect(module: &Module) -> Option<([u32; 2], [u32; 2])> {
    if !module.storage_layouts.is_empty() || !module.required_capabilities.is_empty() {
        return None;
    }
    let [kernel] = module.kernels.as_slice() else {
        return None;
    };
    let [function] = module.functions.as_slice() else {
        return None;
    };
    let [input_type, output_type] = function.signature.parameters.as_slice() else {
        return None;
    };
    if kernel.entry != function.id
        || kernel.id.as_str() != kernel.entry.as_str()
        || function.role != FunctionRole::KernelEntry
        || !function.required_capabilities.is_empty()
        || !kernel.required_capabilities.is_empty()
        || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
        || kernel.domain
            != (LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            })
        || !function.signature.results.is_empty()
        || !Fact::Slice(Side::Input).typed(input_type)
        || !Fact::Slice(Side::Output).typed(output_type)
    {
        return None;
    }
    let body = function.body.as_ref()?;
    let [input, output] = body.parameters.as_slice() else {
        return None;
    };
    if body.blocks.is_empty() || body.blocks.len() > LIMIT {
        return None;
    }
    for (index, block) in body.blocks.iter().enumerate() {
        if !block.parameters.is_empty() || body.blocks[..index].iter().any(|b| b.id == block.id) {
            return None;
        }
    }
    let mut facts = Facts {
        slots: [None; LIMIT + 2],
        used: 0,
    };
    facts.insert(*input, Fact::Slice(Side::Input))?;
    facts.insert(*output, Fact::Slice(Side::Output))?;
    let mut visited = [None; LIMIT];
    let mut visited_count = 0;
    let mut operation_count = 0;
    let mut next = body.blocks[0].id;
    let mut load = None;
    let mut store = None;
    loop {
        if visited_count == LIMIT || visited[..visited_count].contains(&Some(next)) {
            return None;
        }
        visited[visited_count] = Some(next);
        visited_count += 1;
        let block = body.blocks.iter().find(|block| block.id == next)?;
        for (index, operation) in block.operations.iter().enumerate() {
            if operation_count == LIMIT {
                return None;
            }
            operation_count += 1;
            let site = [block.id.0, index as u32];
            let value = operation_fact(&operation.kind, &facts)?;
            match (value, operation.results.as_slice()) {
                (Some(value), [result]) if value.typed(&result.ty) => {
                    if value == Fact::Loaded && load.replace(site).is_some() {
                        return None;
                    }
                    facts.insert(result.id, value)?;
                }
                (None, []) if store.replace(site).is_none() => {}
                _ => return None,
            }
        }
        match &block.terminator {
            Some(Terminator::Branch { target, arguments }) if arguments.is_empty() => {
                next = *target
            }
            Some(Terminator::Return { values }) if values.is_empty() => break,
            _ => return None,
        }
    }
    if visited_count != body.blocks.len() {
        return None;
    }
    Some((load?, store?))
}

fn operation_fact(operation: &OperationKind, facts: &Facts) -> Option<Option<Fact>> {
    use Fact as F;
    let get = |id: &ValueId| facts.get(*id);
    let value = match operation {
        OperationKind::Intrinsic(value) if *value == IntrinsicOperation::global_id_1d() => F::Index,
        OperationKind::Constant(Constant::Index(0)) => F::ZeroIndex,
        OperationKind::Constant(Constant::U32(0)) => F::ZeroU32,
        OperationKind::SliceLength { slice } => {
            let F::Slice(side) = get(slice)? else {
                return None;
            };
            F::Length(side)
        }
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs,
            rhs,
        } if get(lhs) == Some(F::Index) => {
            let F::Length(side) = get(rhs)? else {
                return None;
            };
            F::InBounds(side)
        }
        OperationKind::Binary {
            op: BinaryOp::BitAnd,
            lhs,
            rhs,
        } if matches!(
            (get(lhs), get(rhs)),
            (
                Some(F::InBounds(Side::Input)),
                Some(F::InBounds(Side::Output))
            ) | (
                Some(F::InBounds(Side::Output)),
                Some(F::InBounds(Side::Input))
            )
        ) =>
        {
            F::BothBounds
        }
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } if get(true_value) == Some(F::Index) && get(false_value) == Some(F::ZeroIndex) => {
            let F::InBounds(side) = get(condition)? else {
                return None;
            };
            F::SafeIndex(side)
        }
        OperationKind::SliceData { slice } => {
            let F::Slice(side) = get(slice)? else {
                return None;
            };
            F::Base(side)
        }
        OperationKind::GetElementPointer { base, offset } => {
            let F::Base(side) = get(base)? else {
                return None;
            };
            if get(offset) != Some(F::SafeIndex(side)) {
                return None;
            }
            F::Pointer(side)
        }
        OperationKind::GuardedLoad {
            pointer,
            predicate,
            fallback,
            access,
        } if get(pointer) == Some(F::Pointer(Side::Input))
            && get(predicate) == Some(F::InBounds(Side::Input))
            && get(fallback) == Some(F::ZeroU32)
            && *access == MemoryAccess::new(AddressSpace::Global, 4) =>
        {
            F::Loaded
        }
        OperationKind::GuardedStore {
            pointer,
            predicate,
            value,
            access,
        } if get(pointer) == Some(F::Pointer(Side::Output))
            && get(predicate) == Some(F::BothBounds)
            && get(value) == Some(F::Loaded)
            && *access == MemoryAccess::new(AddressSpace::Global, 4) =>
        {
            return Some(None);
        }
        _ => return None,
    };
    Some(Some(value))
}
