//! Value predicates at expanded endpoints, not a cut or step refinement proof.
//! The paired consumer binds `original`, `source`, `target` and the byte `map`.
use super::*;
use crate::mixed_optimizer_refinement_v26::semantics::byte_function_v30::emit_value_type;
use fe2o3_kernel_ir::FormalIndexWidth;
use std::fmt::Write as _;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

pub(super) fn headers() -> usize {
    2 * size_of::<Option<usize>>()
        + 3 * size_of::<usize>()
        + size_of::<FormalIndexWidth>()
        + 2 * size_of::<Result<()>>()
        + size_of::<&Type>()
}

impl ExpandedScalarBindingsV196<'_, '_, '_, '_> {
    pub(in super::super::super) fn emit_definition_relation(
        &self,
        original: usize,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            let actual = self.definition(original, out)?;
            self.emit_actual_relation(Some(actual), width, out)
        })
    }

    pub(in super::super::super) fn emit_tile_leaf_relation(
        &self,
        original: usize,
        path: &[u32],
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            let actual = self.tile_leaf(original, path, out)?;
            self.emit_actual_relation(actual, width, out)
        })
    }

    fn emit_actual_relation(
        &self,
        actual: Option<usize>,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        out.budget.charge_work(2)?;
        if width == FormalIndexWidth::Unknown {
            return Err(Error::Statement(
                "expanded value relation requires a known INDEX width",
            ));
        }
        emit!(
            out,
            "(target.values.len() == {} && ({{ ",
            self.actual_definitions
        );
        if let Some(index) = actual {
            let inventory = self.target.inventory(out)?;
            let row = inventory.definitions().get(index).ok_or_else(mismatch)?;
            emit!(out, "let actual = target.values[{index}]; ");
            emit_value_type(row.ty, width, "actual", out)?;
            emit!(
                out,
                " && invocation_value_related_v36(original, actual, map, source.machine.memory, target.memory)"
            );
        } else {
            emit!(out, "original == MemoryValueV30::Unit");
        }
        emit!(out, " }}))");
        Ok(())
    }
}
