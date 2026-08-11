//! Bounded, inert semantic Rust type graph schema.
//!
//! This module deliberately has no rustc, artifact, lowering, or execution
//! integration. Nodes are interned by caller-supplied, untrusted keys. The
//! canonical binary representation sorts those keys and rewrites all edges to
//! sorted indices, so it is independent of declaration order. It is not a
//! graph-isomorphism identity and does not authenticate either keys or types.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const MAGIC: &[u8] = b"fe2o3.mir.semantic-type-graph";
const VERSION: u16 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticTypeGraphBudgetsV2 {
    pub max_nodes: u32,
    pub max_edges: u32,
    pub max_fields: u32,
    pub max_variants: u32,
    pub max_validity_ranges: u32,
    pub max_name_bytes: u32,
    pub max_canonical_bytes: u32,
    pub max_validation_work: u64,
}

impl Default for SemanticTypeGraphBudgetsV2 {
    fn default() -> Self {
        Self {
            max_nodes: 16_384,
            max_edges: 65_536,
            max_fields: 65_536,
            max_variants: 16_384,
            max_validity_ranges: 16_384,
            max_name_bytes: 4_096,
            max_canonical_bytes: 8 * 1024 * 1024,
            max_validation_work: 1_000_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticTypeNodeIdV2(u32);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticTypeLayoutV2 {
    pub size: Option<u64>,
    pub align: u64,
}

impl SemanticTypeLayoutV2 {
    pub const fn sized(size: u64, align: u64) -> Self {
        Self {
            size: Some(size),
            align,
        }
    }

    pub const fn dynamically_sized(align: u64) -> Self {
        Self { size: None, align }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticScalarV2 {
    Bool,
    Char,
    Int { signed: bool, bits: u16 },
    Float { bits: u16 },
}

impl SemanticScalarV2 {
    fn byte_width(self) -> Option<u64> {
        match self {
            Self::Bool => Some(1),
            Self::Char => Some(4),
            Self::Int { bits, .. } if matches!(bits, 8 | 16 | 32 | 64 | 128) => {
                Some(u64::from(bits / 8))
            }
            Self::Float { bits } if matches!(bits, 16 | 32 | 64 | 128) => Some(u64::from(bits / 8)),
            Self::Int { .. } | Self::Float { .. } => None,
        }
    }

    fn bits(self) -> Option<u16> {
        match self {
            Self::Bool => Some(1),
            Self::Char => Some(32),
            Self::Int { bits, .. } | Self::Float { bits } => self.byte_width().map(|_| bits),
        }
    }

    fn is_integer(self) -> bool {
        matches!(self, Self::Int { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticMutabilityV2 {
    Immutable,
    Mutable,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PointerMetadataV2 {
    None,
    SliceLength,
    VTable { trait_identity: String },
    Scalar(SemanticScalarV2),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticFieldV2 {
    pub name: Option<String>,
    pub offset: u64,
    pub ty: SemanticTypeNodeIdV2,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ScalarValidityRangeV2 {
    /// Inclusive raw scalar bit patterns; signed ranges are not numerically ordered.
    pub start: u128,
    pub end: u128,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticVariantV2 {
    pub name: String,
    pub discriminant: u128,
    pub fields: Vec<SemanticFieldV2>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticNichePathComponentV2 {
    /// Select a field by declaration index from a variant, tuple, struct, or union.
    Field(u32),
    /// Select an element from an array. The element stride is its sized layout.
    ArrayElement(u64),
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticNicheSourceV2 {
    /// A nonempty, bounded path beginning at the untagged variant payload.
    pub path: Vec<SemanticNichePathComponentV2>,
    /// The independently extracted byte offset; validation derives and compares it.
    pub expected_offset: u64,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticEnumEncodingV2 {
    Uninhabited,
    Single {
        variant: u32,
    },
    Direct {
        tag_offset: u64,
        tag: SemanticScalarV2,
    },
    Niche {
        source: SemanticNicheSourceV2,
        /// Redundant claims that must exactly match the terminal type-owned validity.
        niche_scalar: SemanticScalarV2,
        valid_ranges: Vec<ScalarValidityRangeV2>,
        untagged_variant: u32,
        niche_variants_start: u32,
        niche_variants_end: u32,
        niche_start: u128,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SemanticTypeKindV2 {
    Unit,
    Never,
    Scalar(SemanticScalarV2),
    /// A scalar whose valid bit patterns are part of its semantic type.
    ValidityScalar {
        scalar: SemanticScalarV2,
        valid_ranges: Vec<ScalarValidityRangeV2>,
    },
    RawPointer {
        pointee: SemanticTypeNodeIdV2,
        mutability: SemanticMutabilityV2,
        address_space: u32,
        data_pointer_bytes: u8,
        metadata: PointerMetadataV2,
    },
    Reference {
        referent: SemanticTypeNodeIdV2,
        mutability: SemanticMutabilityV2,
        address_space: u32,
        data_pointer_bytes: u8,
        metadata: PointerMetadataV2,
    },
    Slice {
        element: SemanticTypeNodeIdV2,
    },
    Str,
    Tuple {
        fields: Vec<SemanticFieldV2>,
    },
    Array {
        element: SemanticTypeNodeIdV2,
        length: u64,
    },
    Struct {
        identity: String,
        fields: Vec<SemanticFieldV2>,
    },
    Union {
        identity: String,
        fields: Vec<SemanticFieldV2>,
    },
    OpaqueDst {
        identity: String,
        metadata: PointerMetadataV2,
    },
    Enum {
        identity: String,
        discriminant: SemanticScalarV2,
        encoding: SemanticEnumEncodingV2,
        variants: Vec<SemanticVariantV2>,
    },
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SemanticTypeNodeV2 {
    pub layout: SemanticTypeLayoutV2,
    pub kind: SemanticTypeKindV2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SemanticTypeGraphErrorV2 {
    EmptyKey,
    NameTooLong {
        actual: usize,
        max: u32,
    },
    ResourceLimit {
        resource: &'static str,
        actual: u64,
        max: u64,
    },
    UnknownNode {
        id: u32,
    },
    UndefinedNode {
        key: String,
    },
    DuplicateDefinition {
        key: String,
    },
    UnreachableNode {
        key: String,
    },
    Invalid {
        key: String,
        reason: String,
    },
    ByValueCycle {
        key: String,
    },
    Decode {
        offset: usize,
        reason: &'static str,
    },
    NonCanonical,
}

impl fmt::Display for SemanticTypeGraphErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyKey => f.write_str("semantic type key is empty"),
            Self::NameTooLong { actual, max } => {
                write!(f, "name has {actual} bytes, exceeding {max}")
            }
            Self::ResourceLimit {
                resource,
                actual,
                max,
            } => {
                write!(f, "{resource} uses {actual}, exceeding {max}")
            }
            Self::UnknownNode { id } => write!(f, "semantic type node {id} does not exist"),
            Self::UndefinedNode { key } => write!(f, "semantic type node {key:?} is undefined"),
            Self::DuplicateDefinition { key } => {
                write!(f, "semantic type node {key:?} is already defined")
            }
            Self::UnreachableNode { key } => {
                write!(f, "semantic type node {key:?} is unreachable from the root")
            }
            Self::Invalid { key, reason } => write!(f, "semantic type node {key:?}: {reason}"),
            Self::ByValueCycle { key } => write!(f, "recursive by-value storage reaches {key:?}"),
            Self::Decode { offset, reason } => {
                write!(f, "semantic type graph decode at byte {offset}: {reason}")
            }
            Self::NonCanonical => f.write_str("semantic type graph encoding is not canonical"),
        }
    }
}

impl std::error::Error for SemanticTypeGraphErrorV2 {}

#[derive(Clone, Debug)]
struct SlotV2 {
    key: String,
    node: Option<SemanticTypeNodeV2>,
}

#[derive(Clone, Debug)]
pub struct SemanticTypeGraphBuilderV2 {
    budgets: SemanticTypeGraphBudgetsV2,
    slots: Vec<SlotV2>,
    by_key: BTreeMap<String, SemanticTypeNodeIdV2>,
}

impl SemanticTypeGraphBuilderV2 {
    pub fn new(budgets: SemanticTypeGraphBudgetsV2) -> Self {
        Self {
            budgets,
            slots: Vec::new(),
            by_key: BTreeMap::new(),
        }
    }

    /// Declares a node under a caller-supplied, unauthenticated key.
    pub fn declare(
        &mut self,
        key: impl Into<String>,
    ) -> Result<SemanticTypeNodeIdV2, SemanticTypeGraphErrorV2> {
        let key = key.into();
        validate_name(&key, self.budgets.max_name_bytes)?;
        if let Some(id) = self.by_key.get(&key) {
            return Ok(*id);
        }
        let next =
            self.slots
                .len()
                .checked_add(1)
                .ok_or(SemanticTypeGraphErrorV2::ResourceLimit {
                    resource: "nodes",
                    actual: u64::MAX,
                    max: u64::from(self.budgets.max_nodes),
                })?;
        enforce("nodes", next as u64, u64::from(self.budgets.max_nodes))?;
        let id = SemanticTypeNodeIdV2(u32::try_from(self.slots.len()).map_err(|_| {
            SemanticTypeGraphErrorV2::ResourceLimit {
                resource: "nodes",
                actual: self.slots.len() as u64,
                max: u64::from(u32::MAX),
            }
        })?);
        self.by_key.insert(key.clone(), id);
        self.slots.push(SlotV2 { key, node: None });
        Ok(id)
    }

    pub fn define(
        &mut self,
        id: SemanticTypeNodeIdV2,
        node: SemanticTypeNodeV2,
    ) -> Result<(), SemanticTypeGraphErrorV2> {
        let slot = self
            .slots
            .get_mut(id.0 as usize)
            .ok_or(SemanticTypeGraphErrorV2::UnknownNode { id: id.0 })?;
        if slot.node.is_some() {
            return Err(SemanticTypeGraphErrorV2::DuplicateDefinition {
                key: slot.key.clone(),
            });
        }
        slot.node = Some(node);
        Ok(())
    }

    /// Interns a definition under a caller-supplied, unauthenticated key.
    pub fn intern(
        &mut self,
        key: impl Into<String>,
        node: SemanticTypeNodeV2,
    ) -> Result<SemanticTypeNodeIdV2, SemanticTypeGraphErrorV2> {
        let id = self.declare(key)?;
        self.define(id, node)?;
        Ok(id)
    }

    pub fn finish(
        self,
        root: SemanticTypeNodeIdV2,
    ) -> Result<SemanticTypeGraphV2, SemanticTypeGraphErrorV2> {
        if root.0 as usize >= self.slots.len() {
            return Err(SemanticTypeGraphErrorV2::UnknownNode { id: root.0 });
        }
        let mut nodes = Vec::with_capacity(self.slots.len());
        let mut keys = Vec::with_capacity(self.slots.len());
        for slot in self.slots {
            let node = slot
                .node
                .ok_or_else(|| SemanticTypeGraphErrorV2::UndefinedNode {
                    key: slot.key.clone(),
                })?;
            keys.push(slot.key);
            nodes.push(node);
        }
        let graph = SemanticTypeGraphV2 {
            budgets: self.budgets,
            root,
            keys,
            nodes,
        };
        graph.validate()?;
        graph.canonical_bytes()?;
        Ok(graph)
    }
}

#[derive(Clone, Debug)]
pub struct SemanticTypeGraphV2 {
    budgets: SemanticTypeGraphBudgetsV2,
    root: SemanticTypeNodeIdV2,
    keys: Vec<String>,
    nodes: Vec<SemanticTypeNodeV2>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UntrustedSemanticTypeGraphEncodingV2(Box<[u8]>);

impl UntrustedSemanticTypeGraphEncodingV2 {
    /// Declaration-order-stable bytes containing untrusted caller keys.
    ///
    /// A later rustc extraction boundary must authenticate every key before an
    /// artifact may use these bytes as semantic identity evidence.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl SemanticTypeGraphV2 {
    pub fn root_key(&self) -> &str {
        &self.keys[self.root.0 as usize]
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn node_by_key(&self, key: &str) -> Option<&SemanticTypeNodeV2> {
        self.keys
            .iter()
            .position(|candidate| candidate == key)
            .map(|index| &self.nodes[index])
    }

    pub fn untrusted_canonical_encoding(
        &self,
    ) -> Result<UntrustedSemanticTypeGraphEncodingV2, SemanticTypeGraphErrorV2> {
        Ok(UntrustedSemanticTypeGraphEncodingV2(
            self.canonical_bytes()?.into_boxed_slice(),
        ))
    }

    /// Returns declaration-order-stable bytes, not authenticated semantic identity.
    /// Caller keys remain part of this encoding and are not derived here.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, SemanticTypeGraphErrorV2> {
        self.validate()?;
        let mut order: Vec<usize> = (0..self.nodes.len()).collect();
        order.sort_unstable_by(|left, right| {
            self.keys[*left]
                .as_bytes()
                .cmp(self.keys[*right].as_bytes())
        });
        let mut remap = vec![0_u32; order.len()];
        for (canonical, original) in order.iter().copied().enumerate() {
            remap[original] = canonical as u32;
        }
        let mut writer = WriterV2::new(self.budgets.max_canonical_bytes);
        writer.bytes(MAGIC)?;
        writer.u16(VERSION)?;
        writer.u32(order.len() as u32)?;
        writer.u32(remap[self.root.0 as usize])?;
        for original in order {
            writer.string(&self.keys[original], self.budgets.max_name_bytes)?;
            encode_node(
                &mut writer,
                &self.nodes[original],
                &remap,
                self.budgets.max_name_bytes,
            )?;
        }
        Ok(writer.finish())
    }

    pub fn decode_canonical(
        input: &[u8],
        budgets: SemanticTypeGraphBudgetsV2,
    ) -> Result<Self, SemanticTypeGraphErrorV2> {
        enforce(
            "canonical bytes",
            input.len() as u64,
            u64::from(budgets.max_canonical_bytes),
        )?;
        let mut reader = ReaderV2::new(input);
        if reader.take(MAGIC.len())? != MAGIC {
            return Err(reader.error("wrong semantic type graph magic"));
        }
        if reader.u16()? != VERSION {
            return Err(reader.error("unsupported semantic type graph version"));
        }
        let count = reader.u32()?;
        enforce("nodes", u64::from(count), u64::from(budgets.max_nodes))?;
        let root = reader.u32()?;
        if root >= count {
            return Err(reader.error("root index is out of range"));
        }
        reader.ensure_count_fits(count, 15, "node count exceeds remaining input")?;
        let mut keys = Vec::with_capacity(count as usize);
        let mut nodes = Vec::with_capacity(count as usize);
        let mut decode_totals = DecodeTotalsV2::default();
        for _ in 0..count {
            let key = reader.string(budgets.max_name_bytes)?;
            if keys
                .last()
                .is_some_and(|previous: &String| previous.as_bytes() >= key.as_bytes())
            {
                return Err(reader.error("node keys are not strictly canonical"));
            }
            keys.push(key);
            nodes.push(decode_node(&mut reader, budgets, &mut decode_totals)?);
        }
        if !reader.is_empty() {
            return Err(reader.error("trailing bytes"));
        }
        for node in &nodes {
            for edge in all_edges(&node.kind) {
                if edge.0 >= count {
                    return Err(reader.error("edge index is out of range"));
                }
            }
        }
        let graph = Self {
            budgets,
            root: SemanticTypeNodeIdV2(root),
            keys,
            nodes,
        };
        graph.validate()?;
        if graph.canonical_bytes()?.as_slice() != input {
            return Err(SemanticTypeGraphErrorV2::NonCanonical);
        }
        Ok(graph)
    }

    fn validate(&self) -> Result<(), SemanticTypeGraphErrorV2> {
        enforce(
            "nodes",
            self.nodes.len() as u64,
            u64::from(self.budgets.max_nodes),
        )?;
        if self.nodes.is_empty() || self.root.0 as usize >= self.nodes.len() {
            return Err(SemanticTypeGraphErrorV2::UnknownNode { id: self.root.0 });
        }
        let mut totals = TotalsV2::default();
        let mut work = WorkV2::new(self.budgets.max_validation_work);
        for (index, node) in self.nodes.iter().enumerate() {
            work.one()?;
            validate_name(&self.keys[index], self.budgets.max_name_bytes)?;
            validate_layout(node.layout, &self.keys[index])?;
            validate_node(self, index, node, &mut totals, &mut work)?;
        }
        enforce("edges", totals.edges, u64::from(self.budgets.max_edges))?;
        enforce("fields", totals.fields, u64::from(self.budgets.max_fields))?;
        enforce(
            "variants",
            totals.variants,
            u64::from(self.budgets.max_variants),
        )?;
        enforce(
            "validity ranges",
            totals.ranges,
            u64::from(self.budgets.max_validity_ranges),
        )?;
        self.validate_reachability(&mut work)?;
        self.validate_definition_uniqueness()?;
        self.validate_by_value_acyclic(&mut work)
    }

    fn validate_definition_uniqueness(&self) -> Result<(), SemanticTypeGraphErrorV2> {
        let mut nominal_identities = BTreeMap::new();
        let mut exact_definitions = BTreeMap::new();
        for (index, node) in self.nodes.iter().enumerate() {
            if let Some(identity) = nominal_identity(&node.kind)
                && let Some(previous) = nominal_identities.insert(identity, index)
            {
                return Err(invalid(
                    &self.keys[index],
                    format!(
                        "nominal identity {identity:?} duplicates node {:?}",
                        self.keys[previous]
                    ),
                ));
            }
            if let Some(previous) = exact_definitions.insert(node, index) {
                return Err(invalid(
                    &self.keys[index],
                    format!(
                        "exact type definition duplicates node {:?}",
                        self.keys[previous]
                    ),
                ));
            }
        }
        Ok(())
    }

    fn validate_reachability(&self, work: &mut WorkV2) -> Result<(), SemanticTypeGraphErrorV2> {
        let mut seen = vec![false; self.nodes.len()];
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            work.one()?;
            let index = id.0 as usize;
            if index >= self.nodes.len() {
                return Err(SemanticTypeGraphErrorV2::UnknownNode { id: id.0 });
            }
            if std::mem::replace(&mut seen[index], true) {
                continue;
            }
            stack.extend(all_edges(&self.nodes[index].kind));
        }
        if let Some(index) = seen.iter().position(|seen| !seen) {
            return Err(SemanticTypeGraphErrorV2::UnreachableNode {
                key: self.keys[index].clone(),
            });
        }
        Ok(())
    }

    fn validate_by_value_acyclic(&self, work: &mut WorkV2) -> Result<(), SemanticTypeGraphErrorV2> {
        let mut color = vec![0_u8; self.nodes.len()];
        for start in 0..self.nodes.len() {
            if color[start] != 0 {
                continue;
            }
            color[start] = 1;
            let mut stack = vec![(start, 0_usize, by_value_edges(&self.nodes[start].kind))];
            while let Some((node, next, edges)) = stack.last_mut() {
                work.one()?;
                if *next == edges.len() {
                    color[*node] = 2;
                    stack.pop();
                    continue;
                }
                let child = edges[*next].0 as usize;
                *next += 1;
                if child >= self.nodes.len() {
                    return Err(SemanticTypeGraphErrorV2::UnknownNode { id: child as u32 });
                }
                match color[child] {
                    0 => {
                        color[child] = 1;
                        stack.push((child, 0, by_value_edges(&self.nodes[child].kind)));
                    }
                    1 => {
                        return Err(SemanticTypeGraphErrorV2::ByValueCycle {
                            key: self.keys[child].clone(),
                        });
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
struct TotalsV2 {
    edges: u64,
    fields: u64,
    variants: u64,
    ranges: u64,
}

struct WorkV2 {
    used: u64,
    max: u64,
}
impl WorkV2 {
    fn new(max: u64) -> Self {
        Self { used: 0, max }
    }
    fn one(&mut self) -> Result<(), SemanticTypeGraphErrorV2> {
        self.used = self
            .used
            .checked_add(1)
            .ok_or(SemanticTypeGraphErrorV2::ResourceLimit {
                resource: "validation work",
                actual: u64::MAX,
                max: self.max,
            })?;
        enforce("validation work", self.used, self.max)
    }
}

fn validate_name(name: &str, max: u32) -> Result<(), SemanticTypeGraphErrorV2> {
    if name.is_empty() {
        return Err(SemanticTypeGraphErrorV2::EmptyKey);
    }
    if name.len() > max as usize {
        return Err(SemanticTypeGraphErrorV2::NameTooLong {
            actual: name.len(),
            max,
        });
    }
    Ok(())
}

fn enforce(resource: &'static str, actual: u64, max: u64) -> Result<(), SemanticTypeGraphErrorV2> {
    if actual > max {
        Err(SemanticTypeGraphErrorV2::ResourceLimit {
            resource,
            actual,
            max,
        })
    } else {
        Ok(())
    }
}

fn invalid(key: &str, reason: impl Into<String>) -> SemanticTypeGraphErrorV2 {
    SemanticTypeGraphErrorV2::Invalid {
        key: key.to_owned(),
        reason: reason.into(),
    }
}

fn validate_layout(
    layout: SemanticTypeLayoutV2,
    key: &str,
) -> Result<(), SemanticTypeGraphErrorV2> {
    if layout.align == 0 || !layout.align.is_power_of_two() {
        return Err(invalid(key, "alignment must be a nonzero power of two"));
    }
    if let Some(size) = layout.size
        && !size.is_multiple_of(layout.align)
    {
        return Err(invalid(key, "sized layout must be rounded up to alignment"));
    }
    Ok(())
}

fn node(
    graph: &SemanticTypeGraphV2,
    id: SemanticTypeNodeIdV2,
) -> Result<&SemanticTypeNodeV2, SemanticTypeGraphErrorV2> {
    graph
        .nodes
        .get(id.0 as usize)
        .ok_or(SemanticTypeGraphErrorV2::UnknownNode { id: id.0 })
}

fn nominal_identity(kind: &SemanticTypeKindV2) -> Option<&str> {
    match kind {
        SemanticTypeKindV2::Struct { identity, .. }
        | SemanticTypeKindV2::Union { identity, .. }
        | SemanticTypeKindV2::OpaqueDst { identity, .. }
        | SemanticTypeKindV2::Enum { identity, .. } => Some(identity),
        _ => None,
    }
}

fn validate_node(
    graph: &SemanticTypeGraphV2,
    index: usize,
    node_value: &SemanticTypeNodeV2,
    totals: &mut TotalsV2,
    work: &mut WorkV2,
) -> Result<(), SemanticTypeGraphErrorV2> {
    let key = &graph.keys[index];
    let edge_count = edge_count(&node_value.kind);
    totals.edges = totals
        .edges
        .checked_add(edge_count)
        .ok_or_else(|| invalid(key, "edge count overflows u64"))?;
    enforce("edges", totals.edges, u64::from(graph.budgets.max_edges))?;
    for edge in all_edges(&node_value.kind) {
        node(graph, edge)?;
        work.one()?;
    }
    let mut context = ValidationContextV2 {
        graph,
        totals,
        work,
    };
    match &node_value.kind {
        SemanticTypeKindV2::Unit | SemanticTypeKindV2::Never => {
            if node_value.layout != SemanticTypeLayoutV2::sized(0, 1) {
                return Err(invalid(key, "unit and never require size 0, alignment 1"));
            }
        }
        SemanticTypeKindV2::Scalar(scalar) => {
            let width = scalar
                .byte_width()
                .ok_or_else(|| invalid(key, "unsupported scalar width"))?;
            if node_value.layout != SemanticTypeLayoutV2::sized(width, width) {
                return Err(invalid(
                    key,
                    format!("scalar requires size and alignment {width}"),
                ));
            }
        }
        SemanticTypeKindV2::ValidityScalar {
            scalar,
            valid_ranges,
        } => {
            let width = scalar
                .byte_width()
                .ok_or_else(|| invalid(key, "unsupported validity scalar width"))?;
            if node_value.layout != SemanticTypeLayoutV2::sized(width, width) {
                return Err(invalid(
                    key,
                    format!("validity scalar requires size and alignment {width}"),
                ));
            }
            validate_scalar_validity(&mut context, key, *scalar, valid_ranges)?;
        }
        SemanticTypeKindV2::RawPointer {
            pointee,
            data_pointer_bytes,
            metadata,
            ..
        } => validate_pointer(
            &mut context,
            key,
            node_value.layout,
            *pointee,
            *data_pointer_bytes,
            metadata,
        )?,
        SemanticTypeKindV2::Reference {
            referent,
            data_pointer_bytes,
            metadata,
            ..
        } => validate_pointer(
            &mut context,
            key,
            node_value.layout,
            *referent,
            *data_pointer_bytes,
            metadata,
        )?,
        SemanticTypeKindV2::Slice { element } => {
            let element = node(graph, *element)?;
            if element.layout.size.is_none() {
                return Err(invalid(key, "slice element must be sized"));
            }
            if node_value.layout != SemanticTypeLayoutV2::dynamically_sized(element.layout.align) {
                return Err(invalid(key, "slice alignment must equal element alignment"));
            }
        }
        SemanticTypeKindV2::Str => {
            if node_value.layout != SemanticTypeLayoutV2::dynamically_sized(1) {
                return Err(invalid(key, "str requires dynamic size and alignment 1"));
            }
        }
        SemanticTypeKindV2::Tuple { fields } => validate_aggregate(
            &mut context,
            key,
            node_value.layout,
            fields,
            FieldNamesV2::Absent,
            false,
        )?,
        SemanticTypeKindV2::Array { element, length } => {
            let element = node(graph, *element)?;
            let element_size = element
                .layout
                .size
                .ok_or_else(|| invalid(key, "array element must be sized"))?;
            let size = element_size
                .checked_mul(*length)
                .ok_or_else(|| invalid(key, "array byte size overflows u64"))?;
            if node_value.layout != SemanticTypeLayoutV2::sized(size, element.layout.align) {
                return Err(invalid(
                    key,
                    "array layout does not equal element stride times length",
                ));
            }
        }
        SemanticTypeKindV2::Struct { identity, fields } => {
            validate_name(identity, graph.budgets.max_name_bytes)?;
            validate_aggregate(
                &mut context,
                key,
                node_value.layout,
                fields,
                FieldNamesV2::Required,
                false,
            )?;
        }
        SemanticTypeKindV2::Union { identity, fields } => {
            validate_name(identity, graph.budgets.max_name_bytes)?;
            validate_aggregate(
                &mut context,
                key,
                node_value.layout,
                fields,
                FieldNamesV2::Required,
                true,
            )?;
        }
        SemanticTypeKindV2::OpaqueDst { identity, metadata } => {
            validate_name(identity, graph.budgets.max_name_bytes)?;
            if node_value.layout.size.is_some() || matches!(metadata, PointerMetadataV2::None) {
                return Err(invalid(
                    key,
                    "opaque DST requires dynamic size and explicit metadata",
                ));
            }
            validate_metadata(metadata, graph.budgets.max_name_bytes, key)?;
        }
        SemanticTypeKindV2::Enum {
            identity,
            discriminant,
            encoding,
            variants,
        } => {
            validate_name(identity, graph.budgets.max_name_bytes)?;
            validate_enum(
                &mut context,
                key,
                node_value.layout,
                *discriminant,
                encoding,
                variants,
            )?;
        }
    }
    Ok(())
}

fn expected_metadata(
    graph: &SemanticTypeGraphV2,
    target: SemanticTypeNodeIdV2,
    work: &mut WorkV2,
) -> Result<PointerMetadataV2, SemanticTypeGraphErrorV2> {
    let mut current = target;
    let mut seen = BTreeSet::new();
    loop {
        work.one()?;
        if !seen.insert(current) {
            return Err(invalid(
                &graph.keys[target.0 as usize],
                "DST tail metadata cycle",
            ));
        }
        let target_node = node(graph, current)?;
        if target_node.layout.size.is_some() {
            return Ok(PointerMetadataV2::None);
        }
        match &target_node.kind {
            SemanticTypeKindV2::Slice { .. } | SemanticTypeKindV2::Str => {
                return Ok(PointerMetadataV2::SliceLength);
            }
            SemanticTypeKindV2::OpaqueDst { metadata, .. } => return Ok(metadata.clone()),
            SemanticTypeKindV2::Tuple { fields } | SemanticTypeKindV2::Struct { fields, .. } => {
                current = fields
                    .last()
                    .ok_or_else(|| {
                        invalid(
                            &graph.keys[current.0 as usize],
                            "unsized aggregate has no DST tail",
                        )
                    })?
                    .ty;
            }
            _ => {
                return Err(invalid(
                    &graph.keys[current.0 as usize],
                    "node has dynamic size without a metadata-bearing DST form",
                ));
            }
        }
    }
}

fn validate_pointer(
    context: &mut ValidationContextV2<'_>,
    key: &str,
    layout: SemanticTypeLayoutV2,
    target: SemanticTypeNodeIdV2,
    data_bytes: u8,
    metadata: &PointerMetadataV2,
) -> Result<(), SemanticTypeGraphErrorV2> {
    if !matches!(data_bytes, 1 | 2 | 4 | 8 | 16) {
        return Err(invalid(
            key,
            "data pointer width must be 1, 2, 4, 8, or 16 bytes",
        ));
    }
    validate_metadata(metadata, context.graph.budgets.max_name_bytes, key)?;
    let expected = expected_metadata(context.graph, target, context.work)?;
    if &expected != metadata {
        return Err(invalid(
            key,
            "pointer metadata does not match the pointee DST form",
        ));
    }
    let metadata_bytes = match metadata {
        PointerMetadataV2::None => 0,
        PointerMetadataV2::SliceLength | PointerMetadataV2::VTable { .. } => u64::from(data_bytes),
        PointerMetadataV2::Scalar(scalar) => scalar
            .byte_width()
            .ok_or_else(|| invalid(key, "unsupported scalar metadata width"))?,
    };
    let unrounded_size = u64::from(data_bytes)
        .checked_add(metadata_bytes)
        .ok_or_else(|| invalid(key, "pointer layout size overflows u64"))?;
    let align = u64::from(data_bytes).max(metadata_bytes.max(1));
    let size = unrounded_size
        .checked_add(align - 1)
        .map(|value| value & !(align - 1))
        .ok_or_else(|| invalid(key, "pointer layout rounding overflows u64"))?;
    if layout != SemanticTypeLayoutV2::sized(size, align) {
        return Err(invalid(
            key,
            format!("pointer requires size {size} and alignment {align}"),
        ));
    }
    Ok(())
}

fn validate_metadata(
    metadata: &PointerMetadataV2,
    max_name: u32,
    key: &str,
) -> Result<(), SemanticTypeGraphErrorV2> {
    match metadata {
        PointerMetadataV2::VTable { trait_identity } => validate_name(trait_identity, max_name),
        PointerMetadataV2::Scalar(scalar) if scalar.byte_width().is_none() => {
            Err(invalid(key, "unsupported scalar metadata width"))
        }
        _ => Ok(()),
    }
}

#[derive(Clone, Copy)]
enum FieldNamesV2 {
    Absent,
    Required,
}

struct ValidationContextV2<'a> {
    graph: &'a SemanticTypeGraphV2,
    totals: &'a mut TotalsV2,
    work: &'a mut WorkV2,
}

fn validate_aggregate(
    context: &mut ValidationContextV2<'_>,
    key: &str,
    layout: SemanticTypeLayoutV2,
    fields: &[SemanticFieldV2],
    names: FieldNamesV2,
    union: bool,
) -> Result<(), SemanticTypeGraphErrorV2> {
    context.totals.fields = context
        .totals
        .fields
        .checked_add(fields.len() as u64)
        .ok_or_else(|| invalid(key, "field count overflows u64"))?;
    enforce(
        "fields",
        context.totals.fields,
        u64::from(context.graph.budgets.max_fields),
    )?;
    let mut seen_names = BTreeSet::new();
    let mut ranges = Vec::new();
    let mut unsized_tail = None;
    let mut maximum_align = 1_u64;
    for (position, field) in fields.iter().enumerate() {
        context.work.one()?;
        match names {
            FieldNamesV2::Absent if field.name.is_some() => {
                return Err(invalid(key, "tuple fields must be unnamed"));
            }
            FieldNamesV2::Required => {
                let name = field
                    .name
                    .as_deref()
                    .ok_or_else(|| invalid(key, "named aggregate field is unnamed"))?;
                validate_name(name, context.graph.budgets.max_name_bytes)?;
                if !seen_names.insert(name) {
                    return Err(invalid(key, "aggregate field names are not unique"));
                }
            }
            FieldNamesV2::Absent => {}
        }
        let child = node(context.graph, field.ty)?;
        maximum_align = maximum_align.max(child.layout.align);
        if union && field.offset != 0 {
            return Err(invalid(key, "union fields must start at offset zero"));
        }
        if !union && !field.offset.is_multiple_of(child.layout.align) {
            return Err(invalid(key, "field offset violates field alignment"));
        }
        match child.layout.size {
            Some(0) => {
                if let Some(size) = layout.size
                    && field.offset > size
                {
                    return Err(invalid(
                        key,
                        "zero-sized field offset exceeds aggregate size",
                    ));
                }
            }
            Some(size) => {
                let end = field
                    .offset
                    .checked_add(size)
                    .ok_or_else(|| invalid(key, "field byte range overflows u64"))?;
                ranges.push((field.offset, end));
            }
            None => {
                if union
                    || unsized_tail.replace(field.offset).is_some()
                    || position + 1 != fields.len()
                    || layout.size.is_some()
                {
                    return Err(invalid(
                        key,
                        "only the final field of an unsized non-union aggregate may be unsized",
                    ));
                }
            }
        }
    }
    if layout.align < maximum_align || !layout.align.is_multiple_of(maximum_align) {
        return Err(invalid(
            key,
            format!("aggregate alignment must be a multiple of {maximum_align}"),
        ));
    }
    if union {
        let size = layout
            .size
            .ok_or_else(|| invalid(key, "union must be sized"))?;
        if ranges.iter().any(|(_, end)| *end > size) {
            return Err(invalid(key, "union field exceeds union storage"));
        }
        return Ok(());
    }
    if layout.size.is_none() != unsized_tail.is_some() {
        return Err(invalid(
            key,
            "dynamic aggregate size must exactly correspond to a DST tail",
        ));
    }
    let bound = layout
        .size
        .or(unsized_tail)
        .ok_or_else(|| invalid(key, "aggregate has no sized prefix bound"))?;
    if ranges.iter().any(|(_, end)| *end > bound) {
        return Err(invalid(key, "field exceeds aggregate sized prefix"));
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(invalid(key, "non-zero-sized aggregate fields overlap"));
    }
    Ok(())
}

fn validate_scalar_validity(
    context: &mut ValidationContextV2<'_>,
    key: &str,
    scalar: SemanticScalarV2,
    valid_ranges: &[ScalarValidityRangeV2],
) -> Result<(), SemanticTypeGraphErrorV2> {
    if !scalar.is_integer() || scalar.byte_width().is_none() {
        return Err(invalid(key, "validity scalar must be a supported integer"));
    }
    if valid_ranges.is_empty() {
        return Err(invalid(key, "scalar validity set must not be empty"));
    }
    context.totals.ranges = context
        .totals
        .ranges
        .checked_add(valid_ranges.len() as u64)
        .ok_or_else(|| invalid(key, "validity range count overflows u64"))?;
    enforce(
        "validity ranges",
        context.totals.ranges,
        u64::from(context.graph.budgets.max_validity_ranges),
    )?;
    let bits = scalar.bits().unwrap_or(0);
    let mut previous_end = None;
    for range in valid_ranges {
        context.work.one()?;
        if range.start > range.end
            || !fits_bits(range.end, bits)
            || previous_end.is_some_and(|end| end >= range.start)
        {
            return Err(invalid(
                key,
                "validity ranges must be sorted, disjoint, nonempty, and fit the scalar",
            ));
        }
        previous_end = Some(range.end);
    }
    Ok(())
}

enum NicheCursorV2 {
    VariantFields,
    Node(SemanticTypeNodeIdV2),
}

fn resolve_niche_source<'graph>(
    context: &mut ValidationContextV2<'graph>,
    key: &str,
    variants: &[SemanticVariantV2],
    untagged_variant: u32,
    source: &SemanticNicheSourceV2,
) -> Result<(u64, SemanticScalarV2, &'graph [ScalarValidityRangeV2]), SemanticTypeGraphErrorV2> {
    if source.path.is_empty() {
        return Err(invalid(key, "niche source path must not be empty"));
    }
    let variant = variants
        .get(untagged_variant as usize)
        .ok_or_else(|| invalid(key, "untagged niche variant is out of range"))?;
    let mut cursor = NicheCursorV2::VariantFields;
    let mut offset = 0_u64;
    for component in &source.path {
        context.work.one()?;
        match (component, cursor) {
            (SemanticNichePathComponentV2::Field(index), NicheCursorV2::VariantFields) => {
                let field = variant
                    .fields
                    .get(*index as usize)
                    .ok_or_else(|| invalid(key, "niche field path component is out of range"))?;
                offset = offset
                    .checked_add(field.offset)
                    .ok_or_else(|| invalid(key, "niche field path offset overflows u64"))?;
                cursor = NicheCursorV2::Node(field.ty);
            }
            (SemanticNichePathComponentV2::Field(index), NicheCursorV2::Node(id)) => {
                let fields = match &node(context.graph, id)?.kind {
                    SemanticTypeKindV2::Tuple { fields }
                    | SemanticTypeKindV2::Struct { fields, .. }
                    | SemanticTypeKindV2::Union { fields, .. } => fields,
                    _ => {
                        return Err(invalid(
                            key,
                            "niche field path does not traverse an aggregate",
                        ));
                    }
                };
                let field = fields
                    .get(*index as usize)
                    .ok_or_else(|| invalid(key, "niche field path component is out of range"))?;
                offset = offset
                    .checked_add(field.offset)
                    .ok_or_else(|| invalid(key, "niche field path offset overflows u64"))?;
                cursor = NicheCursorV2::Node(field.ty);
            }
            (SemanticNichePathComponentV2::ArrayElement(index), NicheCursorV2::Node(id)) => {
                let (element, length) = match &node(context.graph, id)?.kind {
                    SemanticTypeKindV2::Array { element, length } => (*element, *length),
                    _ => return Err(invalid(key, "niche array path does not traverse an array")),
                };
                if *index >= length {
                    return Err(invalid(key, "niche array index is out of range"));
                }
                let stride = node(context.graph, element)?
                    .layout
                    .size
                    .ok_or_else(|| invalid(key, "niche array element is unsized"))?;
                let element_offset = stride
                    .checked_mul(*index)
                    .ok_or_else(|| invalid(key, "niche array path offset overflows u64"))?;
                offset = offset
                    .checked_add(element_offset)
                    .ok_or_else(|| invalid(key, "niche array path offset overflows u64"))?;
                cursor = NicheCursorV2::Node(element);
            }
            (SemanticNichePathComponentV2::ArrayElement(_), NicheCursorV2::VariantFields) => {
                return Err(invalid(key, "niche path must begin with a variant field"));
            }
        }
    }
    let NicheCursorV2::Node(terminal) = cursor else {
        return Err(invalid(key, "niche source path has no terminal type"));
    };
    if offset != source.expected_offset {
        return Err(invalid(
            key,
            "niche source offset does not match its traversed payload path",
        ));
    }
    match &node(context.graph, terminal)?.kind {
        SemanticTypeKindV2::ValidityScalar {
            scalar,
            valid_ranges,
        } => Ok((offset, *scalar, valid_ranges)),
        _ => Err(invalid(
            key,
            "niche source must terminate at a validity-constrained scalar type",
        )),
    }
}

fn validate_enum(
    context: &mut ValidationContextV2<'_>,
    key: &str,
    layout: SemanticTypeLayoutV2,
    discriminant: SemanticScalarV2,
    encoding: &SemanticEnumEncodingV2,
    variants: &[SemanticVariantV2],
) -> Result<(), SemanticTypeGraphErrorV2> {
    if !discriminant.is_integer() || discriminant.byte_width().is_none() {
        return Err(invalid(
            key,
            "enum discriminant must be a supported integer",
        ));
    }
    let size = layout
        .size
        .ok_or_else(|| invalid(key, "enum must be sized"))?;
    context.totals.variants = context
        .totals
        .variants
        .checked_add(variants.len() as u64)
        .ok_or_else(|| invalid(key, "variant count overflows u64"))?;
    enforce(
        "variants",
        context.totals.variants,
        u64::from(context.graph.budgets.max_variants),
    )?;
    let mut names = BTreeSet::new();
    let mut values = BTreeSet::new();
    for variant in variants {
        context.work.one()?;
        validate_name(&variant.name, context.graph.budgets.max_name_bytes)?;
        if !names.insert(variant.name.as_str()) || !values.insert(variant.discriminant) {
            return Err(invalid(
                key,
                "variant names and discriminants must be unique",
            ));
        }
        if !fits_bits(variant.discriminant, discriminant.bits().unwrap_or(0)) {
            return Err(invalid(
                key,
                "variant discriminant does not fit its representation",
            ));
        }
        validate_aggregate(
            context,
            key,
            layout,
            &variant.fields,
            FieldNamesV2::Required,
            false,
        )?;
    }
    match encoding {
        SemanticEnumEncodingV2::Uninhabited => {
            if !variants.is_empty() || layout != SemanticTypeLayoutV2::sized(0, 1) {
                return Err(invalid(
                    key,
                    "uninhabited enum requires no variants and zero-sized layout",
                ));
            }
        }
        SemanticEnumEncodingV2::Single { variant } => {
            if variants.len() != 1 || *variant != 0 {
                return Err(invalid(
                    key,
                    "single enum encoding requires exactly variant zero",
                ));
            }
        }
        SemanticEnumEncodingV2::Direct { tag_offset, tag } => {
            if !tag.is_integer() {
                return Err(invalid(key, "direct enum tag must be an integer"));
            }
            let width = tag
                .byte_width()
                .ok_or_else(|| invalid(key, "unsupported direct enum tag width"))?;
            let tag_end = checked_end(*tag_offset, width, size, key, "direct tag")?;
            if variants.is_empty()
                || variants
                    .iter()
                    .any(|variant| !fits_bits(variant.discriminant, tag.bits().unwrap_or(0)))
            {
                return Err(invalid(key, "direct tag cannot represent every variant"));
            }
            for variant in variants {
                for field in &variant.fields {
                    context.work.one()?;
                    if let Some(field_size) = node(context.graph, field.ty)?.layout.size
                        && overlaps(
                            (*tag_offset, tag_end),
                            (field.offset, field.offset + field_size),
                        )
                    {
                        return Err(invalid(key, "direct tag overlaps variant payload"));
                    }
                }
            }
        }
        SemanticEnumEncodingV2::Niche {
            source,
            niche_scalar,
            valid_ranges,
            untagged_variant,
            niche_variants_start,
            niche_variants_end,
            niche_start,
        } => {
            let (niche_offset, terminal_scalar, terminal_ranges) =
                resolve_niche_source(context, key, variants, *untagged_variant, source)?;
            if *niche_scalar != terminal_scalar || valid_ranges.as_slice() != terminal_ranges {
                return Err(invalid(
                    key,
                    "niche scalar and validity ranges must exactly match the terminal type",
                ));
            }
            context.totals.ranges = context
                .totals
                .ranges
                .checked_add(valid_ranges.len() as u64)
                .ok_or_else(|| invalid(key, "validity range count overflows u64"))?;
            enforce(
                "validity ranges",
                context.totals.ranges,
                u64::from(context.graph.budgets.max_validity_ranges),
            )?;
            let width = niche_scalar
                .byte_width()
                .ok_or_else(|| invalid(key, "unsupported niche scalar width"))?;
            checked_end(niche_offset, width, size, key, "niche")?;
            let bits = niche_scalar.bits().unwrap_or(0);
            if !niche_scalar.is_integer() {
                return Err(invalid(key, "niche scalar must be an integer"));
            }
            if *niche_variants_start > *niche_variants_end
                || *niche_variants_end as usize >= variants.len()
                || *untagged_variant as usize >= variants.len()
                || (*niche_variants_start..=*niche_variants_end).contains(untagged_variant)
            {
                return Err(invalid(
                    key,
                    "niche variant range and untagged variant must be valid and disjoint",
                ));
            }
            if valid_ranges.is_empty() {
                return Err(invalid(key, "niche validity set must not be empty"));
            }
            let mut previous_end = None;
            for range in valid_ranges {
                context.work.one()?;
                if range.start > range.end
                    || !fits_bits(range.end, bits)
                    || previous_end.is_some_and(|end| end >= range.start)
                {
                    return Err(invalid(
                        key,
                        "validity ranges must be sorted, disjoint, nonempty, and fit the niche scalar",
                    ));
                }
                previous_end = Some(range.end);
            }
            let count = u128::from(*niche_variants_end - *niche_variants_start) + 1;
            let last = niche_start
                .checked_add(count - 1)
                .ok_or_else(|| invalid(key, "niche value range overflows u128"))?;
            if !fits_bits(last, bits) {
                return Err(invalid(key, "niche values do not fit the niche scalar"));
            }
            if valid_ranges
                .iter()
                .any(|range| ranges_intersect((range.start, range.end), (*niche_start, last)))
            {
                return Err(invalid(key, "niche values overlap the scalar valid set"));
            }
        }
    }
    Ok(())
}

fn fits_bits(value: u128, bits: u16) -> bool {
    bits == 128 || (bits > 0 && value < (1_u128 << bits))
}
fn checked_end(
    offset: u64,
    width: u64,
    size: u64,
    key: &str,
    what: &str,
) -> Result<u64, SemanticTypeGraphErrorV2> {
    let end = offset
        .checked_add(width)
        .ok_or_else(|| invalid(key, format!("{what} range overflows u64")))?;
    if end > size {
        Err(invalid(key, format!("{what} exceeds containing layout")))
    } else {
        Ok(end)
    }
}
fn overlaps(left: (u64, u64), right: (u64, u64)) -> bool {
    left.0 < right.1 && right.0 < left.1
}
fn ranges_intersect(left: (u128, u128), right: (u128, u128)) -> bool {
    left.0 <= right.1 && right.0 <= left.1
}

fn all_edges(kind: &SemanticTypeKindV2) -> Vec<SemanticTypeNodeIdV2> {
    match kind {
        SemanticTypeKindV2::RawPointer { pointee, .. } => vec![*pointee],
        SemanticTypeKindV2::Reference { referent, .. } => vec![*referent],
        SemanticTypeKindV2::Slice { element } | SemanticTypeKindV2::Array { element, .. } => {
            vec![*element]
        }
        SemanticTypeKindV2::Tuple { fields }
        | SemanticTypeKindV2::Struct { fields, .. }
        | SemanticTypeKindV2::Union { fields, .. } => fields.iter().map(|field| field.ty).collect(),
        SemanticTypeKindV2::Enum { variants, .. } => variants
            .iter()
            .flat_map(|variant| variant.fields.iter().map(|field| field.ty))
            .collect(),
        SemanticTypeKindV2::Unit
        | SemanticTypeKindV2::Never
        | SemanticTypeKindV2::Scalar(_)
        | SemanticTypeKindV2::ValidityScalar { .. }
        | SemanticTypeKindV2::Str
        | SemanticTypeKindV2::OpaqueDst { .. } => Vec::new(),
    }
}

fn edge_count(kind: &SemanticTypeKindV2) -> u64 {
    match kind {
        SemanticTypeKindV2::RawPointer { .. }
        | SemanticTypeKindV2::Reference { .. }
        | SemanticTypeKindV2::Slice { .. }
        | SemanticTypeKindV2::Array { .. } => 1,
        SemanticTypeKindV2::Tuple { fields }
        | SemanticTypeKindV2::Struct { fields, .. }
        | SemanticTypeKindV2::Union { fields, .. } => fields.len() as u64,
        SemanticTypeKindV2::Enum {
            encoding, variants, ..
        } => variants
            .iter()
            .map(|variant| variant.fields.len() as u64)
            .sum::<u64>()
            .saturating_add(match encoding {
                SemanticEnumEncodingV2::Niche { source, .. } => source.path.len() as u64,
                _ => 0,
            }),
        SemanticTypeKindV2::Unit
        | SemanticTypeKindV2::Never
        | SemanticTypeKindV2::Scalar(_)
        | SemanticTypeKindV2::ValidityScalar { .. }
        | SemanticTypeKindV2::Str
        | SemanticTypeKindV2::OpaqueDst { .. } => 0,
    }
}

fn by_value_edges(kind: &SemanticTypeKindV2) -> Vec<SemanticTypeNodeIdV2> {
    if matches!(
        kind,
        SemanticTypeKindV2::RawPointer { .. } | SemanticTypeKindV2::Reference { .. }
    ) {
        Vec::new()
    } else {
        all_edges(kind)
    }
}

struct WriterV2 {
    bytes: Vec<u8>,
    max: u32,
}
impl WriterV2 {
    fn new(max: u32) -> Self {
        Self {
            bytes: Vec::new(),
            max,
        }
    }
    fn reserve(&self, added: usize) -> Result<(), SemanticTypeGraphErrorV2> {
        let actual =
            self.bytes
                .len()
                .checked_add(added)
                .ok_or(SemanticTypeGraphErrorV2::ResourceLimit {
                    resource: "canonical bytes",
                    actual: u64::MAX,
                    max: u64::from(self.max),
                })?;
        enforce("canonical bytes", actual as u64, u64::from(self.max))
    }
    fn bytes(&mut self, value: &[u8]) -> Result<(), SemanticTypeGraphErrorV2> {
        self.reserve(value.len())?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }
    fn u8(&mut self, value: u8) -> Result<(), SemanticTypeGraphErrorV2> {
        self.bytes(&[value])
    }
    fn u16(&mut self, value: u16) -> Result<(), SemanticTypeGraphErrorV2> {
        self.bytes(&value.to_le_bytes())
    }
    fn u32(&mut self, value: u32) -> Result<(), SemanticTypeGraphErrorV2> {
        self.bytes(&value.to_le_bytes())
    }
    fn u64(&mut self, value: u64) -> Result<(), SemanticTypeGraphErrorV2> {
        self.bytes(&value.to_le_bytes())
    }
    fn u128(&mut self, value: u128) -> Result<(), SemanticTypeGraphErrorV2> {
        self.bytes(&value.to_le_bytes())
    }
    fn string(&mut self, value: &str, max: u32) -> Result<(), SemanticTypeGraphErrorV2> {
        validate_name(value, max)?;
        self.u32(value.len() as u32)?;
        self.bytes(value.as_bytes())
    }
    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

struct ReaderV2<'a> {
    input: &'a [u8],
    offset: usize,
}
impl<'a> ReaderV2<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }
    fn error(&self, reason: &'static str) -> SemanticTypeGraphErrorV2 {
        SemanticTypeGraphErrorV2::Decode {
            offset: self.offset,
            reason,
        }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], SemanticTypeGraphErrorV2> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or_else(|| self.error("byte offset overflow"))?;
        let value = self
            .input
            .get(self.offset..end)
            .ok_or_else(|| self.error("truncated input"))?;
        self.offset = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, SemanticTypeGraphErrorV2> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, SemanticTypeGraphErrorV2> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, SemanticTypeGraphErrorV2> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, SemanticTypeGraphErrorV2> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn u128(&mut self) -> Result<u128, SemanticTypeGraphErrorV2> {
        Ok(u128::from_le_bytes(self.take(16)?.try_into().unwrap()))
    }
    fn string(&mut self, max: u32) -> Result<String, SemanticTypeGraphErrorV2> {
        let length = self.u32()?;
        enforce("name bytes", u64::from(length), u64::from(max))?;
        let bytes = self.take(length as usize)?;
        let value = std::str::from_utf8(bytes)
            .map_err(|_| self.error("name is not UTF-8"))?
            .to_owned();
        if value.is_empty() {
            return Err(self.error("name is empty"));
        }
        Ok(value)
    }
    fn is_empty(&self) -> bool {
        self.offset == self.input.len()
    }

    fn ensure_count_fits(
        &self,
        count: u32,
        minimum_item_bytes: usize,
        reason: &'static str,
    ) -> Result<(), SemanticTypeGraphErrorV2> {
        if count as usize > (self.input.len() - self.offset) / minimum_item_bytes {
            Err(self.error(reason))
        } else {
            Ok(())
        }
    }
}

fn encode_layout(
    writer: &mut WriterV2,
    layout: SemanticTypeLayoutV2,
) -> Result<(), SemanticTypeGraphErrorV2> {
    match layout.size {
        Some(size) => {
            writer.u8(1)?;
            writer.u64(size)?;
        }
        None => writer.u8(0)?,
    }
    writer.u64(layout.align)
}
fn decode_layout(
    reader: &mut ReaderV2<'_>,
) -> Result<SemanticTypeLayoutV2, SemanticTypeGraphErrorV2> {
    let size = match reader.u8()? {
        0 => None,
        1 => Some(reader.u64()?),
        _ => return Err(reader.error("invalid layout size tag")),
    };
    Ok(SemanticTypeLayoutV2 {
        size,
        align: reader.u64()?,
    })
}

fn encode_scalar(
    writer: &mut WriterV2,
    scalar: SemanticScalarV2,
) -> Result<(), SemanticTypeGraphErrorV2> {
    match scalar {
        SemanticScalarV2::Bool => writer.u8(0),
        SemanticScalarV2::Char => writer.u8(1),
        SemanticScalarV2::Int { signed, bits } => {
            writer.u8(2)?;
            writer.u8(u8::from(signed))?;
            writer.u16(bits)
        }
        SemanticScalarV2::Float { bits } => {
            writer.u8(3)?;
            writer.u16(bits)
        }
    }
}
fn decode_scalar(reader: &mut ReaderV2<'_>) -> Result<SemanticScalarV2, SemanticTypeGraphErrorV2> {
    match reader.u8()? {
        0 => Ok(SemanticScalarV2::Bool),
        1 => Ok(SemanticScalarV2::Char),
        2 => {
            let signed = match reader.u8()? {
                0 => false,
                1 => true,
                _ => return Err(reader.error("invalid signedness tag")),
            };
            Ok(SemanticScalarV2::Int {
                signed,
                bits: reader.u16()?,
            })
        }
        3 => Ok(SemanticScalarV2::Float {
            bits: reader.u16()?,
        }),
        _ => Err(reader.error("invalid scalar tag")),
    }
}
fn encode_mutability(
    writer: &mut WriterV2,
    value: SemanticMutabilityV2,
) -> Result<(), SemanticTypeGraphErrorV2> {
    writer.u8(match value {
        SemanticMutabilityV2::Immutable => 0,
        SemanticMutabilityV2::Mutable => 1,
    })
}
fn decode_mutability(
    reader: &mut ReaderV2<'_>,
) -> Result<SemanticMutabilityV2, SemanticTypeGraphErrorV2> {
    match reader.u8()? {
        0 => Ok(SemanticMutabilityV2::Immutable),
        1 => Ok(SemanticMutabilityV2::Mutable),
        _ => Err(reader.error("invalid mutability tag")),
    }
}
fn encode_metadata(
    writer: &mut WriterV2,
    value: &PointerMetadataV2,
    max_name: u32,
) -> Result<(), SemanticTypeGraphErrorV2> {
    match value {
        PointerMetadataV2::None => writer.u8(0),
        PointerMetadataV2::SliceLength => writer.u8(1),
        PointerMetadataV2::VTable { trait_identity } => {
            writer.u8(2)?;
            writer.string(trait_identity, max_name)
        }
        PointerMetadataV2::Scalar(scalar) => {
            writer.u8(3)?;
            encode_scalar(writer, *scalar)
        }
    }
}
fn decode_metadata(
    reader: &mut ReaderV2<'_>,
    max_name: u32,
) -> Result<PointerMetadataV2, SemanticTypeGraphErrorV2> {
    match reader.u8()? {
        0 => Ok(PointerMetadataV2::None),
        1 => Ok(PointerMetadataV2::SliceLength),
        2 => Ok(PointerMetadataV2::VTable {
            trait_identity: reader.string(max_name)?,
        }),
        3 => Ok(PointerMetadataV2::Scalar(decode_scalar(reader)?)),
        _ => Err(reader.error("invalid pointer metadata tag")),
    }
}
fn encode_id(
    writer: &mut WriterV2,
    id: SemanticTypeNodeIdV2,
    remap: &[u32],
) -> Result<(), SemanticTypeGraphErrorV2> {
    let mapped = remap
        .get(id.0 as usize)
        .ok_or(SemanticTypeGraphErrorV2::UnknownNode { id: id.0 })?;
    writer.u32(*mapped)
}
fn encode_fields(
    writer: &mut WriterV2,
    fields: &[SemanticFieldV2],
    remap: &[u32],
    max_name: u32,
) -> Result<(), SemanticTypeGraphErrorV2> {
    writer.u32(fields.len() as u32)?;
    for field in fields {
        match &field.name {
            Some(name) => {
                writer.u8(1)?;
                writer.string(name, max_name)?;
            }
            None => writer.u8(0)?,
        }
        writer.u64(field.offset)?;
        encode_id(writer, field.ty, remap)?;
    }
    Ok(())
}
#[derive(Default)]
struct DecodeTotalsV2 {
    edges: u64,
    fields: u64,
    variants: u64,
    ranges: u64,
}

fn charge_decode_total(
    total: &mut u64,
    count: u32,
    resource: &'static str,
    max: u32,
) -> Result<(), SemanticTypeGraphErrorV2> {
    *total =
        total
            .checked_add(u64::from(count))
            .ok_or(SemanticTypeGraphErrorV2::ResourceLimit {
                resource,
                actual: u64::MAX,
                max: u64::from(max),
            })?;
    enforce(resource, *total, u64::from(max))
}

fn decode_fields(
    reader: &mut ReaderV2<'_>,
    budgets: SemanticTypeGraphBudgetsV2,
    totals: &mut DecodeTotalsV2,
) -> Result<Vec<SemanticFieldV2>, SemanticTypeGraphErrorV2> {
    let count = reader.u32()?;
    charge_decode_total(&mut totals.fields, count, "fields", budgets.max_fields)?;
    charge_decode_total(&mut totals.edges, count, "edges", budgets.max_edges)?;
    reader.ensure_count_fits(count, 13, "field count exceeds remaining input")?;
    let mut fields = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let name = match reader.u8()? {
            0 => None,
            1 => Some(reader.string(budgets.max_name_bytes)?),
            _ => return Err(reader.error("invalid optional field-name tag")),
        };
        fields.push(SemanticFieldV2 {
            name,
            offset: reader.u64()?,
            ty: SemanticTypeNodeIdV2(reader.u32()?),
        });
    }
    Ok(fields)
}

fn encode_validity_ranges(
    writer: &mut WriterV2,
    valid_ranges: &[ScalarValidityRangeV2],
) -> Result<(), SemanticTypeGraphErrorV2> {
    writer.u32(valid_ranges.len() as u32)?;
    for range in valid_ranges {
        writer.u128(range.start)?;
        writer.u128(range.end)?;
    }
    Ok(())
}

fn decode_validity_ranges(
    reader: &mut ReaderV2<'_>,
    budgets: SemanticTypeGraphBudgetsV2,
    totals: &mut DecodeTotalsV2,
) -> Result<Vec<ScalarValidityRangeV2>, SemanticTypeGraphErrorV2> {
    let count = reader.u32()?;
    charge_decode_total(
        &mut totals.ranges,
        count,
        "validity ranges",
        budgets.max_validity_ranges,
    )?;
    reader.ensure_count_fits(count, 32, "validity range count exceeds remaining input")?;
    let mut valid_ranges = Vec::with_capacity(count as usize);
    for _ in 0..count {
        valid_ranges.push(ScalarValidityRangeV2 {
            start: reader.u128()?,
            end: reader.u128()?,
        });
    }
    Ok(valid_ranges)
}

fn encode_node(
    writer: &mut WriterV2,
    node: &SemanticTypeNodeV2,
    remap: &[u32],
    max_name: u32,
) -> Result<(), SemanticTypeGraphErrorV2> {
    encode_layout(writer, node.layout)?;
    match &node.kind {
        SemanticTypeKindV2::Unit => writer.u8(0)?,
        SemanticTypeKindV2::Never => writer.u8(1)?,
        SemanticTypeKindV2::Scalar(scalar) => {
            writer.u8(2)?;
            encode_scalar(writer, *scalar)?;
        }
        SemanticTypeKindV2::ValidityScalar {
            scalar,
            valid_ranges,
        } => {
            writer.u8(13)?;
            encode_scalar(writer, *scalar)?;
            encode_validity_ranges(writer, valid_ranges)?;
        }
        SemanticTypeKindV2::RawPointer {
            pointee,
            mutability,
            address_space,
            data_pointer_bytes,
            metadata,
        }
        | SemanticTypeKindV2::Reference {
            referent: pointee,
            mutability,
            address_space,
            data_pointer_bytes,
            metadata,
        } => {
            writer.u8(
                if matches!(node.kind, SemanticTypeKindV2::RawPointer { .. }) {
                    3
                } else {
                    4
                },
            )?;
            encode_id(writer, *pointee, remap)?;
            encode_mutability(writer, *mutability)?;
            writer.u32(*address_space)?;
            writer.u8(*data_pointer_bytes)?;
            encode_metadata(writer, metadata, max_name)?;
        }
        SemanticTypeKindV2::Slice { element } => {
            writer.u8(5)?;
            encode_id(writer, *element, remap)?;
        }
        SemanticTypeKindV2::Str => writer.u8(6)?,
        SemanticTypeKindV2::Tuple { fields } => {
            writer.u8(7)?;
            encode_fields(writer, fields, remap, max_name)?;
        }
        SemanticTypeKindV2::Array { element, length } => {
            writer.u8(8)?;
            encode_id(writer, *element, remap)?;
            writer.u64(*length)?;
        }
        SemanticTypeKindV2::Struct { identity, fields }
        | SemanticTypeKindV2::Union { identity, fields } => {
            writer.u8(if matches!(node.kind, SemanticTypeKindV2::Struct { .. }) {
                9
            } else {
                10
            })?;
            writer.string(identity, max_name)?;
            encode_fields(writer, fields, remap, max_name)?;
        }
        SemanticTypeKindV2::OpaqueDst { identity, metadata } => {
            writer.u8(11)?;
            writer.string(identity, max_name)?;
            encode_metadata(writer, metadata, max_name)?;
        }
        SemanticTypeKindV2::Enum {
            identity,
            discriminant,
            encoding,
            variants,
        } => {
            writer.u8(12)?;
            writer.string(identity, max_name)?;
            encode_scalar(writer, *discriminant)?;
            encode_enum_encoding(writer, encoding)?;
            writer.u32(variants.len() as u32)?;
            for variant in variants {
                writer.string(&variant.name, max_name)?;
                writer.u128(variant.discriminant)?;
                encode_fields(writer, &variant.fields, remap, max_name)?;
            }
        }
    }
    Ok(())
}

fn decode_node(
    reader: &mut ReaderV2<'_>,
    budgets: SemanticTypeGraphBudgetsV2,
    totals: &mut DecodeTotalsV2,
) -> Result<SemanticTypeNodeV2, SemanticTypeGraphErrorV2> {
    let max_name = budgets.max_name_bytes;
    let layout = decode_layout(reader)?;
    let kind = match reader.u8()? {
        0 => SemanticTypeKindV2::Unit,
        1 => SemanticTypeKindV2::Never,
        2 => SemanticTypeKindV2::Scalar(decode_scalar(reader)?),
        13 => SemanticTypeKindV2::ValidityScalar {
            scalar: decode_scalar(reader)?,
            valid_ranges: decode_validity_ranges(reader, budgets, totals)?,
        },
        tag @ (3 | 4) => {
            charge_decode_total(&mut totals.edges, 1, "edges", budgets.max_edges)?;
            let target = SemanticTypeNodeIdV2(reader.u32()?);
            let mutability = decode_mutability(reader)?;
            let address_space = reader.u32()?;
            let data_pointer_bytes = reader.u8()?;
            let metadata = decode_metadata(reader, max_name)?;
            if tag == 3 {
                SemanticTypeKindV2::RawPointer {
                    pointee: target,
                    mutability,
                    address_space,
                    data_pointer_bytes,
                    metadata,
                }
            } else {
                SemanticTypeKindV2::Reference {
                    referent: target,
                    mutability,
                    address_space,
                    data_pointer_bytes,
                    metadata,
                }
            }
        }
        5 => {
            charge_decode_total(&mut totals.edges, 1, "edges", budgets.max_edges)?;
            SemanticTypeKindV2::Slice {
                element: SemanticTypeNodeIdV2(reader.u32()?),
            }
        }
        6 => SemanticTypeKindV2::Str,
        7 => SemanticTypeKindV2::Tuple {
            fields: decode_fields(reader, budgets, totals)?,
        },
        8 => {
            charge_decode_total(&mut totals.edges, 1, "edges", budgets.max_edges)?;
            SemanticTypeKindV2::Array {
                element: SemanticTypeNodeIdV2(reader.u32()?),
                length: reader.u64()?,
            }
        }
        9 => SemanticTypeKindV2::Struct {
            identity: reader.string(max_name)?,
            fields: decode_fields(reader, budgets, totals)?,
        },
        10 => SemanticTypeKindV2::Union {
            identity: reader.string(max_name)?,
            fields: decode_fields(reader, budgets, totals)?,
        },
        11 => SemanticTypeKindV2::OpaqueDst {
            identity: reader.string(max_name)?,
            metadata: decode_metadata(reader, max_name)?,
        },
        12 => {
            let identity = reader.string(max_name)?;
            let discriminant = decode_scalar(reader)?;
            let encoding = decode_enum_encoding(reader, budgets, totals)?;
            let count = reader.u32()?;
            charge_decode_total(
                &mut totals.variants,
                count,
                "variants",
                budgets.max_variants,
            )?;
            reader.ensure_count_fits(count, 25, "variant count exceeds remaining input")?;
            let mut variants = Vec::with_capacity(count as usize);
            for _ in 0..count {
                variants.push(SemanticVariantV2 {
                    name: reader.string(max_name)?,
                    discriminant: reader.u128()?,
                    fields: decode_fields(reader, budgets, totals)?,
                });
            }
            SemanticTypeKindV2::Enum {
                identity,
                discriminant,
                encoding,
                variants,
            }
        }
        _ => return Err(reader.error("invalid semantic type kind tag")),
    };
    Ok(SemanticTypeNodeV2 { layout, kind })
}

fn encode_enum_encoding(
    writer: &mut WriterV2,
    value: &SemanticEnumEncodingV2,
) -> Result<(), SemanticTypeGraphErrorV2> {
    match value {
        SemanticEnumEncodingV2::Uninhabited => writer.u8(0),
        SemanticEnumEncodingV2::Single { variant } => {
            writer.u8(1)?;
            writer.u32(*variant)
        }
        SemanticEnumEncodingV2::Direct { tag_offset, tag } => {
            writer.u8(2)?;
            writer.u64(*tag_offset)?;
            encode_scalar(writer, *tag)
        }
        SemanticEnumEncodingV2::Niche {
            source,
            niche_scalar,
            valid_ranges,
            untagged_variant,
            niche_variants_start,
            niche_variants_end,
            niche_start,
        } => {
            writer.u8(3)?;
            writer.u32(source.path.len() as u32)?;
            for component in &source.path {
                match component {
                    SemanticNichePathComponentV2::Field(index) => {
                        writer.u8(0)?;
                        writer.u32(*index)?;
                    }
                    SemanticNichePathComponentV2::ArrayElement(index) => {
                        writer.u8(1)?;
                        writer.u64(*index)?;
                    }
                }
            }
            writer.u64(source.expected_offset)?;
            encode_scalar(writer, *niche_scalar)?;
            encode_validity_ranges(writer, valid_ranges)?;
            writer.u32(*untagged_variant)?;
            writer.u32(*niche_variants_start)?;
            writer.u32(*niche_variants_end)?;
            writer.u128(*niche_start)
        }
    }
}
fn decode_enum_encoding(
    reader: &mut ReaderV2<'_>,
    budgets: SemanticTypeGraphBudgetsV2,
    totals: &mut DecodeTotalsV2,
) -> Result<SemanticEnumEncodingV2, SemanticTypeGraphErrorV2> {
    match reader.u8()? {
        0 => Ok(SemanticEnumEncodingV2::Uninhabited),
        1 => Ok(SemanticEnumEncodingV2::Single {
            variant: reader.u32()?,
        }),
        2 => Ok(SemanticEnumEncodingV2::Direct {
            tag_offset: reader.u64()?,
            tag: decode_scalar(reader)?,
        }),
        3 => {
            let path_count = reader.u32()?;
            charge_decode_total(&mut totals.edges, path_count, "edges", budgets.max_edges)?;
            reader.ensure_count_fits(path_count, 5, "niche path count exceeds remaining input")?;
            let mut path = Vec::with_capacity(path_count as usize);
            for _ in 0..path_count {
                path.push(match reader.u8()? {
                    0 => SemanticNichePathComponentV2::Field(reader.u32()?),
                    1 => SemanticNichePathComponentV2::ArrayElement(reader.u64()?),
                    _ => return Err(reader.error("invalid niche path component tag")),
                });
            }
            let expected_offset = reader.u64()?;
            let niche_scalar = decode_scalar(reader)?;
            let valid_ranges = decode_validity_ranges(reader, budgets, totals)?;
            Ok(SemanticEnumEncodingV2::Niche {
                source: SemanticNicheSourceV2 {
                    path,
                    expected_offset,
                },
                niche_scalar,
                valid_ranges,
                untagged_variant: reader.u32()?,
                niche_variants_start: reader.u32()?,
                niche_variants_end: reader.u32()?,
                niche_start: reader.u128()?,
            })
        }
        _ => Err(reader.error("invalid enum encoding tag")),
    }
}
