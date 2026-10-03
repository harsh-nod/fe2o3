//! Typed storage transport operations. Physical/source validity stays owner-bound.
use crate::{
    AddressSpaceAttr, TargetNeutralGpuOpInterface,
    optimization_v1::{AccessModeAttr, IndexType, PointerType},
    storage_types_v18::{StorageObjectTypeV18, StorageOrdinalAttrV18, StorageTableKeyAttrV18},
};
use pliron::{
    builtin::{
        op_interfaces::NRegionsInterface,
        types::{IntegerType, Signedness},
    },
    common_traits::Verify,
    context::Context,
    derive::{op_interface_impl, pliron_attr, pliron_op},
    op::Op,
    operation::Operation,
    opts::dce::SideEffects,
    result::Result,
    r#type::{TypeHandle, Typed},
    value::Value,
    verify_err,
};

#[pliron_attr(name = "gpu.storage_kind_v18", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StorageKindAttrV18 {
    ProjectField,
    ProjectArray,
    ProjectVariant,
    VariantForWrite,
    ReadValue,
    WriteValue,
    CopyObject,
    SetDiscriminant,
    ReadDiscriminant,
}

#[pliron_attr(name = "gpu.storage_overlap_v18", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum StorageOverlapAttrV18 {
    None,
    NonOverlapping,
    MayOverlap,
}

