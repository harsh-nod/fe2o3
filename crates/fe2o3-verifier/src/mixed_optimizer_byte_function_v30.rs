//! Shared actual-inventory byte dispatch. Allocation labels come only from a
//! retained source/endpoint resolver; static MIR frames are not allocations.
use super::byte_memory_v30::{ByteMemoryContextNamesV30, ByteMemoryStateNamesV30};
use super::index_byte_operations_v37::IndexByteOperationV37;
use super::original_scalar_v30::CanonicalByteScalarV30;
use super::pointer_byte_operations_v30::{
    PointerByteEffectV30, PointerByteOperationV30, pointer_space, scalar_bytes,
};
use super::private_byte_operations_v30::AllocaByteOperationV30;
use super::storage_byte_operations_v37::StorageByteOperationV37;
use super::{Error, Inventory, Resource, Result, Writer, block_index};
use fe2o3_kernel_analysis::CanonicalKirPrivateByteAnalysisV38 as Physical;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirFunctionCoordinateV1 as Function,
    CanonicalKirOperationCoordinateV1 as Operation, CanonicalKirUseCoordinateV1 as Use,
    FormalIndexWidth, KirLocalMemoryEffectRefV1 as Effect, OperationKind, ScalarType, Terminator,
    Type, VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::{cell::Cell, fmt::Write as _, mem::size_of, ops::Range};

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

#[path = "mixed_optimizer_byte_control_v30.rs"]
mod control;

#[path = "mixed_optimizer_private_byte_obligations_v38.rs"]
mod physical;

#[path = "mixed_optimizer_byte_interpretation_context_v39.rs"]
mod interpretation;
pub(crate) use interpretation::ByteInterpretationContextV39;

#[path = "mixed_optimizer_storage_view_byte_operations_v39.rs"]
mod views;
use views::StorageViewByteOperationV39;

#[path = "mixed_optimizer_integral_byte_casts_v40.rs"]
mod integral;
use integral::IntegralByteCastV40;

#[path = "mixed_optimizer_checked_byte_operations_v48.rs"]
mod checked;
use checked::CheckedByteOperationV48;

#[path = "mixed_optimizer_byte_trap_v40.rs"]
mod trap;
use trap::TrapByteOperationV40;

/// The original canonical Alloca occurrence and its physical root's original
/// MIR declaration. A declaring callee is deliberately not the physical owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ByteAllocationSiteV30 {
    pub original: Operation,
    pub physical_root_owner: usize,
}

/// Internal integration interface, implemented by owner-bound retained source
/// or aggregate-endpoint indexes. The dispatcher keeps this resolver borrowed
/// through emission and checks its owner before using any cached site label.
pub(super) trait ByteAllocationResolverV30 {
    fn check_owner(&self, owner: &Owner, out: &mut Writer<'_, '_>) -> Result<()>;

    fn site(&self, operation: Operation, out: &mut Writer<'_, '_>)
    -> Result<ByteAllocationSiteV30>;
}

enum ByteOperationV30<'inventory, 'owner> {
    Pointer(PointerByteOperationV30),
    Alloca(AllocaByteOperationV30),
    Storage(StorageByteOperationV37),
    View(StorageViewByteOperationV39),
    IntegralCast(IntegralByteCastV40),
    Checked(CheckedByteOperationV48),
    Index(IndexByteOperationV37),
    Scalar(CanonicalByteScalarV30<'inventory, 'owner>),
    Trap(TrapByteOperationV40),
}

/// Complete supported operation census over the exact retained Inventory.
pub(super) struct ByteFunctionV30<'a, 'owner, R> {
    inventory: &'a Inventory<'owner>,
    // Keep the exact analyzed inventory borrowed through emission.
    _physical: &'a Physical<'a, 'owner>,
    allocations: &'a R,
    function: Function,
    operations: Vec<ByteOperationV30<'a, 'owner>>,
    interpretation: ByteInterpretationContextV39<'a, 'owner>,
    required: usize,
    slot: usize,
    ledger: Ledger,
    failure: Cell<Option<Resource>>,
}

fn mismatch() -> Error {
    Error::Statement("actual byte function owner or occurrence differs")
}

fn vector<T>(count: usize, out: &mut Writer<'_, '_>) -> Result<Vec<T>> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    out.budget.reserve_storage(bytes)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    out.budget.reserve_storage(
        (rows.capacity() - count)
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(rows)
}

