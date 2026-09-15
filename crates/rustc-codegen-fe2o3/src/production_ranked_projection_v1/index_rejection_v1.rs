//! Rejection-only source coordinates from the existing index query, not a proof.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1;

mod enum_carrier_v1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RejectedUse {
    reason: &'static str,
    block: usize,
    statement: usize,
    local: Option<u32>,
    ty: Option<u32>,
    projections: [Option<SemanticProjectionV1>; 2],
    projection_count: usize,
}

#[derive(Default)]
pub(super) struct Trace {
    enabled: bool,
    first: Option<RejectedUse>,
    enum_carrier: Option<EnumCarrierUse>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EnumCarrierUse {
    block: usize,
    statement: usize,
    local: u32,
    variant: u32,
    carrier_type: u32,
    field: u32,
    field_type: u32,
}

impl EnumCarrierUse {
    fn from_rejection(record: RejectedUse) -> Option<Self> {
        if record.reason != "unsupported-projection" || record.projection_count != 2 {
            return None;
        }
        let [Some(downcast), Some(field)] = record.projections else {
            return None;
        };
        let (
            SemanticProjectionKindV1::Downcast(variant),
            SemanticProjectionKindV1::Field(field_id),
        ) = (downcast.kind(), field.kind())
        else {
            return None;
        };
        Some(Self {
            block: record.block,
            statement: record.statement,
            local: record.local?,
            variant,
            carrier_type: downcast.result_type().index(),
            field: field_id,
            field_type: field.result_type().index(),
        })
    }
}

impl Trace {
    fn note(&mut self, rejected: RejectedUse) {
        if self.enabled && self.enum_carrier.is_none() {
            self.enum_carrier = EnumCarrierUse::from_rejection(rejected);
        }
        if self.enabled && self.first.is_none() {
            self.first = Some(rejected);
        }
    }
}

pub(super) fn enabled(value: Option<&std::ffi::OsStr>) -> bool {
    value == Some(std::ffi::OsStr::new("1"))
}

impl TotalUnsignedIndexProjectorV1<'_, '_, '_> {
    pub(super) fn with_index_rejection_trace_v1(mut self, enabled: bool) -> Self {
        self.rejection_trace.enabled = enabled;
        self
    }

    pub(super) fn note_index_operand_v1(
        &mut self,
        reason: &'static str,
        operand: &SemanticOperandV1,
        block: usize,
        statement: usize,
    ) {
        if !self.rejection_trace.enabled
            || (self.rejection_trace.first.is_some() && self.rejection_trace.enum_carrier.is_some())
        {
            return;
        }
        let place = raw_operand_place(operand);
        self.rejection_trace.note(RejectedUse {
            reason,
            block,
            statement,
            local: place.map(|place| place.local().index()),
            ty: Some(operand.ty().index()),
            projections: [
                place.and_then(|place| place.projections().first().copied()),
                place.and_then(|place| place.projections().get(1).copied()),
            ],
            projection_count: place.map_or(0, |place| place.projections().len()),
        });
    }

    pub(super) fn note_index_local_v1(
        &mut self,
        reason: &'static str,
        local: usize,
        block: usize,
        statement: usize,
    ) {
        if !self.rejection_trace.enabled || self.rejection_trace.first.is_some() {
            return;
        }
        self.rejection_trace.note(RejectedUse {
            reason,
            block,
            statement,
            local: u32::try_from(local).ok(),
            ty: self
                .function
                .locals()
                .get(local)
                .map(|local| local.ty().index()),
            projections: [None; 2],
            projection_count: 0,
        });
    }

    pub(super) fn emit_index_rejection_v1(&self, store_block: usize) {
        // Diagnostic I/O must not panic or become an admission failure.
        let _ = self.write_index_rejection_v1(store_block, &mut std::io::stderr().lock());
    }

    pub(super) fn write_index_rejection_v1(
        &self,
        store_block: usize,
        out: &mut impl std::io::Write,
    ) -> std::io::Result<()> {
        let Some(record) = self.rejection_trace.first else {
            return Ok(());
        };
        let local = record.local.map(|local| local as usize);
        let definition = local.and_then(|local| self.definitions().get(local).copied().flatten());
        let source = self.function.blocks().get(record.block).and_then(|block| {
            if record.statement == block.statements().len() {
                Some(block.terminator().source())
            } else {
                block
                    .statements()
                    .get(record.statement)
                    .map(|statement| statement.source())
            }
        });
        write!(out, "capability-index-rejection body=")?;
        for byte in self.function.identity().as_bytes() {
            write!(out, "{byte:02x}")?;
        }
        writeln!(
            out,
            " store=bb{store_block}:terminator reason={} use=bb{}:s{} local={:?} type={:?} projections={:?} projection_count={} projections_truncated={} definitions={:?} definition={:?} escaped={:?} role={:?} source={source:?} node_work={}",
            record.reason,
            record.block,
            record.statement,
            record.local,
            record.ty,
            record.projections,
            record.projection_count,
            record.projection_count > 2,
            local.and_then(|local| self.local_definitions.get(local)),
            definition.map(|site| (site.block, site.statement)),
            local.and_then(|local| self.address_escaped().get(local)),
            local
                .and_then(|local| self.function.locals().get(local))
                .map(|local| local.role()),
            self.node_work,
        )?;
        if let Some(record) = self.rejection_trace.enum_carrier {
            enum_carrier_v1::write(self, record, out)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
