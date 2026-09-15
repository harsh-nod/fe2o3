//! Closed branded matrix issuer recipe. A body commitment is not provider authentication.

use super::*;

const MAX_BODY_BYTES: u64 = 16 * 1024;

include!("defined_matrix_issuer_v1/types.rs");

#[path = "defined_matrix_issuer_v1/body.rs"]
mod body;
use body::*;

/// A body commitment used only inside a closed recipe. It authenticates no provider.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticMatrixIssuerBodyV1 {
    function: SemanticFunctionIdV1,
    source_identity: SemanticFunctionIdentityV1,
    abi_identity: SemanticAbiIdentityV1,
    body_identity: [u8; 32],
}

impl SemanticMatrixIssuerBodyV1 {
    pub const fn function(self) -> SemanticFunctionIdV1 {
        self.function
    }
    pub const fn source_identity(self) -> SemanticFunctionIdentityV1 {
        self.source_identity
    }
    pub const fn abi_identity(self) -> SemanticAbiIdentityV1 {
        self.abi_identity
    }
    pub const fn body_identity(&self) -> &[u8; 32] {
        &self.body_identity
    }

    pub(super) fn from_encoded_parts(
        function: SemanticFunctionIdV1,
        source_identity: SemanticFunctionIdentityV1,
        abi_identity: SemanticAbiIdentityV1,
        body_identity: [u8; 32],
    ) -> Result<Self, SemanticMirErrorV1> {
        if u64::from(function.index()) >= HARD_MAX_FUNCTIONS_V1
            || source_identity.as_bytes() == &[0; 32]
            || abi_identity.as_bytes() == &[0; 32]
            || body_identity == [0; 32]
        {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        }
        Ok(Self {
            function,
            source_identity,
            abi_identity,
            body_identity,
        })
    }

    pub(super) fn observe(
        function: SemanticFunctionIdV1,
        body: &SemanticFunctionDeclV1,
        budget: &mut u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        require(body.export().is_none())?;
        let unannotated = SemanticFunctionDeclV1::new(
            body.identity(),
            body.role(),
            body.item_definition_identity(),
            body.monomorphization_identity(),
            body.generic_type_arguments_identity(),
            body.const_generic_arguments_identity(),
            body.source(),
            body.abi().clone(),
            body.locals().to_vec(),
            body.entry(),
            body.blocks().to_vec(),
        )?;
        let (identity, bytes) = canonical_semantic_function_fragment_sha256_v1(
            &unannotated,
            SemanticMirWireVersionV1::V19,
            (*budget).min(MAX_BODY_BYTES),
        )?;
        *budget = budget
            .checked_sub(bytes as u64)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
        Self::from_encoded_parts(function, body.identity(), body.abi().identity(), identity)
    }
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticKernelMatrixDeriveV1 {
    origin: SemanticMatrixIssuerBodyV1,
    bridge: SemanticMatrixIssuerBodyV1,
    current_callable: SemanticCallableIdV1,
    current_source_identity: SemanticFunctionIdentityV1,
    current_abi_identity: SemanticAbiIdentityV1,
    types: SemanticKernelMatrixDeriveTypesV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    kernel_brand: SemanticTypeIdentityV1,
}

impl SemanticKernelMatrixDeriveV1 {
    /// Follows the actual getter and bridge calls in the retained Defined roster.
    #[allow(clippy::too_many_arguments)]
    pub fn for_defined_function(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        declarations: &[SemanticTypeDeclV1],
        types: SemanticKernelMatrixDeriveTypesV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        kernel_brand: SemanticTypeIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        let mut budget = 2 * MAX_BODY_BYTES;
        Self::observe(
            function,
            functions,
            callables,
            declarations,
            types,
            provenance,
            kernel_brand,
            &mut budget,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn observe(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        declarations: &[SemanticTypeDeclV1],
        types: SemanticKernelMatrixDeriveTypesV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        kernel_brand: SemanticTypeIdentityV1,
        budget: &mut u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        let checked = body::observe_recipe(
            function,
            functions,
            callables,
            declarations,
            types,
            provenance,
            kernel_brand,
        )?;
        Self::from_encoded_parts(
            SemanticMatrixIssuerBodyV1::observe(function, checked.getter, budget)?,
            SemanticMatrixIssuerBodyV1::observe(checked.bridge_function, checked.bridge, budget)?,
            checked.current_callable,
            checked.current_binding.identity(),
            checked.current_binding.abi().identity(),
            types,
            provenance,
            kernel_brand,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_encoded_parts(
        origin: SemanticMatrixIssuerBodyV1,
        bridge: SemanticMatrixIssuerBodyV1,
        current_callable: SemanticCallableIdV1,
        current_source_identity: SemanticFunctionIdentityV1,
        current_abi_identity: SemanticAbiIdentityV1,
        types: SemanticKernelMatrixDeriveTypesV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        kernel_brand: SemanticTypeIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        require(
            origin.function != bridge.function
                && origin.source_identity != bridge.source_identity
                && current_source_identity != origin.source_identity
                && current_source_identity != bridge.source_identity
                && current_source_identity.as_bytes() != &[0; 32]
                && current_abi_identity.as_bytes() != &[0; 32]
                && u64::from(current_callable.index()) < HARD_MAX_CALLABLES_V1
                && u64::from(provenance.root().index()) < HARD_MAX_FUNCTIONS_V1
                && distinct_ids(&types.all())
                && kernel_brand.as_bytes() != &[0; 32],
        )?;
        Ok(Self {
            origin,
            bridge,
            current_callable,
            current_source_identity,
            current_abi_identity,
            types,
            provenance,
            kernel_brand,
        })
    }

    pub const fn function(self) -> SemanticFunctionIdV1 {
        self.origin.function
    }
    pub const fn source_identity(self) -> SemanticFunctionIdentityV1 {
        self.origin.source_identity
    }
    pub const fn abi_identity(self) -> SemanticAbiIdentityV1 {
        self.origin.abi_identity
    }
    pub const fn body_identity(&self) -> &[u8; 32] {
        &self.origin.body_identity
    }
    pub const fn bridge(self) -> SemanticMatrixIssuerBodyV1 {
        self.bridge
    }
    pub const fn current_callable(self) -> SemanticCallableIdV1 {
        self.current_callable
    }
    pub const fn current_source_identity(self) -> SemanticFunctionIdentityV1 {
        self.current_source_identity
    }
    pub const fn current_abi_identity(self) -> SemanticAbiIdentityV1 {
        self.current_abi_identity
    }
    pub const fn types(self) -> SemanticKernelMatrixDeriveTypesV1 {
        self.types
    }
    pub const fn provenance(self) -> SemanticKernelCapabilityProvenanceV1 {
        self.provenance
    }
    pub const fn kernel_brand(self) -> SemanticTypeIdentityV1 {
        self.kernel_brand
    }
    pub const fn receiver_argument(self) -> u32 {
        0
    }
}

include!("defined_matrix_issuer_v1/schema_hooks.rs");

#[cfg(test)]
#[path = "defined_matrix_issuer_v1/tests.rs"]
pub(super) mod tests;