pub(super) fn value_type(ty: &Type) -> Result<()> {
    match ty {
        Type::Unit | Type::Scalar(_) => Ok(()),
        Type::Vector(vector) => vector
            .validate()
            .map_err(|_| Error::Statement("actual byte vector type")),
        Type::Pointer(pointer) => pointer_space(pointer.address_space).map(|_| ()),
        Type::Slice(slice) if matches!(slice.element.as_ref(), Type::Scalar(_)) => {
            pointer_space(slice.address_space).map(|_| ())
        }
        _ => Err(Error::Statement("actual byte value type is not modeled")),
    }
}

pub(super) fn emit_value_type(
    ty: &Type,
    width: FormalIndexWidth,
    value: &str,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    match ty {
        Type::Unit => emit!(out, "matches!({value}, MemoryValueV30::Unit)"),
        Type::Scalar(scalar) => {
            emit!(
                out,
                "match {value} {{ MemoryValueV30::Scalar(v) => 0 <= v < "
            );
            if *scalar == ScalarType::Bool {
                emit!(out, "2");
            } else {
                emit!(
                    out,
                    "memory_value_modulus_v30({})",
                    scalar_bytes(*scalar, width)?
                );
            }
            emit!(out, ", _ => false }}");
        }
        Type::Vector(vector) => emit!(
            out,
            "match {value} {{ MemoryValueV30::Vector(v) => v.len() == {} && (forall|lane: int| 0 <= lane < v.len() ==> 0 <= v[lane] < memory_value_modulus_v30({})), _ => false }}",
            vector.lanes,
            scalar_bytes(vector.element, width)?
        ),
        Type::Pointer(pointer) => emit!(
            out,
            "match {value} {{ MemoryValueV30::Pointer(p) => byte_pointer_type_v30(p, {}, {}), _ => false }}",
            pointer_space(pointer.address_space)?,
            scalar_bytes(ScalarType::Index, width)?
        ),
        Type::Slice(slice) => emit!(
            out,
            "match {value} {{ MemoryValueV30::Slice(slice) => 0 <= slice.length < memory_value_modulus_v30({}) && byte_pointer_type_v30(slice.pointer, {}, {}), _ => false }}",
            scalar_bytes(ScalarType::Index, width)?,
            pointer_space(slice.address_space)?,
            scalar_bytes(ScalarType::Index, width)?,
        ),
        _ => return Err(Error::Statement("actual byte value type is not modeled")),
    }
    Ok(())
}

