use super::*;

const ROOT: u8 = 1;
const SELECTED_BODY: u8 = 2;
const RETAINED_HELPER: u8 = 4;
const ROOT_ENTRY_SEEN: u8 = 8;

#[derive(Clone, Copy, Default)]
struct SourceFunctionRole {
    flags: u8,
    selected: Option<SemanticFunctionIdV1>,
}

/// Scratch roles of the already replayed source/N owner. A selected body is
/// not thereby an admitted scalar helper or a new executable source owner.
pub(super) struct SourceRoleCensus {
    functions: Vec<SourceFunctionRole>,
}

impl SourceRoleCensus {
    pub(super) fn retained_helper(&self, ordinal: usize) -> R<bool> {
        self.functions
            .get(ordinal)
            .map(|row| row.flags & RETAINED_HELPER != 0)
            .ok_or_else(|| refused("scalar helpers", "source function coordinate"))
    }
}

fn selected_body(
    semantic: &AdmittedInertSemanticMirV1,
    root: SemanticFunctionIdV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<SemanticFunctionIdV1> {
    charge(budget, 32)?;
    let function = semantic
        .functions()
        .get(root.index() as usize)
        .ok_or_else(|| refused("scalar helpers", "source root coordinate"))?;
    let entry = function
        .blocks()
        .get(function.entry().index() as usize)
        .ok_or_else(|| refused("scalar helpers", "source root entry"))?;
    let (arguments, returned_statements) = match entry.terminator().kind() {
        SemanticTerminatorKindV1::Call(call) => (
            call.arguments().len(),
            call.destination()
                .and_then(|destination| {
                    function
                        .blocks()
                        .get(destination.edge().target().index() as usize)
                })
                .map_or(0, |block| block.statements().len()),
        ),
        _ => (0, 0),
    };
    // The admitted selector performs a root binary search, two fixed-width
    // ABI slice comparisons, two administrative-statement scans, and one
    // indexed argument-forwarding scan. It neither walks types recursively
    // nor scans locals per argument. Eight units per item and fixed overhead
    // conservatively pay those comparisons before invoking the selector.
    let search = semantic.roots().len().checked_ilog2().unwrap_or(0) as usize + 1;
    let items = [
        search,
        function.abi().source_input_types().len(),
        function.abi().source_argument_ownership().len(),
        entry.statements().len(),
        returned_statements,
        arguments,
    ]
    .into_iter()
    .try_fold(0usize, |sum, count| sum.checked_add(count))
    .ok_or_else(arithmetic)?;
    let cost = items
        .checked_mul(8)
        .and_then(|cost| cost.checked_add(64))
        .ok_or_else(arithmetic)?;
    charge(budget, cost)?;
    semantic
        .select_kernel_body_for_root_v1(root)
        .filter(|selection| selection.root() == root)
        .map(|selection| selection.body())
        .ok_or_else(|| refused("scalar helpers", "exact selected source body"))
}

/// Complete source/N replay remains mandatory at the caller. In particular,
/// that replay independently checks the exact ordered root-qualified helper
/// roster and its native function IDs; this census never replaces that join.
pub(super) fn check_source(
    owner: &ProductionSemanticKirOwnerV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<SourceRoleCensus> {
    let semantic = owner.semantic().semantic();
    charge(budget, 3)?;
    let mut functions = scratch::<SourceFunctionRole>(semantic.functions().len(), budget)?;
    charge(budget, semantic.functions().len())?;
    functions.resize(semantic.functions().len(), SourceFunctionRole::default());
    for root in semantic.roots() {
        charge(budget, 5)?;
        let row = functions
            .get_mut(root.index() as usize)
            .ok_or_else(|| refused("scalar helpers", "source root coordinate"))?;
        if row.flags & ROOT != 0 {
            return Err(refused("scalar helpers", "unique source roots"));
        }
        row.flags |= ROOT;
        let selected = selected_body(semantic, *root, budget)?;
        functions[root.index() as usize].selected = Some(selected);
        functions
            .get_mut(selected.index() as usize)
            .ok_or_else(|| refused("scalar helpers", "selected source body coordinate"))?
            .flags |= SELECTED_BODY;
    }
    for association in owner.correspondence.lowered_functions.iter() {
        charge(budget, 7)?;
        let root = functions
            .get_mut(association.correspondence_owner.index() as usize)
            .filter(|row| row.flags & ROOT != 0)
            .ok_or_else(|| refused("scalar helpers", "exact source root association"))?;
        match association.role {
            SemanticKirFunctionRoleV1::KernelEntry => {
                if root.flags & ROOT_ENTRY_SEEN != 0
                    || root.selected != Some(association.semantic_function)
                {
                    return Err(refused(
                        "scalar helpers",
                        "exact selected source entry association",
                    ));
                }
                root.flags |= ROOT_ENTRY_SEEN;
            }
            SemanticKirFunctionRoleV1::InternalHelper => {
                functions
                    .get_mut(association.semantic_function.index() as usize)
                    .ok_or_else(|| refused("scalar helpers", "source helper coordinate"))?
                    .flags |= RETAINED_HELPER;
            }
        }
    }
    for (ordinal, (function, row)) in semantic.functions().iter().zip(&functions).enumerate() {
        charge(budget, 4)?;
        match function.role() {
            SemanticFunctionRoleV1::KernelRoot
                if row.flags & (ROOT | ROOT_ENTRY_SEEN) == (ROOT | ROOT_ENTRY_SEEN)
                    && row.flags & RETAINED_HELPER == 0 => {}
            SemanticFunctionRoleV1::InternalHelper
                if row.flags & (ROOT | ROOT_ENTRY_SEEN) == 0
                    && row.flags & (SELECTED_BODY | RETAINED_HELPER) != 0 =>
            {
                // A function selected for one root may also occur as a real
                // retained helper under another. Selection never exempts that
                // occurrence from the existing direct scalar Rust ABI rule.
                if row.flags & RETAINED_HELPER != 0 {
                    let id = u32::try_from(ordinal).map_err(|_| arithmetic())?;
                    if !scalar_helpers::source_helper_abi(
                        semantic,
                        SemanticFunctionIdV1::from_index(id),
                        budget,
                    )? {
                        return Err(refused("scalar helpers", "source helper role"));
                    }
                }
            }
            _ => {
                return Err(refused(
                    "scalar helpers",
                    "complete selected source helper roster",
                ));
            }
        }
    }
    Ok(SourceRoleCensus { functions })
}

#[cfg(test)]
#[path = "production_checked_output_source_roles_v1_tests.rs"]
mod tests;
