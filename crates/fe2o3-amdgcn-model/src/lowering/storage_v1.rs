//! Inert pointer-representation recipes for the existing LLVM emitter.
//!
//! This leaf does not authenticate a Module owner, source correspondence, loan,
//! allocation, initialization, lifetime, scope or bounds. Root's checked storage
//! operation hook must establish those joins before invoking this formatter.
//! The structural table API is pending root-owned export/profile integration.

use super::{LoweringTarget, MAX_COMPILER_MODULE_TEXT_BYTES};
use fe2o3_amd_target::{
    PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1, PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, StorageLayoutIdV1, StorageLayoutKindV1, StoragePointerV1,
    StructurallyCheckedStorageLayoutsV1,
};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StoragePointerEmissionErrorV1 {
    TargetProfile,
    DataLayout,
    TableIdentity,
    InvalidLayout,
    NotPointer,
    Representation,
    ZeroSizedReferent,
    PointerShape,
    OperandName,
    Output,
}

type ResultV1<T> = Result<T, StoragePointerEmissionErrorV1>;

pub(super) fn pointer_encoding(
    target: LoweringTarget,
    pointer: StoragePointerV1,
) -> ResultV1<fe2o3_amd_target::AmdPointerEncodingV1> {
    use fe2o3_amd_target::ProductionAmdTargetProfileV1;
    let profile = match target {
        LoweringTarget::Gfx942XnackMinusV1 => ProductionAmdTargetProfileV1::Gfx942,
        LoweringTarget::Gfx950XnackMinusV1 => ProductionAmdTargetProfileV1::Gfx950,
        _ => return Err(StoragePointerEmissionErrorV1::TargetProfile),
    };
    if pointer.value_space != pointer.encoded_space && pointer.encoded_space != AddressSpace::Generic {
        return Err(StoragePointerEmissionErrorV1::Representation);
    }
    if pointer.value_space == AddressSpace::Constant && pointer.access != AccessMode::ReadOnly {
        return Err(StoragePointerEmissionErrorV1::PointerShape);
    }
    let space = match pointer.encoded_space {
        AddressSpace::Generic => 0,
        AddressSpace::Global => 1,
        AddressSpace::Workgroup => 3,
        AddressSpace::Constant => 4,
        AddressSpace::Private => 5,
    };
    profile.pointer_encoding(space)
        .filter(|encoding| encoding.bits() == pointer.stored_bits)
        .ok_or(StoragePointerEmissionErrorV1::Representation)
}

/// A borrow of inert layout data and exact selected target facts, never a source
/// or executable owner. Equal row IDs in another context are not interchangeable.
#[derive(Debug)]
pub(super) struct StorageTargetContextV1<'table, 'rows> {
    layouts: &'table StructurallyCheckedStorageLayoutsV1<'rows>,
    target: LoweringTarget,
}

impl<'table, 'rows> StorageTargetContextV1<'table, 'rows> {
    pub(super) fn new(
        layouts: &'table StructurallyCheckedStorageLayoutsV1<'rows>,
        target: LoweringTarget,
        source_data_layout: &str,
        worker_data_layout: &str,
    ) -> ResultV1<Self> {
        if !matches!(
            target,
            LoweringTarget::Gfx942XnackMinusV1 | LoweringTarget::Gfx950XnackMinusV1
        ) {
            return Err(StoragePointerEmissionErrorV1::TargetProfile);
        }
        if source_data_layout != PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1
            || worker_data_layout != PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1
        {
            return Err(StoragePointerEmissionErrorV1::DataLayout);
        }
        Ok(Self { layouts, target })
    }

