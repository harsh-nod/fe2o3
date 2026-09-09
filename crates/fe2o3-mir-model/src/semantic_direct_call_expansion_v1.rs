//! Bounded, replayable direct-call expansion over an unchanged admitted source.
//!
//! The function views are not admitted documents or equivalence proofs. Type,
//! callable, allocation and source identity tables always belong to the original
//! document. Execution coordinates are meaningful only with the retained origins.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::semantic_mir_v1::*;

mod evidence_v1;
mod identity;
mod remap;
pub use evidence_v1::{
    InertCanonicalSemanticCallExpansionEvidenceV1, MAX_SEMANTIC_CALL_EXPANSION_EVIDENCE_BYTES_V1,
    SEMANTIC_CALL_EXPANSION_EVIDENCE_POLICY_V1, SEMANTIC_CALL_EXPANSION_EVIDENCE_VERSION_V1,
    SemanticCallExpansionEvidenceErrorV1, SemanticExpandedRootEvidenceV1,
};
#[cfg(test)]
mod tests;

const DOMAIN: &[u8] = b"FE2O3/CHECKED-DIRECT-CALL-EXPANSION/V1\0";

/// Aggregate resource axes, charged across every root and call instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticCallExpansionResourceV1 {
    Roots,
    Instances,
    Locals,
    Blocks,
    Statements,
    Work,
    Depth,
}

/// Caller limits may tighten, but never exceed, [`Self::HARD_MAX`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticCallExpansionLimitsV1 {
    pub roots: usize,
    pub instances: usize,
    pub locals: usize,
    pub blocks: usize,
    pub statements: usize,
    pub work: usize,
    pub depth: usize,
}

impl SemanticCallExpansionLimitsV1 {
    pub const HARD_MAX: Self = Self {
        roots: 4096,
        instances: 16384,
        locals: 262144,
        blocks: 262144,
        statements: 1048576,
        work: 16777216,
        depth: 256,
    };

    fn values(self) -> [usize; 7] {
        [
            self.roots,
            self.instances,
            self.locals,
            self.blocks,
            self.statements,
            self.work,
            self.depth,
        ]
    }
}

impl Default for SemanticCallExpansionLimitsV1 {
    fn default() -> Self {
        Self {
            roots: 256,
            instances: 4096,
            locals: 65536,
            blocks: 65536,
            statements: 262144,
            work: 16777216,
            depth: 64,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticCallExpansionErrorV1 {
    InvalidLimits,
    Limit(SemanticCallExpansionResourceV1),
    InvalidRoot(SemanticFunctionIdV1),
    Unsupported {
        function: SemanticFunctionIdV1,
        block: Option<SemanticBlockIdV1>,
        reason: &'static str,
    },
    SourceMismatch,
    ReplayMismatch,
    Model(SemanticMirErrorV1),
}

impl fmt::Display for SemanticCallExpansionErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "checked semantic call expansion: {self:?}")
    }
}
impl Error for SemanticCallExpansionErrorV1 {}
impl From<SemanticMirErrorV1> for SemanticCallExpansionErrorV1 {
    fn from(error: SemanticMirErrorV1) -> Self {
        Self::Model(error)
    }
}
type Result<T> = std::result::Result<T, SemanticCallExpansionErrorV1>;

/// An index in one root's instance roster, never a source function ID.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticCallInstanceIdV1(u32);
impl SemanticCallInstanceIdV1 {
    pub const fn index(self) -> u32 {
        self.0
    }
}

