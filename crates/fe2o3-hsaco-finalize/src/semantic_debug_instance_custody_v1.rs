//! Exact root-instance ownership for an admitted production semantic debug graph.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fmt,
};

use fe2o3_kernel_ir::{
    MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1, MAX_SEMANTIC_DEBUG_MAPPINGS_V1, SemanticDebugLayerV1,
    SemanticDebugLocationV1, SemanticDebugMapDocumentV1, SemanticDebugTransformationV1,
    SemanticDebugUnavailableReasonV1,
};
use sha2::{Digest, Sha256};

use crate::ContentIdentityV1;

/// Magic for the additive instance-custody wire.
pub const PRODUCTION_SEMANTIC_DEBUG_INSTANCE_CUSTODY_MAGIC_V1: [u8; 8] = *b"F2SDIC1\0";
/// Version of the additive instance-custody wire.
pub const PRODUCTION_SEMANTIC_DEBUG_INSTANCE_CUSTODY_VERSION_V1: u16 = 1;
/// Maximum canonical instance-custody bytes.
pub const MAX_PRODUCTION_SEMANTIC_DEBUG_INSTANCE_CUSTODY_BYTES_V1: usize = 64 * 1024 * 1024;
/// Maximum root-qualified function occurrences.
pub const MAX_PRODUCTION_SEMANTIC_DEBUG_FUNCTION_INSTANCES_V1: usize =
    MAX_SEMANTIC_DEBUG_MAPPINGS_V1;
/// Maximum root-qualified statement occurrences.
pub const MAX_PRODUCTION_SEMANTIC_DEBUG_STATEMENT_INSTANCES_V1: usize =
    MAX_SEMANTIC_DEBUG_MAPPINGS_V1;

const POLICY_V1: u16 = 1;
const HEADER_BYTES_V1: usize = 28;
const CONTENT_IDENTITY_BYTES_V1: usize = 40;
const FUNCTION_BYTES_V1: usize = 48;
const STATEMENT_FIXED_BYTES_V1: usize = 120;
const BINDING_FIELDS_V1: usize = 5;
const FUNCTION_ID_DOMAIN_V1: &[u8] = b"FE2O3/SEMANTIC-DEBUG-FUNCTION-INSTANCE/V1\0";
const STATEMENT_ID_DOMAIN_V1: &[u8] = b"FE2O3/SEMANTIC-DEBUG-STATEMENT-INSTANCE/V1\0";
const DOCUMENT_ID_DOMAIN_V1: &[u8] = b"FE2O3/SEMANTIC-DEBUG-INSTANCE-CUSTODY/V1\0";

/// Exact content axes needed to interpret an instance-custody document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSemanticDebugInstanceCustodyBindingV1 {
    source_map_v2: ContentIdentityV1,
    semantic_map_v1: ContentIdentityV1,
    semantic_mir: ContentIdentityV1,
    canonical_kir_v7: ContentIdentityV1,
    correspondence: ContentIdentityV1,
}

impl ProductionSemanticDebugInstanceCustodyBindingV1 {
    /// Binds five exact immutable producer/finalizer inputs.
    pub fn from_exact_bytes(
        source_map_v2: &[u8],
        semantic_map_v1: &[u8],
        semantic_mir: &[u8],
        canonical_kir_v7: &[u8],
        correspondence: &[u8],
    ) -> Result<Self, ProductionSemanticDebugInstanceCustodyErrorV1> {
        if [
            source_map_v2,
            semantic_map_v1,
            semantic_mir,
            canonical_kir_v7,
            correspondence,
        ]
        .iter()
        .any(|bytes| bytes.is_empty())
        {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidBinding);
        }
        Ok(Self {
            source_map_v2: ContentIdentityV1::calculate(source_map_v2),
            semantic_map_v1: ContentIdentityV1::calculate(semantic_map_v1),
            semantic_mir: ContentIdentityV1::calculate(semantic_mir),
            canonical_kir_v7: ContentIdentityV1::calculate(canonical_kir_v7),
            correspondence: ContentIdentityV1::calculate(correspondence),
        })
    }

    /// Exact Source Map V2 content identity.
    pub const fn source_map_v2(self) -> ContentIdentityV1 {
        self.source_map_v2
    }
    /// Exact finalized Semantic Debug Map V1 content identity.
    pub const fn semantic_map_v1(self) -> ContentIdentityV1 {
        self.semantic_map_v1
    }
    /// Exact canonical semantic MIR content identity.
    pub const fn semantic_mir(self) -> ContentIdentityV1 {
        self.semantic_mir
    }
    /// Exact canonical KIR V7 projection content identity.
    pub const fn canonical_kir_v7(self) -> ContentIdentityV1 {
        self.canonical_kir_v7
    }
    /// Exact root-qualified correspondence content identity.
    pub const fn correspondence(self) -> ContentIdentityV1 {
        self.correspondence
    }

    fn validate(self) -> Result<(), ProductionSemanticDebugInstanceCustodyErrorV1> {
        for identity in [
            self.source_map_v2,
            self.semantic_map_v1,
            self.semantic_mir,
            self.canonical_kir_v7,
            self.correspondence,
        ] {
            if identity.sha256() == &[0; 32] || identity.byte_len() == 0 {
                return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidBinding);
            }
        }
        Ok(())
    }
}

/// Exact role of one root-qualified function occurrence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ProductionSemanticDebugFunctionInstanceRoleV1 {
    /// Unique kernel entry owned by this root.
    KernelEntry,
    /// Internal helper, which may be the same physical KIR function for several roots.
    InternalHelper,
}

impl ProductionSemanticDebugFunctionInstanceRoleV1 {
    const fn code(self) -> u8 {
        match self {
            Self::KernelEntry => 1,
            Self::InternalHelper => 2,
        }
    }
}

/// Input used only after independent correspondence replay.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionSemanticDebugFunctionInstanceInputV1 {
    pub(crate) correspondence_owner: u32,
    pub(crate) semantic_function: u32,
    pub(crate) kernel_ir_function_ordinal: u32,
    pub(crate) role: ProductionSemanticDebugFunctionInstanceRoleV1,
}

/// Input used only after independent statement-span and graph replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProductionSemanticDebugStatementInstanceInputV1 {
    pub(crate) correspondence_owner: u32,
    pub(crate) semantic_function: u32,
    pub(crate) semantic_block: u32,
    pub(crate) statement: u32,
    pub(crate) source_node: [u8; 32],
    pub(crate) mir_node: [u8; 32],
    pub(crate) kir_nodes: Vec<[u8; 32]>,
}

/// One exact root-qualified function occurrence.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProductionSemanticDebugFunctionInstanceV1 {
    identity: [u8; 32],
    correspondence_owner: u32,
    semantic_function: u32,
    kernel_ir_function_ordinal: u32,
    role: ProductionSemanticDebugFunctionInstanceRoleV1,
}

