//! Fixed-size same-owner LDS frame, completion, and publication projection.
//! This is conditional compiler evidence, not a captured or resumable LDS image.
use super::*;
use fe2o3_kernel_ir::{
    AddressSpace, FormalMemoryAccessKind, Gfx942PhysicalLdsExchangeFrameV1 as Frame,
    MemoryOrdering, PhysicalLdsExchangeMemoryObligationsV22 as Report, SynchronizationScope,
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LdsAccess {
    pub location: Location,
    pub site: Site,
    pub kind: FormalMemoryAccessKind,
    pub address: ValueId,
    pub value: ValueId,
    pub exec: ValueId,
    pub width: u32,
    pub alignment: u32,
    pub complete_at: Location,
    pub complete_site: Site,
}
impl From<fe2o3_kernel_ir::PhysicalLdsExchangeLdsAccessV22> for LdsAccess {
    fn from(row: fe2o3_kernel_ir::PhysicalLdsExchangeLdsAccessV22) -> Self {
        Self {
            location: row.location(),
            site: row.source_site(),
            kind: row.kind(),
            address: row.address(),
            value: row.value(),
            exec: row.exec(),
            width: row.byte_width(),
            alignment: row.alignment(),
            complete_at: row.complete_at(),
            complete_site: row.complete_source_site(),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Lds {
    pub declaration: Location,
    pub declaration_site: Site,
    pub frame: Frame,
    pub local_x: ValueId,
    pub peer_xor_mask: u32,
    pub byte_scale: u32,
    pub write: LdsAccess,
    pub read: LdsAccess,
    pub publication: Location,
    pub publication_site: Site,
    pub publication_epoch: u8,
    pub participant_count: u32,
    pub address_space: AddressSpace,
    pub memory_scope: SynchronizationScope,
    pub ordering: MemoryOrdering,
    pub completes_pending_accesses: bool,
    pub grants_global_happens_before: bool,
}
impl Lds {
    pub(super) fn from_report(report: &Report) -> Self {
        let frame = report.lds_frame();
        let publication = report.publication();
        Self {
            declaration: frame.declaration(),
            declaration_site: frame.source_site(),
            frame: frame.descriptor(),
            local_x: frame.local_x(),
            peer_xor_mask: frame.peer_xor_mask(),
            byte_scale: frame.byte_scale(),
            write: report.lds_write().into(),
            read: report.lds_read().into(),
            publication: publication.location(),
            publication_site: publication.source_site(),
            publication_epoch: publication.publication_epoch(),
            participant_count: publication.participant_count(),
            address_space: publication.address_space(),
            memory_scope: publication.memory_scope(),
            ordering: publication.ordering(),
            completes_pending_accesses: publication.completes_pending_accesses(),
            grants_global_happens_before: publication.grants_global_happens_before(),
        }
    }
    pub(super) fn is_exact_required_profile(self, report: &Report) -> bool {
        let block = self.declaration.block;
        self.frame.validate_shape().is_ok()
            && self.peer_xor_mask == 64
            && self.byte_scale == 4
            && self.write.kind == FormalMemoryAccessKind::Write
            && self.read.kind == FormalMemoryAccessKind::Read
            && self.write.width == 4
            && self.write.alignment == 4
            && self.read.width == 4
            && self.read.alignment == 4
            && self.write.value == report.input_read().result()
            && self.read.value == report.output_store().value()
            && self.write.exec == self.read.exec
            && self.write.exec == report.input_read().access().exec()
            && self.publication_epoch == self.frame.publication_epoch
            && self.participant_count == 128
            && self.address_space == AddressSpace::Workgroup
            && self.memory_scope == SynchronizationScope::Workgroup
            && self.ordering == MemoryOrdering::AcquireRelease
            && !self.completes_pending_accesses
            && !self.grants_global_happens_before
            && [
                self.write.location,
                self.write.complete_at,
                self.publication,
                self.read.location,
                self.read.complete_at,
                report.input_read().ready_at(),
                report.output_store().access().location(),
            ]
            .into_iter()
            .all(|location| location.block == block)
            && self.declaration.operation_index
                < report.input_read().access().location().operation_index
            && report.input_read().ready_at().operation_index < self.write.location.operation_index
            && self.write.complete_at.operation_index == self.write.location.operation_index + 1
            && self.write.complete_at.operation_index < self.publication.operation_index
            && self.publication.operation_index < self.read.location.operation_index
            && self.read.complete_at.operation_index == self.read.location.operation_index + 1
            && self.read.complete_at.operation_index
                < report.output_store().access().location().operation_index
    }
}
