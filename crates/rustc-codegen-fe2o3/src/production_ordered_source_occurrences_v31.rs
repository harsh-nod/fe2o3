//! Move-only custody for one actual compiler-observed ordered-region occurrence.
//! Hashes bind the retained preflight transcript and authenticated root contract,
//! not raw-source bytes, a toolchain receipt, or execution/proof authority.

use fe2o3_mir_model::semantic_mir_v1::{
    HARD_MAX_CANONICAL_BYTES_V1, SemanticCallableIdV1, SemanticFunctionIdV1,
    SemanticGfx942OrderedRegionRegistersV31, SemanticOrderedRegionSourceV31,
};
use rustc_middle::mir::{BasicBlock, Operand, TerminatorKind, UnwindAction};
use rustc_middle::ty::{Instance, TyCtxt, TyKind, TypingEnv};

use crate::collector::CollectedFunctionRole;
use crate::production_ordered_region_v31::{
    MAX_PROFILE_BLOCKS, actual_registers, require_unconditional_single_execution,
};
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;
use crate::rustc_semantic_adapter_v1::domain_digest;
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;

const UNIT_DOMAIN: &[u8] = b"fe2o3/semantic-mir/gfx942-ordered-region/compiler-observed-unit/v31";
const CONTRACT_DOMAIN: &[u8] = b"fe2o3/semantic-mir/gfx942-ordered-region/root-contract/v31";
const STATEMENT_DOMAIN: &[u8] = b"fe2o3/semantic-mir/gfx942-ordered-region/static-occurrence/v31";
type Key = (SemanticFunctionIdV1, u32);

struct Pending {
    key: Key,
    callee: SemanticCallableIdV1,
    registers: SemanticGfx942OrderedRegionRegistersV31,
    source: SemanticOrderedRegionSourceV31,
}

/// One private pending slot, not a second executable graph or serialized owner.
#[derive(Default)]
pub(crate) struct OrderedSourceOccurrencesV31 {
    pending: Option<Pending>,
    consumed: Option<Key>,
}