/// One occurrence of an original function in a root's execution view.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticCallInstanceV1 {
    function: SemanticFunctionIdV1,
    function_identity: SemanticFunctionIdentityV1,
    parent: Option<SemanticCallInstanceIdV1>,
    call_block: Option<SemanticBlockIdV1>,
    local_start: u32,
    local_count: u32,
    block_start: u32,
    block_count: u32,
    depth: usize,
}
impl SemanticCallInstanceV1 {
    pub const fn function(&self) -> SemanticFunctionIdV1 {
        self.function
    }
    pub const fn function_identity(&self) -> SemanticFunctionIdentityV1 {
        self.function_identity
    }
    pub const fn parent(&self) -> Option<SemanticCallInstanceIdV1> {
        self.parent
    }
    /// Original block in the parent function containing the call terminator.
    pub const fn call_block(&self) -> Option<SemanticBlockIdV1> {
        self.call_block
    }
    pub const fn local_start(&self) -> u32 {
        self.local_start
    }
    pub const fn local_count(&self) -> u32 {
        self.local_count
    }
    pub const fn block_start(&self) -> u32 {
        self.block_start
    }
    pub const fn block_count(&self) -> u32 {
        self.block_count
    }
    pub const fn depth(&self) -> usize {
        self.depth
    }
}

/// Indexed by expanded local ID; source locals remain in their original function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticExpandedLocalOriginV1 {
    instance: SemanticCallInstanceIdV1,
    function: SemanticFunctionIdV1,
    local: SemanticLocalIdV1,
}
impl SemanticExpandedLocalOriginV1 {
    pub const fn instance(self) -> SemanticCallInstanceIdV1 {
        self.instance
    }
    pub const fn function(self) -> SemanticFunctionIdV1 {
        self.function
    }
    pub const fn local(self) -> SemanticLocalIdV1 {
        self.local
    }
}

