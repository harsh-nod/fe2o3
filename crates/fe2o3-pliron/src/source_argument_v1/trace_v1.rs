//! Inert emission proposals; only the owning replay checker creates checked relations.
use super::*;

/// Exact one-to-one mapping from one selected semantic argument local to a
/// canonical KIR function parameter.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticKirParameterBindingV1 {
    pub correspondence_owner: SemanticFunctionIdV1,
    pub semantic_function: SemanticFunctionIdV1,
    pub semantic_local: SemanticLocalIdV1,
    pub kernel_ir_value: ValueId,
}

/// One exact local projection used to scalarize a by-value function argument.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticKirParameterProjectionV1 {
    /// A tuple or nominal aggregate field.
    Field(u32),
    /// One element of a fixed-size array.
    ArrayIndex(u32),
}

/// Exact one-to-many correspondence from a by-value source argument to
/// canonical KIR function parameters.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticKirParameterComponentBindingV1 {
    pub correspondence_owner: SemanticFunctionIdV1,
    pub semantic_function: SemanticFunctionIdV1,
    pub semantic_local: SemanticLocalIdV1,
    pub semantic_component_type: SemanticTypeIdV1,
    pub projection: Box<[SemanticKirParameterProjectionV1]>,
    pub kernel_ir_value: ValueId,
}

/// Exact correspondence for a by-value argument whose compiler ABI is
/// `Ignore`, such as `()` or a zero-sized aggregate.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticKirIgnoredParameterBindingV1 {
    pub correspondence_owner: SemanticFunctionIdV1,
    pub semantic_function: SemanticFunctionIdV1,
    pub semantic_local: SemanticLocalIdV1,
    pub semantic_type: SemanticTypeIdV1,
}

/// Closed role of one semantic function materialized in the Kernel IR module.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticKirFunctionRoleV1 {
    /// The selected semantic body backing the sole kernel entry.
    KernelEntry,
    /// A reachable pure helper with an admitted argument/result ABI.
    InternalHelper,
}

/// Exact semantic-function to Kernel-IR-function correspondence.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticKirFunctionCorrespondenceV1 {
    pub correspondence_owner: SemanticFunctionIdV1,
    pub semantic_function: SemanticFunctionIdV1,
    pub kernel_ir_function: FunctionId,
    pub role: SemanticKirFunctionRoleV1,
}

impl SemanticKirFunctionCorrespondenceV1 {
    /// Returns the semantic root or helper that owns this KIR function instance.
    pub const fn correspondence_owner(&self) -> SemanticFunctionIdV1 {
        self.correspondence_owner
    }

    /// Returns the retained semantic function identity.
    pub const fn semantic_function(&self) -> SemanticFunctionIdV1 {
        self.semantic_function
    }

    /// Returns the exact Kernel IR function identity emitted for the function.
    pub const fn kernel_ir_function(&self) -> &FunctionId {
        &self.kernel_ir_function
    }

    /// Returns whether this is the kernel entry or an internal helper.
    pub const fn role(&self) -> SemanticKirFunctionRoleV1 {
        self.role
    }
}

impl SemanticKirParameterBindingV1 {
    /// Returns the semantic root or helper that owns this KIR function instance.
    pub const fn correspondence_owner(self) -> SemanticFunctionIdV1 {
        self.correspondence_owner
    }

    /// Returns the semantic function that owns the parameter local.
    pub const fn semantic_function(self) -> SemanticFunctionIdV1 {
        self.semantic_function
    }

    /// Returns the semantic MIR local represented by this binding.
    pub const fn semantic_local(self) -> SemanticLocalIdV1 {
        self.semantic_local
    }

    /// Returns the exact Kernel IR function-parameter value.
    pub const fn kernel_ir_value(self) -> ValueId {
        self.kernel_ir_value
    }
}

impl SemanticKirParameterComponentBindingV1 {
    /// Returns the semantic root whose lowering owns this correspondence.
    pub const fn correspondence_owner(&self) -> SemanticFunctionIdV1 {
        self.correspondence_owner
    }

    /// Returns the semantic function containing the source argument.
    pub const fn semantic_function(&self) -> SemanticFunctionIdV1 {
        self.semantic_function
    }

    /// Returns the semantic argument local being projected.
    pub const fn semantic_local(&self) -> SemanticLocalIdV1 {
        self.semantic_local
    }

    /// Returns the exact semantic leaf type selected by the projection.
    pub const fn semantic_component_type(&self) -> SemanticTypeIdV1 {
        self.semantic_component_type
    }

    /// Returns the canonical argument-local-to-leaf projection path.
    pub fn projection(&self) -> &[SemanticKirParameterProjectionV1] {
        &self.projection
    }

    /// Returns the exact KIR parameter value carrying this leaf.
    pub const fn kernel_ir_value(&self) -> ValueId {
        self.kernel_ir_value
    }
}

impl SemanticKirIgnoredParameterBindingV1 {
    /// Returns the semantic root whose lowering owns this correspondence.
    pub const fn correspondence_owner(self) -> SemanticFunctionIdV1 {
        self.correspondence_owner
    }

    /// Returns the semantic function containing the ignored argument.
    pub const fn semantic_function(self) -> SemanticFunctionIdV1 {
        self.semantic_function
    }

    /// Returns the exact ignored argument local.
    pub const fn semantic_local(self) -> SemanticLocalIdV1 {
        self.semantic_local
    }

    /// Returns the exact zero-sized semantic type reconstructed without a KIR parameter.
    pub const fn semantic_type(self) -> SemanticTypeIdV1 {
        self.semantic_type
    }
}
