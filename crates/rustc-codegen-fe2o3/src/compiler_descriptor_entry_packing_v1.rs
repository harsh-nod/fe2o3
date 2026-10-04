//! Lexical source-to-device entry schedule. No descriptor, memcpy, or launch permit.
#[path = "compiler_descriptor_entry_paths_v1.rs"]
mod paths;
use super::*;
use fe2o3_kernel_descriptor::{
    DeviceLayoutDescriptorV1, MAX_ARGUMENTS_PER_KERNEL, MAX_KERNARG_SEGMENT_BYTES, MAX_KERNELS,
    MAX_PHYSICAL_COMPONENTS_PER_KERNEL,
};
use fe2o3_kernel_ir::ScalarType;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticLocalIdV1, SemanticTypeIdentityV1, SemanticTypeLayoutV1,
};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Kind {
    Scalar(ScalarTypeV1),
    Pointer,
    SliceLength,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Component {
    pub(super) root: usize,
    pub(super) argument: usize,
    pub(super) slot: usize,
    pub(super) value: ValueId,
    pub(super) source_offset: u64,
    pub(super) source_type: SemanticTypeIdentityV1,
    pub(super) kind: Kind,
    pub(super) offset: u32,
    pub(super) size: u16,
    pub(super) alignment: u16,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Argument {
    pub(super) root: usize,
    pub(super) ordinal: usize,
    pub(super) local: SemanticLocalIdV1,
    pub(super) source_type: SemanticTypeIdentityV1,
    pub(super) source_layout: SemanticLayoutIdentityV1,
    pub(super) source_size: u64,
    pub(super) source_alignment: u32,
    pub(super) first_slot: usize,
    pub(super) end_slot: usize,
    pub(super) first_component: usize,
    pub(super) end_component: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Root {
    pub(super) first_argument: usize,
    pub(super) end_argument: usize,
    pub(super) first_component: usize,
    pub(super) end_component: usize,
    pub(super) physical_slots: usize,
    pub(super) explicit_bytes: u32,
    pub(super) segment_bytes: u32,
    pub(super) alignment: u32,
}

// The view is neither Clone nor constructible from detached diagnostic rows.
pub(super) struct Schedule<'scope, 'owner, 'work> {
    owner: OwnerRef<'owner>,
    captured: &'owner [TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    roots: &'scope [Root],
    arguments: &'scope [Argument],
    components: &'scope [Component],
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    required: usize,
    first: Cell<Option<Resource>>,
    budget: &'scope mut Budget<'work>,
}
fn same_owner(a: OwnerRef<'_>, b: OwnerRef<'_>) -> bool {
    match (a, b) {
        (OwnerRef::Direct(a), OwnerRef::Direct(b)) => std::ptr::eq(a, b),
        (OwnerRef::Erased(a), OwnerRef::Erased(b)) => std::ptr::eq(a, b),
        _ => false,
    }
}
fn resource(error: &E) -> Option<Resource> {
    match error {
        E::Resource(error) | E::Source(SourceError::ArgumentCorrespondenceResource(error)) => {
            Some(*error)
        }
        _ => None,
    }
}
impl Schedule<'_, '_, '_> {
    fn retain<T>(&self, result: R<T>) -> R<T> {
        if let Err(error) = &result
            && let Some(error) = resource(error)
        {
            self.first.set(self.first.get().or(Some(error)));
        }
        if let Some(first) = self.first.get() {
            Err(E::Resource(first))
        } else {
            result
        }
    }
    fn check(&mut self) -> R<()> {
        if let Some(first) = self.first.get() {
            return Err(E::Resource(first));
        }
        let result = if self.slot != self.budget as *const Budget<'_> as usize
            || self.ledger != self.budget.work_ledger_identity_v1()
            || self.budget.storage() < self.required
        {
            Err(Resource::Accounting.into())
        } else {
            self.budget.charge_work(1).map_err(E::Resource)
        };
        self.retain(result)
    }
    pub(super) fn check_subject(
        &mut self,
        owner: OwnerRef<'_>,
        captured: &[TypedDescriptorRootV1],
        profile: ProductionAmdTargetProfileV1,
    ) -> R<()> {
        if !same_owner(self.owner, owner)
            || !std::ptr::eq(self.captured, captured)
            || self.profile != profile
        {
            return self.retain(Err(Resource::Accounting.into()));
        }
        self.check()
    }
    pub(super) fn counts(&mut self) -> R<(usize, usize, usize)> {
        self.check()?;
        Ok((
            self.roots.len(),
            self.arguments.len(),
            self.components.len(),
        ))
    }
    pub(super) fn charge_work(&mut self, amount: usize) -> R<()> {
        self.check()?;
        let result = self.budget.charge_work(amount).map_err(E::Resource);
        self.retain(result)
    }
    pub(super) fn root(&mut self, index: usize) -> R<Root> {
        self.check()?;
        self.roots
            .get(index)
            .copied()
            .ok_or(E::Mismatch("packing root ordinal"))
    }
    pub(super) fn argument(&mut self, index: usize) -> R<Argument> {
        self.check()?;
        self.arguments
            .get(index)
            .copied()
            .ok_or(E::Mismatch("packing logical ordinal"))
    }
    pub(super) fn component(&mut self, index: usize) -> R<Component> {
        self.check()?;
        self.components
            .get(index)
            .copied()
            .ok_or(E::Mismatch("packing component ordinal"))
    }
    pub(super) const fn grants_authority(&self) -> bool {
        false
    }
}

#[derive(Default)]
struct Cursor {
    end: u32,
    alignment: u32,
}
// Fixed logical work for checked explicit/implicit boundary arithmetic per root.
const ROOT_FINISH_WORK: usize = 12;
impl Cursor {
    fn advance(&mut self, size: u16, alignment: u16) -> R<u32> {
        if size == 0 || !alignment.is_power_of_two() {
            return Err(E::Mismatch("physical scalar packing dimensions"));
        }
        let alignment = u32::from(alignment);
        let offset = self
            .end
            .checked_add(alignment - 1)
            .ok_or(Resource::Arithmetic)?
            & !(alignment - 1);
        self.end = offset
            .checked_add(u32::from(size))
            .ok_or(Resource::Arithmetic)?;
        self.alignment = self.alignment.max(alignment);
        if self.end > fe2o3_artifacts::MAX_ABI_BYTES as u32 {
            return Err(E::Mismatch("explicit packing byte limit"));
        }
        Ok(offset)
    }
    fn finish(&self) -> R<(u32, u32, u32)> {
        let explicit_alignment = self.alignment.max(1);
        let explicit = self
            .end
            .checked_add(explicit_alignment - 1)
            .ok_or(Resource::Arithmetic)?
            & !(explicit_alignment - 1);
        // The worker forces the COV6 implicit block; the finalizer requires its
        // start aligned to eight, independently of explicit scalar alignment.
        let hidden_start = explicit.checked_add(7).ok_or(Resource::Arithmetic)? & !7;
        let segment = hidden_start.checked_add(256).ok_or(Resource::Arithmetic)?;
        let alignment = explicit_alignment.max(8);
        if explicit > fe2o3_artifacts::MAX_ABI_BYTES as u32 || segment > MAX_KERNARG_SEGMENT_BYTES {
            return Err(E::Mismatch("packing segment byte limit"));
        }
        Ok((explicit, segment, alignment))
    }
}
fn scalar(ty: ScalarType) -> R<ScalarTypeV1> {
    Ok(match ty {
        ScalarType::I8 => ScalarTypeV1::I8,
        ScalarType::U8 => ScalarTypeV1::U8,
        ScalarType::I16 => ScalarTypeV1::I16,
        ScalarType::U16 => ScalarTypeV1::U16,
        ScalarType::I32 => ScalarTypeV1::I32,
        ScalarType::U32 => ScalarTypeV1::U32,
        ScalarType::I64 => ScalarTypeV1::I64,
        ScalarType::U64 => ScalarTypeV1::U64,
        ScalarType::F16 => ScalarTypeV1::F16,
        ScalarType::F32 => ScalarTypeV1::F32,
        ScalarType::F64 => ScalarTypeV1::F64,
        // No new validity, pointer-sized, wide or vector ABI is inferred here.
        _ => {
            return Err(E::Mismatch(
                "packing scalar needs a versioned physical representation",
            ));
        }
    })
}
struct Builder {
    roots: Vec<Root>,
    arguments: Vec<Argument>,
    components: Vec<Component>,
}
fn push<T>(rows: &mut Vec<T>, value: T) -> R<()> {
    if rows.len() == rows.capacity() {
        return Err(Resource::Accounting.into());
    }
    rows.push(value);
    Ok(())
}
impl Builder {
    fn root(
        &mut self,
        ordinal: usize,
        captured: &TypedDescriptorRootV1,
        semantic: &AdmittedInertSemanticMirV1,
        plan: &mut Plan<'_, '_, '_>,
    ) -> R<()> {
        let (logical, physical) = plan.counts()?;
        if ordinal != self.roots.len()
            || logical > MAX_ARGUMENTS_PER_KERNEL
            || physical > MAX_PHYSICAL_COMPONENTS_PER_KERNEL
        {
            return Err(E::Mismatch("complete bounded packing root roster"));
        }
        let first_argument = self.arguments.len();
        let first_component = self.components.len();
        let mut cursor = Cursor::default();
        let mut next_slot = 0;
        for index in 0..logical {
            plan.charge_work(16)?;
            let source = plan.argument(index)?;
            let slots = source.physical_slots();
            let argument = &captured.arguments.as_slice()[index];
            if source.source_argument() as usize != index
                || slots.start != next_slot
                || slots.end > physical
            {
                return Err(E::Mismatch("dense logical to physical packing coverage"));
            }
            let start = self.components.len();
            for slot in slots.clone() {
                plan.charge_work(24)?;
                let (first, slice, base, value, source_type) = {
                    let component = plan.component(slot)?;
                    let entry = component.physical();
                    let ty = semantic
                        .types()
                        .get(component.semantic_type().index() as usize)
                        .ok_or(E::Mismatch("packing original leaf type"))?;
                    if entry.slot() != slot || component.source_argument() as usize != index {
                        return Err(E::Mismatch("packing original physical component"));
                    }
                    let first = match entry.ty() {
                        Type::Scalar(value) => {
                            let value = scalar(*value)?;
                            let layout = DeviceLayoutDescriptorV1::scalar(value);
                            if ty.layout().size_bytes() != Some(u64::from(layout.size_bytes())) {
                                return Err(E::Mismatch("packing source scalar extent"));
                            }
                            (
                                Kind::Scalar(value),
                                layout.size_bytes(),
                                layout.alignment_bytes(),
                            )
                        }
                        Type::Slice(_) | Type::Pointer(_)
                            if argument.kind
                                != DescriptorArgumentKindV1::CompilerLaidOutByValue =>
                        {
                            (Kind::Pointer, 8, 8)
                        }
                        _ => {
                            return Err(E::Mismatch(
                                "aggregate packing requires supported scalar leaves",
                            ));
                        }
                    };
                    let slice = matches!(entry.ty(), Type::Slice(_));
                    (
                        first,
                        slice,
                        component.source_offset_bytes(),
                        entry.value(),
                        ty.identity(),
                    )
                };
                for part in 0..(1 + usize::from(slice)) {
                    plan.charge_work(12)?;
                    let (kind, size, alignment) = if part == 0 {
                        first
                    } else {
                        (Kind::SliceLength, 8, 8)
                    };
                    let source_offset = base
                        .checked_add((part as u64) * 8)
                        .ok_or(Resource::Arithmetic)?;
                    if source_offset
                        .checked_add(u64::from(size))
                        .is_none_or(|end| end > argument.source_size)
                    {
                        return Err(E::Mismatch("packing source field extent"));
                    }
                    let offset = cursor.advance(size, alignment)?;
                    push(
                        &mut self.components,
                        Component {
                            root: ordinal,
                            argument: index,
                            slot,
                            value,
                            source_offset,
                            source_type,
                            kind,
                            offset,
                            size,
                            alignment,
                        },
                    )?;
                }
            }
            next_slot = slots.end;
            push(
                &mut self.arguments,
                Argument {
                    root: ordinal,
                    ordinal: index,
                    local: source.local(),
                    source_type: argument.semantic_type_identity,
                    source_layout: argument.semantic_layout_identity,
                    source_size: argument.source_size,
                    source_alignment: argument.source_alignment,
                    first_slot: slots.start,
                    end_slot: slots.end,
                    first_component: start,
                    end_component: self.components.len(),
                },
            )?;
        }
        if next_slot != physical
            || self.components.len() - first_component > MAX_PHYSICAL_COMPONENTS_PER_KERNEL
        {
            return Err(E::Mismatch("complete bounded packing physical roster"));
        }
        plan.charge_work(ROOT_FINISH_WORK)?;
        let (explicit_bytes, segment_bytes, alignment) = cursor.finish()?;
        push(
            &mut self.roots,
            Root {
                first_argument,
                end_argument: self.arguments.len(),
                first_component,
                end_component: self.components.len(),
                physical_slots: physical,
                explicit_bytes,
                segment_bytes,
                alignment,
            },
        )
    }
}

type BuildFrame<'a, 'work> = (
    OwnerRef<'a>,
    &'a [TypedDescriptorRootV1],
    ProductionAmdTargetProfileV1,
    &'a mut Budget<'work>,
);
type VisitFrame<'a, 'source, 'work> = (
    usize,
    &'a TypedDescriptorRootV1,
    &'a AdmittedInertSemanticMirV1,
    &'a mut Plan<'a, 'source, 'work>,
    &'a mut Builder,
);
type ConsumerCapture<'a, 'owner, 'work, C> = (C, &'a mut Schedule<'a, 'owner, 'work>);
fn headers<C>() -> R<usize> {
    sum_frames(&[
        frame::<C>(),
        align_of::<C>(),
        frame::<ConsumerCapture<'_, '_, '_, C>>(),
        frame::<AssertUnwindSafe<ConsumerCapture<'_, '_, '_, C>>>(),
        frame::<Builder>(),
        frame::<Schedule<'_, '_, '_>>(),
        frame::<Cursor>(),
        frame::<Root>(),
        frame::<Argument>(),
        frame::<Component>(),
        frame::<Option<Root>>(),
        frame::<Option<Argument>>(),
        frame::<Option<Component>>(),
        frame::<&Root>(),
        frame::<&Argument>(),
        frame::<&Component>(),
        frame::<Vec<Root>>(),
        frame::<Vec<Argument>>(),
        frame::<Vec<Component>>(),
        frame::<(&mut Vec<Root>, Root)>(),
        frame::<(&mut Vec<Argument>, Argument)>(),
        frame::<(&mut Vec<Component>, Component)>(),
        frame::<&[Root]>(),
        frame::<&[Argument]>(),
        frame::<&[Component]>(),
        frame::<OwnerRef<'_>>(),
        frame::<&[TypedDescriptorRootV1]>(),
        frame::<&TypedDescriptorRootV1>(),
        frame::<&TypedDescriptorArgumentV1>(),
        frame::<ProductionAmdTargetProfileV1>(),
        frame::<&mut Budget<'_>>(),
        frame::<&Module>(),
        frame::<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12>(),
        frame::<&[SemanticTypeDeclV1]>(),
        frame::<&[TypedDescriptorArgumentV1]>(),
        frame::<&Vec<Type>>(),
        frame::<&Vec<fe2o3_kernel_ir::Kernel>>(),
        frame::<&Vec<Function>>(),
        frame::<&fe2o3_kernel_ir::FunctionId>(),
        frame::<&str>(),
        frame::<&SemanticTypeLayoutV1>(),
        frame::<&Function>(),
        frame::<Option<&Function>>(),
        frame::<&fe2o3_kernel_ir::Kernel>(),
        frame::<&Type>(),
        frame::<&ScalarType>(),
        frame::<&Cursor>(),
        frame::<&mut Cursor>(),
        frame::<(&mut Cursor, u16, u16)>(),
        frame::<&SemanticTypeDeclV1>(),
        frame::<Option<&SemanticTypeDeclV1>>(),
        frame::<&AdmittedInertSemanticMirV1>(),
        frame::<&mut Plan<'_, '_, '_>>(),
        frame::<&mut Builder>(),
        frame::<fe2o3_lower_mir_kernel::ProductionSourceAbiArgumentV1>(),
        frame::<fe2o3_lower_mir_kernel::ProductionSourceAbiComponentV1<'_>>(),
        frame::<fe2o3_lower_mir_kernel::ProductionPhysicalArgumentV1<'_>>(),
        frame::<std::ops::Range<usize>>(),
        frame::<std::ops::Range<usize>>(),
        frame::<std::slice::Iter<'_, TypedDescriptorRootV1>>(),
        frame::<std::slice::Iter<'_, fe2o3_kernel_ir::Kernel>>(),
        frame::<std::slice::Iter<'_, Type>>(),
        frame::<std::slice::Iter<'_, Function>>(),
        frame::<Option<&TypedDescriptorRootV1>>(),
        frame::<Option<&fe2o3_kernel_ir::Kernel>>(),
        frame::<Option<&Type>>(),
        frame::<DeviceLayoutDescriptorV1>(),
        frame::<Kind>(),
        frame::<(Kind, u16, u16)>(),
        frame::<((Kind, u16, u16), bool, u64, ValueId, SemanticTypeIdentityV1)>(),
        frame::<(u32, u32, u32)>(),
        frame::<(usize, usize, usize)>(),
        frame::<Option<Resource>>(),
        frame::<Option<u64>>(),
        frame::<Option<u32>>(),
        frame::<Option<usize>>(),
        frame::<ScalarTypeV1>(),
        frame::<ScalarType>(),
        frame::<ValueId>(),
        frame::<SemanticTypeIdentityV1>(),
        frame::<SemanticLayoutIdentityV1>(),
        frame::<SemanticLocalIdV1>(),
        frame::<bool>(),
        frame::<()>(),
        frame::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        24 * frame::<usize>(),
        4 * frame::<u64>(),
        6 * frame::<u32>(),
        3 * frame::<u16>(),
        frame::<Result<R<()>, Box<dyn std::any::Any + Send>>>(),
        frame::<Result<R<Builder>, Box<dyn std::any::Any + Send>>>(),
        frame::<Result<(), Box<dyn std::any::Any + Send>>>(),
        frame::<Box<dyn std::any::Any + Send>>(),
        2 * frame::<BuildFrame<'_, '_>>(),
        frame::<AssertUnwindSafe<BuildFrame<'_, '_>>>(),
        frame::<VisitFrame<'_, '_, '_>>(),
        frame::<(
            OwnerRef<'_>,
            &[TypedDescriptorRootV1],
            ProductionAmdTargetProfileV1,
            &mut Schedule<'_, '_, '_>,
        )>(),
        frame::<&mut Schedule<'_, '_, '_>>(),
        frame::<(&mut Builder,)>(),
    ])
}
fn vector<T>(count: usize, budget: &mut Budget<'_>) -> R<Vec<T>> {
    super::super::nominal_v3::vector(count, budget).map_err(|error| match error {
        super::super::nominal_v3::NominalDescriptorErrorV3::Resource(error) => E::Resource(error),
        _ => E::Mismatch("packing vector allocation"),
    })
}
fn build(
    owner: OwnerRef<'_>,
    captured: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> R<Builder> {
    if captured.is_empty() || captured.len() > MAX_KERNELS {
        return Err(E::Mismatch("bounded packing roots"));
    }
    let output = match owner {
        OwnerRef::Direct(owner) => owner.output(),
        OwnerRef::Erased(owner) => owner.output(),
    }
    .module();
    let mut logical = 0usize;
    let mut physical = 0usize;
    for root in captured {
        budget.charge_work(2)?;
        if root.arguments.len() > MAX_ARGUMENTS_PER_KERNEL {
            return Err(E::Mismatch("bounded packing arguments"));
        }
        logical = logical
            .checked_add(root.arguments.len())
            .ok_or(Resource::Arithmetic)?;
    }
    for kernel in &output.kernels {
        budget.charge_work(
            output
                .functions
                .len()
                .checked_mul(
                    kernel
                        .entry
                        .as_str()
                        .len()
                        .checked_add(1)
                        .ok_or(Resource::Arithmetic)?,
                )
                .ok_or(Resource::Arithmetic)?,
        )?;
        let function = output
            .function(&kernel.entry)
            .ok_or(E::Mismatch("packing preflight entry"))?;
        if function.signature.parameters.len() > MAX_PHYSICAL_COMPONENTS_PER_KERNEL {
            return Err(E::Mismatch("bounded packing slots"));
        }
        let mut count = 0usize;
        for ty in &function.signature.parameters {
            budget.charge_work(1)?;
            count = count
                .checked_add(1 + usize::from(matches!(ty, Type::Slice(_))))
                .ok_or(Resource::Arithmetic)?;
        }
        if count > MAX_PHYSICAL_COMPONENTS_PER_KERNEL {
            return Err(E::Mismatch("bounded packing components"));
        }
        physical = physical.checked_add(count).ok_or(Resource::Arithmetic)?;
    }
    let mut rows = Builder {
        roots: vector(captured.len(), budget)?,
        arguments: vector(logical, budget)?,
        components: vector(physical, budget)?,
    };
    let mut visit =
        |index,
         root: &TypedDescriptorRootV1,
         semantic: &AdmittedInertSemanticMirV1,
         plan: &mut Plan<'_, '_, '_>| rows.root(index, root, semantic, plan);
    visit_checked(owner, captured, profile, budget, Some(&mut visit))?;
    drop(visit);
    if rows.arguments.len() != logical
        || rows.components.len() != physical
        || rows.roots.len() != captured.len()
    {
        return Err(E::Mismatch("packing preflight and authenticated coverage"));
    }
    Ok(rows)
}

pub(super) fn with_schedule<C>(
    owner: OwnerRef<'_>,
    captured: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
    consume: C,
) -> R<()>
where
    C: for<'scope, 'owner, 'work> FnOnce(&mut Schedule<'scope, 'owner, 'work>) -> R<()>,
{
    let floor = budget.storage();
    let slot = budget as *const Budget<'_> as usize;
    let ledger = budget.work_ledger_identity_v1();
    budget.reserve_storage(headers::<C>()?)?;
    let constructed = catch_unwind(AssertUnwindSafe(|| build(owner, captured, profile, budget)));
    let rows = match constructed {
        Ok(Ok(rows)) => rows,
        other => {
            // A rejected consumer capture is dropped before its paid frame is
            // released, but its destructor cannot skip constructor cleanup.
            let dropped = catch_unwind(AssertUnwindSafe(|| drop(consume)));
            if budget as *const Budget<'_> as usize == slot
                && budget.work_ledger_identity_v1() == ledger
                && budget.storage() >= floor
            {
                budget.release_storage(budget.storage() - floor)?;
            }
            drop(dropped);
            return match other {
                Ok(Err(error)) => Err(error),
                Err(payload) => resume_unwind(payload),
                Ok(Ok(_)) => unreachable!(),
            };
        }
    };
    let required = budget.storage();
    let owned = required.checked_sub(floor).ok_or(Resource::Accounting)?;
    let mut view = Schedule {
        owner,
        captured,
        profile,
        roots: &rows.roots,
        arguments: &rows.arguments,
        components: &rows.components,
        slot,
        ledger,
        required,
        first: Cell::new(None),
        budget,
    };
    let result = catch_unwind(AssertUnwindSafe(|| consume(&mut view)));
    let first = view.first.get();
    let valid = view.budget as *const Budget<'_> as usize == slot
        && view.budget.work_ledger_identity_v1() == ledger
        && view.budget.storage() >= required;
    drop(view);
    drop(rows);
    if valid {
        budget.release_storage(owned)?;
    }
    match result {
        Err(payload) => resume_unwind(payload),
        Ok(result) => match first {
            Some(error) => Err(E::Resource(error)),
            None => match result {
                Err(error) => Err(error),
                Ok(()) if !valid => Err(Resource::Accounting.into()),
                Ok(()) => Ok(()),
            },
        },
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

    #[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
    pub(crate) struct Qualification {
        pub(crate) logical: usize,
        pub(crate) physical: usize,
        pub(crate) components: usize,
        pub(crate) explicit_bytes: u32,
        pub(crate) ignored: usize,
        pub(crate) controls: usize,
    }
    fn inspect(
        view: &mut Schedule<'_, '_, '_>,
        owner: OwnerRef<'_>,
        captured: &[TypedDescriptorRootV1],
        profile: ProductionAmdTargetProfileV1,
    ) -> R<Qualification> {
        view.check_subject(owner, captured, profile)?;
        assert!(!view.grants_authority());
        let (roots, logical, components) = view.counts()?;
        assert_eq!(roots, 1);
        assert_eq!(logical, 3);
        let root = view.root(0)?;
        assert_eq!(
            (
                root.first_argument,
                root.end_argument,
                root.first_component,
                root.end_component
            ),
            (0, 3, 0, components)
        );
        assert_eq!(root.segment_bytes, root.explicit_bytes + 256);
        assert_eq!(root.alignment, 8);
        assert_eq!(components, root.physical_slots + 1);
        let mut prior_slot = 0;
        let mut prior_component = 0;
        let mut ignored = 0;
        for index in 0..logical {
            let row = view.argument(index)?;
            assert_eq!(
                (row.root, row.ordinal, row.first_slot, row.first_component),
                (0, index, prior_slot, prior_component)
            );
            assert_eq!(
                row.source_type,
                captured[0].arguments.as_slice()[index].semantic_type_identity
            );
            assert_eq!(
                row.source_layout,
                captured[0].arguments.as_slice()[index].semantic_layout_identity
            );
            if row.first_slot == row.end_slot {
                assert_eq!(row.source_size, 0);
                assert_eq!(row.first_component, row.end_component);
                ignored += 1;
            }
            for at in row.first_component..row.end_component {
                let part = view.component(at)?;
                assert_eq!((part.root, part.argument), (0, index));
                assert!((row.first_slot..row.end_slot).contains(&part.slot));
                assert!(part.source_offset + u64::from(part.size) <= row.source_size);
            }
            prior_slot = row.end_slot;
            prior_component = row.end_component;
        }
        assert_eq!(
            (prior_slot, prior_component),
            (root.physical_slots, components)
        );
        // This fixture has an aggregate, a slice and a scalar. The physical
        // parameter order, not captured source struct offsets, defines padding.
        let expected: &[u32] = match root.physical_slots {
            2 => &[0, 8, 16],
            4 => &[0, 8, 16, 24, 32],
            6 => &[0, 8, 16, 18, 24, 32, 40],
            other => panic!("unexpected genuine fixture slot count {other}"),
        };
        for (index, expected) in expected.iter().enumerate() {
            assert_eq!(view.component(index)?.offset, *expected);
        }
        assert_eq!(
            root.explicit_bytes,
            match root.physical_slots {
                2 => 24,
                4 => 40,
                6 => 48,
                _ => unreachable!(),
            }
        );
        paths::tests::inspect(view)?;
        Ok(Qualification {
            logical,
            physical: root.physical_slots,
            components,
            explicit_bytes: root.explicit_bytes,
            ignored,
            controls: 0,
        })
    }
    fn measure(
        owner: OwnerRef<'_>,
        roots: &[TypedDescriptorRootV1],
        profile: ProductionAmdTargetProfileV1,
        floor: usize,
        wl: usize,
        sl: usize,
    ) -> (R<()>, Option<Qualification>, usize, usize) {
        let mut work = Work::new(wl);
        let mut budget = Budget::new(&mut work, sl);
        budget.reserve_storage(floor).unwrap();
        let mut observation = None;
        let result = with_schedule(owner, roots, profile, &mut budget, |view| {
            observation = Some(inspect(view, owner, roots, profile)?);
            Ok(())
        });
        assert_eq!(budget.storage(), floor);
        (result, observation, budget.work(), budget.peak_storage())
    }
    pub(crate) fn qualify(
        owner: OwnerRef<'_>,
        roots: &[TypedDescriptorRootV1],
        profile: ProductionAmdTargetProfileV1,
        floor: usize,
    ) -> Qualification {
        let (result, observation, work, storage) =
            measure(owner, roots, profile, floor, usize::MAX, usize::MAX);
        result.unwrap();
        let mut observed = observation.unwrap();
        let (exact, same, used, peak) = measure(owner, roots, profile, floor, work, storage);
        exact.unwrap();
        assert_eq!(same, Some(observed));
        assert_eq!((used, peak), (work, storage));
        for (wl, sl, work_expected) in [(work - 1, storage, true), (work, storage - 1, false)] {
            let (result, value, _, _) = measure(owner, roots, profile, floor, wl, sl);
            assert!(value.is_none());
            let cause = resource(&result.unwrap_err()).expect("exact packing resource cause");
            match (cause, work_expected) {
                (Resource::Work(error), true) => {
                    assert_eq!(error.limit(), wl);
                    assert!(error.actual() > wl);
                }
                (Resource::Storage(error), false) => {
                    assert_eq!(error.limit(), sl);
                    assert!(error.actual() > sl);
                }
                other => panic!("wrong resource cut: {other:?}"),
            }
            observed.controls += 1;
        }
        for mode in 0..10 {
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(floor).unwrap();
            let copied = roots.to_vec();
            let mut reached = false;
            let mut protected = 0;
            let result = catch_unwind(AssertUnwindSafe(|| {
                with_schedule(owner, roots, profile, &mut budget, |view| {
                    inspect(view, owner, roots, profile)?;
                    reached = true;
                    protected = view.budget.storage();
                    match mode {
                        0 => {
                            let before = (view.budget.work(), view.budget.storage());
                            assert!(matches!(
                                view.check_subject(owner, &copied, profile),
                                Err(E::Resource(Resource::Accounting))
                            ));
                            assert!(matches!(
                                view.counts(),
                                Err(E::Resource(Resource::Accounting))
                            ));
                            assert_eq!((view.budget.work(), view.budget.storage()), before);
                        }
                        1 => {
                            let mut work = Work::new(usize::MAX);
                            let foreign = Budget::new(&mut work, usize::MAX);
                            let original = view.ledger;
                            view.ledger = foreign.work_ledger_identity_v1();
                            let before = (view.budget.work(), view.budget.storage());
                            assert!(matches!(
                                view.counts(),
                                Err(E::Resource(Resource::Accounting))
                            ));
                            assert_eq!((foreign.work(), foreign.storage()), (0, 0));
                            view.ledger = original;
                            assert!(matches!(
                                view.counts(),
                                Err(E::Resource(Resource::Accounting))
                            ));
                            assert_eq!((view.budget.work(), view.budget.storage()), before);
                        }
                        2 => {
                            let first = view.charge_work(usize::MAX).unwrap_err();
                            let before = (view.budget.work(), view.budget.storage());
                            let replay = view.counts().unwrap_err();
                            assert_eq!(resource(&first), resource(&replay));
                            assert_eq!((view.budget.work(), view.budget.storage()), before);
                            return Err(E::Mismatch("selected after resource"));
                        }
                        3 => return Err(E::Mismatch("selected packing callback")),
                        4 => panic!("selected packing callback unwind"),
                        // Private hostile access, unavailable to real consumers.
                        5 => view.budget.reserve_storage(13)?,
                        6 => view.budget.release_storage(1)?,
                        7 => {
                            let other = match profile {
                                ProductionAmdTargetProfileV1::Gfx942 => {
                                    ProductionAmdTargetProfileV1::Gfx950
                                }
                                ProductionAmdTargetProfileV1::Gfx950 => {
                                    ProductionAmdTargetProfileV1::Gfx942
                                }
                            };
                            let before = (view.budget.work(), view.budget.storage());
                            assert!(matches!(
                                view.check_subject(owner, roots, other),
                                Err(E::Resource(Resource::Accounting))
                            ));
                            assert!(matches!(
                                view.check_subject(owner, roots, profile),
                                Err(E::Resource(Resource::Accounting))
                            ));
                            assert_eq!((view.budget.work(), view.budget.storage()), before);
                        }
                        8 => {
                            assert!(matches!(
                                view.root(usize::MAX),
                                Err(E::Mismatch("packing root ordinal"))
                            ));
                            assert!(matches!(
                                view.argument(usize::MAX),
                                Err(E::Mismatch("packing logical ordinal"))
                            ));
                            assert!(matches!(
                                view.component(usize::MAX),
                                Err(E::Mismatch("packing component ordinal"))
                            ));
                            let mut expected = observed;
                            expected.controls = 0;
                            assert_eq!(inspect(view, owner, roots, profile)?, expected);
                        }
                        9 => {
                            let before = (view.budget.work(), view.budget.storage());
                            for _ in 0..128 {
                                view.root(0)?;
                                view.argument(0)?;
                                view.component(0)?;
                            }
                            assert_eq!(view.budget.work() - before.0, 3 * 128);
                            assert_eq!(view.budget.storage(), before.1);
                        }
                        _ => unreachable!(),
                    }
                    Ok(())
                })
            }));
            assert!(reached);
            match mode {
                0 | 1 | 6 | 7 => {
                    assert!(matches!(result, Ok(Err(E::Resource(Resource::Accounting)))))
                }
                2 => assert!(matches!(result, Ok(Err(E::Resource(Resource::Work(_)))))),
                3 => assert!(matches!(
                    result,
                    Ok(Err(E::Mismatch("selected packing callback")))
                )),
                4 => assert!(result.is_err()),
                5 | 8 | 9 => result.unwrap().unwrap(),
                _ => unreachable!(),
            }
            assert_eq!(
                budget.storage(),
                match mode {
                    5 => floor + 13,
                    6 => protected - 1,
                    _ => floor,
                }
            );
            observed.controls += 1;
        }
        // Denial before any root is exposed, including a successful roots Vec
        // reservation followed by an arguments Vec refusal.
        for partial in [false, true] {
            let mut work = Work::new(usize::MAX);
            let mut reached = false;
            let consumer = |_: &mut Schedule<'_, '_, '_>| {
                reached = true;
                Ok(())
            };
            fn frame_of<C>(_: &C) -> usize {
                headers::<C>().unwrap()
            }
            let fixed = frame_of(&consumer);
            let limit = if partial {
                floor + fixed + size_of::<Vec<Root>>() + roots.len() * size_of::<Root>()
            } else {
                floor + fixed - 1
            };
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(floor).unwrap();
            let result = with_schedule(owner, roots, profile, &mut budget, consumer);
            assert!(!reached);
            let Some(Resource::Storage(error)) = resource(&result.unwrap_err()) else {
                panic!("constructor storage refusal required");
            };
            assert_eq!(error.limit(), limit);
            assert!(error.actual() > limit);
            assert_eq!(budget.storage(), floor);
            if partial {
                assert!(budget.work() > 0);
                assert!(budget.peak_storage() > floor + fixed);
            } else {
                assert_eq!(budget.work(), 0);
                assert_eq!(budget.peak_storage(), floor);
            }
            observed.controls += 1;
        }
        paths::tests::qualify(owner, roots, profile, floor);
        observed
    }

    #[test]
    fn entry_packing_cursor_separates_scalar_padding_and_zero_component_arguments() {
        let mut cursor = Cursor::default();
        assert_eq!(cursor.finish().unwrap(), (0, 256, 8));
        assert_eq!(cursor.advance(4, 4).unwrap(), 0);
        assert_eq!(cursor.advance(8, 8).unwrap(), 8);
        assert_eq!(cursor.advance(2, 2).unwrap(), 16);
        assert_eq!(cursor.advance(2, 2).unwrap(), 18);
        assert_eq!(cursor.advance(8, 8).unwrap(), 24);
        assert_eq!(cursor.finish().unwrap(), (32, 288, 8));
        assert!(cursor.advance(0, 1).is_err());
        assert!(cursor.advance(8, 3).is_err());
    }
    #[test]
    fn entry_packing_scalar_dimensions_reuse_closed_descriptor_layouts() {
        for ty in [
            ScalarType::I8,
            ScalarType::U8,
            ScalarType::I16,
            ScalarType::U16,
            ScalarType::I32,
            ScalarType::U32,
            ScalarType::I64,
            ScalarType::U64,
            ScalarType::F16,
            ScalarType::F32,
            ScalarType::F64,
        ] {
            let descriptor = scalar(ty).unwrap();
            let layout = DeviceLayoutDescriptorV1::scalar(descriptor);
            assert_eq!(layout.size_bytes(), descriptor.size_bytes());
            assert_eq!(layout.alignment_bytes(), descriptor.alignment_bytes());
            assert!(layout.alignment_bytes() <= 8);
        }
        for ty in [
            ScalarType::Bool,
            ScalarType::Index,
            ScalarType::I128,
            ScalarType::U128,
            ScalarType::Bf16,
        ] {
            assert!(matches!(
                scalar(ty),
                Err(E::Mismatch(
                    "packing scalar needs a versioned physical representation"
                ))
            ));
        }
    }
}

#[cfg(test)]
#[path = "compiler_descriptor_entry_packing_resources_v1_tests.rs"]
mod resource_tests;

#[cfg(test)]
#[path = "compiler_descriptor_entry_packing_multi_v1_tests.rs"]
pub(crate) mod multi_tests;

#[cfg(test)]
#[path = "compiler_descriptor_entry_packing_scalars_v1_tests.rs"]
pub(crate) mod scalar_tests;