impl ProductionSemanticDebugFunctionInstanceV1 {
    /// Stable identity derived from the custody binding and exact coordinates.
    pub const fn identity(self) -> [u8; 32] {
        self.identity
    }
    /// Semantic root that owns this occurrence.
    pub const fn correspondence_owner(self) -> u32 {
        self.correspondence_owner
    }
    /// Semantic function reused by this occurrence.
    pub const fn semantic_function(self) -> u32 {
        self.semantic_function
    }
    /// Absolute physical KIR function ordinal.
    pub const fn kernel_ir_function_ordinal(self) -> u32 {
        self.kernel_ir_function_ordinal
    }
    /// Exact entry/helper role.
    pub const fn role(self) -> ProductionSemanticDebugFunctionInstanceRoleV1 {
        self.role
    }
}

/// One exact root-qualified occurrence of a semantic statement.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ProductionSemanticDebugStatementInstanceV1 {
    identity: [u8; 32],
    correspondence_owner: u32,
    semantic_function: u32,
    semantic_block: u32,
    statement: u32,
    source_node: [u8; 32],
    mir_node: [u8; 32],
    kir_nodes: Box<[[u8; 32]]>,
}

impl ProductionSemanticDebugStatementInstanceV1 {
    /// Stable identity derived from the custody binding and exact coordinates.
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
    /// Semantic root that owns this occurrence.
    pub const fn correspondence_owner(&self) -> u32 {
        self.correspondence_owner
    }
    /// Semantic function containing the statement.
    pub const fn semantic_function(&self) -> u32 {
        self.semantic_function
    }
    /// Semantic basic-block ordinal.
    pub const fn semantic_block(&self) -> u32 {
        self.semantic_block
    }
    /// Semantic statement ordinal.
    pub const fn statement(&self) -> u32 {
        self.statement
    }
    /// Shared Source node identity.
    pub const fn source_node(&self) -> [u8; 32] {
        self.source_node
    }
    /// Shared MIR node identity.
    pub const fn mir_node(&self) -> [u8; 32] {
        self.mir_node
    }
    /// Exact physical KIR nodes, empty only for authenticated elimination.
    pub fn kir_nodes(&self) -> &[[u8; 32]] {
        &self.kir_nodes
    }
    /// Reports exact authenticated elimination for this occurrence.
    pub const fn is_eliminated(&self) -> bool {
        self.kir_nodes.is_empty()
    }
}

/// Identity of one canonical instance-custody document.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionSemanticDebugInstanceCustodyIdentityV1([u8; 32]);

impl ProductionSemanticDebugInstanceCustodyIdentityV1 {
    /// Returns the stable canonical identity.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Canonical, association-only root-instance ownership evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProductionSemanticDebugInstanceCustodyV1 {
    identity: ProductionSemanticDebugInstanceCustodyIdentityV1,
    binding: ProductionSemanticDebugInstanceCustodyBindingV1,
    functions: Box<[ProductionSemanticDebugFunctionInstanceV1]>,
    statements: Box<[ProductionSemanticDebugStatementInstanceV1]>,
    canonical_bytes: Box<[u8]>,
}

/// Canonically decoded custody claims which have not been re-admitted by finalizer replay.
///
/// This type deliberately exposes no function or statement records. Content hashes authenticate
/// bytes, not the claimed root/function roster; only [`Self::admit_exact_replay_v1`] can promote a
/// claim to [`ProductionSemanticDebugInstanceCustodyV1`].
pub struct InertProductionSemanticDebugInstanceCustodyV1 {
    claimed: ProductionSemanticDebugInstanceCustodyV1,
}

impl fmt::Debug for InertProductionSemanticDebugInstanceCustodyV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertProductionSemanticDebugInstanceCustodyV1")
            .field("claimed_identity", &self.claimed_identity())
            .field("claimed_binding", &self.claimed_binding())
            .finish_non_exhaustive()
    }
}

impl InertProductionSemanticDebugInstanceCustodyV1 {
    /// Strictly decodes canonical claims without granting root-instance custody.
    pub fn from_canonical_bytes(
        bytes: &[u8],
        semantic_map_bytes: &[u8],
    ) -> Result<Self, ProductionSemanticDebugInstanceCustodyErrorV1> {
        Ok(Self {
            claimed: ProductionSemanticDebugInstanceCustodyV1::decode_claimed_canonical_bytes(
                bytes,
                semantic_map_bytes,
            )?,
        })
    }

    /// Returns the identity claimed by the inert wire document.
    pub const fn claimed_identity(&self) -> ProductionSemanticDebugInstanceCustodyIdentityV1 {
        self.claimed.identity
    }

    /// Returns the exact content binding claimed by the inert wire document.
    pub const fn claimed_binding(&self) -> ProductionSemanticDebugInstanceCustodyBindingV1 {
        self.claimed.binding
    }

    /// Returns the retained canonical claims for persistence or transport.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.claimed.canonical_bytes
    }

    /// Promotes claims only when they exactly equal a fresh finalizer replay.
    pub fn admit_exact_replay_v1(
        self,
        exact: ProductionSemanticDebugInstanceCustodyV1,
    ) -> Result<
        ProductionSemanticDebugInstanceCustodyV1,
        ProductionSemanticDebugInstanceCustodyErrorV1,
    > {
        if self.claimed != exact {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::ExactReplayMismatch);
        }
        Ok(exact)
    }

    /// An inert wire claim never grants execution or debugger-control authority.
    pub const fn grants_execution_authority(&self) -> bool {
        false
    }
}

