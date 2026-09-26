//! Resolves original deferred registration at the admitted source boundary.
//! The result is a source-owned ABI proposal, not final native ABI authority.

use super::{CompilerDescriptorError, DescriptorArgumentKindV1 as Kind, TypedDescriptorRootV1};
use crate::production_pipeline::ProductionPipelineError as Error;
use fe2o3_artifacts::{MAX_ABI_BYTES, MAX_ABI_FIELDS};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionKernelArgumentAbiArgumentV18 as Argument, ProductionSourceOwnedViewErrorV18,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticAbiArgumentRoleV1, SemanticAbiPassModeV1,
    SemanticFunctionDeclV1,
};

fn mismatch(field: &'static str) -> Error {
    Error::DescriptorEvidence(CompilerDescriptorError::ProductionDescriptorMismatch(field))
}

fn resource(error: Resource) -> Error {
    Error::SourceOwnedEntrance(ProductionSourceOwnedViewErrorV18::Resource(error))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Extent {
    pub(super) bytes: u32,
    pub(super) alignment: u32,
}

pub(super) struct RootArguments {
    pub(super) arguments: Vec<Argument>,
    pub(super) extent: Extent,
}

pub(super) struct Packing<'a> {
    root: &'a TypedDescriptorRootV1,
    semantic: &'a AdmittedInertSemanticMirV1,
    function: &'a SemanticFunctionDeclV1,
    deferred: bool,
    next: usize,
    end: u64,
    alignment: u32,
}

impl<'a> Packing<'a> {
    pub(super) fn new(
        root: &'a TypedDescriptorRootV1,
        semantic: &'a AdmittedInertSemanticMirV1,
        function: &'a SemanticFunctionDeclV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        // Prepay the complete bounded scan and cursor, including failure paths.
        let work = root
            .arguments
            .len()
            .checked_mul(12)
            .and_then(|work| work.checked_add(12))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.charge_work(work).map_err(resource)?;
        if root.arguments.len() > MAX_ABI_FIELDS {
            return Err(mismatch("inline ABI source argument count"));
        }
        super::validate_production_v1_semantic_root_ownership_evidence(root, semantic, function)
            .map_err(Error::DescriptorEvidence)?;
        let deferred = root
            .arguments
            .as_slice()
            .iter()
            .any(|argument| argument.kind == Kind::CompilerLaidOutByValue);
        if deferred {
            if root.explicit_argument_bytes != 0
                || root.kernarg_alignment_bytes != 1
                || root
                    .arguments
                    .as_slice()
                    .iter()
                    .any(|argument| argument.offset != 0)
            {
                return Err(mismatch("original deferred inline ABI sentinel"));
            }
        } else {
            super::laid_out_plan_v1::check(root).map_err(Error::DescriptorEvidence)?;
        }
        Ok(Self {
            root,
            semantic,
            function,
            deferred,
            next: 0,
            end: 0,
            alignment: 1,
        })
    }

    pub(super) fn offset(&mut self, ordinal: usize) -> Result<u32, Error> {
        if ordinal != self.next {
            return Err(mismatch("ordered original inline ABI cursor"));
        }
        let argument = self
            .root
            .arguments
            .as_slice()
            .get(ordinal)
            .ok_or_else(|| mismatch("original inline ABI ordinal"))?;
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        if !self.deferred {
            return Ok(argument.offset);
        }
        let abi = self
            .function
            .abi()
            .adjusted_arguments()
            .get(ordinal)
            .ok_or_else(|| mismatch("original inline source ABI"))?;
        let declaration = self
            .semantic
            .types()
            .get(abi.ty().index() as usize)
            .ok_or_else(|| mismatch("original inline source type"))?;
        if abi.role() != SemanticAbiArgumentRoleV1::Source
            || abi.value().adjusted().is_some()
            || abi.value().pointee_override().is_some()
            || declaration.layout().is_uninhabited()
            || (argument.kind == Kind::CompilerLaidOutByValue
                && argument.access != super::AccessMode::ByValue)
        {
            return Err(mismatch("original inline source ABI role"));
        }
        let size = argument.source_size;
        let ignored = match abi.mode() {
            SemanticAbiPassModeV1::Ignore => true,
            SemanticAbiPassModeV1::Direct(_)
            | SemanticAbiPassModeV1::Pair { .. }
            | SemanticAbiPassModeV1::Cast { .. }
            | SemanticAbiPassModeV1::Indirect { .. } => false,
        };
        if ignored {
            if size != 0 || argument.kind != Kind::CompilerLaidOutByValue {
                return Err(mismatch("ignored source argument has no physical interval"));
            }
            return u32::try_from(self.end).map_err(|_| mismatch("ignored source offset width"));
        }
        let alignment = argument.source_alignment;
        if size == 0 || !alignment.is_power_of_two() {
            return Err(mismatch("nonignored inline ABI source dimensions"));
        }
        let mask = u64::from(alignment) - 1;
        let offset = self
            .end
            .checked_add(mask)
            .map(|value| value & !mask)
            .ok_or_else(|| mismatch("inline ABI offset arithmetic"))?;
        let end = offset
            .checked_add(size)
            .ok_or_else(|| mismatch("inline ABI extent arithmetic"))?;
        if end > MAX_ABI_BYTES {
            return Err(mismatch("inline ABI explicit byte limit"));
        }
        self.end = end;
        self.alignment = self.alignment.max(alignment);
        u32::try_from(offset).map_err(|_| mismatch("inline ABI offset width"))
    }

    pub(super) fn finish(self) -> Result<Extent, Error> {
        if self.next != self.root.arguments.len() {
            return Err(mismatch("complete original inline ABI cursor"));
        }
        if !self.deferred {
            return Ok(Extent {
                bytes: self.root.explicit_argument_bytes,
                alignment: self.root.kernarg_alignment_bytes,
            });
        }
        let mask = u64::from(self.alignment) - 1;
        let end = self
            .end
            .checked_add(mask)
            .map(|value| value & !mask)
            .ok_or_else(|| mismatch("inline ABI final alignment"))?;
        let bytes = u32::try_from(end).map_err(|_| mismatch("inline ABI extent width"))?;
        if end > MAX_ABI_BYTES
            || bytes
                .checked_add(256)
                .is_none_or(|size| size > fe2o3_kernel_descriptor::MAX_KERNARG_SEGMENT_BYTES)
        {
            return Err(mismatch("inline ABI whole segment limit"));
        }
        Ok(Extent {
            bytes,
            alignment: self.alignment,
        })
    }
}

#[cfg(test)]
#[path = "compiler_descriptor_inline_plan_v29_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "compiler_descriptor_inline_plan_resources_v29_tests.rs"]
mod resources;