#[pliron_attr(
    name = "gpu.storage_access_v18",
    format = "`<` $present `,` $space `,` $alignment `,` $volatile `>`"
)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StorageAccessAttrV18 {
    present: bool,
    space: AddressSpaceAttr,
    alignment: u32,
    volatile: bool,
}
impl StorageAccessAttrV18 {
    pub const ABSENT: Self = Self {
        present: false,
        space: AddressSpaceAttr::Private,
        alignment: 0,
        volatile: false,
    };
    pub const fn new(space: AddressSpaceAttr, alignment: u32, volatile: bool) -> Self {
        Self {
            present: true,
            space,
            alignment,
            volatile,
        }
    }
    pub const fn present(self) -> bool {
        self.present
    }
    pub const fn space(self) -> AddressSpaceAttr {
        self.space
    }
    pub const fn alignment(self) -> u32 {
        self.alignment
    }
    pub const fn volatile(self) -> bool {
        self.volatile
    }
}
impl Verify for StorageAccessAttrV18 {
    fn verify(&self, _context: &Context) -> Result<()> {
        let valid = if self.present {
            self.alignment != 0 && self.alignment.is_power_of_two()
        } else {
            *self == Self::ABSENT
        };
        if !valid {
            return verify_err!(
                pliron::location::Location::Unknown,
                "storage access must be aligned or canonically absent"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageDescriptorV18 {
    pub kind: StorageKindAttrV18,
    pub selector: u32,
    pub overlap: StorageOverlapAttrV18,
    pub read: StorageAccessAttrV18,
    pub write: StorageAccessAttrV18,
}

/// All carriers are conservative SideEffects, even pure address projections.
/// No generic pass may erase a storage obligation using an incomplete model.
#[pliron_op(
    name = "gpu.storage_v18",
    format,
    interfaces = [TargetNeutralGpuOpInterface, NRegionsInterface<0>],
    attributes = (
        gpu_storage_kind_v18: StorageKindAttrV18,
        gpu_storage_selector_v18: StorageOrdinalAttrV18,
        gpu_storage_overlap_v18: StorageOverlapAttrV18,
        gpu_storage_read_v18: StorageAccessAttrV18,
        gpu_storage_write_v18: StorageAccessAttrV18
    )
)]
pub struct StorageOpV18;

impl StorageOpV18 {
    pub fn new(
        context: &mut Context,
        descriptor: StorageDescriptorV18,
        operands: Vec<Value>,
        results: Vec<TypeHandle>,
    ) -> Self {
        let operation = Self {
            op: Operation::new(
                context,
                Self::get_concrete_op_info(),
                results,
                operands,
                vec![],
                0,
            ),
        };
        operation.set_attr_gpu_storage_kind_v18(context, descriptor.kind);
        operation
            .set_attr_gpu_storage_selector_v18(context, StorageOrdinalAttrV18(descriptor.selector));
        operation.set_attr_gpu_storage_overlap_v18(context, descriptor.overlap);
        operation.set_attr_gpu_storage_read_v18(context, descriptor.read);
        operation.set_attr_gpu_storage_write_v18(context, descriptor.write);
        operation
    }
    pub fn descriptor(&self, context: &Context) -> Option<StorageDescriptorV18> {
        Some(StorageDescriptorV18 {
            kind: *self.get_attr_gpu_storage_kind_v18(context)?,
            selector: self.get_attr_gpu_storage_selector_v18(context)?.0,
            overlap: *self.get_attr_gpu_storage_overlap_v18(context)?,
            read: *self.get_attr_gpu_storage_read_v18(context)?,
            write: *self.get_attr_gpu_storage_write_v18(context)?,
        })
    }
}

type PointerFacts = (
    StorageTableKeyAttrV18,
    u32,
    AddressSpaceAttr,
    AccessModeAttr,
);
fn pointer(context: &Context, ty: TypeHandle) -> Option<PointerFacts> {
    let raw = ty.deref(context);
    let pointer = raw.downcast_ref::<PointerType>()?;
    let pointee = pointer.pointee().deref(context);
    let storage = pointee.downcast_ref::<StorageObjectTypeV18>()?;
    Some((
        storage.table(),
        storage.row(),
        pointer.address_space(),
        pointer.access(),
    ))
}
fn permits(access: AccessModeAttr, write: bool) -> bool {
    if write {
        access != AccessModeAttr::ReadOnly
    } else {
        access != AccessModeAttr::WriteOnly
    }
}
fn restricted(before: AccessModeAttr, after: AccessModeAttr) -> bool {
    before == after || before == AccessModeAttr::ReadWrite
}
fn memory_matches(facts: PointerFacts, access: StorageAccessAttrV18, write: bool) -> bool {
    access.present
        && facts.2 == access.space
        && permits(facts.3, write)
        && (!write || facts.2 != AddressSpaceAttr::Constant)
}

impl Verify for StorageOpV18 {
    fn verify(&self, context: &Context) -> Result<()> {
        use StorageKindAttrV18::*;
        let Some(d) = self.descriptor(context) else {
            return verify_err!(
                self.loc(context),
                "storage carrier has incomplete typed fields"
            );
        };
        d.read.verify(context)?;
        d.write.verify(context)?;
        let (operands, results, read, write) = match d.kind {
            ProjectField | VariantForWrite => (1, 1, false, false),
            ProjectArray => (2, 1, false, false),
            ProjectVariant | ReadValue | ReadDiscriminant => (1, 1, true, false),
            WriteValue => (2, 0, false, true),
            CopyObject => (2, 0, true, true),
            SetDiscriminant => (1, 0, false, true),
        };
        let raw = self.get_operation().deref(context);
        let selector_used = matches!(
            d.kind,
            ProjectField | ProjectVariant | VariantForWrite | SetDiscriminant
        );
        if raw.get_num_operands() != operands
            || raw.get_num_results() != results
            || raw.get_num_successors() != 0
            || raw.num_regions() != 0
            || d.read.present != read
            || d.write.present != write
            || (!selector_used && d.selector != 0)
            || (d.kind == CopyObject) == (d.overlap == StorageOverlapAttrV18::None)
            || (d.kind == SetDiscriminant && d.write.volatile)
            || (d.kind == ReadDiscriminant && d.read.volatile)
        {
            return verify_err!(
                self.loc(context),
                "storage carrier shape or access roster is invalid"
            );
        }
        let Some(base) = pointer(context, raw.get_operand(0).get_type(context)) else {
            return verify_err!(
                self.loc(context),
                "storage operand requires an owner-keyed storage pointer"
            );
        };
        if read && !memory_matches(base, d.read, false) {
            return verify_err!(
                self.loc(context),
                "storage read violates holder space or rights"
            );
        }
        if d.kind == CopyObject {
            let Some(destination) = pointer(context, raw.get_operand(1).get_type(context)) else {
                return verify_err!(
                    self.loc(context),
                    "storage copy requires a storage destination"
                );
            };
            if base.0 != destination.0
                || base.1 != destination.1
                || !memory_matches(destination, d.write, true)
            {
                return verify_err!(
                    self.loc(context),
                    "storage copy requires one table/row and writable destination"
                );
            }
        } else if write && !memory_matches(base, d.write, true) {
            return verify_err!(
                self.loc(context),
                "storage write violates holder space or rights"
            );
        }
        if matches!(
            d.kind,
            ProjectField | ProjectArray | ProjectVariant | VariantForWrite
        ) {
            let Some(result) = pointer(context, raw.get_result(0).get_type(context)) else {
                return verify_err!(
                    self.loc(context),
                    "storage projection result must remain a storage pointer"
                );
            };
            let construction = d.kind == VariantForWrite;
            if result.0 != base.0
                || result.2 != base.2
                || !restricted(base.3, result.3)
                || (construction
                    && (result.3 != AccessModeAttr::WriteOnly
                        || !permits(base.3, true)
                        || base.2 == AddressSpaceAttr::Constant))
            {
                return verify_err!(
                    self.loc(context),
                    "storage projection changes its table, space or access rights"
                );
            }
            if d.kind == ProjectArray
                && !raw
                    .get_operand(1)
                    .get_type(context)
                    .deref(context)
                    .is::<IndexType>()
            {
                return verify_err!(self.loc(context), "storage array projection requires Index");
            }
        }
        if d.kind == ReadValue
            && raw
                .get_result(0)
                .get_type(context)
                .deref(context)
                .is::<StorageObjectTypeV18>()
            || d.kind == WriteValue
                && raw
                    .get_operand(1)
                    .get_type(context)
                    .deref(context)
                    .is::<StorageObjectTypeV18>()
        {
            return verify_err!(
                self.loc(context),
                "storage objects are not SSA payload values"
            );
        }
        if d.kind == ReadDiscriminant {
            let result = raw.get_result(0).get_type(context);
            let result = result.deref(context);
            if !result.downcast_ref::<IntegerType>().is_some_and(|integer| {
                integer.width() == 128 && integer.signedness() == Signedness::Unsigned
            }) {
                return verify_err!(
                    self.loc(context),
                    "discriminant read requires U128 raw bits"
                );
            }
        }
        // Exact row/field/variant and value-type checks require the owning table.
        Ok(())
    }
}

#[op_interface_impl]
impl SideEffects for StorageOpV18 {
    fn has_side_effects(&self, _context: &Context) -> bool {
        true
    }
}
