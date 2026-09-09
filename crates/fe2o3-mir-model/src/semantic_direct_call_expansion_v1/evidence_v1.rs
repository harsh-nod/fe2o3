//! Canonical inert custody of existing execution maps, never a second expander.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub const SEMANTIC_CALL_EXPANSION_EVIDENCE_VERSION_V1: u16 = 1;
pub const SEMANTIC_CALL_EXPANSION_EVIDENCE_POLICY_V1: u16 = 1;
/// Nested lineage envelopes must additionally enforce their remaining byte budget.
pub const MAX_SEMANTIC_CALL_EXPANSION_EVIDENCE_BYTES_V1: usize = 4 * 1024 * 1024;

const MAGIC: &[u8; 8] = b"F2SCEX1\0";
const EVIDENCE_DOMAIN: &[u8] = b"FE2O3/INERT-CALL-EXPANSION-EVIDENCE/V1\0";
const HEADER_BYTES: usize = 152;
const ABSENT: u32 = u32::MAX;
type EvidenceResult<T> = std::result::Result<T, SemanticCallExpansionEvidenceErrorV1>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticCallExpansionEvidenceErrorV1 {
    InvalidHeader,
    InvalidLength,
    InvalidIdentity,
    InvalidLimits,
    InvalidRecord,
    NonCanonical,
    TooLarge,
    WorkLimit,
    AllocationFailure,
    SourceMismatch,
    ReplayMismatch,
    Expansion(SemanticCallExpansionErrorV1),
}
impl fmt::Display for SemanticCallExpansionEvidenceErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "inert call-expansion evidence: {self:?}")
    }
}
impl Error for SemanticCallExpansionEvidenceErrorV1 {}
impl From<SemanticCallExpansionErrorV1> for SemanticCallExpansionEvidenceErrorV1 {
    fn from(error: SemanticCallExpansionErrorV1) -> Self {
        Self::Expansion(error)
    }
}
use SemanticCallExpansionEvidenceErrorV1 as E;

/// One root's coordinate map. Indices refer to execution storage only through
/// these origins; identities alone do not authenticate source semantics.
#[derive(Debug, Eq, PartialEq)]
pub struct SemanticExpandedRootEvidenceV1 {
    root: SemanticFunctionIdV1,
    source_body: SemanticFunctionIdV1,
    source_root_identity: SemanticFunctionIdentityV1,
    identity: [u8; 32],
    expanded_function_identity: SemanticFunctionIdentityV1,
    instances: Box<[SemanticCallInstanceV1]>,
    local_origins: Box<[SemanticExpandedLocalOriginV1]>,
    local_identities: Box<[[u8; 32]]>,
    block_origins: Box<[SemanticExpandedBlockOriginV1]>,
    block_identities: Box<[[u8; 32]]>,
}
impl SemanticExpandedRootEvidenceV1 {
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        self.root
    }
    pub const fn source_body(&self) -> SemanticFunctionIdV1 {
        self.source_body
    }
    pub const fn source_root_identity(&self) -> SemanticFunctionIdentityV1 {
        self.source_root_identity
    }
    /// Existing per-root content identity, distinct from the aggregate expansion identity.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn expanded_function_identity(&self) -> SemanticFunctionIdentityV1 {
        self.expanded_function_identity
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
    pub fn local_identity(&self, local: SemanticLocalIdV1) -> Option<&[u8; 32]> {
        self.local_identities.get(local.index() as usize)
    }
    pub fn block_identity(&self, block: SemanticBlockIdV1) -> Option<&[u8; 32]> {
        self.block_identities.get(block.index() as usize)
    }
}

/// Move-only, authority-free bytes. Decoding never constructs a live expansion,
/// an SSA owner, or a proof. Production must revalidate with its retained source.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCanonicalSemanticCallExpansionEvidenceV1 {
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
    source_semantic_sha256: [u8; 32],
    expansion_identity: [u8; 32],
    limits: SemanticCallExpansionLimitsV1,
    work_units: usize,
    roots: Box<[SemanticExpandedRootEvidenceV1]>,
}
impl InertCanonicalSemanticCallExpansionEvidenceV1 {
    pub fn from_checked_expansion(
        source: &AdmittedInertSemanticMirV1,
        expansion: &SemanticCallExpansionV1,
    ) -> EvidenceResult<Self> {
        expansion.verify_replay(source)?;
        Self::decode(&encode(source, expansion)?)
    }