impl ProductionSemanticDebugInstanceCustodyV1 {
    pub(crate) fn from_replayed_inputs(
        binding: ProductionSemanticDebugInstanceCustodyBindingV1,
        function_inputs: Vec<ProductionSemanticDebugFunctionInstanceInputV1>,
        statement_inputs: Vec<ProductionSemanticDebugStatementInstanceInputV1>,
        semantic_map: &SemanticDebugMapDocumentV1,
    ) -> Result<Self, ProductionSemanticDebugInstanceCustodyErrorV1> {
        binding.validate()?;
        if function_inputs.is_empty()
            || function_inputs.len() > MAX_PRODUCTION_SEMANTIC_DEBUG_FUNCTION_INSTANCES_V1
            || statement_inputs.len() > MAX_PRODUCTION_SEMANTIC_DEBUG_STATEMENT_INSTANCES_V1
        {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::ResourceLimit);
        }
        let mut functions = Vec::new();
        functions
            .try_reserve_exact(function_inputs.len())
            .map_err(|_| ProductionSemanticDebugInstanceCustodyErrorV1::AllocationFailure)?;
        for input in function_inputs {
            functions.push(ProductionSemanticDebugFunctionInstanceV1 {
                identity: function_identity(binding, input),
                correspondence_owner: input.correspondence_owner,
                semantic_function: input.semantic_function,
                kernel_ir_function_ordinal: input.kernel_ir_function_ordinal,
                role: input.role,
            });
        }
        functions.sort_unstable_by_key(function_key);
        let mut statements = Vec::new();
        statements
            .try_reserve_exact(statement_inputs.len())
            .map_err(|_| ProductionSemanticDebugInstanceCustodyErrorV1::AllocationFailure)?;
        for mut input in statement_inputs {
            input.kir_nodes.sort_unstable();
            statements.push(ProductionSemanticDebugStatementInstanceV1 {
                identity: statement_identity(
                    binding,
                    StatementIdentityFieldsV1 {
                        correspondence_owner: input.correspondence_owner,
                        semantic_function: input.semantic_function,
                        semantic_block: input.semantic_block,
                        statement: input.statement,
                        source_node: input.source_node,
                        mir_node: input.mir_node,
                        kir_nodes: &input.kir_nodes,
                    },
                ),
                correspondence_owner: input.correspondence_owner,
                semantic_function: input.semantic_function,
                semantic_block: input.semantic_block,
                statement: input.statement,
                source_node: input.source_node,
                mir_node: input.mir_node,
                kir_nodes: input.kir_nodes.into_boxed_slice(),
            });
        }
        statements.sort_unstable_by_key(statement_key);
        validate_records(binding, &functions, &statements, semantic_map)?;
        let canonical_bytes = encode(binding, &functions, &statements)?;
        let identity = document_identity(&canonical_bytes);
        Ok(Self {
            identity,
            binding,
            functions: functions.into_boxed_slice(),
            statements: statements.into_boxed_slice(),
            canonical_bytes: canonical_bytes.into_boxed_slice(),
        })
    }

    fn decode_claimed_canonical_bytes(
        bytes: &[u8],
        semantic_map_bytes: &[u8],
    ) -> Result<Self, ProductionSemanticDebugInstanceCustodyErrorV1> {
        if bytes.len() < HEADER_BYTES_V1 + BINDING_FIELDS_V1 * CONTENT_IDENTITY_BYTES_V1
            || bytes.len() > MAX_PRODUCTION_SEMANTIC_DEBUG_INSTANCE_CUSTODY_BYTES_V1
        {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidLength);
        }
        let semantic_map =
            SemanticDebugMapDocumentV1::from_canonical_json_bytes(semantic_map_bytes)
                .map_err(|_| ProductionSemanticDebugInstanceCustodyErrorV1::InvalidSemanticMap)?;
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != PRODUCTION_SEMANTIC_DEBUG_INSTANCE_CUSTODY_MAGIC_V1
            || reader.u16()? != PRODUCTION_SEMANTIC_DEBUG_INSTANCE_CUSTODY_VERSION_V1
            || reader.u16()? != POLICY_V1
        {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidEncoding);
        }
        let function_count = reader.count(MAX_PRODUCTION_SEMANTIC_DEBUG_FUNCTION_INSTANCES_V1)?;
        let statement_count = reader.count(MAX_PRODUCTION_SEMANTIC_DEBUG_STATEMENT_INSTANCES_V1)?;
        let kir_reference_count = reader.count(MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1)?;
        if reader.u32()? != 0 {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidEncoding);
        }
        let binding = ProductionSemanticDebugInstanceCustodyBindingV1 {
            source_map_v2: reader.content_identity()?,
            semantic_map_v1: reader.content_identity()?,
            semantic_mir: reader.content_identity()?,
            canonical_kir_v7: reader.content_identity()?,
            correspondence: reader.content_identity()?,
        };
        binding.validate()?;
        if !binding.semantic_map_v1.matches(semantic_map_bytes) {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::BindingMismatch);
        }
        let expected_body_bytes = function_count
            .checked_mul(FUNCTION_BYTES_V1)
            .and_then(|length| {
                length.checked_add(statement_count.checked_mul(STATEMENT_FIXED_BYTES_V1)?)
            })
            .and_then(|length| length.checked_add(kir_reference_count.checked_mul(32)?))
            .ok_or(ProductionSemanticDebugInstanceCustodyErrorV1::ResourceLimit)?;
        if reader.remaining() != expected_body_bytes {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidLength);
        }
        let mut functions = Vec::new();
        functions
            .try_reserve_exact(function_count)
            .map_err(|_| ProductionSemanticDebugInstanceCustodyErrorV1::AllocationFailure)?;
        for _ in 0..function_count {
            let identity = reader.fixed()?;
            let correspondence_owner = reader.u32()?;
            let semantic_function = reader.u32()?;
            let kernel_ir_function_ordinal = reader.u32()?;
            let role = match reader.u8()? {
                1 => ProductionSemanticDebugFunctionInstanceRoleV1::KernelEntry,
                2 => ProductionSemanticDebugFunctionInstanceRoleV1::InternalHelper,
                _ => return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidRole),
            };
            if reader.fixed::<3>()? != [0; 3] {
                return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidEncoding);
            }
            functions.push(ProductionSemanticDebugFunctionInstanceV1 {
                identity,
                correspondence_owner,
                semantic_function,
                kernel_ir_function_ordinal,
                role,
            });
        }
        let mut statements = Vec::new();
        statements
            .try_reserve_exact(statement_count)
            .map_err(|_| ProductionSemanticDebugInstanceCustodyErrorV1::AllocationFailure)?;
        let mut observed_references = 0_usize;
        for _ in 0..statement_count {
            let identity = reader.fixed()?;
            let correspondence_owner = reader.u32()?;
            let semantic_function = reader.u32()?;
            let semantic_block = reader.u32()?;
            let statement = reader.u32()?;
            let source_node = reader.fixed()?;
            let mir_node = reader.fixed()?;
            let eliminated = match reader.u8()? {
                0 => false,
                1 => true,
                _ => return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidEncoding),
            };
            if reader.fixed::<3>()? != [0; 3] {
                return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidEncoding);
            }
            let count = reader.count(MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1)?;
            observed_references = observed_references
                .checked_add(count)
                .ok_or(ProductionSemanticDebugInstanceCustodyErrorV1::ResourceLimit)?;
            let mut kir_nodes = Vec::new();
            kir_nodes
                .try_reserve_exact(count)
                .map_err(|_| ProductionSemanticDebugInstanceCustodyErrorV1::AllocationFailure)?;
            for _ in 0..count {
                kir_nodes.push(reader.fixed()?);
            }
            if eliminated != kir_nodes.is_empty() {
                return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidStatement);
            }
            statements.push(ProductionSemanticDebugStatementInstanceV1 {
                identity,
                correspondence_owner,
                semantic_function,
                semantic_block,
                statement,
                source_node,
                mir_node,
                kir_nodes: kir_nodes.into_boxed_slice(),
            });
        }
        if observed_references != kir_reference_count || !reader.finished() {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidEncoding);
        }
        validate_records(binding, &functions, &statements, &semantic_map)?;
        if encode(binding, &functions, &statements)? != bytes {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::NonCanonicalEncoding);
        }
        Ok(Self {
            identity: document_identity(bytes),
            binding,
            functions: functions.into_boxed_slice(),
            statements: statements.into_boxed_slice(),
            canonical_bytes: {
                let mut canonical = Vec::new();
                canonical.try_reserve_exact(bytes.len()).map_err(|_| {
                    ProductionSemanticDebugInstanceCustodyErrorV1::AllocationFailure
                })?;
                canonical.extend_from_slice(bytes);
                canonical.into_boxed_slice()
            },
        })
    }

    /// Revalidates every exact content axis without granting authority.
    pub fn validate_exact_inputs(
        &self,
        source_map_v2: &[u8],
        semantic_map_v1: &[u8],
        semantic_mir: &[u8],
        canonical_kir_v7: &[u8],
        correspondence: &[u8],
    ) -> Result<(), ProductionSemanticDebugInstanceCustodyErrorV1> {
        if !self.binding.source_map_v2.matches(source_map_v2)
            || !self.binding.semantic_map_v1.matches(semantic_map_v1)
            || !self.binding.semantic_mir.matches(semantic_mir)
            || !self.binding.canonical_kir_v7.matches(canonical_kir_v7)
            || !self.binding.correspondence.matches(correspondence)
        {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::BindingMismatch);
        }
        Ok(())
    }

    /// Canonical document identity.
    pub const fn identity(&self) -> ProductionSemanticDebugInstanceCustodyIdentityV1 {
        self.identity
    }
    /// Exact content binding.
    pub const fn binding(&self) -> ProductionSemanticDebugInstanceCustodyBindingV1 {
        self.binding
    }
    /// Canonically ordered function occurrences.
    pub fn functions(&self) -> &[ProductionSemanticDebugFunctionInstanceV1] {
        &self.functions
    }
    /// Canonically ordered statement occurrences.
    pub fn statements(&self) -> &[ProductionSemanticDebugStatementInstanceV1] {
        &self.statements
    }
    /// Exact canonical bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Finds one exact `(root owner, semantic function)` occurrence.
    pub fn function_instance(
        &self,
        owner: u32,
        semantic_function: u32,
    ) -> Option<&ProductionSemanticDebugFunctionInstanceV1> {
        self.functions
            .binary_search_by_key(&(owner, semantic_function), function_key)
            .ok()
            .map(|index| &self.functions[index])
    }
    /// Deterministic forward query from a semantic function to every root occurrence.
    pub fn function_instances_for_semantic(
        &self,
        semantic_function: u32,
    ) -> impl Iterator<Item = &ProductionSemanticDebugFunctionInstanceV1> {
        self.functions
            .iter()
            .filter(move |record| record.semantic_function == semantic_function)
    }
    /// Deterministic reverse query from one physical KIR function to every root occurrence.
    pub fn function_instances_for_kir(
        &self,
        function_ordinal: u32,
    ) -> impl Iterator<Item = &ProductionSemanticDebugFunctionInstanceV1> {
        self.functions
            .iter()
            .filter(move |record| record.kernel_ir_function_ordinal == function_ordinal)
    }
    /// Finds one exact root-qualified statement occurrence.
    pub fn statement_instance(
        &self,
        owner: u32,
        semantic_function: u32,
        semantic_block: u32,
        statement: u32,
    ) -> Option<&ProductionSemanticDebugStatementInstanceV1> {
        self.statements
            .binary_search_by_key(
                &(owner, semantic_function, semantic_block, statement),
                statement_key,
            )
            .ok()
            .map(|index| &self.statements[index])
    }
    /// Deterministic reverse query from a physical KIR node to all owning root occurrences.
    pub fn statement_instances_for_kir_node(
        &self,
        identity: [u8; 32],
    ) -> impl Iterator<Item = &ProductionSemanticDebugStatementInstanceV1> {
        self.statements
            .iter()
            .filter(move |record| record.kir_nodes.binary_search(&identity).is_ok())
    }
    /// This association-only record never grants execution or debugger-control authority.
    pub const fn grants_execution_authority(&self) -> bool {
        false
    }
}