/// Exact attribution of an execution statement, including frame transfers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticExpandedStatementOriginV1 {
    Source {
        statement: u32,
    },
    ParameterTransfer {
        callee: SemanticCallInstanceIdV1,
        argument: u32,
    },
    ReturnTransfer {
        callee: SemanticCallInstanceIdV1,
    },
    FrameStorageLive {
        callee: SemanticCallInstanceIdV1,
        local: SemanticLocalIdV1,
    },
    FrameStorageDead {
        callee: SemanticCallInstanceIdV1,
        local: SemanticLocalIdV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticExpandedTerminatorOriginV1 {
    Source,
    CallEntry { callee: SemanticCallInstanceIdV1 },
    CallReturn { callee: SemanticCallInstanceIdV1 },
}

/// Indexed by expanded block ID. Statement origins are indexed identically to
/// the execution block; source provenance is retained on the MIR nodes as well.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticExpandedBlockOriginV1 {
    instance: SemanticCallInstanceIdV1,
    function: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    statements: Box<[SemanticExpandedStatementOriginV1]>,
    terminator: SemanticExpandedTerminatorOriginV1,
}
impl SemanticExpandedBlockOriginV1 {
    pub const fn instance(&self) -> SemanticCallInstanceIdV1 {
        self.instance
    }
    pub const fn function(&self) -> SemanticFunctionIdV1 {
        self.function
    }
    pub const fn block(&self) -> SemanticBlockIdV1 {
        self.block
    }
    pub fn statements(&self) -> &[SemanticExpandedStatementOriginV1] {
        &self.statements
    }
    pub const fn terminator(&self) -> SemanticExpandedTerminatorOriginV1 {
        self.terminator
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct SemanticExpandedRootV1 {
    root: SemanticFunctionIdV1,
    source_body: SemanticFunctionIdV1,
    identity: [u8; 32],
    body: SemanticFunctionDeclV1,
    instances: Box<[SemanticCallInstanceV1]>,
    local_origins: Box<[SemanticExpandedLocalOriginV1]>,
    block_origins: Box<[SemanticExpandedBlockOriginV1]>,
}
impl SemanticExpandedRootV1 {
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        self.root
    }
    pub const fn source_body(&self) -> SemanticFunctionIdV1 {
        self.source_body
    }
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn body(&self) -> &SemanticFunctionDeclV1 {
        &self.body
    }
    pub fn has_expanded_calls(&self) -> bool {
        self.instances.len() > 1
    }
    pub fn instances(&self) -> &[SemanticCallInstanceV1] {
        &self.instances
    }
    pub fn local_origins(&self) -> &[SemanticExpandedLocalOriginV1] {
        &self.local_origins
    }
    pub fn block_origins(&self) -> &[SemanticExpandedBlockOriginV1] {
        &self.block_origins
    }
}

/// Compiler-derived execution views. No constructor accepts arbitrary MIR or
/// caller-provided identity. This value grants no proof or production authority.
///
/// ```compile_fail
/// use fe2o3_mir_model::SemanticCallExpansionV1;
/// let forged = SemanticCallExpansionV1 { source_semantic_sha256: [0; 32] };
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct SemanticCallExpansionV1 {
    source_semantic_sha256: [u8; 32],
    identity: [u8; 32],
    limits: SemanticCallExpansionLimitsV1,
    roots: Box<[SemanticExpandedRootV1]>,
    work_units: usize,
}
impl SemanticCallExpansionV1 {
    pub fn try_new(
        source: &AdmittedInertSemanticMirV1,
        limits: SemanticCallExpansionLimitsV1,
    ) -> Result<Self> {
        let mut budget = Budget::new(limits)?;
        budget.charge(SemanticCallExpansionResourceV1::Roots, source.roots().len())?;
        budget.work(source.canonical_encoding().len())?;
        let source_semantic_sha256 = *source.semantic_sha256().as_bytes();
        let mut roots = Vec::new();
        let mut digest = Sha256::new();
        budget.work(DOMAIN.len() + source_semantic_sha256.len())?;
        digest.update(DOMAIN);
        digest.update(source_semantic_sha256);
        for root in source.roots() {
            let body = match source.select_kernel_body_for_root_v1(*root) {
                Some(selection) => selection.body(),
                None => {
                    let function = &source.functions()[root.index() as usize];
                    for block in function.blocks() {
                        budget.work(1)?;
                        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                            && matches!(
                                source.callables()[call.callee().index() as usize],
                                SemanticCallableDeclV1::Defined { .. }
                            )
                        {
                            return Err(SemanticCallExpansionErrorV1::InvalidRoot(*root));
                        }
                    }
                    *root
                }
            };
            let expanded = expand_root(source, *root, body, &mut budget)?;
            budget.work(expanded.identity.len())?;
            digest.update(expanded.identity);
            roots.push(expanded);
        }
        Ok(Self {
            source_semantic_sha256,
            identity: digest.finalize().into(),
            limits,
            roots: roots.into_boxed_slice(),
            work_units: budget.used[5],
        })
    }

