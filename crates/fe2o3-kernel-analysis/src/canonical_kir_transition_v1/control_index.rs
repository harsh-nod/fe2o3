//! Queryable control and occurrence facts from the fixed checked transition.

use super::{
    Budget, CheckedCanonicalKirTransitionV1, Error, INTERNAL, Inventory, Literal, NONE, Resource,
    Result, State, allocate, index,
};
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgument, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirUseCoordinateV1 as Use, ScalarType, Terminator,
};
use std::mem::size_of;

/// Actual output block and the input block's position within its checked chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirBlockPlacementV1 {
    pub output: Block,
    pub segment: u32,
}

/// Omission is distinct from a consumed executable merge connector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirEdgePlacementV1 {
    Retained(Edge),
    InternalConnector(CanonicalKirBlockPlacementV1),
    Omitted,
}

/// Checked input-control facts. Unreachable blocks may still have output placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirBlockControlV1 {
    pub reachable: bool,
    pub placement: Option<CanonicalKirBlockPlacementV1>,
    /// Independently derived Boolean selection, not inferred from missing rows.
    pub selected_successor: Option<Edge>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirEdgeControlV1 {
    pub placement: CanonicalKirEdgePlacementV1,
    /// Both source reachability and the fixed scalar/CFG selection rules hold.
    pub executable: bool,
}

/// Actual definition consumed by one actual output operand occurrence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirOutputUseV1 {
    pub coordinate: Use,
    pub definition: Definition,
}

/// Owner-bound index, not semantic equivalence or assertion-discharge authority.
/// No constructor accepts unchecked candidate rows or a claimed graph hash.
#[derive(Debug)]
pub struct CheckedCanonicalKirControlIndexV1<'a, 'input, 'output> {
    input: &'a Inventory<'input>,
    output: &'a Inventory<'output>,
    blocks: Vec<CanonicalKirBlockControlV1>,
    edges: Vec<CanonicalKirEdgeControlV1>,
    uses: Vec<Option<CanonicalKirOutputUseV1>>,
    arguments: Vec<Option<EdgeArgument>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirControlIndexStorageV1(usize);
impl CanonicalKirControlIndexStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

impl<'a, 'input, 'output> CheckedCanonicalKirControlIndexV1<'a, 'input, 'output> {
    /// Reuses the checker's private structure, monotone scalar and control rules.
    /// The checker and its existing receipt/wire contracts are unchanged. This
    /// opt-in query index pays the same bounded solver cost plus linear output
    /// copying; all solver scratch is dropped before returning the index.
    ///
    /// Input/output owners, inventories and the checked row owner are borrowed
    /// and caller-reserved. Every returned exit restores the incoming floor;
    /// success transfers the separate index receipt. No executable is rebuilt.
    pub fn derive(
        checked: &CheckedCanonicalKirTransitionV1<'a, 'input, 'output, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirControlIndexStorageV1)> {
        let floor = budget.storage();
        let result = Self::build(checked, budget);
        let retained = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?;
        budget.release_storage(retained)?;
        result.map(|index| (index, CanonicalKirControlIndexStorageV1(retained)))
    }

