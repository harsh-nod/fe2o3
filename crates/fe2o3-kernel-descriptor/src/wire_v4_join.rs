//! Structural joins only. Source/graph/receipt authentication is owner work.
use crate::conditional_invocation::ConditionalArgumentRoleV1 as Role;
use crate::conditional_wire_common::{Contract, Error as JoinError, Result as JoinResult};
use crate::*;

pub(crate) struct ArgumentProjection {
    source_index: u16,
    source_type: RustTypeIdentity,
    device_layout: DeviceLayoutIdentity,
    descriptor: SourceTypeDescriptorV3,
    ownership: OwnershipSemantics,
    access: AccessMode,
    alias: AliasSemantics,
}
pub(crate) fn input_argument<E>(
    table: &DeviceDescriptorTableInputV3<'_>,
    kernel: &KernelDescriptorInputV3<'_>,
    field: usize,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> JoinResult<ArgumentProjection, E> {
    crate::nominal_v3::pay(c, 1)?;
    let a = kernel
        .arguments
        .get(field)
        .ok_or(JoinError::Invalid("generated field"))?;
    let mut found = None;
    for t in table.type_records {
        crate::nominal_v3::pay(c, 33)?;
        if t.identity() == a.source_type {
            found = Some(t.descriptor());
            break;
        }
    }
    Ok(ArgumentProjection {
        source_index: a.source_index,
        source_type: a.source_type,
        device_layout: a.device_layout,
        descriptor: found.ok_or(JoinError::Invalid("argument type"))?,
        ownership: a.ownership,
        access: a.access,
        alias: a.alias,
    })
}
pub(crate) fn view_argument<E>(
    table: &DeviceDescriptorTableV3<'_>,
    kernel: &KernelDescriptorRefV3<'_, '_>,
    field: usize,
    c: &mut impl FnMut(usize) -> Result<(), E>,
) -> JoinResult<ArgumentProjection, E> {
    let mut cursor = kernel.arguments();
    for i in 0..kernel.argument_count() {
        let a = cursor
            .next(c)?
            .ok_or(JoinError::Invalid("generated field"))?;
        if i == field {
            return Ok(ArgumentProjection {
                source_index: a.source_index(),
                source_type: a.source_type(),
                device_layout: a.device_layout(),
                descriptor: table.source_type(a.source_type(), c)?.descriptor(),
                ownership: a.ownership(),
                access: a.access(),
                alias: a.alias(),
            });
        }
    }
    Err(JoinError::Invalid("generated field"))
}
pub(crate) fn join<'wire, E, C: FnMut(usize) -> Result<(), E>>(
    id: KernelId,
    rank: u8,
    argument_count: usize,
    contract: &impl Contract<'wire>,
    mut argument: impl FnMut(usize, &mut C) -> JoinResult<ArgumentProjection, E>,
    c: &mut C,
) -> JoinResult<(), E> {
    crate::nominal_v3::pay(c, 33)?;
    if id.as_bytes() != contract.kernel_id() || rank != 1 {
        return Err(JoinError::Invalid("conditional kernel/D1 binding"));
    }
    let mut cursor = contract.arguments();
    while let Some(a) = cursor.next(c)? {
        if usize::from(a.generated_field) >= argument_count {
            return Err(JoinError::Invalid("generated field range"));
        }
        let row = argument(usize::from(a.generated_field), c)?;
        crate::nominal_v3::pay(c, 80)?;
        if u32::from(row.source_index) != a.source_argument
            || row.source_type.as_bytes() != &a.source_type_identity
            || row.device_layout.as_bytes() != &a.device_layout_identity
        {
            return Err(JoinError::Invalid("source/generated/type/layout binding"));
        }
        let kind = match a.role {
            Role::Input => {
                matches!(row.descriptor, SourceTypeDescriptorV3::SharedSlice(_))
                    && row.ownership == OwnershipSemantics::SharedBorrow
                    && row.access == AccessMode::ReadOnly
                    && row.alias == AliasSemantics::SharedReadOnly
            }
            Role::Output => {
                matches!(row.descriptor, SourceTypeDescriptorV3::DisjointSlice(_))
                    && row.ownership == OwnershipSemantics::UniqueBorrow
                    && matches!(row.access, AccessMode::WriteOnly | AccessMode::ReadWrite)
                    && row.alias == AliasSemantics::Exclusive
            }
        };
        if !kind {
            return Err(JoinError::Invalid("conditional slice/access/ownership"));
        }
    }
    let out = contract.output();
    let binding = contract.argument(usize::from(out.argument), c)?;
    let row = argument(usize::from(binding.generated_field), c)?;
    element_layout(row.descriptor, out.element_bytes, out.alignment)?;
    let mut cursor = contract.reads();
    while let Some(read) = cursor.next(c)? {
        let binding = contract.argument(usize::from(read.argument), c)?;
        let row = argument(usize::from(binding.generated_field), c)?;
        crate::nominal_v3::pay(c, 4)?;
        element_layout(row.descriptor, read.element_bytes, read.alignment)?;
    }
    Ok(())
}
fn element_layout<E>(
    kind: SourceTypeDescriptorV3,
    width: u64,
    alignment: u32,
) -> JoinResult<(), E> {
    let scalar = kind.physical_scalar();
    if width != u64::from(scalar.size_bytes()) || alignment > u32::from(scalar.alignment_bytes()) {
        return Err(JoinError::Invalid("conditional element layout"));
    }
    Ok(())
}