    /// Reconstructs the complete view and every origin from the exact original.
    /// Replay detects corruption; it is not an independent semantic proof.
    pub fn verify_replay(&self, source: &AdmittedInertSemanticMirV1) -> Result<()> {
        if self.source_semantic_sha256 != *source.semantic_sha256().as_bytes() {
            return Err(SemanticCallExpansionErrorV1::SourceMismatch);
        }
        if Self::try_new(source, self.limits)? != *self {
            return Err(SemanticCallExpansionErrorV1::ReplayMismatch);
        }
        Ok(())
    }
    pub const fn source_semantic_sha256(&self) -> &[u8; 32] {
        &self.source_semantic_sha256
    }
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub fn roots(&self) -> &[SemanticExpandedRootV1] {
        &self.roots
    }
    pub fn root(&self, root: SemanticFunctionIdV1) -> Option<&SemanticExpandedRootV1> {
        self.roots
            .binary_search_by_key(&root, SemanticExpandedRootV1::root)
            .ok()
            .map(|index| &self.roots[index])
    }
    pub const fn work_units(&self) -> usize {
        self.work_units
    }
    pub const fn grants_proof_or_artifact_authority(&self) -> bool {
        false
    }
}

struct Budget {
    limits: SemanticCallExpansionLimitsV1,
    used: [usize; 7],
}
impl Budget {
    fn new(limits: SemanticCallExpansionLimitsV1) -> Result<Self> {
        if limits
            .values()
            .into_iter()
            .zip(SemanticCallExpansionLimitsV1::HARD_MAX.values())
            .any(|(limit, hard)| limit > hard)
        {
            return Err(SemanticCallExpansionErrorV1::InvalidLimits);
        }
        Ok(Self {
            limits,
            used: [0; 7],
        })
    }
    fn charge(&mut self, resource: SemanticCallExpansionResourceV1, amount: usize) -> Result<()> {
        let index = resource as usize;
        let value = self.used[index]
            .checked_add(amount)
            .ok_or(SemanticCallExpansionErrorV1::Limit(resource))?;
        if value > self.limits.values()[index] {
            return Err(SemanticCallExpansionErrorV1::Limit(resource));
        }
        self.used[index] = value;
        Ok(())
    }
    fn work(&mut self, amount: usize) -> Result<()> {
        self.charge(SemanticCallExpansionResourceV1::Work, amount)
    }
}

fn unsupported(
    function: SemanticFunctionIdV1,
    block: Option<SemanticBlockIdV1>,
    reason: &'static str,
) -> SemanticCallExpansionErrorV1 {
    SemanticCallExpansionErrorV1::Unsupported {
        function,
        block,
        reason,
    }
}

fn derived_identity(
    parent: &[u8; 32],
    kind: &[u8],
    index: u32,
    budget: &mut Budget,
) -> Result<[u8; 32]> {
    budget.work(DOMAIN.len() + parent.len() + 4 + kind.len() + 4)?;
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update(parent);
    digest.update((kind.len() as u32).to_le_bytes());
    digest.update(kind);
    digest.update(index.to_le_bytes());
    Ok(digest.finalize().into())
}

struct Frame {
    instance: SemanticCallInstanceV1,
    destination: Option<SemanticCallDestinationV1>,
    always_live: Vec<SemanticLocalIdV1>,
    arguments: Vec<SemanticLocalIdV1>,
    return_local: SemanticLocalIdV1,
}

fn allocate_frame(
    source: &AdmittedInertSemanticMirV1,
    function_id: SemanticFunctionIdV1,
    call_site: Option<(
        SemanticCallInstanceIdV1,
        SemanticBlockIdV1,
        SemanticCallDestinationV1,
    )>,
    frames: &mut Vec<Frame>,
    origins: &mut Vec<SemanticExpandedLocalOriginV1>,
    block_count: &mut u32,
    budget: &mut Budget,
) -> Result<SemanticCallInstanceIdV1> {
    let parent = call_site
        .as_ref()
        .map(|(parent, block, _)| (*parent, *block));
    let function = &source.functions()[function_id.index() as usize];
    let depth = parent.map_or(0, |(parent, _)| {
        frames[parent.0 as usize].instance.depth + 1
    });
    if depth > budget.limits.depth {
        return Err(SemanticCallExpansionErrorV1::Limit(
            SemanticCallExpansionResourceV1::Depth,
        ));
    }
    let mut ancestor = parent.map(|(id, _)| id);
    while let Some(id) = ancestor {
        budget.work(1)?;
        let frame = &frames[id.0 as usize];
        if frame.instance.function == function_id {
            return Err(unsupported(function_id, None, "recursive defined call"));
        }
        ancestor = frame.instance.parent;
    }
    budget.charge(SemanticCallExpansionResourceV1::Instances, 1)?;
    budget.charge(
        SemanticCallExpansionResourceV1::Locals,
        function.locals().len(),
    )?;
    budget.charge(
        SemanticCallExpansionResourceV1::Blocks,
        function.blocks().len(),
    )?;
    budget.work(function.locals().len() + function.abi().arguments().len())?;
    let id = SemanticCallInstanceIdV1(frames.len() as u32);
    let instance = SemanticCallInstanceV1 {
        function: function_id,
        function_identity: function.identity(),
        parent: parent.map(|(id, _)| id),
        call_block: parent.map(|(_, block)| block),
        local_start: origins.len() as u32,
        local_count: function.locals().len() as u32,
        block_start: *block_count,
        block_count: function.blocks().len() as u32,
        depth,
    };
    *block_count = block_count.checked_add(instance.block_count).ok_or(
        SemanticCallExpansionErrorV1::Limit(SemanticCallExpansionResourceV1::Blocks),
    )?;
    let mut explicit_storage = vec![false; function.locals().len()];
    for block in function.blocks() {
        budget.work(1 + block.statements().len())?;
        for statement in block.statements() {
            if let SemanticStatementKindV1::StorageLive(local)
            | SemanticStatementKindV1::StorageDead(local) = statement.kind()
            {
                explicit_storage[local.index() as usize] = true;
            }
        }
    }
    let mut always_live = Vec::new();
    let mut arguments = vec![None; function.abi().source_input_types().len()];
    let mut return_local = None;
    for (index, explicit) in explicit_storage.into_iter().enumerate() {
        let local = SemanticLocalIdV1::from_index(index as u32);
        origins.push(SemanticExpandedLocalOriginV1 {
            instance: id,
            function: function_id,
            local,
        });
        // Arguments and the return place belong to the frame on entry even
        // when their source lifetime ends with an explicit StorageDead.
        if !explicit || function.locals()[index].role() != SemanticLocalRoleV1::Temporary {
            always_live.push(local);
        }
        match function.locals()[index].role() {
            SemanticLocalRoleV1::Return => return_local = Some(local),
            SemanticLocalRoleV1::Argument(argument) => arguments[argument as usize] = Some(local),
            SemanticLocalRoleV1::Temporary => {}
        }
    }
    let arguments = arguments
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| unsupported(function_id, None, "missing argument local"))?;
    let return_local =
        return_local.ok_or_else(|| unsupported(function_id, None, "missing return local"))?;
    frames.push(Frame {
        instance,
        destination: call_site.map(|(_, _, destination)| destination),
        always_live,
        arguments,
        return_local,
    });
    Ok(id)
}

