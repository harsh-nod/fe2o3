//! Inert portable CPU reference records and the shared live/replay engine.
//!
//! No input or result authenticates source, a provider, proof, or native custody.
//! Callers retain ownership of admitted inputs and supply their original budget.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

mod cfg;
pub mod codec;
mod data;
mod hash;
mod ir;
mod replay;
mod resolver;
pub mod signature;
mod work;

use cfg::*;
pub use data::*;
use hash::*;
pub use replay::{
    ReferenceReplayInputV1, ReplayedCpuEffectsV1, ReplayedCpuValueV1,
    with_replayed_output_writes_v1,
};
use resolver::*;
use work::{InspectionWorkV1, ReferenceWorkV1};

/// Low-level shared extraction machinery, not an admission or authentication API.
#[doc(hidden)]
pub mod extraction {
    pub use super::cfg::{reference_predicate_and_atom_v1, reference_successors_v1};
    pub use super::hash::digest_effect_expression_v1;
    pub use super::resolver::{ReferenceExpressionResolverV1, substitute_helper_summary_v2};
    pub use super::work::{ReferenceWorkV1, add};
}

#[cfg(test)]
mod tests;
