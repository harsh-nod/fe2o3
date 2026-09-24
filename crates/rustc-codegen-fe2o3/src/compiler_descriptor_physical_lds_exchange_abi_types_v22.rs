//! Fixed-size preparation facts copied only from the same checked source owner.
//! Copy here means retained conditional evidence, never runtime-binding authority.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Read {
    pub location: Location,
    pub site: Site,
    pub offset: u32,
    pub width: u32,
    pub alignment: u32,
    pub slot: Slot,
    pub base: [ValueId; 2],
    pub results: [ValueId; 2],
    pub ready_at: Location,
    pub ready_site: Site,
}
impl From<fe2o3_kernel_ir::PhysicalLdsExchangeKernargReadV22> for Read {
    fn from(row: fe2o3_kernel_ir::PhysicalLdsExchangeKernargReadV22) -> Self {
        Self {
            location: row.location(),
            site: row.source_site(),
            offset: row.byte_offset(),
            width: row.byte_width(),
            alignment: row.alignment(),
            slot: row.slot(),
            base: row.base(),
            results: row.results(),
            ready_at: row.ready_at(),
            ready_site: row.ready_source_site(),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Access {
    pub location: Location,
    pub site: Site,
    pub allocation: fe2o3_kernel_ir::FormalAllocationIdentity,
    pub parameter: ValueId,
    pub address: [ValueId; 2],
    pub index: [ValueId; 2],
    pub exec: ValueId,
}
impl From<fe2o3_kernel_ir::PhysicalLdsExchangeAccessV22> for Access {
    fn from(row: fe2o3_kernel_ir::PhysicalLdsExchangeAccessV22) -> Self {
        Self {
            location: row.location(),
            site: row.source_site(),
            allocation: row.allocation(),
            parameter: row.parameter(),
            address: row.address(),
            index: row.index(),
            exec: row.exec(),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GlobalRead {
    pub access: Access,
    pub result: ValueId,
    pub ready_at: Location,
    pub ready_site: Site,
}
impl From<fe2o3_kernel_ir::PhysicalLdsExchangeReadV22> for GlobalRead {
    fn from(row: fe2o3_kernel_ir::PhysicalLdsExchangeReadV22) -> Self {
        Self {
            access: row.access().into(),
            result: row.result(),
            ready_at: row.ready_at(),
            ready_site: row.ready_source_site(),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GlobalStore {
    pub access: Access,
    pub value: ValueId,
    pub mask_at: Location,
    pub mask_site: Site,
    pub comparison_at: Location,
    pub comparison_site: Site,
    pub length: [ValueId; 2],
    pub ready_at: Location,
    pub ready_site: Site,
    pub restore_at: Location,
    pub restore_site: Site,
}
impl From<fe2o3_kernel_ir::PhysicalLdsExchangeStoreV22> for GlobalStore {
    fn from(row: fe2o3_kernel_ir::PhysicalLdsExchangeStoreV22) -> Self {
        Self {
            access: row.access().into(),
            value: row.value(),
            mask_at: row.mask_at(),
            mask_site: row.mask_source_site(),
            comparison_at: row.comparison_at(),
            comparison_site: row.comparison_source_site(),
            length: row.length(),
            ready_at: row.ready_at(),
            ready_site: row.ready_source_site(),
            restore_at: row.restore_at(),
            restore_site: row.restore_source_site(),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Conditions {
    pub input: fe2o3_kernel_ir::FormalAllocationIdentity,
    pub output: fe2o3_kernel_ir::FormalAllocationIdentity,
    pub minimum_input_bytes: u64,
    pub minimum_output_bytes: u64,
    pub input_readable: bool,
    pub input_initialized: bool,
    pub output_writable: bool,
    pub input_output_disjoint: bool,
    pub kernarg_bytes: u32,
    pub kernarg_alignment: u32,
    pub kernarg_disjoint_output: fe2o3_kernel_ir::FormalAllocationIdentity,
    pub kernarg_readable: bool,
    pub kernarg_live: bool,
    pub kernarg_immutable: bool,
    pub workgroup: [u32; 3],
    pub workgroups: [u32; 3],
    pub all_workgroup_invocations: bool,
}
impl Conditions {
    pub(super) fn from_report(
        report: &fe2o3_kernel_ir::PhysicalLdsExchangeMemoryObligationsV22,
    ) -> Self {
        let runtime = report.runtime_requirements();
        let abi = report.kernarg_abi();
        Self {
            input: runtime.input(),
            output: runtime.output(),
            minimum_input_bytes: runtime.minimum_input_bytes(),
            minimum_output_bytes: runtime.minimum_output_bytes(),
            input_readable: runtime.requires_input_readable(),
            input_initialized: runtime.requires_input_initialized(),
            output_writable: runtime.requires_output_writable(),
            input_output_disjoint: runtime.requires_input_output_disjoint(),
            kernarg_bytes: abi.minimum_bytes(),
            kernarg_alignment: abi.alignment(),
            kernarg_disjoint_output: abi.disjoint_output(),
            kernarg_readable: abi.requires_readable_kernarg(),
            kernarg_live: abi.requires_live_kernarg(),
            kernarg_immutable: abi.requires_immutable_kernarg(),
            workgroup: runtime.required_workgroup(),
            workgroups: runtime.required_workgroups(),
            all_workgroup_invocations: runtime.requires_all_workgroup_invocations(),
        }
    }
    pub(super) fn is_exact_required_profile(self) -> bool {
        self.input.parameter_index() == 0
            && self.output.parameter_index() == 1
            && self.minimum_input_bytes == 512
            && self.minimum_output_bytes == 512
            && self.input_readable
            && self.input_initialized
            && self.output_writable
            && self.input_output_disjoint
            && self.kernarg_bytes == 32
            && self.kernarg_alignment == 8
            && self.kernarg_disjoint_output == self.output
            && self.kernarg_readable
            && self.kernarg_live
            && self.kernarg_immutable
            && self.workgroup == [128, 1, 1]
            && self.workgroups == [1, 1, 1]
            && self.all_workgroup_invocations
    }
}
