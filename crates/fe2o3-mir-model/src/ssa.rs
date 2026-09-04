//! Representation-neutral construction plan for mixed memory/SSA IRs.
//!
//! The source adapter owns alias analysis and aggregate decomposition. It must
//! set `promotable` only for independently address-free variables; everything
//! else remains memory. The planner then computes a bounded, deterministic
//! transport plan without assuming reducible control flow or unique CFG edges.

use std::fmt;

use sha2::{Digest as _, Sha256};

mod planner;
mod support;

use planner::Planner;
use support::WorkBudget;

const SSA_PLAN_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.ssa-construction-plan.v1\0";

pub const HARD_MAX_SSA_VARIABLES_V1: usize = 262_144;
pub const HARD_MAX_SSA_BLOCKS_V1: usize = 262_144;
pub const HARD_MAX_SSA_EDGES_V1: usize = 65_536;
pub const HARD_MAX_SSA_EVENTS_V1: usize = 1_048_576;
pub const HARD_MAX_SSA_EDGE_DEFINITIONS_V1: usize = 65_536;
pub const HARD_MAX_SSA_OUTPUT_ITEMS_V1: usize = 1_048_576;
pub const HARD_MAX_SSA_STORAGE_WORDS_V1: usize = 2_097_152;
pub const HARD_MAX_SSA_WORK_UNITS_V1: usize = 67_108_864;

macro_rules! dense_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);

        impl $name {
            pub const fn new(identity: u32) -> Self {
                Self(identity)
            }

            pub const fn get(self) -> u32 {
                self.0
            }
        }
    };
}

