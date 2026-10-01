//! Canonical INDEX intrinsics read only the invocation's explicit coordinates.
use super::byte_memory_v30::ByteMemoryStateNamesV30;
use super::pointer_byte_operations_v30::scalar_bytes;
use super::{Error, Inventory, Result, Writer};
use fe2o3_kernel_ir::{
    Axis, FormalIndexWidth, IndexKind, IntrinsicKind, OperationKind, ScalarType, Type,
};
use std::{fmt::Write as _, mem::size_of};

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

#[derive(Clone, Copy)]
pub(super) struct IndexByteOperationV37 {
    kind: IntrinsicKind,
    axis: usize,
    result: usize,
    bytes: usize,
}

impl IndexByteOperationV37 {
    pub(super) fn derive(
        inventory: &Inventory<'_>,
        operation: usize,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<Self>> {
        out.budget.charge_work(2)?;
        let row = inventory
            .operations()
            .get(operation)
            .ok_or(Error::Statement("byte INDEX operation coordinate"))?;
        let OperationKind::Intrinsic(intrinsic) = &row.operation.kind else {
            return Ok(None);
        };
        out.budget.charge_work(8)?;
        if !row.operands.is_empty()
            || !row.effects.is_empty()
            || row.results.len() != 1
            || intrinsic.result_type != Type::INDEX
            || inventory
                .definitions()
                .get(row.results.start)
                .map(|definition| definition.ty)
                != Some(&Type::INDEX)
        {
            return Err(Error::Statement("byte INDEX exact signature and effects"));
        }
        Ok(Some(Self {
            kind: intrinsic.kind,
            axis: match intrinsic.kind.axis() {
                Axis::X => 0,
                Axis::Y => 1,
                Axis::Z => 2,
            },
            result: row.results.start,
            bytes: scalar_bytes(ScalarType::Index, width)?,
        }))
    }

    fn emit_coordinate(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(1)?;
        let axis = self.axis;
        match self.kind {
            IntrinsicKind::InvocationIndex { kind, .. } => match kind {
                IndexKind::Global | IndexKind::Workgroup | IndexKind::Local => {
                    let hierarchy = match kind {
                        IndexKind::Global => 0,
                        IndexKind::Workgroup => 1,
                        IndexKind::Local => 2,
                        _ => unreachable!(),
                    };
                    emit!(
                        out,
                        "byte_execution_index_v37(execution, {hierarchy}, {axis})"
                    );
                }
                IndexKind::WorkgroupSize => emit!(out, "execution.workgroup[{axis}]"),
                IndexKind::WorkgroupCount => emit!(
                    out,
                    "((execution.extent[{axis}] - 1) / execution.workgroup[{axis}] + 1)"
                ),
            },
            IntrinsicKind::LaunchExtent { .. } => emit!(out, "execution.extent[{axis}]"),
        }
        Ok(())
    }

    pub(super) fn emit_step(
        &self,
        before: ByteMemoryStateNamesV30<'_>,
        after: ByteMemoryStateNamesV30<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " let {} = {} && match {}.execution {{ Some(execution) => byte_execution_well_formed_v37(execution) && {} < execution.rank && {{ let coordinate = ",
            after.valid,
            before.valid,
            before.frames,
            self.axis
        );
        self.emit_coordinate(out)?;
        emit!(
            out,
            "; 0 <= coordinate < memory_value_modulus_v30({}) }}, None => false }};\n let {} = {}.update({}, if {} {{ match {}.execution {{ Some(execution) => MemoryValueV30::Scalar(",
            self.bytes,
            after.values,
            before.values,
            self.result,
            after.valid,
            before.frames
        );
        self.emit_coordinate(out)?;
        emit!(
            out,
            "), None => MemoryValueV30::Undefined }} }} else {{ MemoryValueV30::Undefined }});\n let {} = {};\n let {} = {};\n let {} = {};\n",
            after.memory,
            before.memory,
            after.generations,
            before.generations,
            after.frames,
            before.frames
        );
        Ok(())
    }
}

pub(super) fn headers() -> usize {
    2 * size_of::<IndexByteOperationV37>()
        + 2 * size_of::<Result<Option<IndexByteOperationV37>>>()
        + 2 * size_of::<ByteMemoryStateNamesV30<'_>>()
        + size_of::<(
            FormalIndexWidth,
            [&(); 5],
            [usize; 4],
            IntrinsicKind,
            IndexKind,
        )>()
}