fn require_defined_abi(
    function_id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    call: &SemanticDirectCallV1,
) -> Result<()> {
    let abi = function.abi();
    if function.role() != SemanticFunctionRoleV1::InternalHelper
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.c_variadic()
        || !abi.hidden_arguments().is_empty()
        || !call.variadic_argument_abis().is_empty()
        || call.unwind() != SemanticUnwindActionV1::Unreachable
    {
        return Err(unsupported(
            function_id,
            None,
            "defined call requires ordinary nonvariadic Rust ABI and unreachable unwind",
        ));
    }
    let Some(destination) = call.destination() else {
        return Err(unsupported(
            function_id,
            None,
            "defined call without a return destination",
        ));
    };
    if !destination.place().projections().is_empty() {
        return Err(unsupported(
            function_id,
            None,
            "projected call return destination",
        ));
    }
    Ok(())
}

fn push_statement(
    statements: &mut Vec<SemanticStatementV1>,
    origins: &mut Vec<SemanticExpandedStatementOriginV1>,
    statement: SemanticStatementV1,
    origin: SemanticExpandedStatementOriginV1,
    budget: &mut Budget,
) -> Result<()> {
    budget.charge(SemanticCallExpansionResourceV1::Statements, 1)?;
    budget.work(1)?;
    statements.push(statement);
    origins.push(origin);
    Ok(())
}

fn transfer(
    destination: SemanticPlaceV1,
    value: SemanticOperandV1,
    source: SemanticSourceProvenanceV1,
) -> SemanticStatementV1 {
    let ty = destination.ty();
    SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(ty, SemanticRvalueKindV1::Use(value)),
        )),
    )
}