impl<'a, 'owner, R: ByteAllocationResolverV30> ByteFunctionV30<'a, 'owner, R> {
    pub(super) fn derive(
        inventory: &'a Inventory<'owner>,
        physical: &'a Physical<'a, 'owner>,
        function: Function,
        interpretation: ByteInterpretationContextV39<'a, 'owner>,
        allocations: &'a R,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        interpretation.check_owner(inventory.owner(), out)?;
        allocations.check_owner(inventory.owner(), out)?;
        let width = interpretation.width;
        out.budget.reserve_storage(headers::<R>())?;
        out.budget.charge_work(4)?;
        if !physical.is_for(inventory) {
            return Err(mismatch());
        }
        scalar_bytes(ScalarType::Index, width)?;
        let row = inventory
            .functions()
            .get(function.0 as usize)
            .ok_or_else(mismatch)?;
        if row.coordinate != function || row.blocks.is_empty() {
            return Err(mismatch());
        }
        for definition in &inventory.definitions()[row.definitions.clone()] {
            out.budget.charge_work(1)?;
            value_type(definition.ty)?;
        }
        let mut operations = vector(row.operations.len(), out)?;
        for operation in row.operations.clone() {
            out.budget.charge_work(4)?;
            let actual = &inventory.operations()[operation];
            if actual.coordinate.block.function != function {
                return Err(mismatch());
            }
            let plan = if let Some(alloca) =
                AllocaByteOperationV30::derive(inventory, operation, width, allocations, out)?
            {
                ByteOperationV30::Alloca(alloca)
            } else if let Some(storage) =
                StorageByteOperationV37::derive(inventory, operation, width, out)?
            {
                ByteOperationV30::Storage(storage)
            } else if matches!(actual.operation.kind, OperationKind::Storage(_)) {
                ByteOperationV30::View(StorageViewByteOperationV39::derive(
                    inventory,
                    operation,
                    interpretation,
                    out,
                )?)
            } else if let Some(pointer) =
                PointerByteOperationV30::derive(inventory, operation, width, out)?
            {
                ByteOperationV30::Pointer(pointer)
            } else if matches!(actual.operation.kind, OperationKind::Cast { .. }) {
                ByteOperationV30::IntegralCast(IntegralByteCastV40::derive(
                    inventory, operation, width, out,
                )?)
            } else if matches!(
                actual.operation.kind,
                OperationKind::Binary {
                    op: fe2o3_kernel_ir::BinaryOp::Checked(_),
                    ..
                }
            ) {
                ByteOperationV30::Checked(CheckedByteOperationV48::derive(
                    inventory, operation, width, out,
                )?)
            } else if let Some(index) =
                IndexByteOperationV37::derive(inventory, operation, width, out)?
            {
                ByteOperationV30::Index(index)
            } else if let Some(trap) = TrapByteOperationV40::derive(inventory, operation, out)? {
                ByteOperationV30::Trap(trap)
            } else {
                ByteOperationV30::Scalar(CanonicalByteScalarV30::derive(
                    inventory, operation, width, out,
                )?)
            };
            physical::check(physical, operation, &plan, &actual.operation.kind, out)?;
            // This census includes unused results and unreachable blocks.
            if let OperationKind::Storage(storage) = actual.operation.kind {
                let mut ordinal = 0usize;
                storage.try_visit_memory_accesses(|_, access, kind| {
                    out.budget.charge_work(3)?;
                    let effect = inventory
                        .effects()
                        .get(actual.effects.start + ordinal)
                        .filter(|_| ordinal < actual.effects.len())
                        .ok_or_else(mismatch)?;
                    let matches = match (kind, effect.effect) {
                        (fe2o3_kernel_ir::StorageMemoryAccessKindV1::Read, Effect::Read(space))
                        | (
                            fe2o3_kernel_ir::StorageMemoryAccessKindV1::Write,
                            Effect::Write(space),
                        ) => space == access.address_space && !access.volatile,
                        _ => false,
                    };
                    if !matches {
                        return Err(mismatch());
                    }
                    ordinal = ordinal.checked_add(1).ok_or(Resource::Arithmetic)?;
                    Ok(())
                })?;
                if ordinal != actual.effects.len() {
                    return Err(mismatch());
                }
            } else {
                let expected = match &actual.operation.kind {
                    OperationKind::Alloca { address_space, .. } => Some((0, *address_space)),
                    OperationKind::Load { access, .. } => Some((1, access.address_space)),
                    OperationKind::Store { access, .. } => Some((2, access.address_space)),
                    _ => None,
                };
                if actual.effects.len() != usize::from(expected.is_some()) {
                    return Err(mismatch());
                }
                for effect in &inventory.effects()[actual.effects.clone()] {
                    out.budget.charge_work(2)?;
                    let found = match effect.effect {
                        Effect::Allocate(space) => Some((0, space)),
                        Effect::Read(space) => Some((1, space)),
                        Effect::Write(space) => Some((2, space)),
                        _ => None,
                    };
                    if expected != found {
                        return Err(mismatch());
                    }
                }
            }
            operations.push(plan);
        }
        for block in row.blocks.clone() {
            control::check(inventory, block, out)?;
        }
        Ok(Self {
            inventory,
            _physical: physical,
            allocations,
            function,
            operations,
            interpretation,
            required: out.budget.storage(),
            slot: std::ptr::from_ref(&*out.budget) as usize,
            ledger: out.budget.work_ledger_identity_v1(),
            failure: Cell::new(None),
        })
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        if let Some(error) = self.failure.get() {
            return Err(error.into());
        }
        if self.slot != std::ptr::from_ref(&*out.budget) as usize
            || self.ledger != out.budget.work_ledger_identity_v1()
            || out.budget.storage() < self.required
        {
            self.failure.set(Some(Resource::Accounting));
            return Err(Resource::Accounting.into());
        }
        let result = self
            .allocations
            .check_owner(self.inventory.owner(), out)
            .and_then(|()| self.interpretation.check_owner(self.inventory.owner(), out));
        if let Err(Error::Resource(error)) = &result {
            self.failure.set(Some(*error));
        }
        result
    }

    pub(super) fn operation_range(&self, out: &mut Writer<'_, '_>) -> Result<Range<usize>> {
        self.check(out)?;
        Ok(self.inventory.functions()[self.function.0 as usize]
            .operations
            .clone())
    }

