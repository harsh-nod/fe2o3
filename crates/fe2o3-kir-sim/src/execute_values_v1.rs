//! Continuous typed SSA retention. This changes storage, never scheduling or liveness.
//! Layouts borrow the already admitted immutable function types. Every frame
//! reserves the same per-column high water before binding its first argument.
use super::*;

pub(crate) const fn uses_legacy_frame_values(wire_version: u16) -> bool {
    matches!(
        wire_version,
        fe2o3_kernel_ir::KERNEL_IR_VERSION_V20
            | fe2o3_kernel_ir::KERNEL_IR_VERSION_V21
            | fe2o3_kernel_ir::KERNEL_IR_VERSION_V22
    )
}

pub(super) fn fixed_temporary_bytes() -> Option<usize> {
    // Layout/plan construction and one owned scalar/non-scalar reconstruction
    // can overlap existing frames and ordinary debug output. No heap clones.
    size_of::<RuntimeValue>()
        .checked_mul(2)?
        .checked_add(size_of::<Option<RuntimeValue>>().checked_mul(2)?)?
        .checked_add(size_of::<ScalarBitsV1>().checked_mul(2)?)?
        .checked_add(size_of::<FrameValuePlan>().checked_mul(2)?)?
        .checked_add(size_of::<ValueLayout<'static>>().checked_mul(2)?)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FrameValuePlan {
    pub(super) scalars: usize,
    pub(super) others: usize,
    pub(super) shared_bytes: usize,
    pub(super) legacy: Option<usize>,
}

impl FrameValuePlan {
    pub(super) const fn legacy(values: usize) -> Self {
        Self {
            scalars: 0,
            others: 0,
            shared_bytes: size_of::<Vec<ValueLayout<'static>>>(),
            legacy: Some(values),
        }
    }
    pub(super) fn frame_bytes(self) -> Option<usize> {
        if let Some(values) = self.legacy {
            return reserved_hash_map_bytes::<ValueId, RuntimeValue>(values);
        }
        self.scalars
            .checked_mul(size_of::<u128>())?
            .checked_add(self.scalars.checked_mul(size_of::<bool>())?)?
            .checked_add(self.others.checked_mul(size_of::<Option<RuntimeValue>>())?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Slot<'a> {
    id: ValueId,
    ty: &'a Type,
    column: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ValueLayout<'a> {
    slots: Vec<Slot<'a>>,
    scalars: usize,
    others: usize,
}

fn definitions(function: &Function) -> impl Iterator<Item = (ValueId, &Type)> {
    function.body.iter().flat_map(move |body| {
        body.parameters
            .iter()
            .copied()
            .zip(function.signature.parameters.iter())
            .chain(body.blocks.iter().flat_map(|block| {
                block
                    .parameters
                    .iter()
                    .chain(
                        block
                            .operations
                            .iter()
                            .flat_map(|operation| operation.results.iter()),
                    )
                    .map(|value| (value.id, &value.ty))
            }))
    })
}

pub(crate) fn frame_value_plan(
    module: &Module,
    reachable: &[usize],
    legacy: bool,
) -> Option<FrameValuePlan> {
    let mut plan = FrameValuePlan {
        scalars: 0,
        others: 0,
        shared_bytes: 0,
        legacy: None,
    };
    if legacy {
        let maximum = reachable.iter().try_fold(0usize, |maximum, index| {
            let function = module.functions.get(*index)?;
            Some(maximum.max(function_ssa_definition_count(function).unwrap_or(0)))
        })?;
        return Some(FrameValuePlan::legacy(maximum));
    }
    plan.shared_bytes = size_of::<Vec<ValueLayout<'static>>>().checked_add(
        reachable
            .len()
            .checked_mul(size_of::<ValueLayout<'static>>())?,
    )?;
    for index in reachable {
        let function = module.functions.get(*index)?;
        if let Some(body) = &function.body {
            if body.parameters.len() != function.signature.parameters.len() {
                return None;
            }
        }
        let (mut scalars, mut others) = (0usize, 0usize);
        for (_, ty) in definitions(function) {
            if matches!(ty, Type::Scalar(_)) {
                scalars = scalars.checked_add(1)?;
            } else {
                others = others.checked_add(1)?;
            }
        }
        plan.scalars = plan.scalars.max(scalars);
        plan.others = plan.others.max(others);
        plan.shared_bytes = plan.shared_bytes.checked_add(
            scalars
                .checked_add(others)?
                .checked_mul(size_of::<Slot<'static>>())?,
        )?;
    }
    Some(plan)
}

pub(super) fn build_layouts<'a>(
    module: &'a Module,
    reachable: &[usize],
    plan: FrameValuePlan,
) -> Result<Vec<ValueLayout<'a>>, SimulationExecutionErrorKindV1> {
    if plan.legacy.is_some() {
        return Ok(Vec::new());
    }
    let invalid =
        || SimulationExecutionErrorKindV1::InternalInvariant("preflighted SSA column layout");
    let mut layouts = Vec::new();
    layouts
        .try_reserve_exact(reachable.len())
        .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)?;
    if layouts.capacity() != reachable.len() {
        return Err(invalid());
    }
    for index in reachable {
        let function = module.functions.get(*index).ok_or_else(invalid)?;
        let count = function_ssa_definition_count(function).unwrap_or(0);
        let mut layout = ValueLayout {
            slots: Vec::new(),
            scalars: 0,
            others: 0,
        };
        layout
            .slots
            .try_reserve_exact(count)
            .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)?;
        if layout.slots.capacity() != count {
            return Err(invalid());
        }
        for (id, ty) in definitions(function) {
            let column = if matches!(ty, Type::Scalar(_)) {
                let column = layout.scalars;
                layout.scalars = layout.scalars.checked_add(1).ok_or_else(invalid)?;
                column
            } else {
                let column = layout.others;
                layout.others = layout.others.checked_add(1).ok_or_else(invalid)?;
                column
            };
            // Canonical admission bounds the complete definition roster.
            if layout.slots.len() == count {
                return Err(invalid());
            }
            layout.slots.push(Slot { id, ty, column });
        }
        if layout.slots.len() != count
            || layout.scalars > plan.scalars
            || layout.others > plan.others
        {
            return Err(invalid());
        }
        layout.slots.sort_unstable_by_key(|slot| slot.id);
        if layout.slots.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(invalid());
        }
        layouts.push(layout);
    }
    let actual = layouts
        .iter()
        .try_fold(
            size_of::<Vec<ValueLayout<'static>>>()
                .checked_add(
                    layouts
                        .capacity()
                        .checked_mul(size_of::<ValueLayout<'static>>())
                        .ok_or_else(invalid)?,
                )
                .ok_or_else(invalid)?,
            |bytes, layout| {
                bytes.checked_add(
                    layout
                        .slots
                        .capacity()
                        .checked_mul(size_of::<Slot<'static>>())?,
                )
            },
        )
        .ok_or_else(invalid)?;
    if actual > plan.shared_bytes {
        return Err(invalid());
    }
    Ok(layouts)
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, Eq, PartialEq))]
pub(super) enum RuntimeValues<'a> {
    Legacy(HashMap<ValueId, RuntimeValue>),
    Columns {
        layout: &'a ValueLayout<'a>,
        target: SimulationTargetV1,
        scalars: Vec<u128>,
        present: Vec<bool>,
        others: Vec<Option<RuntimeValue>>,
        len: usize,
    },
}

