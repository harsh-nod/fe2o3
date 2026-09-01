//! Sealed target-specific operations admitted after exact target binding.

use std::collections::BTreeSet;

use crate::{Gfx950LdsTransposeOperationV1, MemoryEffect, TargetCapability, ValueId};

/// A compiler-owned target operation introduced only after exact target binding.
///
/// Its representation is private so frontends cannot register arbitrary
/// behavior or bypass the Kernel IR verifier. Constructors are deliberately
/// limited to reviewed target families.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetExtensionOperation {
    kind: TargetExtensionOperationKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TargetExtensionOperationKind {
    AmdgcnGfx950LdsTranspose(Gfx950LdsTransposeOperationV1),
}

impl TargetExtensionOperation {
    pub fn amdgcn_gfx950_lds_transpose(operation: Gfx950LdsTransposeOperationV1) -> Self {
        Self {
            kind: TargetExtensionOperationKind::AmdgcnGfx950LdsTranspose(operation),
        }
    }

    pub const fn as_amdgcn_gfx950_lds_transpose(&self) -> Option<&Gfx950LdsTransposeOperationV1> {
        match &self.kind {
            TargetExtensionOperationKind::AmdgcnGfx950LdsTranspose(operation) => Some(operation),
        }
    }

    pub fn as_amdgcn_gfx950_lds_transpose_mut(
        &mut self,
    ) -> Option<&mut Gfx950LdsTransposeOperationV1> {
        match &mut self.kind {
            TargetExtensionOperationKind::AmdgcnGfx950LdsTranspose(operation) => Some(operation),
        }
    }

    pub fn operands(&self) -> Vec<ValueId> {
        match &self.kind {
            TargetExtensionOperationKind::AmdgcnGfx950LdsTranspose(operation) => {
                operation.operands()
            }
        }
    }

    pub fn required_capabilities(&self) -> BTreeSet<TargetCapability> {
        match &self.kind {
            TargetExtensionOperationKind::AmdgcnGfx950LdsTranspose(operation) => {
                operation.required_capabilities()
            }
        }
    }

    pub fn memory_effects(&self) -> Vec<MemoryEffect> {
        match &self.kind {
            TargetExtensionOperationKind::AmdgcnGfx950LdsTranspose(operation) => {
                operation.memory_effects()
            }
        }
    }
}

impl From<Gfx950LdsTransposeOperationV1> for TargetExtensionOperation {
    fn from(operation: Gfx950LdsTransposeOperationV1) -> Self {
        Self::amdgcn_gfx950_lds_transpose(operation)
    }
}