dense_id!(SsaVariableIdV1);
dense_id!(SsaBlockIdV1);
dense_id!(SsaDefinitionIdV1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SsaEdgeRoleV1(u16);

impl SsaEdgeRoleV1 {
    pub const fn new(role: u16) -> Self {
        Self(role)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SsaEdgeIdV1 {
    source: SsaBlockIdV1,
    ordinal: u32,
}

impl SsaEdgeIdV1 {
    pub const fn new(source: SsaBlockIdV1, ordinal: u32) -> Self {
        Self { source, ordinal }
    }

    pub const fn source(self) -> SsaBlockIdV1 {
        self.source
    }

    pub const fn ordinal(self) -> u32 {
        self.ordinal
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SsaEventV1 {
    Use(SsaVariableIdV1),
    Define(SsaVariableIdV1),
    Kill(SsaVariableIdV1),
}

impl SsaEventV1 {
    pub const fn variable(self) -> SsaVariableIdV1 {
        match self {
            Self::Use(variable) | Self::Define(variable) | Self::Kill(variable) => variable,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsaEdgeInputV1 {
    role: SsaEdgeRoleV1,
    target: SsaBlockIdV1,
    definitions: Vec<SsaVariableIdV1>,
}

impl SsaEdgeInputV1 {
    pub fn new(
        role: SsaEdgeRoleV1,
        target: SsaBlockIdV1,
        definitions: Vec<SsaVariableIdV1>,
    ) -> Self {
        Self {
            role,
            target,
            definitions,
        }
    }

    pub const fn role(&self) -> SsaEdgeRoleV1 {
        self.role
    }

    pub const fn target(&self) -> SsaBlockIdV1 {
        self.target
    }

    pub fn definitions(&self) -> &[SsaVariableIdV1] {
        &self.definitions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsaBlockInputV1 {
    events: Vec<SsaEventV1>,
    edges: Vec<SsaEdgeInputV1>,
}

impl SsaBlockInputV1 {
    pub fn new(events: Vec<SsaEventV1>, edges: Vec<SsaEdgeInputV1>) -> Self {
        Self { events, edges }
    }

    pub fn events(&self) -> &[SsaEventV1] {
        &self.events
    }

    pub fn edges(&self) -> &[SsaEdgeInputV1] {
        &self.edges
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsaConstructionInputV1 {
    entry: SsaBlockIdV1,
    variable_count: u32,
    promotable: Vec<bool>,
    entry_definitions: Vec<SsaVariableIdV1>,
    blocks: Vec<SsaBlockInputV1>,
}

impl SsaConstructionInputV1 {
    pub fn new(
        entry: SsaBlockIdV1,
        variable_count: u32,
        promotable: Vec<bool>,
        entry_definitions: Vec<SsaVariableIdV1>,
        blocks: Vec<SsaBlockInputV1>,
    ) -> Self {
        Self {
            entry,
            variable_count,
            promotable,
            entry_definitions,
            blocks,
        }
    }

    pub const fn entry(&self) -> SsaBlockIdV1 {
        self.entry
    }

    pub const fn variable_count(&self) -> u32 {
        self.variable_count
    }

    pub fn promotable(&self) -> &[bool] {
        &self.promotable
    }

    pub fn entry_definitions(&self) -> &[SsaVariableIdV1] {
        &self.entry_definitions
    }

    pub fn blocks(&self) -> &[SsaBlockInputV1] {
        &self.blocks
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SsaPlannerLimitsV1 {
    max_variables: usize,
    max_blocks: usize,
    max_edges: usize,
    max_events: usize,
    max_edge_definitions: usize,
    max_output_items: usize,
    max_storage_words: usize,
    max_work_units: usize,
}

impl SsaPlannerLimitsV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        max_variables: usize,
        max_blocks: usize,
        max_edges: usize,
        max_events: usize,
        max_edge_definitions: usize,
        max_output_items: usize,
        max_storage_words: usize,
        max_work_units: usize,
    ) -> Result<Self, SsaPlannerErrorV1> {
        let limits = Self {
            max_variables,
            max_blocks,
            max_edges,
            max_events,
            max_edge_definitions,
            max_output_items,
            max_storage_words,
            max_work_units,
        };
        limits.validate()?;
        Ok(limits)
    }

    pub const fn max_variables(self) -> usize {
        self.max_variables
    }

    pub const fn max_blocks(self) -> usize {
        self.max_blocks
    }

    pub const fn max_edges(self) -> usize {
        self.max_edges
    }

    pub const fn max_events(self) -> usize {
        self.max_events
    }

    pub const fn max_edge_definitions(self) -> usize {
        self.max_edge_definitions
    }

    pub const fn max_output_items(self) -> usize {
        self.max_output_items
    }

    pub const fn max_storage_words(self) -> usize {
        self.max_storage_words
    }

    pub const fn max_work_units(self) -> usize {
        self.max_work_units
    }

    fn validate(self) -> Result<(), SsaPlannerErrorV1> {
        let invalid = self.max_variables > HARD_MAX_SSA_VARIABLES_V1
            || self.max_blocks == 0
            || self.max_blocks > HARD_MAX_SSA_BLOCKS_V1
            || self.max_edges > HARD_MAX_SSA_EDGES_V1
            || self.max_events > HARD_MAX_SSA_EVENTS_V1
            || self.max_edge_definitions > HARD_MAX_SSA_EDGE_DEFINITIONS_V1
            || self.max_output_items > HARD_MAX_SSA_OUTPUT_ITEMS_V1
            || self.max_storage_words == 0
            || self.max_storage_words > HARD_MAX_SSA_STORAGE_WORDS_V1
            || self.max_work_units == 0
            || self.max_work_units > HARD_MAX_SSA_WORK_UNITS_V1;
        if invalid {
            Err(SsaPlannerErrorV1::InvalidLimits)
        } else {
            Ok(())
        }
    }
}

impl Default for SsaPlannerLimitsV1 {
    fn default() -> Self {
        Self {
            max_variables: HARD_MAX_SSA_VARIABLES_V1,
            max_blocks: HARD_MAX_SSA_BLOCKS_V1,
            max_edges: HARD_MAX_SSA_EDGES_V1,
            max_events: HARD_MAX_SSA_EVENTS_V1,
            max_edge_definitions: HARD_MAX_SSA_EDGE_DEFINITIONS_V1,
            max_output_items: HARD_MAX_SSA_OUTPUT_ITEMS_V1,
            max_storage_words: HARD_MAX_SSA_STORAGE_WORDS_V1,
            max_work_units: HARD_MAX_SSA_WORK_UNITS_V1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SsaPlannerResourceV1 {
    Variables,
    Blocks,
    Edges,
    Events,
    EdgeDefinitions,
    OutputItems,
    StorageWords,
    WorkUnits,
}

impl fmt::Display for SsaPlannerResourceV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Variables => "variables",
            Self::Blocks => "blocks",
            Self::Edges => "edges",
            Self::Events => "events",
            Self::EdgeDefinitions => "edge definitions",
            Self::OutputItems => "output items",
            Self::StorageWords => "storage words",
            Self::WorkUnits => "work units",
        };
        formatter.write_str(name)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SsaInputSiteV1 {
    EntryDefinition(u32),
    Event { block: SsaBlockIdV1, event: u32 },
    EdgeDefinition { edge: SsaEdgeIdV1, definition: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SsaPlannerErrorV1 {
    InvalidLimits,
    EmptyControlFlow,
    InvalidEntry {
        entry: SsaBlockIdV1,
        block_count: usize,
    },
    PromotableLengthMismatch {
        variable_count: usize,
        bitmap_len: usize,
    },
    ResourceLimitExceeded {
        resource: SsaPlannerResourceV1,
        required: usize,
        limit: usize,
    },
    InvalidEdgeRole {
        edge: SsaEdgeIdV1,
    },
    UnknownTarget {
        edge: SsaEdgeIdV1,
        target: SsaBlockIdV1,
        block_count: usize,
    },
    UnknownVariable {
        site: SsaInputSiteV1,
        variable: SsaVariableIdV1,
        variable_count: usize,
    },
    NonCanonicalDefinitions {
        edge: Option<SsaEdgeIdV1>,
    },
    UndefinedAtUse {
        block: SsaBlockIdV1,
        event: u32,
        variable: SsaVariableIdV1,
    },
    UndefinedAtEdge {
        edge: SsaEdgeIdV1,
        target: SsaBlockIdV1,
        variable: SsaVariableIdV1,
    },
    UndefinedAtEntry {
        variable: SsaVariableIdV1,
    },
    IdentityOverflow,
    ReplayMismatch {
        expected: SsaPlanIdentityV1,
        actual: SsaPlanIdentityV1,
    },
}

impl fmt::Display for SsaPlannerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => formatter.write_str("SSA planner limits are invalid"),
            Self::EmptyControlFlow => formatter.write_str("SSA input has no control-flow blocks"),
            Self::InvalidEntry { entry, block_count } => write!(
                formatter,
                "SSA entry block {} is outside 0..{block_count}",
                entry.get()
            ),
            Self::PromotableLengthMismatch {
                variable_count,
                bitmap_len,
            } => write!(
                formatter,
                "SSA promotable bitmap has {bitmap_len} entries for {variable_count} variables"
            ),
            Self::ResourceLimitExceeded {
                resource,
                required,
                limit,
            } => write!(
                formatter,
                "SSA planner requires {required} {resource}, exceeding the limit {limit}"
            ),
            Self::InvalidEdgeRole { edge } => write!(
                formatter,
                "SSA edge {}:{} uses reserved role zero",
                edge.source.get(),
                edge.ordinal
            ),
            Self::UnknownTarget {
                edge,
                target,
                block_count,
            } => write!(
                formatter,
                "SSA edge {}:{} targets block {}, outside 0..{block_count}",
                edge.source.get(),
                edge.ordinal,
                target.get()
            ),
            Self::UnknownVariable {
                site,
                variable,
                variable_count,
            } => write!(
                formatter,
                "SSA input site {site:?} names variable {}, outside 0..{variable_count}",
                variable.get()
            ),
            Self::NonCanonicalDefinitions { edge: None } => {
                formatter.write_str("SSA entry definitions must be strictly ordered")
            }
            Self::NonCanonicalDefinitions { edge: Some(edge) } => write!(
                formatter,
                "SSA edge {}:{} definitions must be strictly ordered",
                edge.source.get(),
                edge.ordinal
            ),
            Self::UndefinedAtUse {
                block,
                event,
                variable,
            } => write!(
                formatter,
                "SSA variable {} is undefined at block {} event {event}",
                variable.get(),
                block.get()
            ),
            Self::UndefinedAtEdge {
                edge,
                target,
                variable,
            } => write!(
                formatter,
                "SSA variable {} is undefined on edge {}:{} to block {}",
                variable.get(),
                edge.source.get(),
                edge.ordinal,
                target.get()
            ),
            Self::UndefinedAtEntry { variable } => write!(
                formatter,
                "SSA variable {} is live at a cyclic entry without an entry definition",
                variable.get()
            ),
            Self::IdentityOverflow => formatter.write_str("SSA planner identity space overflowed"),
            Self::ReplayMismatch { expected, actual } => write!(
                formatter,
                "SSA plan replay identity mismatch: expected {expected}, found {actual}"
            ),
        }
    }
}

impl std::error::Error for SsaPlannerErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SsaPlanIdentityV1([u8; 32]);

impl SsaPlanIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for SsaPlanIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SsaValueV1 {
    Definition(SsaDefinitionIdV1),
    BlockArgument {
        block: SsaBlockIdV1,
        variable: SsaVariableIdV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SsaArgumentV1 {
    variable: SsaVariableIdV1,
    value: SsaValueV1,
}

impl SsaArgumentV1 {
    /// Constructs one exact SSA edge definition or transported argument.
    pub const fn new(variable: SsaVariableIdV1, value: SsaValueV1) -> Self {
        Self { variable, value }
    }

    pub const fn variable(self) -> SsaVariableIdV1 {
        self.variable
    }

    pub const fn value(self) -> SsaValueV1 {
        self.value
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SsaResolvedEventV1 {
    Use {
        variable: SsaVariableIdV1,
        value: SsaValueV1,
    },
    Define {
        variable: SsaVariableIdV1,
        value: SsaValueV1,
    },
    Kill {
        variable: SsaVariableIdV1,
        previous: Option<SsaValueV1>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsaPlannerResourceReportV1 {
    input_blocks: usize,
    reachable_blocks: usize,
    pruned_blocks: usize,
    input_edges: usize,
    input_events: usize,
    input_edge_definitions: usize,
    generated_definitions: usize,
    output_items: usize,
    storage_words: usize,
    work_units: usize,
}

impl SsaPlannerResourceReportV1 {
    pub const fn input_blocks(&self) -> usize {
        self.input_blocks
    }

    pub const fn reachable_blocks(&self) -> usize {
        self.reachable_blocks
    }

    pub const fn pruned_blocks(&self) -> usize {
        self.pruned_blocks
    }

    pub const fn input_edges(&self) -> usize {
        self.input_edges
    }

    pub const fn input_events(&self) -> usize {
        self.input_events
    }

    pub const fn input_edge_definitions(&self) -> usize {
        self.input_edge_definitions
    }

    pub const fn generated_definitions(&self) -> usize {
        self.generated_definitions
    }

    pub const fn output_items(&self) -> usize {
        self.output_items
    }

    pub const fn storage_words(&self) -> usize {
        self.storage_words
    }

    pub const fn work_units(&self) -> usize {
        self.work_units
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SsaConstructionPlanV1 {
    identity: SsaPlanIdentityV1,
    resources: SsaPlannerResourceReportV1,
    reachable: Vec<bool>,
    reverse_postorder: Vec<SsaBlockIdV1>,
    promoted_variables: Vec<SsaVariableIdV1>,
    live_in: Vec<Vec<SsaVariableIdV1>>,
    merge_variables: Vec<Vec<SsaVariableIdV1>>,
    transport_variables: Vec<Vec<SsaVariableIdV1>>,
    entry_definitions: Vec<SsaArgumentV1>,
    entry_arguments: Vec<SsaArgumentV1>,
    resolved_events: Vec<Vec<(u32, SsaResolvedEventV1)>>,
    edge_definitions: Vec<Vec<Vec<SsaArgumentV1>>>,
    edge_arguments: Vec<Vec<Vec<SsaArgumentV1>>>,
}

impl SsaConstructionPlanV1 {
    pub const fn identity(&self) -> SsaPlanIdentityV1 {
        self.identity
    }

    pub const fn resources(&self) -> &SsaPlannerResourceReportV1 {
        &self.resources
    }

    pub fn is_reachable(&self, block: SsaBlockIdV1) -> bool {
        self.reachable
            .get(block.get() as usize)
            .copied()
            .unwrap_or(false)
    }

    pub fn reverse_postorder(&self) -> &[SsaBlockIdV1] {
        &self.reverse_postorder
    }

    pub fn promoted_variables(&self) -> &[SsaVariableIdV1] {
        &self.promoted_variables
    }

    pub fn live_in(&self, block: SsaBlockIdV1) -> Option<&[SsaVariableIdV1]> {
        self.reachable_slice(block, &self.live_in)
    }

    pub fn merge_variables(&self, block: SsaBlockIdV1) -> Option<&[SsaVariableIdV1]> {
        self.reachable_slice(block, &self.merge_variables)
    }

    pub fn transport_variables(&self, block: SsaBlockIdV1) -> Option<&[SsaVariableIdV1]> {
        self.reachable_slice(block, &self.transport_variables)
    }

    pub fn entry_arguments(&self) -> &[SsaArgumentV1] {
        &self.entry_arguments
    }

    /// Exact SSA identities assigned to promotable external-entry definitions.
    pub fn entry_definitions(&self) -> &[SsaArgumentV1] {
        &self.entry_definitions
    }

    /// Resolved promoted events in source-event order.
    pub fn resolved_events(&self, block: SsaBlockIdV1) -> Option<&[(u32, SsaResolvedEventV1)]> {
        self.reachable_slice(block, &self.resolved_events)
    }

    pub fn resolved_event(&self, block: SsaBlockIdV1, event: u32) -> Option<&SsaResolvedEventV1> {
        let events = self.resolved_events.get(block.get() as usize)?;
        let index = events
            .binary_search_by_key(&event, |(event, _)| *event)
            .ok()?;
        Some(&events[index].1)
    }

    pub fn edge_arguments(&self, edge: SsaEdgeIdV1) -> Option<&[SsaArgumentV1]> {
        if !self.is_reachable(edge.source) {
            return None;
        }
        self.edge_arguments
            .get(edge.source.get() as usize)?
            .get(edge.ordinal as usize)
            .map(Vec::as_slice)
    }

    /// Exact SSA identities assigned by one conceptual CFG edge.
    pub fn edge_definitions(&self, edge: SsaEdgeIdV1) -> Option<&[SsaArgumentV1]> {
        if !self.is_reachable(edge.source) {
            return None;
        }
        self.edge_definitions
            .get(edge.source.get() as usize)?
            .get(edge.ordinal as usize)
            .map(Vec::as_slice)
    }

    pub const fn definition_count(&self) -> usize {
        self.resources.generated_definitions
    }

    pub fn verify_replay(
        &self,
        input: &SsaConstructionInputV1,
        limits: SsaPlannerLimitsV1,
    ) -> Result<(), SsaPlannerErrorV1> {
        let replay = plan_ssa_with_limits_v1(input, limits)?;
        if replay == *self {
            Ok(())
        } else {
            Err(SsaPlannerErrorV1::ReplayMismatch {
                expected: self.identity,
                actual: replay.identity,
            })
        }
    }

    fn reachable_slice<'a, T>(&self, block: SsaBlockIdV1, values: &'a [Vec<T>]) -> Option<&'a [T]> {
        self.is_reachable(block)
            .then(|| values.get(block.get() as usize).map(Vec::as_slice))
            .flatten()
    }
}

pub fn plan_ssa_v1(
    input: &SsaConstructionInputV1,
) -> Result<SsaConstructionPlanV1, SsaPlannerErrorV1> {
    plan_ssa_with_limits_v1(input, SsaPlannerLimitsV1::default())
}

pub fn plan_ssa_with_limits_v1(
    input: &SsaConstructionInputV1,
    limits: SsaPlannerLimitsV1,
) -> Result<SsaConstructionPlanV1, SsaPlannerErrorV1> {
    limits.validate()?;
    Planner::new(input, limits)?.build()
}

#[allow(clippy::too_many_arguments)]
fn compute_identity(
    input: &SsaConstructionInputV1,
    reachable: &[bool],
    live_in: &[Vec<SsaVariableIdV1>],
    merges: &[Vec<SsaVariableIdV1>],
    transport: &[Vec<SsaVariableIdV1>],
    entry_definitions: &[SsaArgumentV1],
    entry_arguments: &[SsaArgumentV1],
    events: &[Vec<(u32, SsaResolvedEventV1)>],
    edge_definitions: &[Vec<Vec<SsaArgumentV1>>],
    edge_arguments: &[Vec<Vec<SsaArgumentV1>>],
    work: &mut WorkBudget,
) -> Result<SsaPlanIdentityV1, SsaPlannerErrorV1> {
    work.charge(3)?;
    work.charge(input.promotable.len())?;
    work.charge(input.entry_definitions.len())?;
    work.charge(reachable.len())?;
    let mut digest = Sha256::new();
    digest.update(SSA_PLAN_IDENTITY_DOMAIN_V1);
    hash_u32(&mut digest, input.entry.get());
    hash_u32(&mut digest, input.variable_count);
    hash_usize(&mut digest, input.promotable.len());
    for promotable in &input.promotable {
        digest.update([u8::from(*promotable)]);
    }
    hash_variables(&mut digest, &input.entry_definitions);
    hash_usize(
        &mut digest,
        reachable.iter().filter(|reachable| **reachable).count(),
    );
    for (block_index, block) in input.blocks.iter().enumerate() {
        if !reachable[block_index] {
            continue;
        }
        work.charge(1 + block.events.len() + block.edges.len())?;
        hash_u32(&mut digest, block_index as u32);
        hash_usize(&mut digest, block.events.len());
        for event in &block.events {
            match event {
                SsaEventV1::Use(_) => digest.update([1]),
                SsaEventV1::Define(_) => digest.update([2]),
                SsaEventV1::Kill(_) => digest.update([3]),
            }
            hash_u32(&mut digest, event.variable().get());
        }
        hash_usize(&mut digest, block.edges.len());
        for edge in &block.edges {
            work.charge(edge.definitions.len())?;
            hash_u16(&mut digest, edge.role.get());
            hash_u32(&mut digest, edge.target.get());
            hash_variables(&mut digest, &edge.definitions);
        }
        hash_variables(&mut digest, &live_in[block_index]);
        hash_variables(&mut digest, &merges[block_index]);
        hash_variables(&mut digest, &transport[block_index]);
        work.charge(
            live_in[block_index].len() + merges[block_index].len() + transport[block_index].len(),
        )?;
        // Preserve the v1 identity encoding while retaining only promoted
        // events in memory.
        hash_usize(&mut digest, block.events.len());
        work.charge(block.events.len())?;
        let mut resolved = events[block_index].iter().peekable();
        for event_index in 0..block.events.len() {
            let event = resolved
                .next_if(|(resolved_index, _)| *resolved_index as usize == event_index)
                .map(|(_, event)| event);
            match event {
                None => digest.update([0]),
                Some(SsaResolvedEventV1::Use { variable, value }) => {
                    digest.update([1]);
                    hash_u32(&mut digest, variable.get());
                    hash_value(&mut digest, *value);
                }
                Some(SsaResolvedEventV1::Define { variable, value }) => {
                    digest.update([2]);
                    hash_u32(&mut digest, variable.get());
                    hash_value(&mut digest, *value);
                }
                Some(SsaResolvedEventV1::Kill { variable, previous }) => {
                    digest.update([3]);
                    hash_u32(&mut digest, variable.get());
                    match previous {
                        Some(value) => {
                            digest.update([1]);
                            hash_value(&mut digest, *value);
                        }
                        None => digest.update([0]),
                    }
                }
            }
        }
        hash_usize(&mut digest, edge_arguments[block_index].len());
        for (definitions, arguments) in edge_definitions[block_index]
            .iter()
            .zip(&edge_arguments[block_index])
        {
            work.charge(definitions.len() + arguments.len())?;
            hash_arguments(&mut digest, definitions);
            hash_arguments(&mut digest, arguments);
        }
    }
    work.charge(entry_definitions.len() + entry_arguments.len())?;
    hash_arguments(&mut digest, entry_definitions);
    hash_arguments(&mut digest, entry_arguments);
    Ok(SsaPlanIdentityV1(digest.finalize().into()))
}

fn hash_variables(digest: &mut Sha256, values: &[SsaVariableIdV1]) {
    hash_usize(digest, values.len());
    for value in values {
        hash_u32(digest, value.get());
    }
}

fn hash_arguments(digest: &mut Sha256, arguments: &[SsaArgumentV1]) {
    hash_usize(digest, arguments.len());
    for argument in arguments {
        hash_u32(digest, argument.variable.get());
        hash_value(digest, argument.value);
    }
}

fn hash_value(digest: &mut Sha256, value: SsaValueV1) {
    match value {
        SsaValueV1::Definition(definition) => {
            digest.update([1]);
            hash_u32(digest, definition.get());
        }
        SsaValueV1::BlockArgument { block, variable } => {
            digest.update([2]);
            hash_u32(digest, block.get());
            hash_u32(digest, variable.get());
        }
    }
}

fn hash_u16(digest: &mut Sha256, value: u16) {
    digest.update(value.to_le_bytes());
}

fn hash_u32(digest: &mut Sha256, value: u32) {
    digest.update(value.to_le_bytes());
}

fn hash_usize(digest: &mut Sha256, value: usize) {
    digest.update((value as u64).to_le_bytes());
}
