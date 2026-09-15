//! Closed records for original Math getter/constructor bodies, not SSA authority.

use super::*;

const MAX_BODY_BYTES: u64 = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticKernelMathDeriveTypesV1 {
    pub context_reference: SemanticTypeIdV1,
    pub context: SemanticTypeIdV1,
    pub math: SemanticTypeIdV1,
    pub unbranded_math: SemanticTypeIdV1,
}

impl SemanticKernelMathDeriveTypesV1 {
    pub const fn new(ids: [SemanticTypeIdV1; 4]) -> Self {
        let [context_reference, context, math, unbranded_math] = ids;
        Self {
            context_reference,
            context,
            math,
            unbranded_math,
        }
    }
    pub const fn all(self) -> [SemanticTypeIdV1; 4] {
        [
            self.context_reference,
            self.context,
            self.math,
            self.unbranded_math,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPolicyMathBindTypesV1 {
    pub math_reference: SemanticTypeIdV1,
    pub math: SemanticTypeIdV1,
    pub policy_reference: SemanticTypeIdV1,
    pub capability: SemanticTypeIdV1,
    pub bound: SemanticTypeIdV1,
}

impl SemanticPolicyMathBindTypesV1 {
    pub const fn new(ids: [SemanticTypeIdV1; 5]) -> Self {
        let [math_reference, math, policy_reference, capability, bound] = ids;
        Self {
            math_reference,
            math,
            policy_reference,
            capability,
            bound,
        }
    }
    pub const fn all(self) -> [SemanticTypeIdV1; 5] {
        [
            self.math_reference,
            self.math,
            self.policy_reference,
            self.capability,
            self.bound,
        ]
    }
}

/// A body commitment used only inside a closed recipe. It authenticates no provider.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticDefinedMathBodyV1 {
    function: SemanticFunctionIdV1,
    source_identity: SemanticFunctionIdentityV1,
    abi_identity: SemanticAbiIdentityV1,
    body_identity: [u8; 32],
}

impl SemanticDefinedMathBodyV1 {
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

    fn observe(
        function: SemanticFunctionIdV1,
        body: &SemanticFunctionDeclV1,
        budget: &mut u64,
    ) -> Result<Self, SemanticMirErrorV1> {
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
pub struct SemanticKernelMathDeriveV1 {
    origin: SemanticDefinedMathBodyV1,
    bridge: SemanticDefinedMathBodyV1,
    current_callable: SemanticCallableIdV1,
    current_source_identity: SemanticFunctionIdentityV1,
    current_abi_identity: SemanticAbiIdentityV1,
    types: SemanticKernelMathDeriveTypesV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    kernel_brand: SemanticTypeIdentityV1,
}

impl SemanticKernelMathDeriveV1 {
    /// Follows the actual getter and bridge calls in the retained Defined roster.
    #[allow(clippy::too_many_arguments)]
    pub fn for_defined_function(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        declarations: &[SemanticTypeDeclV1],
        types: SemanticKernelMathDeriveTypesV1,
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
    fn observe(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        declarations: &[SemanticTypeDeclV1],
        types: SemanticKernelMathDeriveTypesV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        kernel_brand: SemanticTypeIdentityV1,
        budget: &mut u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        let body = defined_body(function, functions, callables)?;
        require(derive_types_match(declarations, types, kernel_brand))?;
        let bridge_callable =
            getter_body(body, types).ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
        let Some(SemanticCallableDeclV1::Defined {
            function: bridge_function,
        }) = callables.get(bridge_callable.index() as usize)
        else {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        };
        require(*bridge_function != function)?;
        let bridge_body = defined_body(*bridge_function, functions, callables)?;
        let current_callable =
            bridge_body_call(bridge_body, types).ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
        let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            binding, operation, ..
        }) = callables.get(current_callable.index() as usize)
        else {
            return Err(SemanticMirErrorV1::InvalidFunctionAbi);
        };
        require(
            *operation
                == (SemanticCompilerIntrinsicOperationV1::MathContextCurrent {
                    context: types.unbranded_math,
                })
                && abi_matches(binding.abi(), &[], types.unbranded_math, ReturnMode::Ignore),
        )?;
        Self::from_encoded_parts(
            SemanticDefinedMathBodyV1::observe(function, body, budget)?,
            SemanticDefinedMathBodyV1::observe(*bridge_function, bridge_body, budget)?,
            current_callable,
            binding.identity(),
            binding.abi().identity(),
            types,
            provenance,
            kernel_brand,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn from_encoded_parts(
        origin: SemanticDefinedMathBodyV1,
        bridge: SemanticDefinedMathBodyV1,
        current_callable: SemanticCallableIdV1,
        current_source_identity: SemanticFunctionIdentityV1,
        current_abi_identity: SemanticAbiIdentityV1,
        types: SemanticKernelMathDeriveTypesV1,
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
    pub const fn bridge(self) -> SemanticDefinedMathBodyV1 {
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
    pub const fn types(self) -> SemanticKernelMathDeriveTypesV1 {
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

    /// Payload only. The shared closed enum must assign its own discriminant.
    pub(super) fn encode_payload(
        self,
        writer: &mut CanonicalWriterV1,
    ) -> Result<(), SemanticMirErrorV1> {
        self.origin.encode(writer)?;
        self.bridge.encode(writer)?;
        writer.u32(self.current_callable.index())?;
        writer.identity(*self.current_source_identity.as_bytes())?;
        writer.identity(*self.current_abi_identity.as_bytes())?;
        for id in self.types.all() {
            writer.u32(id.index())?;
        }
        encode_kernel_capability_provenance(writer, self.provenance)?;
        writer.identity(*self.kernel_brand.as_bytes())
    }
}

#[must_use]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticPolicyMathBindV1 {
    origin: SemanticDefinedMathBodyV1,
    types: SemanticPolicyMathBindTypesV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    policy: SemanticTypeIdentityV1,
    kernel_brand: SemanticTypeIdentityV1,
}

impl SemanticPolicyMathBindV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn for_defined_function(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        declarations: &[SemanticTypeDeclV1],
        types: SemanticPolicyMathBindTypesV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        policy: SemanticTypeIdentityV1,
        kernel_brand: SemanticTypeIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        let mut budget = MAX_BODY_BYTES;
        Self::observe(
            function,
            functions,
            callables,
            declarations,
            types,
            provenance,
            policy,
            kernel_brand,
            &mut budget,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn observe(
        function: SemanticFunctionIdV1,
        functions: &[SemanticFunctionDeclV1],
        callables: &[SemanticCallableDeclV1],
        declarations: &[SemanticTypeDeclV1],
        types: SemanticPolicyMathBindTypesV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        policy: SemanticTypeIdentityV1,
        kernel_brand: SemanticTypeIdentityV1,
        budget: &mut u64,
    ) -> Result<Self, SemanticMirErrorV1> {
        let body = defined_body(function, functions, callables)?;
        require(
            bind_types_match(declarations, types, policy, kernel_brand)
                && bind_body(body, types, declarations),
        )?;
        Self::from_encoded_parts(
            SemanticDefinedMathBodyV1::observe(function, body, budget)?,
            types,
            provenance,
            policy,
            kernel_brand,
        )
    }

    pub(super) fn from_encoded_parts(
        origin: SemanticDefinedMathBodyV1,
        types: SemanticPolicyMathBindTypesV1,
        provenance: SemanticKernelCapabilityProvenanceV1,
        policy: SemanticTypeIdentityV1,
        kernel_brand: SemanticTypeIdentityV1,
    ) -> Result<Self, SemanticMirErrorV1> {
        require(
            distinct_ids(&types.all())
                && policy.as_bytes() != &[0; 32]
                && u64::from(provenance.root().index()) < HARD_MAX_FUNCTIONS_V1
                && kernel_brand.as_bytes() != &[0; 32]
                && policy != kernel_brand,
        )?;
        Ok(Self {
            origin,
            types,
            provenance,
            policy,
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
    pub const fn types(self) -> SemanticPolicyMathBindTypesV1 {
        self.types
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
        for id in self.types.all() {
            writer.u32(id.index())?;
        }
        encode_kernel_capability_provenance(writer, self.provenance)?;
        writer.identity(*self.policy.as_bytes())?;
        writer.identity(*self.kernel_brand.as_bytes())
    }
}

fn require(condition: bool) -> Result<(), SemanticMirErrorV1> {
    if condition {
        Ok(())
    } else {
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    }
}

fn distinct_ids(ids: &[SemanticTypeIdV1]) -> bool {
    ids.iter()
        .enumerate()
        .all(|(index, id)| u64::from(id.index()) < HARD_MAX_TYPES_V1 && !ids[..index].contains(id))
}

fn nominal_types(
    declarations: &[SemanticTypeDeclV1],
    ids: &[SemanticTypeIdV1],
    markers: &[SemanticTypeIdentityV1],
) -> bool {
    distinct_ids(ids)
        && ids.iter().enumerate().all(|(index, id)| {
            declarations.get(id.index() as usize).is_some_and(|ty| {
                ty.identity().as_bytes() != &[0; 32]
                    && !ty.layout().is_uninhabited()
                    && !markers.contains(&ty.identity())
                    && ids[..index].iter().all(|previous| {
                        declarations
                            .get(previous.index() as usize)
                            .is_some_and(|previous| previous.identity() != ty.identity())
                    })
            })
        })
}

fn aggregate_zst(declarations: &[SemanticTypeDeclV1], id: SemanticTypeIdV1) -> bool {
    declarations.get(id.index() as usize).is_some_and(|ty| {
        matches!(ty.shape(), SemanticTypeShapeV1::Aggregate(_))
            && ty.layout().size_bytes() == Some(0)
            && ty.layout().alignment_bytes() == 1
            && !ty.layout().is_uninhabited()
    })
}

fn shared_type(
    declarations: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    pointee: SemanticTypeIdV1,
) -> bool {
    matches!(declarations.get(reference.index() as usize).map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Pointer(pointer)) if pointer.pointee() == pointee
            && pointer.kind() == SemanticPointerKindV1::Reference
            && pointer.mutability() == SemanticMutabilityV1::Immutable
            && pointer.address_space() == 0 && pointer.pointer_width_bits() == 64
            && pointer.metadata() == SemanticPointerMetadataV1::None)
}

fn derive_types_match(
    declarations: &[SemanticTypeDeclV1],
    types: SemanticKernelMathDeriveTypesV1,
    brand: SemanticTypeIdentityV1,
) -> bool {
    nominal_types(declarations, &types.all(), &[brand])
        && shared_type(declarations, types.context_reference, types.context)
        && [types.context, types.math, types.unbranded_math]
            .iter()
            .all(|id| aggregate_zst(declarations, *id))
}

fn bind_types_match(
    declarations: &[SemanticTypeDeclV1],
    types: SemanticPolicyMathBindTypesV1,
    policy: SemanticTypeIdentityV1,
    brand: SemanticTypeIdentityV1,
) -> bool {
    let Some(marker) = bound_marker(declarations, types) else {
        return false;
    };
    let mut all = types.all().to_vec();
    all.push(marker);
    nominal_types(declarations, &all, &[policy, brand])
        && shared_type(declarations, types.math_reference, types.math)
        && shared_type(declarations, types.policy_reference, types.capability)
        && [types.math, types.capability]
            .iter()
            .all(|id| aggregate_zst(declarations, *id))
}

fn bound_marker(
    declarations: &[SemanticTypeDeclV1],
    types: SemanticPolicyMathBindTypesV1,
) -> Option<SemanticTypeIdV1> {
    let ty = declarations.get(types.bound.index() as usize)?;
    let SemanticTypeShapeV1::Aggregate(aggregate) = ty.shape() else {
        return None;
    };
    let [math, policy, marker] = aggregate.fields() else {
        return None;
    };
    (*math == types.math_reference
        && *policy == types.policy_reference
        && ty.layout().size_bytes() == Some(16)
        && ty.layout().alignment_bytes() == 8
        && aggregate_zst(declarations, *marker))
    .then_some(*marker)
}

#[derive(Clone, Copy)]
enum ReturnMode {
    Ignore,
    Pair,
}

fn abi_matches(
    abi: &SemanticFunctionAbiV1,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    return_mode: ReturnMode,
) -> bool {
    abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.can_unwind()
        && !abi.c_variadic()
        && abi.fixed_count() as usize == inputs.len()
        && abi.hidden_arguments().is_empty()
        && abi.source_input_types() == inputs
        && abi.source_output_type() == output
        && abi.arguments().len() == inputs.len()
        && abi.source_argument_ownership().len() == inputs.len()
        && abi
            .source_argument_ownership()
            .iter()
            .all(|ownership| *ownership == SemanticSourceArgumentOwnershipV1::SharedBorrow)
        && abi.arguments().iter().zip(inputs).all(|(argument, ty)| {
            argument.role() == SemanticAbiArgumentRoleV1::Source
                && argument.ty() == *ty
                && matches!(argument.value().mode(), SemanticAbiPassModeV1::Direct(_))
                && argument.value().adjusted().is_none()
                && argument.value().pointee_override().is_none()
        })
        && abi.return_value().ty() == output
        && abi.return_value().adjusted().is_none()
        && abi.return_value().pointee_override().is_none()
        && match return_mode {
            ReturnMode::Ignore => {
                matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
            }
            ReturnMode::Pair => matches!(
                abi.return_value().mode(),
                SemanticAbiPassModeV1::Pair { .. }
            ),
        }
}

fn defined_body<'a>(
    function: SemanticFunctionIdV1,
    functions: &'a [SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
) -> Result<&'a SemanticFunctionDeclV1, SemanticMirErrorV1> {
    require(
        callables.get(function.index() as usize)
            == Some(&SemanticCallableDeclV1::Defined { function }),
    )?;
    let body = functions
        .get(function.index() as usize)
        .ok_or(SemanticMirErrorV1::InvalidFunctionAbi)?;
    require(
        body.role() == SemanticFunctionRoleV1::InternalHelper
            && body.export().is_none()
            && body.blocks().get(body.entry().index() as usize).is_some(),
    )?;
    Ok(body)
}

fn local(
    body: &SemanticFunctionDeclV1,
    ty: SemanticTypeIdV1,
    role: SemanticLocalRoleV1,
) -> Option<SemanticLocalIdV1> {
    let mut matching = body
        .locals()
        .iter()
        .enumerate()
        .filter(|(_, local)| local.role() == role);
    let (index, local) = matching.next()?;
    if local.ty() != ty || matching.next().is_some() {
        return None;
    }
    Some(SemanticLocalIdV1::from_index(u32::try_from(index).ok()?))
}

fn forwarding_call(
    body: &SemanticFunctionDeclV1,
    output_local: SemanticLocalIdV1,
    output: SemanticTypeIdV1,
) -> Option<SemanticCallableIdV1> {
    if body.blocks().len() != 2 {
        return None;
    }
    // Canonical order follows identity, not the original rustc block ordinal.
    let entry = body.blocks().get(body.entry().index() as usize)?;
    let SemanticTerminatorKindV1::Call(call) = entry.terminator().kind() else {
        return None;
    };
    let destination = call.destination()?;
    if destination.edge().target() == body.entry() {
        return None;
    }
    let exit = body
        .blocks()
        .get(destination.edge().target().index() as usize)?;
    if !entry.statements().is_empty()
        || !exit.statements().is_empty()
        || !matches!(exit.terminator().kind(), SemanticTerminatorKindV1::Return)
    {
        return None;
    }
    (call.arguments().is_empty()
        && destination.place().local() == output_local
        && call.variadic_argument_abis().is_empty()
        && destination.place().projections().is_empty()
        && destination.place().ty() == output
        && destination.edge().role() == SemanticEdgeRoleV1::CallReturn
        && matches!(
            call.unwind(),
            SemanticUnwindActionV1::Unreachable | SemanticUnwindActionV1::Continue
        ))
    .then_some(call.callee())
}

fn getter_body(
    body: &SemanticFunctionDeclV1,
    types: SemanticKernelMathDeriveTypesV1,
) -> Option<SemanticCallableIdV1> {
    if body.locals().len() != 2
        || !abi_matches(
            body.abi(),
            &[types.context_reference],
            types.math,
            ReturnMode::Ignore,
        )
    {
        return None;
    }
    let output = local(body, types.math, SemanticLocalRoleV1::Return)?;
    local(
        body,
        types.context_reference,
        SemanticLocalRoleV1::Argument(0),
    )?;
    forwarding_call(body, output, types.math)
}

fn bridge_body_call(
    body: &SemanticFunctionDeclV1,
    types: SemanticKernelMathDeriveTypesV1,
) -> Option<SemanticCallableIdV1> {
    if body.locals().len() != 2 || !abi_matches(body.abi(), &[], types.math, ReturnMode::Ignore) {
        return None;
    }
    local(body, types.math, SemanticLocalRoleV1::Return)?;
    let output = local(body, types.unbranded_math, SemanticLocalRoleV1::Temporary)?;
    forwarding_call(body, output, types.unbranded_math)
}

fn bind_body(
    body: &SemanticFunctionDeclV1,
    types: SemanticPolicyMathBindTypesV1,
    declarations: &[SemanticTypeDeclV1],
) -> bool {
    let Some(marker) = bound_marker(declarations, types) else {
        return false;
    };
    bind_body_marker(body, types) == Some(marker)
}

fn bind_body_marker(
    body: &SemanticFunctionDeclV1,
    types: SemanticPolicyMathBindTypesV1,
) -> Option<SemanticTypeIdV1> {
    if body.locals().len() != 3
        || !abi_matches(
            body.abi(),
            &[types.math_reference, types.policy_reference],
            types.bound,
            ReturnMode::Pair,
        )
    {
        return None;
    }
    let output = local(body, types.bound, SemanticLocalRoleV1::Return)?;
    let math = local(body, types.math_reference, SemanticLocalRoleV1::Argument(0))?;
    let policy = local(
        body,
        types.policy_reference,
        SemanticLocalRoleV1::Argument(1),
    )?;
    if body.blocks().len() != 1 {
        return None;
    }
    let block = body.blocks().get(body.entry().index() as usize)?;
    if !matches!(block.terminator().kind(), SemanticTerminatorKindV1::Return) {
        return None;
    }
    let [statement] = block.statements() else {
        return None;
    };
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return None;
    };
    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind() else {
        return None;
    };
    let operands = aggregate.operands();
    let [_, _, SemanticOperandV1::Constant(marker)] = operands else {
        return None;
    };
    (assignment.destination().local() == output
        && assignment.destination().projections().is_empty()
        && assignment.destination().ty() == types.bound
        && assignment.value().result_type() == types.bound
        && *aggregate.kind() == SemanticAggregateKindV1::Aggregate
        && operands[..2]
            .iter()
            .zip([
                (math, types.math_reference),
                (policy, types.policy_reference),
            ])
            .all(|(operand, (local, ty))| {
                matches!(operand, SemanticOperandV1::Copy(place) if place.local() == local
                && place.projections().is_empty() && place.ty() == ty)
            })
        && matches!(marker.value(), SemanticConstantValueV1::ZeroSized))
    .then_some(marker.ty())
}

pub(super) fn validate_math_derive_attachment(
    body: &SemanticFunctionDeclV1,
    record: SemanticKernelMathDeriveV1,
) -> Result<(), SemanticMirErrorV1> {
    let mut budget = MAX_BODY_BYTES;
    require(
        body.role() == SemanticFunctionRoleV1::InternalHelper
            && body.export().is_none()
            && getter_body(body, record.types)
                == Some(SemanticCallableIdV1::from_index(
                    record.bridge.function.index(),
                ))
            && SemanticDefinedMathBodyV1::observe(record.function(), body, &mut budget)?
                == record.origin,
    )
}

pub(super) fn validate_math_bind_attachment(
    body: &SemanticFunctionDeclV1,
    record: SemanticPolicyMathBindV1,
) -> Result<(), SemanticMirErrorV1> {
    let mut budget = MAX_BODY_BYTES;
    require(
        body.role() == SemanticFunctionRoleV1::InternalHelper
            && body.export().is_none()
            && bind_body_marker(body, record.types).is_some()
            && SemanticDefinedMathBodyV1::observe(record.function(), body, &mut budget)?
                == record.origin,
    )
}

/// Full roster/type/root revalidation is mandatory before attachment is admitted.
pub(super) fn validate_math_derive(
    context: &mut ValidationContextV1<'_>,
    function: SemanticFunctionIdV1,
    record: SemanticKernelMathDeriveV1,
) -> Result<(), SemanticMirErrorV1> {
    context.one()?;
    require(
        record.function() == function
            && kernel_capability_provenance_matches(context.request, record.provenance),
    )?;
    for ty in record.types.all() {
        context.type_reference(ty, SemanticMirLocationV1::Function(function))?;
    }
    let mut budget = validation_digest_budget(context, 2 * MAX_BODY_BYTES);
    let before = budget;
    let expected = SemanticKernelMathDeriveV1::observe(
        function,
        &context.request.functions,
        &context.request.callables,
        &context.request.types,
        record.types,
        record.provenance,
        record.kernel_brand,
        &mut budget,
    )
    .map_err(|error| digest_budget_error(context, error))?;
    charge_validation_work(context, ((before - budget) * 2) as usize)?;
    require(expected == record)
}

pub(super) fn validate_math_bind(
    context: &mut ValidationContextV1<'_>,
    function: SemanticFunctionIdV1,
    record: SemanticPolicyMathBindV1,
) -> Result<(), SemanticMirErrorV1> {
    context.one()?;
    require(
        record.function() == function
            && kernel_capability_provenance_matches(context.request, record.provenance),
    )?;
    for ty in record.types.all() {
        context.type_reference(ty, SemanticMirLocationV1::Function(function))?;
    }
    let mut budget = validation_digest_budget(context, MAX_BODY_BYTES);
    let before = budget;
    let expected = SemanticPolicyMathBindV1::observe(
        function,
        &context.request.functions,
        &context.request.callables,
        &context.request.types,
        record.types,
        record.provenance,
        record.policy,
        record.kernel_brand,
        &mut budget,
    )
    .map_err(|error| digest_budget_error(context, error))?;
    charge_validation_work(context, ((before - budget) * 2) as usize)?;
    require(expected == record)
}

fn validation_digest_budget(context: &ValidationContextV1<'_>, max: u64) -> u64 {
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
