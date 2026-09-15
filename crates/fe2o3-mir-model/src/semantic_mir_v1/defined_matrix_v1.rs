//! Closed original matrix pairing/narrowing recipes, never SSA authority.
//!
//! See INTEGRATION.md for source custody and the separate SSA obligations.

use super::*;

mod body;

const MAX_BODY_BYTES: u64 = 16 * 1024;

impl InertSemanticMirRequestV1 {
    pub fn admit_exact_v23(
        self,
        limits: SemanticMirLimitsV1,
    ) -> Result<AdmittedInertSemanticMirV1, SemanticMirErrorV1> {
        self.admit_for_wire_version(SemanticMirWireVersionV1::V23, limits)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPolicyMatrixBindTypesV1 {
    pub matrix_reference: SemanticTypeIdV1,
    pub matrix: SemanticTypeIdV1,
    pub policy_reference: SemanticTypeIdV1,
    pub capability: SemanticTypeIdV1,
    pub bound: SemanticTypeIdV1,
}

impl SemanticPolicyMatrixBindTypesV1 {
    pub const fn new(ids: [SemanticTypeIdV1; 5]) -> Self {
        let [
            matrix_reference,
            matrix,
            policy_reference,
            capability,
            bound,
        ] = ids;
        Self {
            matrix_reference,
            matrix,
            policy_reference,
            capability,
            bound,
        }
    }

    pub const fn all(self) -> [SemanticTypeIdV1; 5] {
        [
            self.matrix_reference,
            self.matrix,
            self.policy_reference,
            self.capability,
            self.bound,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPolicyGfx950NarrowTypesV1 {
    pub bind: SemanticPolicyMatrixBindTypesV1,
    pub bound_reference: SemanticTypeIdV1,
    pub narrowed: SemanticTypeIdV1,
}

impl SemanticPolicyGfx950NarrowTypesV1 {
    pub const fn new(ids: [SemanticTypeIdV1; 7]) -> Self {
        let [
            matrix_reference,
            matrix,
            policy_reference,
            capability,
            bound,
            bound_reference,
            narrowed,
        ] = ids;
        Self {
            bind: SemanticPolicyMatrixBindTypesV1::new([
                matrix_reference,
                matrix,
                policy_reference,
                capability,
                bound,
            ]),
            bound_reference,
            narrowed,
        }
    }

    pub const fn all(self) -> [SemanticTypeIdV1; 7] {
        let [
            matrix_reference,
            matrix,
            policy_reference,
            capability,
            bound,
        ] = self.bind.all();
        [
            matrix_reference,
            matrix,
            policy_reference,
            capability,
            bound,
            self.bound_reference,
            self.narrowed,
        ]
    }
}

/// Full subgroup brand and epoch are independent of the root Global brand.
/// The only recipe here is an existing wave64 MatrixAccess plus StrictIeee.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticDefinedMatrixIdentityV1 {
    provenance: SemanticKernelCapabilityProvenanceV1,
    policy: SemanticTypeIdentityV1,
    kernel_brand: SemanticTypeIdentityV1,
    matrix_brand: SemanticTypeIdentityV1,
    epoch: SemanticTypeIdentityV1,
    execution_brand: Option<SemanticTypeIdentityV1>,
}

impl SemanticDefinedMatrixIdentityV1 {
    pub fn new(
        provenance: SemanticKernelCapabilityProvenanceV1,
        policy: SemanticTypeIdentityV1,
        kernel_brand: SemanticTypeIdentityV1,
        matrix_brand: SemanticTypeIdentityV1,
        epoch: SemanticTypeIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        let markers = [policy, kernel_brand, matrix_brand, epoch];
        require(
            u64::from(provenance.root().index()) < HARD_MAX_FUNCTIONS_V1
                && markers.iter().enumerate().all(|(index, marker)| {
                    marker.as_bytes() != &[0; 32] && !markers[..index].contains(marker)
                }),
        )?;
        Ok(Self {
            provenance,
            policy,
            kernel_brand,
            matrix_brand,
            epoch,
            execution_brand: None,
        })
    }

    /// Retains a distinct source-observed execution brand. Sealed ancestry is
    /// authenticated by live source replay, not by this inert identity pair.
    pub fn with_execution_brand(
        mut self,
        execution_brand: SemanticTypeIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        require(
            self.execution_brand
                .is_none_or(|existing| existing == execution_brand)
                && execution_brand.as_bytes() != &[0; 32]
                && ![
                    self.policy,
                    self.kernel_brand,
                    self.matrix_brand,
                    self.epoch,
                ]
                .contains(&execution_brand),
        )?;
        self.execution_brand = Some(execution_brand);
        Ok(self)
    }

    pub const fn execution_brand(self) -> SemanticTypeIdentityV1 {
        match self.execution_brand {
            Some(brand) => brand,
            None => self.kernel_brand,
        }
    }

    pub const fn has_distinct_execution_brand(self) -> bool {
        self.execution_brand.is_some()
    }

    pub(super) const fn minimum_wire_version(self) -> SemanticMirWireVersionV1 {
        if self.has_distinct_execution_brand() {
            SemanticMirWireVersionV1::V23
        } else {
            SemanticMirWireVersionV1::V22
        }
    }

    fn validate(self) -> Result<(), SemanticMirErrorV1> {
        let direct = Self::new(
            self.provenance,
            self.policy,
            self.kernel_brand,
            self.matrix_brand,
            self.epoch,
        )?;
        if let Some(execution) = self.execution_brand {
            direct.with_execution_brand(execution)?;
        }
        Ok(())
    }

    pub const fn provenance(self) -> SemanticKernelCapabilityProvenanceV1 {
        self.provenance
    }
    pub const fn policy(self) -> SemanticTypeIdentityV1 {
        self.policy
    }
    pub const fn kernel_brand(self) -> SemanticTypeIdentityV1 {
        self.kernel_brand
    }
    pub const fn matrix_brand(self) -> SemanticTypeIdentityV1 {
        self.matrix_brand
    }
    pub const fn epoch(self) -> SemanticTypeIdentityV1 {
        self.epoch
    }
    pub const fn width(self) -> u32 {
        64
    }
    pub const fn mode(self) -> SemanticNumericalModeV1 {
        SemanticNumericalModeV1::StrictIeee
    }

    fn encode(self, writer: &mut CanonicalWriterV1) -> Result<(), SemanticMirErrorV1> {
        encode_kernel_capability_provenance(writer, self.provenance)?;
        for identity in [
            self.policy,
            self.kernel_brand,
            self.matrix_brand,
            self.epoch,
        ] {
            writer.identity(*identity.as_bytes())?;
        }
        if let Some(execution_brand) = self.execution_brand {
            writer.identity(*execution_brand.as_bytes())?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticDefinedMatrixBodyV1 {
    function: SemanticFunctionIdV1,
    source_identity: SemanticFunctionIdentityV1,
    abi_identity: SemanticAbiIdentityV1,
    body_identity: [u8; 32],
}

impl SemanticDefinedMatrixBodyV1 {
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
        require(
            u64::from(function.index()) < HARD_MAX_FUNCTIONS_V1
                && source_identity.as_bytes() != &[0; 32]
                && abi_identity.as_bytes() != &[0; 32]
                && body_identity != [0; 32],
        )?;
        Ok(Self {
            function,
            source_identity,
            abi_identity,
            body_identity,
        })
    }

    fn observe(
        function: SemanticFunctionIdV1,
        body: &SemanticFunctionDeclV1,
        budget: &mut u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        // Every caller first checks the closed, at-most-three-local recipe.
        let mut unannotated = body.clone();
        unannotated.defined_capability_contract = None;
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

    fn encode(self, writer: &mut CanonicalWriterV1) -> Result<(), SemanticMirErrorV1> {
        writer.u32(self.function.index())?;
        writer.identity(*self.source_identity.as_bytes())?;
        writer.identity(*self.abi_identity.as_bytes())?;
        writer.identity(self.body_identity)
    }
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPolicyMatrixBindV1 {
    origin: SemanticDefinedMatrixBodyV1,
    types: SemanticPolicyMatrixBindTypesV1,
    identity: SemanticDefinedMatrixIdentityV1,
}

impl SemanticPolicyMatrixBindV1 {
    pub fn for_defined_function(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        declarations: &[SemanticTypeDeclV1],
        types: SemanticPolicyMatrixBindTypesV1,
        identity: SemanticDefinedMatrixIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        let mut budget = MAX_BODY_BYTES;
        Self::observe(
            function,
            functions,
            callables,
            declarations,
            types,
            identity,
            &mut budget,
        )
    }

    fn observe(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        declarations: &[SemanticTypeDeclV1],
        types: SemanticPolicyMatrixBindTypesV1,
        identity: SemanticDefinedMatrixIdentityV1,
        budget: &mut u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        let function_body = body::defined(function, functions, callables)?;
        require(
            body::bind_types(declarations, types, identity)
                && body::bind(function_body, types, declarations),
        )?;
        Self::from_encoded_parts(
            SemanticDefinedMatrixBodyV1::observe(function, function_body, budget)?,
            types,
            identity,
        )
    }

    pub(super) fn from_encoded_parts(
        origin: SemanticDefinedMatrixBodyV1,
        types: SemanticPolicyMatrixBindTypesV1,
        identity: SemanticDefinedMatrixIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        require(body::distinct_ids(&types.all()))?;
        identity.validate()?;
        Ok(Self {
            origin,
            types,
            identity,
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
    pub const fn types(self) -> SemanticPolicyMatrixBindTypesV1 {
        self.types
    }
    pub const fn identity(self) -> SemanticDefinedMatrixIdentityV1 {
        self.identity
    }
    pub const fn provenance(self) -> SemanticKernelCapabilityProvenanceV1 {
        self.identity.provenance
    }
    pub const fn reference_arguments(self) -> [u32; 2] {
        [0, 1]
    }
    pub const fn reference_fields(self) -> [u32; 2] {
        [0, 1]
    }

    pub(super) fn encode_payload(
        self,
        writer: &mut CanonicalWriterV1,
    ) -> Result<(), SemanticMirErrorV1> {
        self.origin.encode(writer)?;
        for ty in self.types.all() {
            writer.u32(ty.index())?;
        }
        self.identity.encode(writer)
    }
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPolicyGfx950NarrowV1 {
    origin: SemanticDefinedMatrixBodyV1,
    projection: SemanticDefinedMatrixBodyV1,
    types: SemanticPolicyGfx950NarrowTypesV1,
    identity: SemanticDefinedMatrixIdentityV1,
}

impl SemanticPolicyGfx950NarrowV1 {
    pub fn for_defined_function(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        declarations: &[SemanticTypeDeclV1],
        types: SemanticPolicyGfx950NarrowTypesV1,
        identity: SemanticDefinedMatrixIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        let mut budget = 2 * MAX_BODY_BYTES;
        Self::observe(
            function,
            functions,
            callables,
            declarations,
            types,
            identity,
            &mut budget,
        )
    }

    fn observe(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        declarations: &[SemanticTypeDeclV1],
        types: SemanticPolicyGfx950NarrowTypesV1,
        identity: SemanticDefinedMatrixIdentityV1,
        budget: &mut u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        let function_body = body::defined(function, functions, callables)?;
        require(body::narrow_types(declarations, types, identity))?;
        let projection = body::narrow(function_body, types, declarations)
            .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
        let projection = {
            let Some(SemanticCallableDeclV1::Defined { function: getter }) =
                callables.get(projection.index() as usize)
            else {
                return Err(SemanticMirErrorV1::InvalidFunctionAbi);
            };
            require(*getter != function)?;
            let getter_body = body::defined(*getter, functions, callables)?;
            require(body::projection(getter_body, types))?;
            SemanticDefinedMatrixBodyV1::observe(*getter, getter_body, budget)?
        };
        Self::from_encoded_parts(
            SemanticDefinedMatrixBodyV1::observe(function, function_body, budget)?,
            projection,
            types,
            identity,
        )
    }

    pub(super) fn from_encoded_parts(
        origin: SemanticDefinedMatrixBodyV1,
        projection: SemanticDefinedMatrixBodyV1,
        types: SemanticPolicyGfx950NarrowTypesV1,
        identity: SemanticDefinedMatrixIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        require(
            body::distinct_ids(&types.all())
                && projection.function != origin.function
                && projection.source_identity != origin.source_identity,
        )?;
        identity.validate()?;
        Ok(Self {
            origin,
            projection,
            types,
            identity,
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
    pub const fn projection(self) -> SemanticDefinedMatrixBodyV1 {
        self.projection
    }
    pub const fn types(self) -> SemanticPolicyGfx950NarrowTypesV1 {
        self.types
    }
    pub const fn identity(self) -> SemanticDefinedMatrixIdentityV1 {
        self.identity
    }
    pub const fn provenance(self) -> SemanticKernelCapabilityProvenanceV1 {
        self.identity.provenance
    }
    pub const fn receiver_argument(self) -> u32 {
        0
    }
    pub const fn reference_field(self) -> u32 {
        0
    }

    pub(super) fn encode_payload(
        self,
        writer: &mut CanonicalWriterV1,
    ) -> Result<(), SemanticMirErrorV1> {
        self.origin.encode(writer)?;
        self.projection.encode(writer)?;
        for ty in self.types.all() {
            writer.u32(ty.index())?;
        }
        self.identity.encode(writer)
    }
}

fn require(condition: bool) -> Result<(), SemanticMirErrorV1> {
    condition
        .then_some(())
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)
}

pub(super) fn record_claim(
    claims: &mut IntrinsicCapabilityClaimsV1,
    types: SemanticPolicyMatrixBindTypesV1,
    identity: SemanticDefinedMatrixIdentityV1,
) -> bool {
    if !claims.record_policy_math_binding(
        types.capability,
        identity.policy,
        identity.provenance,
        identity.kernel_brand,
    ) {
        return false;
    }
    match claims
        .execution_workgroups
        .entry(identity.execution_brand())
    {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(identity.provenance);
            true
        }
        std::collections::btree_map::Entry::Occupied(entry) => *entry.get() == identity.provenance,
    }
}

pub(super) fn validate_bind_attachment(
    function: &SemanticFunctionDeclV1,
    record: SemanticPolicyMatrixBindV1,
) -> Result<(), SemanticMirErrorV1> {
    require(body::bind_markers(function, record.types).is_some())?;
    let mut budget = MAX_BODY_BYTES;
    require(
        SemanticDefinedMatrixBodyV1::observe(record.function(), function, &mut budget)?
            == record.origin,
    )
}

pub(super) fn validate_narrow_attachment(
    function: &SemanticFunctionDeclV1,
    record: SemanticPolicyGfx950NarrowV1,
) -> Result<(), SemanticMirErrorV1> {
    let (_, projection) = body::narrow_recipe(function, record.types)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    require(projection == SemanticCallableIdV1::from_index(record.projection.function.index()))?;
    let mut budget = MAX_BODY_BYTES;
    require(
        SemanticDefinedMatrixBodyV1::observe(record.function(), function, &mut budget)?
            == record.origin,
    )
}

fn validate_issuers(
    context: &mut ValidationContextV1<'_>,
    function: SemanticFunctionIdV1,
    types: SemanticPolicyMatrixBindTypesV1,
    identity: SemanticDefinedMatrixIdentityV1,
) -> Result<(), SemanticMirErrorV1> {
    identity.validate()?;
    require(kernel_capability_provenance_matches(
        context.request,
        identity.provenance,
    ))?;
    context.one()?;
    for ty in types.all() {
        context.type_reference(ty, SemanticMirLocationV1::Function(function))?;
    }
    let callable_count = context.request.callables.len();
    charge_validation_work(context, callable_count)?;
    let mut matrix = false;
    let mut policy = false;
    for callable in &context.request.callables {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ..
        } = callable
        else {
            continue;
        };
        if contract.provenance() != identity.provenance {
            continue;
        }
        match contract.operation() {
            SemanticExecutionCapabilityOperationV1::MatrixAccess {
                matrix: ty,
                subgroup_brand,
                width,
                ..
            } if ty == types.matrix => {
                require(
                    subgroup_brand == identity.matrix_brand
                        && width == 64
                        && contract.workgroup_brand() == Some(identity.execution_brand())
                        && contract.epoch_before() == Some(identity.epoch)
                        && contract.epoch_after().is_none(),
                )?;
                matrix = true;
            }
            SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
                capability,
                policy: marker,
                ..
            } if capability == types.capability => {
                require(marker == identity.policy)?;
                policy = true;
            }
            _ => {}
        }
    }
    require(matrix && policy)
}

pub(super) fn validate_bind(
    context: &mut ValidationContextV1<'_>,
    function: SemanticFunctionIdV1,
    record: SemanticPolicyMatrixBindV1,
) -> Result<(), SemanticMirErrorV1> {
    require(record.function() == function)?;
    validate_issuers(context, function, record.types, record.identity)?;
    let mut budget = digest_budget(context, MAX_BODY_BYTES);
    let before = budget;
    let observed = SemanticPolicyMatrixBindV1::observe(
        function,
        &context.request.functions,
        &context.request.callables,
        &context.request.types,
        record.types,
        record.identity,
        &mut budget,
    )
    .map_err(|error| digest_budget_error(context, error))?;
    charge_validation_work(context, ((before - budget) * 2) as usize)?;
    require(observed == record)
}

pub(super) fn validate_narrow(
    context: &mut ValidationContextV1<'_>,
    function: SemanticFunctionIdV1,
    record: SemanticPolicyGfx950NarrowV1,
) -> Result<(), SemanticMirErrorV1> {
    require(record.function() == function)?;
    validate_issuers(context, function, record.types.bind, record.identity)?;
    for ty in [record.types.bound_reference, record.types.narrowed] {
        context.type_reference(ty, SemanticMirLocationV1::Function(function))?;
    }
    let mut budget = digest_budget(context, 2 * MAX_BODY_BYTES);
    let before = budget;
    let observed = SemanticPolicyGfx950NarrowV1::observe(
        function,
        &context.request.functions,
        &context.request.callables,
        &context.request.types,
        record.types,
        record.identity,
        &mut budget,
    )
    .map_err(|error| digest_budget_error(context, error))?;
    charge_validation_work(context, ((before - budget) * 2) as usize)?;
    require(observed == record)
}

fn digest_budget(context: &ValidationContextV1<'_>, max: u64) -> u64 {
    (context
        .limits
        .limit(SemanticMirResourceV1::ValidationWork)
        .saturating_sub(context.work)
        / 2)
    .min(max)
}

fn digest_budget_error(
    context: &ValidationContextV1<'_>,
    error: SemanticMirErrorV1,
) -> SemanticMirErrorV1 {
    match error {
        SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        } => {
            let max = context.limits.limit(SemanticMirResourceV1::ValidationWork);
            SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                actual: max.saturating_add(1),
                max,
            }
        }
        error => error,
    }
}

#[cfg(test)]
pub(super) mod tests;