fn function_key(record: &ProductionSemanticDebugFunctionInstanceV1) -> (u32, u32) {
    (record.correspondence_owner, record.semantic_function)
}
fn statement_key(record: &ProductionSemanticDebugStatementInstanceV1) -> (u32, u32, u32, u32) {
    (
        record.correspondence_owner,
        record.semantic_function,
        record.semantic_block,
        record.statement,
    )
}

fn validate_records(
    binding: ProductionSemanticDebugInstanceCustodyBindingV1,
    functions: &[ProductionSemanticDebugFunctionInstanceV1],
    statements: &[ProductionSemanticDebugStatementInstanceV1],
    map: &SemanticDebugMapDocumentV1,
) -> Result<(), ProductionSemanticDebugInstanceCustodyErrorV1> {
    if functions.is_empty()
        || functions.len() > MAX_PRODUCTION_SEMANTIC_DEBUG_FUNCTION_INSTANCES_V1
        || statements.len() > MAX_PRODUCTION_SEMANTIC_DEBUG_STATEMENT_INSTANCES_V1
    {
        return Err(ProductionSemanticDebugInstanceCustodyErrorV1::ResourceLimit);
    }
    if functions
        .windows(2)
        .any(|pair| function_key(&pair[0]) >= function_key(&pair[1]))
    {
        return Err(ProductionSemanticDebugInstanceCustodyErrorV1::NonCanonicalOrder);
    }
    let mut entries = BTreeMap::<u32, usize>::new();
    let mut physical = BTreeMap::new();
    for record in functions {
        let input = ProductionSemanticDebugFunctionInstanceInputV1 {
            correspondence_owner: record.correspondence_owner,
            semantic_function: record.semantic_function,
            kernel_ir_function_ordinal: record.kernel_ir_function_ordinal,
            role: record.role,
        };
        if record.identity != function_identity(binding, input) {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidIdentity);
        }
        if record.role == ProductionSemanticDebugFunctionInstanceRoleV1::KernelEntry {
            if record.correspondence_owner != record.semantic_function {
                return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidFunction);
            }
            *entries.entry(record.correspondence_owner).or_default() += 1;
        } else {
            entries.entry(record.correspondence_owner).or_default();
        }
        if let Some((semantic, role)) = physical.insert(
            record.kernel_ir_function_ordinal,
            (record.semantic_function, record.role),
        ) && (semantic != record.semantic_function
            || role != ProductionSemanticDebugFunctionInstanceRoleV1::InternalHelper
            || record.role != ProductionSemanticDebugFunctionInstanceRoleV1::InternalHelper)
        {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::PhysicalFunctionOverlap);
        }
    }
    if entries.values().any(|entry_count| *entry_count != 1) {
        return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidFunction);
    }
    if statements
        .windows(2)
        .any(|pair| statement_key(&pair[0]) >= statement_key(&pair[1]))
    {
        return Err(ProductionSemanticDebugInstanceCustodyErrorV1::NonCanonicalOrder);
    }
    let mut groups = BTreeMap::new();
    let mut kir_owners = BTreeMap::new();
    let mut total_references = 0_usize;
    for record in statements {
        total_references = total_references
            .checked_add(record.kir_nodes.len())
            .ok_or(ProductionSemanticDebugInstanceCustodyErrorV1::ResourceLimit)?;
        if total_references > MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1
            || record.source_node == [0; 32]
            || record.mir_node == [0; 32]
            || record.kir_nodes.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidStatement);
        }
        let function = functions
            .binary_search_by_key(
                &(record.correspondence_owner, record.semantic_function),
                function_key,
            )
            .ok()
            .map(|index| functions[index])
            .ok_or(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidStatement)?;
        if record.identity
            != statement_identity(
                binding,
                StatementIdentityFieldsV1 {
                    correspondence_owner: record.correspondence_owner,
                    semantic_function: record.semantic_function,
                    semantic_block: record.semantic_block,
                    statement: record.statement,
                    source_node: record.source_node,
                    mir_node: record.mir_node,
                    kir_nodes: &record.kir_nodes,
                },
            )
        {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidIdentity);
        }
        if map
            .node(record.source_node)
            .is_none_or(|node| node.layer() != SemanticDebugLayerV1::Source)
            || map.node(record.mir_node).is_none_or(|node| {
                node.location()
                    != SemanticDebugLocationV1::Mir {
                        body_ordinal: u64::from(record.semantic_function),
                        block_ordinal: u64::from(record.semantic_block),
                        statement_ordinal: u64::from(record.statement),
                    }
            })
        {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidSemanticMap);
        }
        for identity in record.kir_nodes.iter().copied() {
            let Some(node) = map.node(identity) else {
                return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidSemanticMap);
            };
            if !matches!(node.location(), SemanticDebugLocationV1::Kir { function_ordinal, .. } if function_ordinal == u64::from(function.kernel_ir_function_ordinal))
            {
                return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidSemanticMap);
            }
            let site = (
                record.semantic_function,
                record.semantic_block,
                record.statement,
            );
            if let Some(previous) = kir_owners.insert(identity, site)
                && previous != site
            {
                return Err(ProductionSemanticDebugInstanceCustodyErrorV1::NodeOverlap);
            }
        }
        let key = (
            record.semantic_function,
            record.semantic_block,
            record.statement,
        );
        let value = (
            record.source_node,
            record.mir_node,
            record.kir_nodes.as_ref(),
        );
        if let Some(previous) = groups.insert(key, value)
            && previous != value
        {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InstanceSubstitution);
        }
    }
    let mut expected_nodes = BTreeSet::new();
    let mut expected_mapping_inputs = BTreeSet::new();
    let mut mappings_by_input = BTreeMap::new();
    for (index, mapping) in map.mappings().iter().enumerate() {
        if mapping.inputs().len() != 1
            || mappings_by_input
                .insert(mapping.inputs()[0], index)
                .is_some()
        {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidSemanticMap);
        }
    }
    for (source, mir, kir) in groups.values() {
        expected_nodes.insert(*source);
        expected_nodes.insert(*mir);
        expected_nodes.extend(kir.iter().copied());
        expected_mapping_inputs.insert(*source);
        expected_mapping_inputs.insert(*mir);
        let source_mapping = mappings_by_input
            .get(source)
            .and_then(|index| map.mappings().get(*index))
            .ok_or(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidSemanticMap)?;
        if source_mapping.input_layer() != SemanticDebugLayerV1::Source
            || source_mapping.output_layer() != SemanticDebugLayerV1::Mir
            || source_mapping.inputs() != [*source]
            || source_mapping.output().nodes() != [*mir]
            || source_mapping.transformation() != SemanticDebugTransformationV1::Preserved
        {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidSemanticMap);
        }
        let mir_mapping = mappings_by_input
            .get(mir)
            .and_then(|index| map.mappings().get(*index))
            .ok_or(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidSemanticMap)?;
        let valid_output = if kir.is_empty() {
            mir_mapping.output().nodes().is_empty()
                && mir_mapping.output().reason()
                    == Some(SemanticDebugUnavailableReasonV1::Eliminated)
                && mir_mapping.transformation() == SemanticDebugTransformationV1::Eliminated
        } else {
            mir_mapping.output().nodes() == *kir
                && mir_mapping.output().reason().is_none()
                && mir_mapping.transformation()
                    == if kir.len() == 1 {
                        SemanticDebugTransformationV1::Preserved
                    } else {
                        SemanticDebugTransformationV1::Duplicated
                    }
        };
        if mir_mapping.input_layer() != SemanticDebugLayerV1::Mir
            || mir_mapping.output_layer() != SemanticDebugLayerV1::Kir
            || mir_mapping.inputs() != [*mir]
            || !valid_output
        {
            return Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidSemanticMap);
        }
    }
    if map
        .nodes()
        .iter()
        .map(|node| node.identity())
        .collect::<BTreeSet<_>>()
        != expected_nodes
        || mappings_by_input.keys().copied().collect::<BTreeSet<_>>() != expected_mapping_inputs
    {
        return Err(ProductionSemanticDebugInstanceCustodyErrorV1::IncompleteCoverage);
    }
    Ok(())
}