impl Default for RuntimeValues<'_> {
    fn default() -> Self {
        Self::Legacy(HashMap::new())
    }
}

impl<const N: usize> From<[(ValueId, RuntimeValue); N]> for RuntimeValues<'_> {
    fn from(values: [(ValueId, RuntimeValue); N]) -> Self {
        Self::Legacy(HashMap::from(values))
    }
}

impl<'a> RuntimeValues<'a> {
    #[cfg(test)]
    pub(super) fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self::Legacy(HashMap::with_capacity(capacity))
    }

    #[cfg(test)]
    pub(super) fn insert(&mut self, id: ValueId, value: RuntimeValue) -> Option<RuntimeValue> {
        let previous = self.get(&id);
        self.try_insert(id, value).expect("test runtime value slot");
        previous
    }

    // Only physical Legacy cells support in-place state transitions. Columns
    // must use try_insert so slot types and initialization flags remain checked.
    pub(super) fn get_mut(&mut self, id: &ValueId) -> Option<&mut RuntimeValue> {
        match self {
            Self::Legacy(values) => values.get_mut(id),
            Self::Columns { .. } => None,
        }
    }

    #[cfg(test)]
    pub(super) fn lookup_work(&self) -> Option<usize> {
        match self {
            Self::Legacy(_) => Some(self.len()),
            Self::Columns { layout, .. } => {
                let rounds = usize::BITS - layout.slots.len().leading_zeros();
                self.len()
                    .checked_mul(usize::try_from(rounds).ok()?.checked_add(1)?)
            }
        }
    }

    pub(super) fn prepared(
        layout: Option<&'a ValueLayout<'a>>,
        plan: FrameValuePlan,
        target: SimulationTargetV1,
        legacy_capacity: usize,
    ) -> Result<Self, SimulationExecutionErrorKindV1> {
        let Some(layout) = layout else {
            if plan.legacy.is_none() {
                return Err(SimulationExecutionErrorKindV1::InternalInvariant(
                    "missing preflighted SSA column layout",
                ));
            }
            let mut values = HashMap::new();
            values
                .try_reserve(legacy_capacity)
                .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)?;
            return Ok(Self::Legacy(values));
        };
        let invalid =
            || SimulationExecutionErrorKindV1::InternalInvariant("preflighted SSA column capacity");
        if plan.legacy.is_some() || layout.scalars > plan.scalars || layout.others > plan.others {
            return Err(invalid());
        }
        let quoted_bytes = plan.frame_bytes().ok_or_else(invalid)?;
        let mut scalars = Vec::new();
        let mut present = Vec::new();
        let mut others = Vec::new();
        // All three reservations precede any initialization or argument binding.
        scalars
            .try_reserve_exact(plan.scalars)
            .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)?;
        if scalars.capacity() != plan.scalars {
            return Err(invalid());
        }
        present
            .try_reserve_exact(plan.scalars)
            .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)?;
        if present.capacity() != plan.scalars {
            return Err(invalid());
        }
        others
            .try_reserve_exact(plan.others)
            .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)?;
        if others.capacity() != plan.others {
            return Err(invalid());
        }
        let actual = scalars
            .capacity()
            .checked_mul(size_of::<u128>())
            .and_then(|n| n.checked_add(present.capacity().checked_mul(size_of::<bool>())?))
            .and_then(|n| {
                n.checked_add(
                    others
                        .capacity()
                        .checked_mul(size_of::<Option<RuntimeValue>>())?,
                )
            })
            .ok_or_else(invalid)?;
        if actual > quoted_bytes {
            return Err(invalid());
        }
        scalars.resize(plan.scalars, 0);
        present.resize(plan.scalars, false);
        others.resize_with(plan.others, || None);
        Ok(Self::Columns {
            layout,
            target,
            scalars,
            present,
            others,
            len: 0,
        })
    }

    pub(super) fn reset(
        &mut self,
        layout: Option<&'a ValueLayout<'a>>,
        legacy_capacity: usize,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        let invalid =
            || SimulationExecutionErrorKindV1::InternalInvariant("SSA frame storage mode changed");
        match (self, layout) {
            (Self::Legacy(values), None) => {
                values.clear();
                if values.capacity() < legacy_capacity {
                    values
                        .try_reserve(legacy_capacity)
                        .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)?;
                }
            }
            (
                Self::Columns {
                    layout: old,
                    present,
                    scalars,
                    others,
                    len,
                    ..
                },
                Some(new),
            ) => {
                if new.scalars > scalars.len() || new.others > others.len() {
                    return Err(invalid());
                }
                // Clear every retained slot, including inactive sibling high water.
                present.fill(false);
                others.iter_mut().for_each(|slot| *slot = None);
                *len = 0;
                *old = new;
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }

    pub(super) fn len(&self) -> usize {
        match self {
            Self::Legacy(values) => values.len(),
            Self::Columns { len, .. } => *len,
        }
    }
    pub(super) fn capacity(&self) -> usize {
        match self {
            Self::Legacy(values) => values.capacity(),
            Self::Columns { layout, .. } => layout.slots.len(),
        }
    }
    pub(super) fn contains_key(&self, id: &ValueId) -> bool {
        match self {
            Self::Legacy(values) => values.contains_key(id),
            Self::Columns {
                layout,
                present,
                others,
                ..
            } => {
                let Ok(index) = layout.slots.binary_search_by_key(id, |slot| slot.id) else {
                    return false;
                };
                let slot = &layout.slots[index];
                if matches!(slot.ty, Type::Scalar(_)) {
                    present[slot.column]
                } else {
                    others[slot.column].is_some()
                }
            }
        }
    }
    pub(super) fn get(&self, id: &ValueId) -> Option<RuntimeValue> {
        match self {
            Self::Legacy(values) => values.get(id).cloned(),
            Self::Columns {
                layout,
                target,
                scalars,
                present,
                others,
                ..
            } => {
                let slot =
                    &layout.slots[layout.slots.binary_search_by_key(id, |slot| slot.id).ok()?];
                if let Type::Scalar(ty) = slot.ty {
                    if !present[slot.column] {
                        return None;
                    }
                    Some(RuntimeValue::Scalar(
                        ScalarBitsV1::new(*ty, scalars[slot.column], *target).ok()?,
                    ))
                } else {
                    others[slot.column].clone()
                }
            }
        }
    }
    // Borrowed physical/pointer consumers never borrow a reconstructed scalar.
    pub(super) fn legacy_iter(
        &self,
    ) -> Option<std::collections::hash_map::Iter<'_, ValueId, RuntimeValue>> {
        match self {
            Self::Legacy(values) => Some(values.iter()),
            Self::Columns { .. } => None,
        }
    }

    pub(super) fn get_ref(&self, id: &ValueId) -> Option<&RuntimeValue> {
        match self {
            Self::Legacy(values) => values.get(id),
            Self::Columns { layout, others, .. } => {
                let slot =
                    &layout.slots[layout.slots.binary_search_by_key(id, |slot| slot.id).ok()?];
                if matches!(slot.ty, Type::Scalar(_)) {
                    None
                } else {
                    others[slot.column].as_ref()
                }
            }
        }
    }
    pub(super) fn try_insert(&mut self, id: ValueId, value: RuntimeValue) -> Result<(), ()> {
        match self {
            Self::Legacy(values) => {
                values.insert(id, value);
                Ok(())
            }
            Self::Columns {
                layout,
                target,
                scalars,
                present,
                others,
                len,
            } => {
                let slot = &layout.slots[layout
                    .slots
                    .binary_search_by_key(&id, |slot| slot.id)
                    .map_err(|_| ())?];
                if let Type::Scalar(ty) = slot.ty {
                    let RuntimeValue::Scalar(value) = value else {
                        return Err(());
                    };
                    if ScalarBitsV1::new(*ty, value.bits(), *target).ok() != Some(value) {
                        return Err(());
                    }
                    if !present[slot.column] {
                        *len += 1;
                    }
                    scalars[slot.column] = value.bits();
                    present[slot.column] = true;
                } else {
                    if !matches_type(&value, slot.ty) {
                        return Err(());
                    }
                    if others[slot.column].is_none() {
                        *len += 1;
                    }
                    others[slot.column] = Some(value);
                }
                Ok(())
            }
        }
    }
    pub(super) fn remove(&mut self, id: &ValueId) -> Option<RuntimeValue> {
        let value = self.get(id)?;
        match self {
            Self::Legacy(values) => {
                values.remove(id);
            }
            Self::Columns {
                layout,
                present,
                others,
                len,
                ..
            } => {
                let slot =
                    &layout.slots[layout.slots.binary_search_by_key(id, |slot| slot.id).ok()?];
                if matches!(slot.ty, Type::Scalar(_)) {
                    present[slot.column] = false;
                } else {
                    others[slot.column] = None;
                }
                *len -= 1;
            }
        }
        Some(value)
    }
    pub(super) fn ids(&self) -> impl Iterator<Item = ValueId> + '_ {
        let (legacy, columns) = match self {
            Self::Legacy(values) => (Some(values.keys().copied()), None),
            Self::Columns {
                layout,
                present,
                others,
                ..
            } => {
                let initialized = layout.slots.iter().filter_map(move |slot| {
                    let live = if matches!(slot.ty, Type::Scalar(_)) {
                        present[slot.column]
                    } else {
                        others[slot.column].is_some()
                    };
                    live.then_some(slot.id)
                });
                (None, Some(initialized))
            }
        };
        legacy
            .into_iter()
            .flatten()
            .chain(columns.into_iter().flatten())
    }
}

fn matches_type(value: &RuntimeValue, ty: &Type) -> bool {
    match (value, ty) {
        (RuntimeValue::Execution(value), Type::Execution(_)) => value.ty() == *ty,
        (RuntimeValue::Pointer(value), Type::Pointer(ty)) => {
            ty.pointee.as_ref() == &Type::Scalar(value.element)
                && ty.address_space == value.logical_address_space()
                && ty.access == value.access
        }
        (RuntimeValue::StoragePointer(value), Type::Pointer(ty)) => {
            ty.pointee.as_ref() == &Type::StorageObject(value.layout)
                && ty.address_space == value.pointer.address_space
                && ty.access == value.pointer.access
        }
        (RuntimeValue::Slice(value), Type::Slice(ty)) => {
            ty.element.as_ref() == &Type::Scalar(value.element)
                && ty.address_space == value.logical_address_space()
                && ty.access == value.access
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "execute_values_v1_tests.rs"]
mod tests;