    pub(super) fn operation(
        &self,
        operation: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'owner>> {
        self.check(out)?;
        if let Err(error) = out.budget.charge_work(1) {
            self.failure.set(Some(error));
            return Err(error.into());
        }
        if !self.inventory.functions()[self.function.0 as usize]
            .operations
            .contains(&operation)
        {
            return Err(mismatch());
        }
        self.inventory
            .operations()
            .get(operation)
            .ok_or_else(mismatch)
    }

    pub(super) fn emit(&self, namespace: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        let result = self.emit_checked(namespace, out);
        if let Err(Error::Resource(error)) = &result {
            self.failure.set(Some(*error));
        }
        result
    }

    fn emit_checked(&self, namespace: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        let function = &self.inventory.functions()[self.function.0 as usize];
        for plan in &self.operations {
            out.budget.charge_work(1)?;
            if let ByteOperationV30::Scalar(scalar) = plan {
                scalar.emit_definition(namespace, out)?;
            }
        }
        for (offset, plan) in self.operations.iter().enumerate() {
            out.budget.charge_work(2)?;
            let operation = function
                .operations
                .start
                .checked_add(offset)
                .ok_or(Resource::Arithmetic)?;
            self.emit_operation(namespace, operation, plan, out)?;
        }
        for block in function.blocks.clone() {
            control::emit_block(self, namespace, block, out)?;
        }
        emit!(
            out,
            "open spec fn byte_micro_begin_{namespace}_v30(s: MemoryStateV30) -> MemoryMicroStateV30 {{\n"
        );
        for block in function.blocks.clone() {
            out.budget.charge_work(1)?;
            let operations = &self.inventory.blocks()[block].operations;
            emit!(
                out,
                " if s.pc == {block} {{ MemoryMicroStateV30 {{ state: s, observations: Seq::empty(), next_operation: "
            );
            if operations.is_empty() {
                emit!(out, "-1");
            } else {
                emit!(out, "{}", operations.start);
            }
            emit!(out, " }} }} else\n");
        }
        emit!(
            out,
            " {{ MemoryMicroStateV30 {{ state: MemoryStateV30 {{ valid: false, ..s }}, observations: Seq::empty(), next_operation: -1 }} }}\n}}\n"
        );
        emit!(
            out,
            "open spec fn byte_micro_step_{namespace}_v30(m: MemoryMicroStateV30, little_endian: bool) -> MemoryMicroResultV30 {{\n"
        );
        for operation in function.operations.clone() {
            out.budget.charge_work(2)?;
            let block = block_index(
                self.inventory,
                self.inventory.operations()[operation].coordinate.block,
            )?;
            let next = operation.checked_add(1).ok_or(Resource::Arithmetic)?;
            let prefix = operation
                .checked_sub(self.inventory.blocks()[block].operations.start)
                .ok_or(Resource::Arithmetic)?;
            emit!(
                out,
                " if m.state.pc == {block} && m.next_operation == {operation} && m.observations.len() == {prefix} {{ let result = byte_operation_{namespace}_{operation}_v30(m.state, little_endian); MemoryMicroResultV30 {{ next: MemoryMicroStateV30 {{ state: result.state, observations: m.observations.push(result.observation), next_operation: "
            );
            if next == self.inventory.blocks()[block].operations.end {
                emit!(out, "-1");
            } else {
                emit!(out, "{next}");
            }
            emit!(out, " }}, observation: result.observation }} }} else\n");
        }
        emit!(
            out,
            " {{ let state = MemoryStateV30 {{ valid: false, ..m.state }}; let observation = MemoryOperationObservationV30 {{ operation: MemorySourceOperationV30 {{ function: -1, block: -1, operation: -1 }}, before: m.state, after: state, valid_before: m.state.valid, valid_after: false, effect: MemoryOperationEffectV30::Refused }}; MemoryMicroResultV30 {{ next: MemoryMicroStateV30 {{ state, observations: m.observations.push(observation), next_operation: -1 }}, observation }} }}\n}}\n"
        );
        emit!(
            out,
            "open spec fn byte_micro_finish_{namespace}_v30(m: MemoryMicroStateV30) -> MemoryBlockResultV30 {{\n"
        );
        for block in function.blocks.clone() {
            out.budget.charge_work(1)?;
            emit!(
                out,
                " if m.next_operation == -1 && (m.state.pc == {block} || ("
            );
            self.emit_trap_terminal(block, "m.state", "m.observations", out)?;
            emit!(
                out,
                ")) && m.observations.len() == {} {{ byte_control_{namespace}_{block}_v30(m.state, m.observations) }} else\n",
                self.inventory.blocks()[block].operations.len()
            );
        }
        emit!(
            out,
            " {{ MemoryBlockResultV30 {{ state: MemoryStateV30 {{ valid: false, ..m.state }}, observations: m.observations, returned: Seq::empty() }} }}\n}}\n"
        );
        emit!(
            out,
            "open spec fn byte_block_step_{namespace}_v30(s: MemoryStateV30, little_endian: bool) -> MemoryBlockResultV30 {{\n"
        );
        for block in function.blocks.clone() {
            out.budget.charge_work(1)?;
            emit!(
                out,
                " if s.pc == {block} {{ byte_block_{namespace}_{block}_v30(s, little_endian) }} else\n"
            );
        }
        emit!(
            out,
            " {{ MemoryBlockResultV30 {{ state: MemoryStateV30 {{ valid: false, ..s }}, observations: Seq::empty(), returned: Seq::empty() }} }}\n}}\n"
        );
        self.check(out)
    }