    fn build(
        checked: &CheckedCanonicalKirTransitionV1<'a, 'input, 'output, '_>,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(1)?;
        budget.reserve_storage(size_of::<Self>())?;
        let input = checked.input();
        let output = checked.output();
        let mut result = Self {
            input,
            output,
            blocks: allocate(
                input.blocks().len(),
                CanonicalKirBlockControlV1 {
                    reachable: false,
                    placement: None,
                    selected_successor: None,
                },
                budget,
            )?,
            edges: allocate(
                input.edges().len(),
                CanonicalKirEdgeControlV1 {
                    placement: CanonicalKirEdgePlacementV1::Omitted,
                    executable: false,
                },
                budget,
            )?,
            uses: allocate(input.uses().len(), None, budget)?,
            arguments: allocate(input.edge_arguments().len(), None, budget)?,
        };
        let state_floor = budget.storage();
        let mut state = State::new(input, output, checked.rows(), budget)?;
        state.check_structure(budget)?;
        state.solve_values(budget)?;
        state.check_control(budget)?;
        let state_storage = budget
            .storage()
            .checked_sub(state_floor)
            .ok_or(Resource::Accounting)?;
        for (ordinal, block) in input.blocks().iter().enumerate() {
            budget.charge_work(1)?;
            let placement = if state.block_output[ordinal] == NONE {
                None
            } else {
                Some(CanonicalKirBlockPlacementV1 {
                    output: output.blocks()[state.block_output[ordinal]].coordinate,
                    segment: u32::try_from(state.block_position[ordinal])
                        .map_err(|_| Error::Arithmetic)?,
                })
            };
            let selected_successor =
                if matches!(block.terminator, Terminator::ConditionalBranch { .. }) {
                    let definition = input.uses()[block.terminator_uses.start].definition;
                    match state.literal(definition, budget)? {
                        Some(Literal {
                            ty: ScalarType::Bool,
                            bits: 0,
                        }) => Some(Edge {
                            source: block.coordinate,
                            successor: 1,
                        }),
                        Some(Literal {
                            ty: ScalarType::Bool,
                            bits: 1,
                        }) => Some(Edge {
                            source: block.coordinate,
                            successor: 0,
                        }),
                        _ => None,
                    }
                } else {
                    None
                };
            result.blocks[ordinal] = CanonicalKirBlockControlV1 {
                reachable: state.reachable[ordinal] != 0,
                placement,
                selected_successor,
            };
        }
        for (ordinal, edge) in input.edges().iter().enumerate() {
            budget.charge_work(1)?;
            let source = index::block(input, edge.coordinate.source, budget)?;
            let placement = match state.edge_output[ordinal] {
                NONE => CanonicalKirEdgePlacementV1::Omitted,
                INTERNAL => CanonicalKirEdgePlacementV1::InternalConnector(
                    result.blocks[source]
                        .placement
                        .ok_or(Error::Rule("internal connector placement"))?,
                ),
                output_edge => {
                    CanonicalKirEdgePlacementV1::Retained(output.edges()[output_edge].coordinate)
                }
            };
            result.edges[ordinal] = CanonicalKirEdgeControlV1 {
                placement,
                executable: state.reachable[source] != 0 && state.edge_possible(ordinal, budget)?,
            };
        }
        for row in checked.rows().uses {
            budget.charge_work(1)?;
            let input_use = index::used(input, row.input, budget)?;
            let output_use = index::used(output, row.output, budget)?;
            if result.uses[input_use].is_some() {
                return Err(Error::Rule("duplicate transported operand occurrence"));
            }
            result.uses[input_use] = Some(CanonicalKirOutputUseV1 {
                coordinate: row.output,
                definition: output.definitions()[output.uses()[output_use].definition].coordinate,
            });
        }
        for row in checked.rows().edge_arguments {
            budget.charge_work(1)?;
            let argument = index::edge_argument(input, row.input, budget)?;
            if result.arguments[argument].is_some() {
                return Err(Error::Rule(
                    "duplicate transported edge argument occurrence",
                ));
            }
            result.arguments[argument] = Some(row.output);
        }
        drop(state);
        budget.release_storage(state_storage)?;
        Ok(result)
    }

    pub const fn input(&self) -> &'a Inventory<'input> {
        self.input
    }
    pub const fn output(&self) -> &'a Inventory<'output> {
        self.output
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub fn block(
        &self,
        input: Block,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalKirBlockControlV1> {
        let ordinal = index::block(self.input, input, budget)?;
        budget.charge_work(1)?;
        Ok(self.blocks[ordinal])
    }
    pub fn edge(&self, input: Edge, budget: &mut Budget<'_>) -> Result<CanonicalKirEdgeControlV1> {
        let ordinal = index::edge(self.input, input, budget)?;
        budget.charge_work(1)?;
        Ok(self.edges[ordinal])
    }
    pub fn operand(
        &self,
        input: Use,
        budget: &mut Budget<'_>,
    ) -> Result<Option<CanonicalKirOutputUseV1>> {
        let ordinal = index::used(self.input, input, budget)?;
        budget.charge_work(1)?;
        Ok(self.uses[ordinal])
    }
    pub fn edge_argument(
        &self,
        input: EdgeArgument,
        budget: &mut Budget<'_>,
    ) -> Result<Option<EdgeArgument>> {
        let ordinal = index::edge_argument(self.input, input, budget)?;
        budget.charge_work(1)?;
        Ok(self.arguments[ordinal])
    }
    /// Borrows exact original argument occurrences; callers meter each visit.
    pub fn input_edge_arguments(
        &self,
        input: Edge,
        budget: &mut Budget<'_>,
    ) -> Result<&[crate::CanonicalKirEdgeArgumentRefV1]> {
        let ordinal = index::edge(self.input, input, budget)?;
        Ok(&self.input.edge_arguments()[self.input.edges()[ordinal].bindings.clone()])
    }
}