fn function_identity(
    binding: ProductionSemanticDebugInstanceCustodyBindingV1,
    input: ProductionSemanticDebugFunctionInstanceInputV1,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(FUNCTION_ID_DOMAIN_V1);
    digest.update(binding.correspondence.sha256());
    digest.update(binding.canonical_kir_v7.sha256());
    digest.update(input.correspondence_owner.to_le_bytes());
    digest.update(input.semantic_function.to_le_bytes());
    digest.update(input.kernel_ir_function_ordinal.to_le_bytes());
    digest.update([input.role.code()]);
    digest.finalize().into()
}
struct StatementIdentityFieldsV1<'a> {
    correspondence_owner: u32,
    semantic_function: u32,
    semantic_block: u32,
    statement: u32,
    source_node: [u8; 32],
    mir_node: [u8; 32],
    kir_nodes: &'a [[u8; 32]],
}

fn statement_identity(
    binding: ProductionSemanticDebugInstanceCustodyBindingV1,
    fields: StatementIdentityFieldsV1<'_>,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(STATEMENT_ID_DOMAIN_V1);
    digest.update(binding.semantic_map_v1.sha256());
    digest.update(binding.correspondence.sha256());
    for value in [
        fields.correspondence_owner,
        fields.semantic_function,
        fields.semantic_block,
        fields.statement,
    ] {
        digest.update(value.to_le_bytes());
    }
    digest.update(fields.source_node);
    digest.update(fields.mir_node);
    digest.update((fields.kir_nodes.len() as u64).to_le_bytes());
    for node in fields.kir_nodes {
        digest.update(node);
    }
    digest.finalize().into()
}
fn document_identity(bytes: &[u8]) -> ProductionSemanticDebugInstanceCustodyIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(DOCUMENT_ID_DOMAIN_V1);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    ProductionSemanticDebugInstanceCustodyIdentityV1(digest.finalize().into())
}