    fn emit_operation(
        &self,
        namespace: usize,
        operation: usize,
        plan: &ByteOperationV30<'_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let row = &self.inventory.operations()[operation];
        let coordinate = row.coordinate;
        let block = block_index(self.inventory, coordinate.block)?;
        let definitions = self.inventory.definitions().len();
        emit!(
            out,
            "open spec fn byte_operation_{namespace}_{operation}_v30(s: MemoryStateV30, little_endian: bool) -> MemoryOperationResultV30 {{\n let operation = MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }};\n if s.values.len() != {definitions} || s.pc != {block} || !byte_state_memory_well_formed_v30(s) || !",
            coordinate.block.function.0,
            coordinate.block.block,
            coordinate.operation
        );
        self.interpretation.emit_state_predicate(
            "s.memory",
            "s.values",
            Some("little_endian"),
            out,
        )?;
        emit!(
            out,
            " {{ let state = MemoryStateV30 {{ valid: false, ..s }}; MemoryOperationResultV30 {{ state, observation: MemoryOperationObservationV30 {{ operation, before: s, after: state, valid_before: s.valid, valid_after: false, effect: MemoryOperationEffectV30::Refused }} }} }} else {{\n"
        );
        let before = ByteMemoryStateNamesV30 {
            values: "s.values",
            memory: "s.memory",
            generations: "s.generations",
            frames: "s.frames",
            valid: "s.valid",
        };
        let after = ByteMemoryStateNamesV30 {
            values: "values",
            memory: "memory",
            generations: "generations",
            frames: "frames",
            valid: "valid",
        };
        let effect = match plan {
            ByteOperationV30::Trap(trap) => {
                trap.emit_step(before, after, out)?;
                PointerByteEffectV30::None
            }
            ByteOperationV30::Scalar(scalar) => {
                scalar.emit_step(namespace, before, after, out)?;
                PointerByteEffectV30::None
            }
            ByteOperationV30::Index(index) => {
                index.emit_step(before, after, out)?;
                PointerByteEffectV30::None
            }
            ByteOperationV30::IntegralCast(cast) => {
                cast.emit_step(before, after, out)?;
                PointerByteEffectV30::None
            }
            ByteOperationV30::Checked(checked) => {
                checked.emit_step(before, after, out)?;
                PointerByteEffectV30::None
            }
            ByteOperationV30::Pointer(pointer) => {
                pointer.emit_step(
                    before,
                    after,
                    ByteMemoryContextNamesV30 {
                        owner: "0",
                        invocation: "0",
                        little_endian: "little_endian",
                    },
                    out,
                )?;
                pointer.effect()
            }
            ByteOperationV30::Alloca(alloca) => {
                alloca.emit_step(before, after, out)?;
                PointerByteEffectV30::None
            }
            ByteOperationV30::Storage(storage) => {
                storage.emit_step(
                    before,
                    after,
                    ByteMemoryContextNamesV30 {
                        owner: "0",
                        invocation: "0",
                        little_endian: "little_endian",
                    },
                    out,
                )?;
                storage.effect()
            }
            ByteOperationV30::View(view) => {
                view.emit_step(
                    before,
                    after,
                    ByteMemoryContextNamesV30 {
                        owner: "0",
                        invocation: "0",
                        little_endian: "little_endian",
                    },
                    out,
                )?;
                PointerByteEffectV30::None
            }
        };
        emit!(out, " let effect = ");
        if matches!(plan, ByteOperationV30::Trap(_)) {
            emit!(
                out,
                "if valid {{ MemoryOperationEffectV30::Trap }} else {{ MemoryOperationEffectV30::Refused }}"
            );
        } else if let ByteOperationV30::Alloca(alloca) = plan {
            emit!(
                out,
                "MemoryOperationEffectV30::Allocate {{ address: values[{}], extent: {}, alignment: {} }}",
                alloca.result,
                alloca.extent,
                alloca.alignment
            );
        } else if let ByteOperationV30::View(view) = plan {
            view.emit_effect("s.values", out)?;
        } else {
            match effect {
                PointerByteEffectV30::None => emit!(out, "MemoryOperationEffectV30::Pure"),
                PointerByteEffectV30::Copy {
                    source,
                    destination,
                    bytes,
                    source_alignment,
                    destination_alignment,
                    nonoverlapping,
                } => emit!(
                    out,
                    "MemoryOperationEffectV30::Copy {{ source: s.values[{source}], destination: s.values[{destination}], width: {bytes}, source_alignment: {source_alignment}, destination_alignment: {destination_alignment}, nonoverlapping: {nonoverlapping} }}"
                ),
                PointerByteEffectV30::Read {
                    pointer,
                    bytes,
                    alignment,
                    result,
                } => emit!(
                    out,
                    "MemoryOperationEffectV30::Read {{ address: s.values[{pointer}], width: {bytes}, alignment: {alignment}, value: values[{result}] }}"
                ),
                PointerByteEffectV30::Write {
                    pointer,
                    bytes,
                    alignment,
                    value,
                } => emit!(
                    out,
                    "MemoryOperationEffectV30::Write {{ address: s.values[{pointer}], width: {bytes}, alignment: {alignment}, value: s.values[{value}] }}"
                ),
            }
        }
        let pc = if matches!(plan, ByteOperationV30::Trap(_)) {
            "if valid { -2 } else { s.pc }"
        } else {
            "s.pc"
        };
        emit!(
            out,
            ";\n let state = MemoryStateV30 {{ pc: {pc}, values, memory, generations, frames, valid }}; MemoryOperationResultV30 {{ state, observation: MemoryOperationObservationV30 {{ operation, before: s, after: state, valid_before: s.valid, valid_after: valid, effect }} }}\n }}\n}}\n"
        );
        Ok(())
    }