    /// Replays the actual source and compares all canonical records, not only
    /// caller-supplied digests. This still grants no proof or artifact authority.
    pub fn verify_against_checked_expansion(
        &self,
        source: &AdmittedInertSemanticMirV1,
        expansion: &SemanticCallExpansionV1,
    ) -> EvidenceResult<()> {
        if self.source_semantic_sha256 != *source.semantic_sha256().as_bytes() {
            return Err(E::SourceMismatch);
        }
        expansion.verify_replay(source)?;
        if encode(source, expansion)?.as_slice() != self.canonical_bytes.as_ref() {
            return Err(E::ReplayMismatch);
        }
        Ok(())
    }

    pub fn decode(bytes: &[u8]) -> EvidenceResult<Self> {
        if bytes.len() > MAX_SEMANTIC_CALL_EXPANSION_EVIDENCE_BYTES_V1 {
            return Err(E::TooLarge);
        }
        let mut r = Reader::new(bytes);
        if r.take(8)? != MAGIC
            || r.u16()? != SEMANTIC_CALL_EXPANSION_EVIDENCE_VERSION_V1
            || r.u16()? != SEMANTIC_CALL_EXPANSION_EVIDENCE_POLICY_V1
            || r.u32()? != 0
        {
            return Err(E::InvalidHeader);
        }
        if r.u32()? as usize != bytes.len() {
            return Err(E::InvalidLength);
        }
        let source_semantic_sha256 = r.identity()?;
        let expansion_identity = r.identity()?;
        let limits = SemanticCallExpansionLimitsV1 {
            roots: r.usize()?,
            instances: r.usize()?,
            locals: r.usize()?,
            blocks: r.usize()?,
            statements: r.usize()?,
            work: r.usize()?,
            depth: r.usize()?,
        };
        if limits
            .values()
            .into_iter()
            .zip(SemanticCallExpansionLimitsV1::HARD_MAX.values())
            .any(|(limit, max)| limit > max)
        {
            return Err(E::InvalidLimits);
        }
        let work_units = r.usize()?;
        if work_units > limits.work {
            return Err(E::InvalidLimits);
        }
        let count = r.count(limits.roots, 116)?;
        if count == 0 {
            return Err(E::InvalidRecord);
        }
        let mut roots = reserve(count)?;
        let mut totals = [0_usize; 4];
        let mut source_functions = BTreeMap::new();
        for _ in 0..count {
            let root = decode_root(&mut r, limits, &mut totals)?;
            if roots
                .last()
                .is_some_and(|previous: &SemanticExpandedRootEvidenceV1| previous.root >= root.root)
            {
                return Err(E::NonCanonical);
            }
            for (function, identity) in std::iter::once((root.root, root.source_root_identity))
                .chain(
                    root.instances
                        .iter()
                        .map(|instance| (instance.function, instance.function_identity)),
                )
            {
                r.charge(1)?;
                if source_functions
                    .insert(function, identity)
                    .is_some_and(|previous| previous != identity)
                {
                    return Err(E::InvalidIdentity);
                }
            }
            roots.push(root);
        }
        if r.offset != bytes.len() {
            return Err(E::InvalidLength);
        }
        // This checks only the existing aggregate commitment. Per-root content
        // and source-derived transfers are authenticated by replay, not decoding.
        let mut digest = Sha256::new();
        r.charge(DOMAIN.len() + 32)?;
        digest.update(DOMAIN);
        digest.update(source_semantic_sha256);
        for root in &roots {
            r.charge(32)?;
            digest.update(root.identity);
        }
        if <[u8; 32]>::from(digest.finalize()) != expansion_identity {
            return Err(E::InvalidIdentity);
        }
        let mut digest = Sha256::new();
        r.charge(EVIDENCE_DOMAIN.len())?;
        r.charge(bytes.len())?;
        digest.update(EVIDENCE_DOMAIN);
        digest.update(bytes);
        r.charge(bytes.len())?;
        let mut canonical_bytes = reserve(bytes.len())?;
        canonical_bytes.extend_from_slice(bytes);
        Ok(Self {
            canonical_bytes: canonical_bytes.into_boxed_slice(),
            identity: digest.finalize().into(),
            source_semantic_sha256,
            expansion_identity,
            limits,
            work_units,
            roots: roots.into_boxed_slice(),
        })
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Identity of these evidence bytes; not the existing expansion identity.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn source_semantic_sha256(&self) -> &[u8; 32] {
        &self.source_semantic_sha256
    }
    pub const fn expansion_identity(&self) -> &[u8; 32] {
        &self.expansion_identity
    }
    pub const fn limits(&self) -> SemanticCallExpansionLimitsV1 {
        self.limits
    }
    pub const fn work_units(&self) -> usize {
        self.work_units
    }
    pub fn roots(&self) -> &[SemanticExpandedRootEvidenceV1] {
        &self.roots
    }
    pub fn root(&self, root: SemanticFunctionIdV1) -> Option<&SemanticExpandedRootEvidenceV1> {
        self.roots
            .binary_search_by_key(&root, SemanticExpandedRootEvidenceV1::root)
            .ok()
            .map(|index| &self.roots[index])
    }
    pub const fn grants_proof_or_artifact_authority(&self) -> bool {
        false
    }
}

fn reserve<T>(count: usize) -> EvidenceResult<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| E::AllocationFailure)?;
    Ok(values)
}