fn encode(
    binding: ProductionSemanticDebugInstanceCustodyBindingV1,
    functions: &[ProductionSemanticDebugFunctionInstanceV1],
    statements: &[ProductionSemanticDebugStatementInstanceV1],
) -> Result<Vec<u8>, ProductionSemanticDebugInstanceCustodyErrorV1> {
    let references = statements
        .iter()
        .try_fold(0_usize, |count, record| {
            count.checked_add(record.kir_nodes.len())
        })
        .ok_or(ProductionSemanticDebugInstanceCustodyErrorV1::ResourceLimit)?;
    if references > MAX_SEMANTIC_DEBUG_MAPPING_REFERENCES_V1 {
        return Err(ProductionSemanticDebugInstanceCustodyErrorV1::ResourceLimit);
    }
    let length = HEADER_BYTES_V1
        .checked_add(BINDING_FIELDS_V1 * CONTENT_IDENTITY_BYTES_V1)
        .and_then(|value| value.checked_add(functions.len().checked_mul(FUNCTION_BYTES_V1)?))
        .and_then(|value| {
            value.checked_add(statements.len().checked_mul(STATEMENT_FIXED_BYTES_V1)?)
        })
        .and_then(|value| value.checked_add(references.checked_mul(32)?))
        .ok_or(ProductionSemanticDebugInstanceCustodyErrorV1::ResourceLimit)?;
    if length > MAX_PRODUCTION_SEMANTIC_DEBUG_INSTANCE_CUSTODY_BYTES_V1 {
        return Err(ProductionSemanticDebugInstanceCustodyErrorV1::ResourceLimit);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| ProductionSemanticDebugInstanceCustodyErrorV1::AllocationFailure)?;
    bytes.extend_from_slice(&PRODUCTION_SEMANTIC_DEBUG_INSTANCE_CUSTODY_MAGIC_V1);
    bytes.extend_from_slice(&PRODUCTION_SEMANTIC_DEBUG_INSTANCE_CUSTODY_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&POLICY_V1.to_le_bytes());
    bytes.extend_from_slice(&(functions.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(statements.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&(references as u32).to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    for identity in [
        binding.source_map_v2,
        binding.semantic_map_v1,
        binding.semantic_mir,
        binding.canonical_kir_v7,
        binding.correspondence,
    ] {
        bytes.extend_from_slice(identity.sha256());
        bytes.extend_from_slice(&identity.byte_len().to_le_bytes());
    }
    for record in functions {
        bytes.extend_from_slice(&record.identity);
        bytes.extend_from_slice(&record.correspondence_owner.to_le_bytes());
        bytes.extend_from_slice(&record.semantic_function.to_le_bytes());
        bytes.extend_from_slice(&record.kernel_ir_function_ordinal.to_le_bytes());
        bytes.push(record.role.code());
        bytes.extend_from_slice(&[0; 3]);
    }
    for record in statements {
        bytes.extend_from_slice(&record.identity);
        bytes.extend_from_slice(&record.correspondence_owner.to_le_bytes());
        bytes.extend_from_slice(&record.semantic_function.to_le_bytes());
        bytes.extend_from_slice(&record.semantic_block.to_le_bytes());
        bytes.extend_from_slice(&record.statement.to_le_bytes());
        bytes.extend_from_slice(&record.source_node);
        bytes.extend_from_slice(&record.mir_node);
        bytes.push(u8::from(record.kir_nodes.is_empty()));
        bytes.extend_from_slice(&[0; 3]);
        bytes.extend_from_slice(&(record.kir_nodes.len() as u32).to_le_bytes());
        for identity in record.kir_nodes.iter() {
            bytes.extend_from_slice(identity);
        }
    }
    debug_assert_eq!(bytes.len(), length);
    Ok(bytes)
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], ProductionSemanticDebugInstanceCustodyErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ProductionSemanticDebugInstanceCustodyErrorV1::ResourceLimit)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProductionSemanticDebugInstanceCustodyErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], ProductionSemanticDebugInstanceCustodyErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProductionSemanticDebugInstanceCustodyErrorV1::Truncated)
    }
    fn u8(&mut self) -> Result<u8, ProductionSemanticDebugInstanceCustodyErrorV1> {
        Ok(self.fixed::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, ProductionSemanticDebugInstanceCustodyErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }
    fn u32(&mut self) -> Result<u32, ProductionSemanticDebugInstanceCustodyErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }
    fn count(
        &mut self,
        maximum: usize,
    ) -> Result<usize, ProductionSemanticDebugInstanceCustodyErrorV1> {
        let count = self.u32()? as usize;
        if count > maximum || count > self.bytes.len() {
            Err(ProductionSemanticDebugInstanceCustodyErrorV1::ResourceLimit)
        } else {
            Ok(count)
        }
    }
    fn content_identity(
        &mut self,
    ) -> Result<ContentIdentityV1, ProductionSemanticDebugInstanceCustodyErrorV1> {
        Ok(ContentIdentityV1::from_parts(
            self.fixed()?,
            u64::from_le_bytes(self.fixed()?),
        ))
    }
    const fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
    const fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }
}

/// Typed instance-custody validation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticDebugInstanceCustodyErrorV1 {
    InvalidLength,
    InvalidEncoding,
    NonCanonicalEncoding,
    NonCanonicalOrder,
    InvalidBinding,
    BindingMismatch,
    InvalidRole,
    InvalidIdentity,
    InvalidFunction,
    InvalidStatement,
    InstanceSubstitution,
    PhysicalFunctionOverlap,
    NodeOverlap,
    IncompleteCoverage,
    ExactReplayMismatch,
    InvalidSemanticMap,
    ResourceLimit,
    AllocationFailure,
    Truncated,
}
impl fmt::Display for ProductionSemanticDebugInstanceCustodyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid production semantic-debug instance custody: {self:?}"
        )
    }
}
impl Error for ProductionSemanticDebugInstanceCustodyErrorV1 {}

/// Typed absence for paths without authenticated root-instance correspondence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionSemanticDebugInstanceCustodyUnavailableV1 {
    /// Artifact-only or caller-supplied map admission has no production correspondence.
    CorrespondenceUnavailable,
    /// Frozen legacy V4 correspondence has no exact root-owner field.
    LegacyCorrespondenceV4,
}

