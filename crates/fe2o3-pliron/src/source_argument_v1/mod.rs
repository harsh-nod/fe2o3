//! Source-owned argument correspondence, shared by emission and conditional replay.
//!
//! Trace rows are inert proposals. The checked relation is minted only by a
//! complete replay against the actual source owner and canonical module.

use crate::ProductionSemanticSsaOwnerV1;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, Function, FunctionId, Module, ScalarType, Type, ValueId,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use std::collections::BTreeSet;

mod trace_v1;
pub use trace_v1::*;
mod relation_v1;
#[cfg(test)]
mod view_tests;
pub use relation_v1::{
    ProductionSourceArgumentBindingV1, ProductionSourceArgumentRelationV1,
    charge_source_body_selection_v1,
};

#[derive(Debug)]
pub enum ProductionSourceArgumentErrorV1 {
    ArgumentCorrespondenceResource(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1),
    CorrespondenceMismatch,
    Unsupported {
        function: u32,
        block: Option<u32>,
        statement: Option<u32>,
        detail: &'static str,
    },
    ScalarTypeUnavailable {
        semantic_type: u32,
        shape: String,
    },
    AllocationFailure {
        resource: ProductionSourceArgumentResourceV1,
    },
    /// Private adapter convention: the caller retains its original visitor error.
    Visitor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSourceArgumentResourceV1 {
    DebugBindings,
}

impl std::fmt::Display for ProductionSourceArgumentErrorV1 {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "source argument correspondence: {self:?}")
    }
}
impl std::error::Error for ProductionSourceArgumentErrorV1 {}

const fn unsupported(
    function: u32,
    block: Option<u32>,
    statement: Option<u32>,
    detail: &'static str,
) -> ProductionSourceArgumentErrorV1 {
    ProductionSourceArgumentErrorV1::Unsupported {
        function,
        block,
        statement,
        detail,
    }
}

pub const MAX_SSA_VALUE_COMPONENTS_V1: usize = 256;
pub type ByValueKernelParameterComponentV1 = (
    Vec<SemanticKirParameterProjectionV1>,
    SemanticTypeIdV1,
    Type,
    u64,
    ParameterAbiLeafV1,
);

include!("correspondence_v1.rs");
include!("view_v1.rs");
include!("shapes_v1.rs");
include!("structure_v1.rs");
include!("representation_v1.rs");

pub mod complete_body_parameter_vnext;
pub mod physical_entry_parameter_v20;
pub mod physical_global_copy_parameter_v21;
pub mod physical_lds_exchange_parameter_v22;