fn encode(
    source: &AdmittedInertSemanticMirV1,
    expansion: &SemanticCallExpansionV1,
) -> EvidenceResult<Vec<u8>> {
    let mut w = Writer(Vec::new());
    w.bytes(MAGIC)?;
    w.bytes(&SEMANTIC_CALL_EXPANSION_EVIDENCE_VERSION_V1.to_le_bytes())?;
    w.bytes(&SEMANTIC_CALL_EXPANSION_EVIDENCE_POLICY_V1.to_le_bytes())?;
    w.u32(0)?;
    w.u32(0)?;
    w.bytes(&expansion.source_semantic_sha256)?;
    w.bytes(&expansion.identity)?;
    for limit in expansion.limits.values() {
        w.usize(limit)?;
    }
    w.usize(expansion.work_units)?;
    w.count(expansion.roots.len())?;
    debug_assert_eq!(w.0.len(), HEADER_BYTES);
    for root in &expansion.roots {
        w.u32(root.root.index())?;
        w.u32(root.source_body.index())?;
        w.bytes(
            source
                .functions()
                .get(root.root.index() as usize)
                .ok_or(E::SourceMismatch)?
                .identity()
                .as_bytes(),
        )?;
        w.bytes(&root.identity)?;
        w.bytes(root.body.identity().as_bytes())?;
        w.count(root.instances.len())?;
        w.count(root.local_origins.len())?;
        w.count(root.block_origins.len())?;
        for instance in &root.instances {
            w.u32(instance.function.index())?;
            w.bytes(instance.function_identity.as_bytes())?;
            w.u32(instance.parent.map_or(ABSENT, |parent| parent.0))?;
            w.u32(instance.call_block.map_or(ABSENT, SemanticBlockIdV1::index))?;
            for count in [
                instance.local_start,
                instance.local_count,
                instance.block_start,
                instance.block_count,
            ] {
                w.u32(count)?;
            }
            w.count(instance.depth)?;
        }
        for (origin, local) in root.local_origins.iter().zip(root.body.locals()) {
            w.u32(origin.instance.0)?;
            w.u32(origin.function.index())?;
            w.u32(origin.local.index())?;
            w.bytes(local.identity().as_bytes())?;
        }
        for (origin, block) in root.block_origins.iter().zip(root.body.blocks()) {
            w.u32(origin.instance.0)?;
            w.u32(origin.function.index())?;
            w.u32(origin.block.index())?;
            w.bytes(block.identity().as_bytes())?;
            w.count(origin.statements.len())?;
            for statement in &origin.statements {
                match *statement {
                    SemanticExpandedStatementOriginV1::Source { statement } => {
                        w.tag(0)?;
                        w.u32(statement)?;
                    }
                    SemanticExpandedStatementOriginV1::ParameterTransfer { callee, argument } => {
                        w.tag(1)?;
                        w.u32(callee.0)?;
                        w.u32(argument)?;
                    }
                    SemanticExpandedStatementOriginV1::ReturnTransfer { callee } => {
                        w.tag(2)?;
                        w.u32(callee.0)?;
                    }
                    SemanticExpandedStatementOriginV1::FrameStorageLive { callee, local } => {
                        w.tag(3)?;
                        w.u32(callee.0)?;
                        w.u32(local.index())?;
                    }
                    SemanticExpandedStatementOriginV1::FrameStorageDead { callee, local } => {
                        w.tag(4)?;
                        w.u32(callee.0)?;
                        w.u32(local.index())?;
                    }
                }
            }
            match origin.terminator {
                SemanticExpandedTerminatorOriginV1::Source => w.tag(0)?,
                SemanticExpandedTerminatorOriginV1::CallEntry { callee } => {
                    w.tag(1)?;
                    w.u32(callee.0)?;
                }
                SemanticExpandedTerminatorOriginV1::CallReturn { callee } => {
                    w.tag(2)?;
                    w.u32(callee.0)?;
                }
            }
        }
    }
    let length = u32::try_from(w.0.len()).map_err(|_| E::TooLarge)?;
    w.0[16..20].copy_from_slice(&length.to_le_bytes());
    Ok(w.0)
}