/// Available exact custody or a truthful typed boundary.
#[derive(Debug)]
pub enum ProductionSemanticDebugInstanceCustodyAvailabilityV1 {
    /// Exact content-bound root-instance ownership.
    Available(Box<ProductionSemanticDebugInstanceCustodyV1>),
    /// Exact reason custody cannot be established.
    Unavailable(ProductionSemanticDebugInstanceCustodyUnavailableV1),
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        DebugSourceMapSpanV1, SemanticDebugBoundaryDirectionV1, SemanticDebugBoundaryReasonV1,
        SemanticDebugBoundaryV1, SemanticDebugContentIdentityV1, SemanticDebugLocationV1,
        SemanticDebugMapBindingV1, SemanticDebugMapErrorV1, SemanticDebugMappingOutputV1,
        SemanticDebugMappingV1, SemanticDebugNodeV1,
    };

    use super::*;

    const SOURCE_MAP: &[u8] = b"source-map-v2";
    const SEMANTIC_MIR: &[u8] = b"semantic-mir";
    const CANONICAL_KIR: &[u8] = b"canonical-kir-v7";
    const CORRESPONDENCE: &[u8] = b"multi-root-correspondence-v2";

    fn content(bytes: &[u8]) -> SemanticDebugContentIdentityV1 {
        SemanticDebugContentIdentityV1::calculate(bytes).unwrap()
    }

    fn semantic_map() -> SemanticDebugMapDocumentV1 {
        let binding = SemanticDebugMapBindingV1::new(
            content(SOURCE_MAP),
            content(SEMANTIC_MIR),
            content(CANONICAL_KIR),
            content(b"schedule"),
            content(b"llvm"),
            content(b"hsaco"),
        )
        .unwrap();
        let source = [0x11; 32];
        let mir = [0x12; 32];
        let kir = [0x13; 32];
        SemanticDebugMapDocumentV1::new_partial(
            binding,
            vec![
                SemanticDebugNodeV1::new(
                    source,
                    SemanticDebugLocationV1::Source {
                        span: DebugSourceMapSpanV1::new([0x31; 32], 0, 4, 1, 1).unwrap(),
                    },
                )
                .unwrap(),
                SemanticDebugNodeV1::new(
                    mir,
                    SemanticDebugLocationV1::Mir {
                        body_ordinal: 2,
                        block_ordinal: 0,
                        statement_ordinal: 0,
                    },
                )
                .unwrap(),
                SemanticDebugNodeV1::new(
                    kir,
                    SemanticDebugLocationV1::Kir {
                        function_ordinal: 2,
                        block_ordinal: 0,
                        operation_ordinal: 0,
                    },
                )
                .unwrap(),
            ],
            vec![
                SemanticDebugMappingV1::new(
                    [0x21; 32],
                    SemanticDebugLayerV1::Source,
                    SemanticDebugLayerV1::Mir,
                    SemanticDebugTransformationV1::Preserved,
                    vec![source],
                    SemanticDebugMappingOutputV1::available(vec![mir]),
                )
                .unwrap(),
                SemanticDebugMappingV1::new(
                    [0x22; 32],
                    SemanticDebugLayerV1::Mir,
                    SemanticDebugLayerV1::Kir,
                    SemanticDebugTransformationV1::Preserved,
                    vec![mir],
                    SemanticDebugMappingOutputV1::available(vec![kir]),
                )
                .unwrap(),
            ],
            vec![
                SemanticDebugBoundaryV1::new(
                    kir,
                    SemanticDebugBoundaryDirectionV1::SuccessorUnavailable,
                    SemanticDebugBoundaryReasonV1::UnsupportedLayer,
                )
                .unwrap(),
            ],
        )
        .unwrap()
    }

    fn function_inputs() -> Vec<ProductionSemanticDebugFunctionInstanceInputV1> {
        vec![
            ProductionSemanticDebugFunctionInstanceInputV1 {
                correspondence_owner: 0,
                semantic_function: 0,
                kernel_ir_function_ordinal: 0,
                role: ProductionSemanticDebugFunctionInstanceRoleV1::KernelEntry,
            },
            ProductionSemanticDebugFunctionInstanceInputV1 {
                correspondence_owner: 0,
                semantic_function: 2,
                kernel_ir_function_ordinal: 2,
                role: ProductionSemanticDebugFunctionInstanceRoleV1::InternalHelper,
            },
            ProductionSemanticDebugFunctionInstanceInputV1 {
                correspondence_owner: 1,
                semantic_function: 1,
                kernel_ir_function_ordinal: 1,
                role: ProductionSemanticDebugFunctionInstanceRoleV1::KernelEntry,
            },
            ProductionSemanticDebugFunctionInstanceInputV1 {
                correspondence_owner: 1,
                semantic_function: 2,
                kernel_ir_function_ordinal: 2,
                role: ProductionSemanticDebugFunctionInstanceRoleV1::InternalHelper,
            },
        ]
    }

    fn statement_inputs() -> Vec<ProductionSemanticDebugStatementInstanceInputV1> {
        [0, 1]
            .into_iter()
            .map(
                |correspondence_owner| ProductionSemanticDebugStatementInstanceInputV1 {
                    correspondence_owner,
                    semantic_function: 2,
                    semantic_block: 0,
                    statement: 0,
                    source_node: [0x11; 32],
                    mir_node: [0x12; 32],
                    kir_nodes: vec![[0x13; 32]],
                },
            )
            .collect()
    }

    fn build(
        functions: Vec<ProductionSemanticDebugFunctionInstanceInputV1>,
        statements: Vec<ProductionSemanticDebugStatementInstanceInputV1>,
        map: &SemanticDebugMapDocumentV1,
    ) -> Result<
        ProductionSemanticDebugInstanceCustodyV1,
        ProductionSemanticDebugInstanceCustodyErrorV1,
    > {
        let map_bytes = map.to_canonical_json_bytes().unwrap();
        let binding = ProductionSemanticDebugInstanceCustodyBindingV1::from_exact_bytes(
            SOURCE_MAP,
            &map_bytes,
            SEMANTIC_MIR,
            CANONICAL_KIR,
            CORRESPONDENCE,
        )?;
        ProductionSemanticDebugInstanceCustodyV1::from_replayed_inputs(
            binding, functions, statements, map,
        )
    }

    #[test]
    fn shared_helper_has_exact_deterministic_forward_and_reverse_owners() {
        let map = semantic_map();
        let custody = build(function_inputs(), statement_inputs(), &map).unwrap();
        let map_bytes = map.to_canonical_json_bytes().unwrap();
        let inert = InertProductionSemanticDebugInstanceCustodyV1::from_canonical_bytes(
            custody.canonical_bytes(),
            &map_bytes,
        )
        .unwrap();
        assert_eq!(inert.claimed_identity(), custody.identity());
        assert_eq!(inert.claimed_binding(), custody.binding());
        assert_eq!(inert.canonical_bytes(), custody.canonical_bytes());
        assert!(!inert.grants_execution_authority());
        let decoded = inert.admit_exact_replay_v1(custody.clone()).unwrap();

        assert_eq!(decoded, custody);
        assert_eq!(decoded.functions().len(), 4);
        assert_eq!(decoded.statements().len(), 2);
        assert_eq!(
            decoded
                .function_instances_for_semantic(2)
                .map(|record| record.correspondence_owner())
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(
            decoded
                .function_instances_for_kir(2)
                .map(|record| record.correspondence_owner())
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(
            decoded
                .statement_instances_for_kir_node([0x13; 32])
                .map(|record| record.correspondence_owner())
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(
            decoded.statement_instance(1, 2, 0, 0).unwrap().kir_nodes(),
            &[[0x13; 32]]
        );
        assert!(!decoded.grants_execution_authority());
        decoded
            .validate_exact_inputs(
                SOURCE_MAP,
                &map_bytes,
                SEMANTIC_MIR,
                CANONICAL_KIR,
                CORRESPONDENCE,
            )
            .unwrap();
        assert_eq!(
            decoded.validate_exact_inputs(
                b"substituted-source-map",
                &map_bytes,
                SEMANTIC_MIR,
                CANONICAL_KIR,
                CORRESPONDENCE,
            ),
            Err(ProductionSemanticDebugInstanceCustodyErrorV1::BindingMismatch)
        );
    }

    #[test]
    fn duplicate_reordered_and_substituted_function_instances_are_rejected() {
        let map = semantic_map();
        let mut duplicate = function_inputs();
        duplicate.push(duplicate[1]);
        assert_eq!(
            build(duplicate, statement_inputs(), &map),
            Err(ProductionSemanticDebugInstanceCustodyErrorV1::NonCanonicalOrder)
        );

        let mut substituted = function_inputs();
        substituted[3].semantic_function = 3;
        assert_eq!(
            build(substituted, statement_inputs(), &map),
            Err(ProductionSemanticDebugInstanceCustodyErrorV1::PhysicalFunctionOverlap)
        );

        let custody = build(function_inputs(), statement_inputs(), &map).unwrap();
        let map_bytes = map.to_canonical_json_bytes().unwrap();
        let mut reordered = custody.canonical_bytes().to_vec();
        let records = HEADER_BYTES_V1 + BINDING_FIELDS_V1 * CONTENT_IDENTITY_BYTES_V1;
        let first: [u8; FUNCTION_BYTES_V1] = reordered[records..records + FUNCTION_BYTES_V1]
            .try_into()
            .unwrap();
        reordered.copy_within(
            records + FUNCTION_BYTES_V1..records + 2 * FUNCTION_BYTES_V1,
            records,
        );
        reordered[records + FUNCTION_BYTES_V1..records + 2 * FUNCTION_BYTES_V1]
            .copy_from_slice(&first);
        assert!(matches!(
            InertProductionSemanticDebugInstanceCustodyV1::from_canonical_bytes(
                &reordered, &map_bytes,
            ),
            Err(ProductionSemanticDebugInstanceCustodyErrorV1::NonCanonicalOrder)
        ));
    }

    #[test]
    fn appended_no_statement_kernel_entry_remains_inert_and_cannot_be_admitted() {
        let map = semantic_map();
        let exact = build(function_inputs(), statement_inputs(), &map).unwrap();
        let map_bytes = map.to_canonical_json_bytes().unwrap();
        let forged_input = ProductionSemanticDebugFunctionInstanceInputV1 {
            correspondence_owner: 3,
            semantic_function: 3,
            kernel_ir_function_ordinal: 3,
            role: ProductionSemanticDebugFunctionInstanceRoleV1::KernelEntry,
        };
        let forged_record = ProductionSemanticDebugFunctionInstanceV1 {
            identity: function_identity(exact.binding(), forged_input),
            correspondence_owner: forged_input.correspondence_owner,
            semantic_function: forged_input.semantic_function,
            kernel_ir_function_ordinal: forged_input.kernel_ir_function_ordinal,
            role: forged_input.role,
        };
        let mut encoded_record = Vec::new();
        encoded_record.extend_from_slice(&forged_record.identity);
        encoded_record.extend_from_slice(&forged_record.correspondence_owner.to_le_bytes());
        encoded_record.extend_from_slice(&forged_record.semantic_function.to_le_bytes());
        encoded_record.extend_from_slice(&forged_record.kernel_ir_function_ordinal.to_le_bytes());
        encoded_record.push(forged_record.role.code());
        encoded_record.extend_from_slice(&[0; 3]);
        assert_eq!(encoded_record.len(), FUNCTION_BYTES_V1);

        let mut forged = exact.canonical_bytes().to_vec();
        forged[12..16].copy_from_slice(&5_u32.to_le_bytes());
        let function_records = HEADER_BYTES_V1 + BINDING_FIELDS_V1 * CONTENT_IDENTITY_BYTES_V1;
        let statements = function_records + 4 * FUNCTION_BYTES_V1;
        forged.splice(statements..statements, encoded_record);

        let inert = InertProductionSemanticDebugInstanceCustodyV1::from_canonical_bytes(
            &forged, &map_bytes,
        )
        .unwrap();
        assert_eq!(inert.claimed_binding(), exact.binding());
        assert_eq!(
            inert.admit_exact_replay_v1(exact),
            Err(ProductionSemanticDebugInstanceCustodyErrorV1::ExactReplayMismatch)
        );
    }

    #[test]
    fn entry_reuse_duplicate_references_and_incomplete_owners_are_rejected() {
        let map = semantic_map();
        let mut entry_reuse = function_inputs();
        entry_reuse[2].kernel_ir_function_ordinal = 0;
        assert_eq!(
            build(entry_reuse, statement_inputs(), &map),
            Err(ProductionSemanticDebugInstanceCustodyErrorV1::PhysicalFunctionOverlap)
        );

        let mut duplicate_reference = statement_inputs();
        duplicate_reference[0].kir_nodes.push([0x13; 32]);
        assert_eq!(
            build(function_inputs(), duplicate_reference, &map),
            Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidStatement)
        );

        let mut incomplete = function_inputs();
        incomplete.remove(2);
        assert_eq!(
            build(incomplete, statement_inputs(), &map),
            Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidFunction)
        );
    }

    #[test]
    fn distinct_semantic_sites_cannot_claim_one_shared_physical_node() {
        let map = semantic_map();
        let mut nodes = map.nodes().to_vec();
        nodes.extend([
            SemanticDebugNodeV1::new(
                [0x14; 32],
                SemanticDebugLocationV1::Source {
                    span: DebugSourceMapSpanV1::new([0x31; 32], 4, 8, 2, 1).unwrap(),
                },
            )
            .unwrap(),
            SemanticDebugNodeV1::new(
                [0x15; 32],
                SemanticDebugLocationV1::Mir {
                    body_ordinal: 2,
                    block_ordinal: 0,
                    statement_ordinal: 1,
                },
            )
            .unwrap(),
        ]);
        let mut mappings = map.mappings().to_vec();
        mappings.extend([
            SemanticDebugMappingV1::new(
                [0x23; 32],
                SemanticDebugLayerV1::Source,
                SemanticDebugLayerV1::Mir,
                SemanticDebugTransformationV1::Preserved,
                vec![[0x14; 32]],
                SemanticDebugMappingOutputV1::available(vec![[0x15; 32]]),
            )
            .unwrap(),
            SemanticDebugMappingV1::new(
                [0x24; 32],
                SemanticDebugLayerV1::Mir,
                SemanticDebugLayerV1::Kir,
                SemanticDebugTransformationV1::Preserved,
                vec![[0x15; 32]],
                SemanticDebugMappingOutputV1::available(vec![[0x13; 32]]),
            )
            .unwrap(),
        ]);
        assert_eq!(
            SemanticDebugMapDocumentV1::new_partial(
                map.binding(),
                nodes,
                mappings,
                map.boundaries().to_vec(),
            ),
            Err(SemanticDebugMapErrorV1::ContradictoryMapping)
        );
    }

    #[test]
    fn corrupt_binding_role_counts_and_truncation_are_typed() {
        let map = semantic_map();
        let custody = build(function_inputs(), statement_inputs(), &map).unwrap();
        let map_bytes = map.to_canonical_json_bytes().unwrap();

        let mut changed_binding = custody.canonical_bytes().to_vec();
        changed_binding[HEADER_BYTES_V1 + CONTENT_IDENTITY_BYTES_V1] ^= 1;
        assert!(matches!(
            InertProductionSemanticDebugInstanceCustodyV1::from_canonical_bytes(
                &changed_binding,
                &map_bytes,
            ),
            Err(ProductionSemanticDebugInstanceCustodyErrorV1::BindingMismatch)
        ));

        let mut changed_role = custody.canonical_bytes().to_vec();
        let first_role = HEADER_BYTES_V1 + BINDING_FIELDS_V1 * CONTENT_IDENTITY_BYTES_V1 + 44;
        changed_role[first_role] = 9;
        assert!(matches!(
            InertProductionSemanticDebugInstanceCustodyV1::from_canonical_bytes(
                &changed_role,
                &map_bytes,
            ),
            Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidRole)
        ));

        let mut oversize = custody.canonical_bytes().to_vec();
        oversize[12..16].copy_from_slice(
            &(u32::try_from(MAX_PRODUCTION_SEMANTIC_DEBUG_FUNCTION_INSTANCES_V1).unwrap() + 1)
                .to_le_bytes(),
        );
        assert!(matches!(
            InertProductionSemanticDebugInstanceCustodyV1::from_canonical_bytes(
                &oversize, &map_bytes,
            ),
            Err(ProductionSemanticDebugInstanceCustodyErrorV1::ResourceLimit)
        ));

        let mut truncated = custody.canonical_bytes().to_vec();
        truncated.pop();
        assert!(matches!(
            InertProductionSemanticDebugInstanceCustodyV1::from_canonical_bytes(
                &truncated, &map_bytes,
            ),
            Err(ProductionSemanticDebugInstanceCustodyErrorV1::InvalidLength)
        ));
    }
}