impl OrderedSourceOccurrencesV31 {
    pub(crate) fn from_plan<'tcx>(
        tcx: TyCtxt<'tcx>,
        plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    ) -> Result<Self, &'static str> {
        let recipes = plan.terminal_expansion_producers();
        let mut regions = recipes.iter().filter(|recipe| {
            recipe.expansion == ProductionTerminalExpansionV1::Gfx942OrderedXorAddE32
        });
        let Some(recipe) = regions.next() else {
            return Ok(Self::default());
        };
        if recipes.len() > MAX_PROFILE_BLOCKS || regions.next().is_some() {
            return Err("ordered region requires one bounded source occurrence");
        }
        let functions = plan.function_producers();
        let [root_id] = plan.roots() else {
            return Err("ordered region requires one authenticated kernel root");
        };
        let [root] = functions else {
            return Err("ordered region source profile excludes helper bodies");
        };
        if root_id.index() != 0
            || recipe.caller != *root_id
            || root.role != CollectedFunctionRole::KernelEntry
        {
            return Err("ordered region source caller is not the unique kernel root");
        }
        let frontend = root
            .frontend_contract
            .as_ref()
            .ok_or("ordered region requires an authenticated frontend contract")?;
        if frontend
            .contract()
            .launch()
            .and_then(|launch| launch.required())
            .map(|dimensions| dimensions.as_array())
            != Some([64, 1, 1])
        {
            return Err("ordered region requires an explicit 64x1x1 workgroup");
        }
        let target = crate::semantic_layout_bridge::rustc_semantic_layout_target_v1(tcx)
            .map_err(|_| "ordered region active target is not admitted")?;
        if target.llvm_target() != "amdgcn-amd-amdhsa"
            || !target
                .has_exact_codegen_profile("gfx942", "-wavefrontsize32,+wavefrontsize64,-xnack")
        {
            return Err("ordered region requires exact gfx942 xnack-off wave64");
        }
        if plan.canonical_transcript().is_empty()
            || plan.canonical_transcript().len() as u64 > HARD_MAX_CANONICAL_BYTES_V1
            || frontend.canonical_bytes().is_empty()
        {
            return Err("ordered region canonical compiler input is absent or too large");
        }
        let terminal = plan
            .terminal_producers()
            .get(recipe.terminal as usize)
            .ok_or("ordered region terminal producer missing")?;
        if terminal.instance != recipe.instance
            || terminal.identities != recipe.identities
            || terminal.expansion != recipe.expansion
            || recipe.arguments != 8
        {
            return Err("ordered region authenticated terminal recipe mismatch");
        }
        let body = tcx.instance_mir(root.instance.def);
        require_unconditional_single_execution(body, recipe.block)?;
        let raw = body
            .basic_blocks
            .get(BasicBlock::from_u32(recipe.block))
            .ok_or("ordered region actual MIR block missing")?;
        let TerminatorKind::Call {
            func,
            args,
            target: Some(_),
            unwind: UnwindAction::Continue | UnwindAction::Unreachable,
            ..
        } = &raw.terminator().kind
        else {
            return Err("ordered region requires a returning nonunwinding direct call");
        };
        let Operand::Constant(callee) = func else {
            return Err("ordered region callee is not an actual constant function");
        };
        let TyKind::FnDef(definition, generic_args) = callee.const_.ty().kind() else {
            return Err("ordered region callee is not a direct function");
        };
        if !generic_args.is_empty()
            || args.len() != 8
            || Instance::try_resolve(
                tcx,
                TypingEnv::fully_monomorphized(),
                *definition,
                generic_args,
            )
            .ok()
            .flatten()
                != Some(recipe.instance)
        {
            return Err("ordered region live MIR call differs from its authenticated recipe");
        }
        let registers = actual_registers(
            tcx,
            [
                &args[3].node,
                &args[4].node,
                &args[5].node,
                &args[6].node,
                &args[7].node,
            ],
        )?;
        let [retained_body] = plan.body_producers() else {
            return Err("ordered region retained root body missing");
        };
        if retained_body.function != *root_id
            || retained_body.blocks.len() > MAX_PROFILE_BLOCKS
            || retained_body.raw_to_semantic_blocks.len() > MAX_PROFILE_BLOCKS
        {
            return Err("ordered region retained root body is inconsistent or too large");
        }
        let semantic_block = retained_body
            .raw_to_semantic_blocks
            .get(recipe.block as usize)
            .ok_or("ordered region raw-to-semantic block binding missing")?;
        let block = retained_body
            .blocks
            .get(semantic_block.index() as usize)
            .ok_or("ordered region retained block missing")?;
        if block.rustc_block != recipe.block {
            return Err("ordered region raw-to-semantic block binding differs");
        }
        let unit = domain_digest(UNIT_DOMAIN, &[plan.canonical_transcript()]);
        let resource = frontend.resource_canonical_bytes();
        let contract = domain_digest(
            CONTRACT_DOMAIN,
            &[
                frontend.canonical_bytes(),
                &[u8::from(resource.is_some())],
                resource.unwrap_or_default(),
            ],
        );
        let [input0, input1, input2] = registers.inputs();
        let physical = [
            registers.scratch(),
            registers.output(),
            input0,
            input1,
            input2,
        ];
        let statement = domain_digest(
            STATEMENT_DOMAIN,
            &[
                &unit,
                &contract,
                root.identities.function().as_bytes(),
                &recipe.block.to_le_bytes(),
                block.identity.as_bytes(),
                &[133, 0],
                terminal.identities.function().as_bytes(),
                &8_u32.to_le_bytes(),
                &physical,
            ],
        );
        let source = SemanticOrderedRegionSourceV31::new(
            unit,
            root.identities.function(),
            contract,
            statement,
        )
        .map_err(|_| "ordered region source identity is incomplete")?;
        let callable = functions
            .len()
            .checked_add(recipe.terminal as usize)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or("ordered region semantic callable index overflow")?;
        Ok(Self {
            pending: Some(Pending {
                key: (recipe.caller, recipe.block),
                callee: SemanticCallableIdV1::from_index(callable),
                registers,
                source,
            }),
            consumed: None,
        })
    }

    /// The body owner has already checked the actual direct-call recipe.
    pub(crate) fn take(
        &mut self,
        caller: SemanticFunctionIdV1,
        block: u32,
        callee: SemanticCallableIdV1,
        registers: Option<SemanticGfx942OrderedRegionRegistersV31>,
    ) -> Result<Option<SemanticOrderedRegionSourceV31>, &'static str> {
        let key = (caller, block);
        if self.consumed == Some(key) {
            return Err("ordered region occurrence was already consumed");
        }
        let Some(pending) = self.pending.as_ref().filter(|pending| pending.key == key) else {
            if registers.is_some() {
                return Err("ordered region occurrence missing");
            }
            return Ok(None);
        };
        if pending.callee != callee || registers != Some(pending.registers) {
            return Err("ordered region occurrence callee or physical roles differ");
        }
        let source = self
            .pending
            .take()
            .ok_or("ordered region occurrence disappeared")?
            .source;
        self.consumed = Some(key);
        Ok(Some(source))
    }

    pub(crate) fn require_drained(&self) -> Result<(), &'static str> {
        if self.pending.is_none() {
            Ok(())
        } else {
            Err("ordered region occurrence was not consumed")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdentityV1;

    fn owner() -> OrderedSourceOccurrencesV31 {
        OrderedSourceOccurrencesV31 {
            pending: Some(Pending {
                key: (SemanticFunctionIdV1::from_index(0), 3),
                callee: SemanticCallableIdV1::from_index(2),
                registers: SemanticGfx942OrderedRegionRegistersV31::new(32, 33, [34, 35, 36])
                    .unwrap(),
                source: SemanticOrderedRegionSourceV31::new(
                    [1; 32],
                    SemanticFunctionIdentityV1::from_sha256([2; 32]),
                    [3; 32],
                    [4; 32],
                )
                .unwrap(),
            }),
            consumed: None,
        }
    }

    #[test]
    fn exact_occurrence_drains_once_without_serializable_custody() {
        let mut owner = owner();
        let source = owner.pending.as_ref().unwrap().source;
        let registers = owner.pending.as_ref().unwrap().registers;
        assert!(owner.require_drained().is_err());
        assert_eq!(
            owner.take(
                SemanticFunctionIdV1::from_index(0),
                3,
                SemanticCallableIdV1::from_index(2),
                Some(registers),
            ),
            Ok(Some(source))
        );
        owner.require_drained().unwrap();
        assert!(
            owner
                .take(
                    SemanticFunctionIdV1::from_index(0),
                    3,
                    SemanticCallableIdV1::from_index(2),
                    Some(registers),
                )
                .is_err()
        );
    }

    #[test]
    fn substituted_caller_block_callee_or_physical_roles_cannot_consume() {
        for (caller, block, callee, registers) in [
            (1, 3, 2, Some([32, 33, 34, 35, 36])),
            (0, 4, 2, Some([32, 33, 34, 35, 36])),
            (0, 3, 3, Some([32, 33, 34, 35, 36])),
            (0, 3, 2, Some([31, 33, 34, 35, 36])),
            (0, 3, 2, None),
        ] {
            let mut owner = owner();
            let registers = registers.map(|v| {
                SemanticGfx942OrderedRegionRegistersV31::new(v[0], v[1], [v[2], v[3], v[4]])
                    .unwrap()
            });
            assert!(
                owner
                    .take(
                        SemanticFunctionIdV1::from_index(caller),
                        block,
                        SemanticCallableIdV1::from_index(callee),
                        registers,
                    )
                    .is_err()
            );
            assert!(owner.require_drained().is_err());
            assert!(owner.consumed.is_none());
        }
    }

    #[test]
    fn absence_is_inert_for_ordinary_calls_but_not_a_region() {
        let mut owner = OrderedSourceOccurrencesV31::default();
        assert_eq!(
            owner.take(
                SemanticFunctionIdV1::from_index(0),
                0,
                SemanticCallableIdV1::from_index(1),
                None,
            ),
            Ok(None)
        );
        owner.require_drained().unwrap();
        assert!(
            owner
                .take(
                    SemanticFunctionIdV1::from_index(0),
                    0,
                    SemanticCallableIdV1::from_index(1),
                    Some(SemanticGfx942OrderedRegionRegistersV31::new(0, 63, [1, 62, 31]).unwrap()),
                )
                .is_err()
        );
    }
}