struct Writer(Vec<u8>);
impl Writer {
    fn bytes(&mut self, bytes: &[u8]) -> EvidenceResult<()> {
        if self
            .0
            .len()
            .checked_add(bytes.len())
            .is_none_or(|length| length > MAX_SEMANTIC_CALL_EXPANSION_EVIDENCE_BYTES_V1)
        {
            return Err(E::TooLarge);
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(|_| E::AllocationFailure)?;
        self.0.extend_from_slice(bytes);
        Ok(())
    }
    fn tag(&mut self, value: u8) -> EvidenceResult<()> {
        self.bytes(&[value])
    }
    fn u32(&mut self, value: u32) -> EvidenceResult<()> {
        self.bytes(&value.to_le_bytes())
    }
    fn count(&mut self, value: usize) -> EvidenceResult<()> {
        self.u32(u32::try_from(value).map_err(|_| E::TooLarge)?)
    }
    fn usize(&mut self, value: usize) -> EvidenceResult<()> {
        self.bytes(&u64::try_from(value).map_err(|_| E::TooLarge)?.to_le_bytes())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
    work: usize,
}
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            work: 0,
        }
    }
    fn charge(&mut self, count: usize) -> EvidenceResult<()> {
        self.work = self.work.checked_add(count).ok_or(E::WorkLimit)?;
        if self.work > SemanticCallExpansionLimitsV1::HARD_MAX.work {
            return Err(E::WorkLimit);
        }
        Ok(())
    }
    fn take(&mut self, count: usize) -> EvidenceResult<&'a [u8]> {
        self.charge(count)?;
        let end = self.offset.checked_add(count).ok_or(E::InvalidLength)?;
        let bytes = self.bytes.get(self.offset..end).ok_or(E::InvalidLength)?;
        self.offset = end;
        Ok(bytes)
    }
    fn u8(&mut self) -> EvidenceResult<u8> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> EvidenceResult<u16> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().map_err(|_| E::InvalidLength)?,
        ))
    }
    fn u32(&mut self) -> EvidenceResult<u32> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| E::InvalidLength)?,
        ))
    }
    fn usize(&mut self) -> EvidenceResult<usize> {
        usize::try_from(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| E::InvalidLength)?,
        ))
        .map_err(|_| E::InvalidLimits)
    }
    fn identity(&mut self) -> EvidenceResult<[u8; 32]> {
        let value = self.take(32)?.try_into().map_err(|_| E::InvalidLength)?;
        if value == [0; 32] {
            return Err(E::InvalidIdentity);
        }
        Ok(value)
    }
    fn count(&mut self, max: usize, minimum_bytes: usize) -> EvidenceResult<usize> {
        let count = self.u32()? as usize;
        if count > max
            || count
                .checked_mul(minimum_bytes)
                .is_none_or(|minimum| minimum > self.bytes.len() - self.offset)
        {
            return Err(E::InvalidLength);
        }
        Ok(count)
    }
}