    pub(super) fn pointer_recipe(
        &self,
        layout: StorageLayoutIdV1,
    ) -> ResultV1<StoragePointerRecipeV1<'_, 'table, 'rows>> {
        let row = self
            .layouts
            .row(layout)
            .ok_or(StoragePointerEmissionErrorV1::InvalidLayout)?;
        let StorageLayoutKindV1::Pointer(pointer) = &row.kind else {
            return Err(StoragePointerEmissionErrorV1::NotPointer);
        };
        pointer_encoding(self.target, *pointer)?;
        let referent = self
            .layouts
            .row(pointer.pointee)
            .ok_or(StoragePointerEmissionErrorV1::InvalidLayout)?;
        if referent.size == 0 {
            return Err(StoragePointerEmissionErrorV1::ZeroSizedReferent);
        }
        let value = PointerWordV1::for_space(pointer.value_space);
        let stored = PointerWordV1::for_space(pointer.encoded_space);
        if pointer.value_space == AddressSpace::Constant && pointer.access != AccessMode::ReadOnly {
            return Err(StoragePointerEmissionErrorV1::PointerShape);
        }
        if pointer.value_space != pointer.encoded_space
            && pointer.encoded_space != AddressSpace::Generic
        {
            return Err(StoragePointerEmissionErrorV1::Representation);
        }
        if pointer.stored_bits != stored.bits
            || row.size != u64::from(stored.bits / 8)
            || row.alignment != u32::from(stored.bits / 8)
        {
            return Err(StoragePointerEmissionErrorV1::Representation);
        }
        Ok(StoragePointerRecipeV1 {
            context: self,
            layout,
            pointer: *pointer,
            value,
            stored,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PointerWordV1 {
    bits: u16,
    llvm: &'static str,
}

impl PointerWordV1 {
    fn for_space(space: AddressSpace) -> Self {
        match space {
            AddressSpace::Private => Self {
                bits: 32,
                llvm: "ptr addrspace(5)",
            },
            AddressSpace::Workgroup => Self {
                bits: 32,
                llvm: "ptr addrspace(3)",
            },
            AddressSpace::Global => Self {
                bits: 64,
                llvm: "ptr addrspace(1)",
            },
            AddressSpace::Constant => Self {
                bits: 64,
                llvm: "ptr addrspace(4)",
            },
            AddressSpace::Generic => Self {
                bits: 64,
                llvm: "ptr",
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StoragePointerDirectionV1 {
    ValueToStored,
    StoredToValue,
}

/// Inert shape copied from the current typed operation, not memory authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StoragePointerShapeV1 {
    pub(super) pointee: StorageLayoutIdV1,
    pub(super) space: AddressSpace,
    pub(super) access: AccessMode,
}

/// Describes an emitted textual operand, not a pointer/provenance capability.
#[derive(Debug, Eq, PartialEq)]
pub(super) struct StoragePointerTextOperandV1<'name> {
    name: &'name str,
    llvm_type: &'static str,
}

impl<'name> StoragePointerTextOperandV1<'name> {
    pub(super) fn name(&self) -> &'name str {
        self.name
    }

    pub(super) fn llvm_type(&self) -> &'static str {
        self.llvm_type
    }
}

#[derive(Debug)]
pub(super) struct StoragePointerRecipeV1<'context, 'table, 'rows> {
    context: &'context StorageTargetContextV1<'table, 'rows>,
    layout: StorageLayoutIdV1,
    pointer: StoragePointerV1,
    value: PointerWordV1,
    stored: PointerWordV1,
}

impl StoragePointerRecipeV1<'_, '_, '_> {
    /// Checks shape only. The caller must additionally join the actual current
    /// typed operation, exact owner/table, source validity and memory authority.
    pub(super) fn check_value_shape(&self, shape: StoragePointerShapeV1) -> ResultV1<()> {
        if shape.pointee != self.pointer.pointee
            || shape.space != self.pointer.value_space
            || shape.access != self.pointer.access
        {
            return Err(StoragePointerEmissionErrorV1::PointerShape);
        }
        Ok(())
    }

    fn check_current(&self, current: &StorageTargetContextV1<'_, '_>) -> ResultV1<()> {
        if !std::ptr::eq(self.context, current)
            || !std::ptr::eq(self.context.layouts, current.layouts)
            || self.context.target != current.target
        {
            return Err(StoragePointerEmissionErrorV1::TableIdentity);
        }
        let fresh = current.pointer_recipe(self.layout)?;
        if fresh.pointer != self.pointer || fresh.value != self.value || fresh.stored != self.stored
        {
            return Err(StoragePointerEmissionErrorV1::Representation);
        }
        Ok(())
    }

    /// Writes into the shared emitter's output (normally CapacityLimitedText).
    /// Identity returns the original name: the caller must alias its binding,
    /// not emit a same-address-space cast or manufacture a second SSA definition.
    ///
    /// StoredToValue is not a generic flat-pointer narrowing operation. Its
    /// production caller must have validated the complete typed pointer load and
    /// matching relocation/provenance; an aperture or these bytes are not proof.
    pub(super) fn emit<'name>(
        &self,
        current: &StorageTargetContextV1<'_, '_>,
        shape: StoragePointerShapeV1,
        direction: StoragePointerDirectionV1,
        source: &'name str,
        destination: &'name str,
        output: &mut dyn fmt::Write,
    ) -> ResultV1<StoragePointerTextOperandV1<'name>> {
        self.check_current(current)?;
        self.check_value_shape(shape)?;
        if !operand_name(source, true) || !operand_name(destination, false) {
            return Err(StoragePointerEmissionErrorV1::OperandName);
        }
        let (from, to) = match direction {
            StoragePointerDirectionV1::ValueToStored => (self.value, self.stored),
            StoragePointerDirectionV1::StoredToValue => (self.stored, self.value),
        };
        if from == to {
            return Ok(StoragePointerTextOperandV1 {
                name: source,
                llvm_type: to.llvm,
            });
        }
        if source == destination {
            return Err(StoragePointerEmissionErrorV1::OperandName);
        }
        // Leave the target's aperture and distinct-null lowering to LLVM. No
        // integer cast, arithmetic, nonnull promise or pointer bit padding.
        writeln!(
            output,
            "  {destination} = addrspacecast {} {source} to {}",
            from.llvm, to.llvm
        )
        .map_err(|_| StoragePointerEmissionErrorV1::Output)?;
        Ok(StoragePointerTextOperandV1 {
            name: destination,
            llvm_type: to.llvm,
        })
    }
}

fn operand_name(name: &str, allow_global: bool) -> bool {
    // FunctionLowerer uses named %v{id}/named aliases and named @LDS symbols.
    // Numeric LLVM SSA slots require function-wide numbering; this formatter
    // does not invent that state or accept arbitrary LLVM operand syntax.
    if name.len() < 2 || name.len() > MAX_COMPILER_MODULE_TEXT_BYTES {
        return false;
    }
    let bytes = name.as_bytes();
    if bytes[0] != b'%' && !(allow_global && bytes[0] == b'@') {
        return false;
    }
    let start = |byte: u8| byte.is_ascii_alphabetic() || matches!(byte, b'-' | b'$' | b'.' | b'_');
    start(bytes[1])
        && bytes[2..]
            .iter()
            .all(|&byte| start(byte) || byte.is_ascii_digit())
}

#[cfg(test)]
#[path = "storage_v1_tests.rs"]
mod tests;