    fn emit_trap_terminal(
        &self,
        block: usize,
        state: &str,
        observations: &str,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(1)?;
        let row = &self.inventory.blocks()[block];
        let start = self.inventory.functions()[self.function.0 as usize]
            .operations
            .start;
        let last = row
            .operations
            .end
            .checked_sub(1)
            .filter(|last| row.operations.contains(last));
        if let Some(operation) = last.filter(|last| {
            matches!(
                self.operations.get(last - start),
                Some(ByteOperationV30::Trap(_))
            )
        }) {
            let coordinate = self.inventory.operations()[operation].coordinate;
            emit!(
                out,
                "byte_trap_terminal_v40({state}, {observations}, MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }}, {block}, {})",
                coordinate.block.function.0,
                coordinate.block.block,
                coordinate.operation,
                row.operations.len()
            );
        } else {
            emit!(out, "false");
        }
        Ok(())
    }
}

fn headers<R>() -> usize {
    super::pointer_byte_operations_v30::headers()
        + super::private_byte_operations_v30::headers()
        + super::storage_byte_operations_v37::headers()
        + super::index_byte_operations_v37::headers()
        + control::headers()
        + physical::headers()
        + interpretation::headers()
        + views::headers()
        + integral::headers()
        + checked::headers()
        + trap::headers()
        + size_of::<ByteFunctionV30<'_, '_, R>>()
        + 2 * size_of::<Result<ByteFunctionV30<'_, '_, R>>>()
        + size_of::<ByteOperationV30<'_, '_>>()
        + 2 * size_of::<Result<Option<ByteOperationV30<'_, '_>>>>()
        + size_of::<(Result<()>, Option<Resource>)>()
        + size_of::<(
            [&Inventory<'_>; 3],
            [&R; 2],
            &mut Writer<'_, '_>,
            [usize; 32],
            [Range<usize>; 5],
            [Result<()>; 6],
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryStateNamesV30<'static>,
            ByteMemoryContextNamesV30<'static>,
            PointerByteEffectV30,
            [&Type; 4],
            [&str; 8],
            [Option<usize>; 3],
        )>()
}

#[cfg(test)]
#[path = "mixed_optimizer_byte_function_v30_tests.rs"]
mod tests;