fn total(count: usize, sum: &mut usize, limit: usize) -> EvidenceResult<()> {
    *sum = sum
        .checked_add(count)
        .filter(|sum| *sum <= limit)
        .ok_or(E::InvalidLimits)?;
    Ok(())
}

fn decode_root(
    r: &mut Reader<'_>,
    limits: SemanticCallExpansionLimitsV1,
    totals: &mut [usize; 4],
) -> EvidenceResult<SemanticExpandedRootEvidenceV1> {
    let root = SemanticFunctionIdV1::from_index(r.u32()?);
    let source_body = SemanticFunctionIdV1::from_index(r.u32()?);
    let source_root_identity = SemanticFunctionIdentityV1::from_sha256(r.identity()?);
    let identity = r.identity()?;
    let expanded_function_identity = SemanticFunctionIdentityV1::from_sha256(r.identity()?);
    let instance_count = r.count(limits.instances, 64)?;
    let local_count = r.count(limits.locals, 44)?;
    let block_count = r.count(limits.blocks, 49)?;
    if instance_count == 0 || local_count == 0 || block_count == 0 {
        return Err(E::InvalidRecord);
    }
    for (axis, count, limit) in [
        (0, instance_count, limits.instances),
        (1, local_count, limits.locals),
        (2, block_count, limits.blocks),
    ] {
        total(count, &mut totals[axis], limit)?;
    }
    let minimum = instance_count
        .checked_mul(64)
        .and_then(|n| n.checked_add(local_count.checked_mul(44)?))
        .and_then(|n| n.checked_add(block_count.checked_mul(49)?))
        .ok_or(E::InvalidLength)?;
    if minimum > r.bytes.len() - r.offset {
        return Err(E::InvalidLength);
    }
    let mut instances: Vec<SemanticCallInstanceV1> = reserve(instance_count)?;
    let (mut locals_end, mut blocks_end) = (0_u32, 0_u32);
    let mut previous_call = None;
    for index in 0..instance_count {
        let function = SemanticFunctionIdV1::from_index(r.u32()?);
        let function_identity = SemanticFunctionIdentityV1::from_sha256(r.identity()?);
        let parent = r.u32()?;
        let call_block = r.u32()?;
        let local_start = r.u32()?;
        let locals = r.u32()?;
        let block_start = r.u32()?;
        let blocks = r.u32()?;
        let depth = r.u32()? as usize;
        if local_start != locals_end
            || block_start != blocks_end
            || locals == 0
            || blocks == 0
            || depth > limits.depth
        {
            return Err(E::InvalidRecord);
        }
        locals_end = locals_end.checked_add(locals).ok_or(E::InvalidRecord)?;
        blocks_end = blocks_end.checked_add(blocks).ok_or(E::InvalidRecord)?;
        if locals_end as usize > local_count || blocks_end as usize > block_count {
            return Err(E::InvalidRecord);
        }
        if index == 0 {
            if parent != ABSENT
                || call_block != ABSENT
                || depth != 0
                || function != source_body
                || (root == source_body && source_root_identity != function_identity)
                || (instance_count == 1 && expanded_function_identity != function_identity)
            {
                return Err(E::InvalidRecord);
            }
        } else {
            let p = instances.get(parent as usize).ok_or(E::InvalidRecord)?;
            if call_block >= p.block_count
                || depth != p.depth + 1
                || previous_call.is_some_and(|previous| previous >= (parent, call_block))
            {
                return Err(E::NonCanonical);
            }
            previous_call = Some((parent, call_block));
            let mut ancestor = Some(parent);
            while let Some(id) = ancestor {
                r.charge(1)?;
                let a = &instances[id as usize];
                if a.function == function {
                    return Err(E::InvalidRecord);
                }
                ancestor = a.parent.map(|parent| parent.0);
            }
        }
        instances.push(SemanticCallInstanceV1 {
            function,
            function_identity,
            parent: (parent != ABSENT).then_some(SemanticCallInstanceIdV1(parent)),
            call_block: (call_block != ABSENT).then_some(SemanticBlockIdV1::from_index(call_block)),
            local_start,
            local_count: locals,
            block_start,
            block_count: blocks,
            depth,
        });
    }
    if locals_end as usize != local_count || blocks_end as usize != block_count {
        return Err(E::InvalidRecord);
    }
    let mut local_origins = reserve(local_count)?;
    let mut local_identities = reserve(local_count)?;
    let mut unique = BTreeSet::new();
    for (instance_id, instance) in instances.iter().enumerate() {
        for local in 0..instance.local_count {
            if r.u32()? as usize != instance_id
                || r.u32()? != instance.function.index()
                || r.u32()? != local
            {
                return Err(E::NonCanonical);
            }
            let identity = r.identity()?;
            if !unique.insert(identity) {
                return Err(E::NonCanonical);
            }
            local_origins.push(SemanticExpandedLocalOriginV1 {
                instance: SemanticCallInstanceIdV1(instance_id as u32),
                function: instance.function,
                local: SemanticLocalIdV1::from_index(local),
            });
            local_identities.push(identity);
        }
    }
    let mut block_origins = reserve(block_count)?;
    let mut block_identities = reserve(block_count)?;
    unique.clear();
    let mut entered = BTreeSet::new();
    for (instance_id, instance) in instances.iter().enumerate() {
        for block in 0..instance.block_count {
            if r.u32()? as usize != instance_id
                || r.u32()? != instance.function.index()
                || r.u32()? != block
            {
                return Err(E::NonCanonical);
            }
            let identity = r.identity()?;
            if !unique.insert(identity) {
                return Err(E::NonCanonical);
            }
            let count = r.count(limits.statements, 5)?;
            total(count, &mut totals[3], limits.statements)?;
            let mut statements = reserve(count)?;
            for _ in 0..count {
                statements.push(match r.u8()? {
                    0 => SemanticExpandedStatementOriginV1::Source {
                        statement: r.u32()?,
                    },
                    1 => SemanticExpandedStatementOriginV1::ParameterTransfer {
                        callee: SemanticCallInstanceIdV1(r.u32()?),
                        argument: r.u32()?,
                    },
                    2 => SemanticExpandedStatementOriginV1::ReturnTransfer {
                        callee: SemanticCallInstanceIdV1(r.u32()?),
                    },
                    3 => SemanticExpandedStatementOriginV1::FrameStorageLive {
                        callee: SemanticCallInstanceIdV1(r.u32()?),
                        local: SemanticLocalIdV1::from_index(r.u32()?),
                    },
                    4 => SemanticExpandedStatementOriginV1::FrameStorageDead {
                        callee: SemanticCallInstanceIdV1(r.u32()?),
                        local: SemanticLocalIdV1::from_index(r.u32()?),
                    },
                    _ => return Err(E::InvalidRecord),
                });
            }
            let terminator = match r.u8()? {
                0 => SemanticExpandedTerminatorOriginV1::Source,
                1 => SemanticExpandedTerminatorOriginV1::CallEntry {
                    callee: SemanticCallInstanceIdV1(r.u32()?),
                },
                2 => SemanticExpandedTerminatorOriginV1::CallReturn {
                    callee: SemanticCallInstanceIdV1(r.u32()?),
                },
                _ => return Err(E::InvalidRecord),
            };
            r.charge(statements.len() + 1)?;
            validate_statement_origins(
                &statements,
                terminator,
                instance_id as u32,
                block,
                &instances,
                &mut entered,
            )?;
            block_origins.push(SemanticExpandedBlockOriginV1 {
                instance: SemanticCallInstanceIdV1(instance_id as u32),
                function: instance.function,
                block: SemanticBlockIdV1::from_index(block),
                statements: statements.into_boxed_slice(),
                terminator,
            });
            block_identities.push(identity);
        }
    }
    if entered.len() != instance_count - 1 {
        return Err(E::InvalidRecord);
    }
    Ok(SemanticExpandedRootEvidenceV1 {
        root,
        source_body,
        source_root_identity,
        identity,
        expanded_function_identity,
        instances: instances.into_boxed_slice(),
        local_origins: local_origins.into_boxed_slice(),
        local_identities: local_identities.into_boxed_slice(),
        block_origins: block_origins.into_boxed_slice(),
        block_identities: block_identities.into_boxed_slice(),
    })
}