fn expand_root(
    source: &AdmittedInertSemanticMirV1,
    root: SemanticFunctionIdV1,
    source_body: SemanticFunctionIdV1,
    budget: &mut Budget,
) -> Result<SemanticExpandedRootV1> {
    let original = &source.functions()[source_body.index() as usize];
    let node_seed = derived_identity(
        &derived_identity(
            source.semantic_sha256().as_bytes(),
            b"root",
            root.index(),
            budget,
        )?,
        b"body",
        source_body.index(),
        budget,
    )?;
    let mut frames = Vec::new();
    let mut local_origins = Vec::new();
    let mut block_count = 0;
    allocate_frame(
        source,
        source_body,
        None,
        &mut frames,
        &mut local_origins,
        &mut block_count,
        budget,
    )?;
    let mut blocks = Vec::new();
    let mut block_origins = Vec::new();
    let mut cursor = 0;
    // Breadth-first static instances avoid Rust recursion and retain all CFG
    // blocks, including loops, unreachable blocks, and repeated call sites.
    while cursor < frames.len() {
        let id = SemanticCallInstanceIdV1(cursor as u32);
        let instance = frames[cursor].instance.clone();
        let function = &source.functions()[instance.function.index() as usize];
        for (block_index, block) in function.blocks().iter().enumerate() {
            budget.work(1)?;
            let source_block = SemanticBlockIdV1::from_index(block_index as u32);
            let mut statements = Vec::new();
            let mut statement_origins = Vec::new();
            for (index, statement) in block.statements().iter().enumerate() {
                let mapped = remap::statement(statement, &instance, budget)?;
                push_statement(
                    &mut statements,
                    &mut statement_origins,
                    mapped,
                    SemanticExpandedStatementOriginV1::Source {
                        statement: index as u32,
                    },
                    budget,
                )?;
            }
            let provenance = block.terminator().source();
            let mut terminator_origin = SemanticExpandedTerminatorOriginV1::Source;
            let terminator = match block.terminator().kind() {
                SemanticTerminatorKindV1::Call(call)
                    if matches!(
                        source.callables()[call.callee().index() as usize],
                        SemanticCallableDeclV1::Defined { .. }
                    ) =>
                {
                    let SemanticCallableDeclV1::Defined { function: callee } =
                        source.callables()[call.callee().index() as usize]
                    else {
                        unreachable!()
                    };
                    let callee_function = &source.functions()[callee.index() as usize];
                    require_defined_abi(callee, callee_function, call)?;
                    let destination = remap::destination(
                        call.destination().expect("checked destination"),
                        &instance,
                        budget,
                    )?;
                    let child = allocate_frame(
                        source,
                        callee,
                        Some((id, source_block, destination)),
                        &mut frames,
                        &mut local_origins,
                        &mut block_count,
                        budget,
                    )?;
                    let child_frame = &frames[child.0 as usize];
                    for local in &child_frame.always_live {
                        push_statement(
                            &mut statements,
                            &mut statement_origins,
                            SemanticStatementV1::new(
                                provenance,
                                SemanticStatementKindV1::StorageLive(remap::local(
                                    *local,
                                    &child_frame.instance,
                                )),
                            ),
                            SemanticExpandedStatementOriginV1::FrameStorageLive {
                                callee: child,
                                local: *local,
                            },
                            budget,
                        )?;
                    }
                    for (argument, argument_local) in child_frame.arguments.iter().enumerate() {
                        budget.work(1)?;
                        let local = &callee_function.locals()[argument_local.index() as usize];
                        let operand =
                            remap::operand(&call.arguments()[argument], &instance, budget)?;
                        let destination = SemanticPlaceV1::new(
                            remap::local(*argument_local, &child_frame.instance),
                            vec![],
                            local.ty(),
                        )?;
                        push_statement(
                            &mut statements,
                            &mut statement_origins,
                            transfer(destination, operand, provenance),
                            SemanticExpandedStatementOriginV1::ParameterTransfer {
                                callee: child,
                                argument: argument as u32,
                            },
                            budget,
                        )?;
                    }
                    terminator_origin =
                        SemanticExpandedTerminatorOriginV1::CallEntry { callee: child };
                    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::Goto,
                        remap::block(callee_function.entry(), &child_frame.instance),
                    ))
                }
                SemanticTerminatorKindV1::Return if instance.parent.is_some() => {
                    let destination = frames[cursor]
                        .destination
                        .as_ref()
                        .expect("child return destination");
                    let output_type = function.abi().source_output_type();
                    let value = if matches!(
                        source.types()[output_type.index() as usize].shape(),
                        SemanticTypeShapeV1::Unit
                    ) {
                        // Unit Return does not read the often-unwritten MIR
                        // return local. Do not generalize this to branded ZSTs.
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            output_type,
                            SemanticConstantValueV1::ZeroSized,
                        ))
                    } else {
                        SemanticOperandV1::Move(SemanticPlaceV1::new(
                            remap::local(frames[cursor].return_local, &instance),
                            vec![],
                            output_type,
                        )?)
                    };
                    budget.work(function.locals().len())?;
                    push_statement(
                        &mut statements,
                        &mut statement_origins,
                        transfer(destination.place().clone(), value, provenance),
                        SemanticExpandedStatementOriginV1::ReturnTransfer { callee: id },
                        budget,
                    )?;
                    for local_index in 0..function.locals().len() {
                        let local = SemanticLocalIdV1::from_index(local_index as u32);
                        push_statement(
                            &mut statements,
                            &mut statement_origins,
                            SemanticStatementV1::new(
                                provenance,
                                SemanticStatementKindV1::StorageDead(remap::local(
                                    local, &instance,
                                )),
                            ),
                            SemanticExpandedStatementOriginV1::FrameStorageDead {
                                callee: id,
                                local,
                            },
                            budget,
                        )?;
                    }
                    terminator_origin =
                        SemanticExpandedTerminatorOriginV1::CallReturn { callee: id };
                    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::Goto,
                        destination.edge().target(),
                    ))
                }
                kind => remap::terminator(kind, &instance, source_block, budget)?,
            };
            let block_id = instance.block_start + block_index as u32;
            blocks.push(SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256(derived_identity(
                    &node_seed, b"block", block_id, budget,
                )?),
                block.source(),
                statements,
                SemanticTerminatorV1::new(provenance, terminator),
            )?);
            block_origins.push(SemanticExpandedBlockOriginV1 {
                instance: id,
                function: instance.function,
                block: source_block,
                statements: statement_origins.into_boxed_slice(),
                terminator: terminator_origin,
            });
        }
        cursor += 1;
    }
    let body = if frames.len() == 1 {
        original.clone()
    } else {
        let mut locals = Vec::new();
        for (index, origin) in local_origins.iter().enumerate() {
            budget.work(1)?;
            let local = &source.functions()[origin.function.index() as usize].locals()
                [origin.local.index() as usize];
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256(derived_identity(
                    &node_seed,
                    b"local",
                    index as u32,
                    budget,
                )?),
                local.ty(),
                if origin.instance.0 == 0 {
                    local.role()
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                local.source(),
            ));
        }
        let mut body = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(derived_identity(
                &node_seed,
                b"function",
                0,
                budget,
            )?),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            original.source(),
            original.abi().clone(),
            locals,
            original.entry(),
            blocks,
        )?;
        if let Some(entry) = original.kernel_entry() {
            body = body.with_kernel_entry(entry.clone());
        }
        body
    };
    let mut expanded = SemanticExpandedRootV1 {
        root,
        source_body,
        identity: [0; 32],
        body,
        instances: frames
            .into_iter()
            .map(|frame| frame.instance)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
        local_origins: local_origins.into_boxed_slice(),
        block_origins: block_origins.into_boxed_slice(),
    };
    expanded.identity = identity::root_content_identity(source, &expanded, budget)?;
    Ok(expanded)
}