fn validate_statement_origins(
    statements: &[SemanticExpandedStatementOriginV1],
    terminator: SemanticExpandedTerminatorOriginV1,
    instance: u32,
    block: u32,
    instances: &[SemanticCallInstanceV1],
    entered: &mut BTreeSet<u32>,
) -> EvidenceResult<()> {
    let mut source_count = 0;
    while let Some(SemanticExpandedStatementOriginV1::Source { statement }) =
        statements.get(source_count)
    {
        if *statement as usize != source_count {
            return Err(E::NonCanonical);
        }
        source_count += 1;
    }
    let suffix = &statements[source_count..];
    match terminator {
        SemanticExpandedTerminatorOriginV1::Source => {
            if !suffix.is_empty() {
                return Err(E::InvalidRecord);
            }
        }
        SemanticExpandedTerminatorOriginV1::CallEntry { callee } => {
            let child = instances.get(callee.0 as usize).ok_or(E::InvalidRecord)?;
            if child.parent != Some(SemanticCallInstanceIdV1(instance))
                || child.call_block != Some(SemanticBlockIdV1::from_index(block))
                || !entered.insert(callee.0)
            {
                return Err(E::InvalidRecord);
            }
            let mut previous_live = None;
            let mut parameter = 0;
            for origin in suffix {
                match *origin {
                    SemanticExpandedStatementOriginV1::FrameStorageLive {
                        callee: actual,
                        local,
                    } if actual == callee
                        && parameter == 0
                        && local.index() < child.local_count
                        && previous_live.is_none_or(|previous| previous < local.index()) =>
                    {
                        previous_live = Some(local.index());
                    }
                    SemanticExpandedStatementOriginV1::ParameterTransfer {
                        callee: actual,
                        argument,
                    } if actual == callee
                        && argument == parameter
                        && argument < child.local_count =>
                    {
                        parameter += 1;
                    }
                    _ => return Err(E::NonCanonical),
                }
            }
        }
        SemanticExpandedTerminatorOriginV1::CallReturn { callee } => {
            let frame = &instances[instance as usize];
            if callee.0 != instance
                || frame.parent.is_none()
                || suffix.len() != frame.local_count as usize + 1
                || suffix.first()
                    != Some(&SemanticExpandedStatementOriginV1::ReturnTransfer { callee })
            {
                return Err(E::InvalidRecord);
            }
            for (local, origin) in suffix[1..].iter().enumerate() {
                if *origin
                    != (SemanticExpandedStatementOriginV1::FrameStorageDead {
                        callee,
                        local: SemanticLocalIdV1::from_index(local as u32),
                    })
                {
                    return Err(E::NonCanonical);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod work_tests {
    use super::*;

    #[test]
    fn codec_work_is_cumulative_and_overflow_fails_closed() {
        let mut reader = Reader::new(&[0]);
        reader
            .charge(SemanticCallExpansionLimitsV1::HARD_MAX.work - 1)
            .unwrap();
        assert_eq!(reader.take(1).unwrap(), &[0]);
        assert_eq!(reader.charge(1), Err(E::WorkLimit));
        let mut reader = Reader::new(&[]);
        reader.charge(1).unwrap();
        assert_eq!(reader.charge(usize::MAX), Err(E::WorkLimit));
    }
}
