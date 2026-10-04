//! One-use custody obtained by replaying the real compiler MIR program call.
//! Source hashes bind compiler observations, not raw-source authentication,
//! final code, register contents, proof, launch or resume authority.

use fe2o3_mir_model::semantic_mir_v1::{
    HARD_MAX_CANONICAL_BYTES_V1, SemanticCallableIdV1, SemanticFunctionIdV1,
    SemanticGfx942OrderedProgramRegistersV32, SemanticGfx942U32ProgramV32,
    SemanticOrderedProgramSourceV32,
};
use rustc_middle::mir::{BasicBlock, TerminatorKind, UnwindAction};
use rustc_middle::ty::{Instance, TyCtxt};

use crate::collector::CollectedFunctionRole;
use crate::production_ordered_program_v32::{
    ActualOrderedProgramCallV32, MAX_PROFILE_BLOCKS, observe_call, parse_program_consts,
    require_unconditional_single_execution,
};
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;
use crate::rustc_semantic_adapter_v1::{canonical_function_identities_v1, domain_digest};
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;

const UNIT_DOMAIN: &[u8] = b"fe2o3/semantic-mir/gfx942-ordered-program/compiler-observed-unit/v32";
const CONTRACT_DOMAIN: &[u8] = b"fe2o3/semantic-mir/gfx942-ordered-program/root-contract/v32";
const STATEMENT_DOMAIN: &[u8] = b"fe2o3/semantic-mir/gfx942-ordered-program/static-occurrence/v32";
type Key = (SemanticFunctionIdV1, u32);

#[derive(Clone, Copy, Eq, PartialEq)]
struct Binding<I> {
    callee: SemanticCallableIdV1,
    instance: I,
    program: SemanticGfx942U32ProgramV32,
    registers: SemanticGfx942OrderedProgramRegistersV32,
}

struct Pending<I> {
    key: Key,
    binding: Binding<I>,
    source: SemanticOrderedProgramSourceV32,
}

// The generic parameter keeps the one-use state-machine testable without
// manufacturing rustc Instances. Production only instantiates it with Instance.
struct Slot<I> {
    pending: Option<Pending<I>>,
    consumed: Option<Key>,
}

impl<I> Default for Slot<I> {
    fn default() -> Self {
        Self {
            pending: None,
            consumed: None,
        }
    }
}

impl<I: Eq> Slot<I> {
    fn take(
        &mut self,
        key: Key,
        binding: Option<Binding<I>>,
    ) -> Result<Option<SemanticOrderedProgramSourceV32>, &'static str> {
        if self.consumed == Some(key) {
            return Err("ordered program occurrence was already consumed");
        }
        let Some(pending) = self.pending.as_ref().filter(|pending| pending.key == key) else {
            return if binding.is_some() {
                Err("ordered program occurrence missing")
            } else {
                Ok(None)
            };
        };
        if binding.as_ref() != Some(&pending.binding) {
            return Err(
                "ordered program occurrence Instance, program, callee or physical roles differ",
            );
        }
        let source = self
            .pending
            .take()
            .ok_or("ordered program occurrence disappeared")?
            .source;
        self.consumed = Some(key);
        Ok(Some(source))
    }

    fn require_drained(&self) -> Result<(), &'static str> {
        if self.pending.is_none() {
            Ok(())
        } else {
            Err("ordered program occurrence was not consumed")
        }
    }
}

/// Private move-only owner; it cannot be reconstructed from serialized IDs.
#[derive(Default)]
pub(crate) struct OrderedProgramSourceOccurrencesV32<'tcx> {
    slot: Slot<Instance<'tcx>>,
}

impl<'tcx> OrderedProgramSourceOccurrencesV32<'tcx> {
    pub(crate) fn from_plan(
        tcx: TyCtxt<'tcx>,
        plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    ) -> Result<Self, &'static str> {
        let recipes = plan.terminal_expansion_producers();
        if recipes.len() > MAX_PROFILE_BLOCKS {
            return Err("ordered program source recipe bound exceeded");
        }
        let mut programs = recipes.iter().filter(|recipe| {
            recipe.expansion == ProductionTerminalExpansionV1::Gfx942OrderedProgramE32
        });
        let Some(recipe) = programs.next() else {
            return Ok(Self::default());
        };
        if programs.next().is_some() {
            return Err("ordered program requires one source occurrence");
        }
        let functions = plan.function_producers();
        let [root_id] = plan.roots() else {
            return Err("ordered program requires one authenticated kernel root");
        };
        let [root] = functions else {
            return Err("ordered program source profile excludes helper bodies");
        };
        if root_id.index() != 0
            || recipe.caller != *root_id
            || root.role != CollectedFunctionRole::KernelEntry
        {
            return Err("ordered program source caller is not the unique kernel root");
        }
        let frontend = root
            .frontend_contract
            .as_ref()
            .ok_or("ordered program requires an authenticated frontend contract")?;
        let launch = frontend
            .contract()
            .launch()
            .ok_or("ordered program requires explicit launch bounds")?;
        if launch.required().map(|value| value.as_array()) != Some([64, 1, 1])
            || launch.maximum().map(|value| value.as_array()) != Some([64, 1, 1])
        {
            return Err("ordered program requires required and maximum 64x1x1 workgroup bounds");
        }
        let target = crate::semantic_layout_bridge::rustc_semantic_layout_target_v1(tcx)
            .map_err(|_| "ordered program active target is not admitted")?;
        if target.llvm_target() != "amdgcn-amd-amdhsa"
            || !target
                .has_exact_codegen_profile("gfx942", "-wavefrontsize32,+wavefrontsize64,-xnack")
        {
            return Err("ordered program requires exact gfx942 xnack-off wave64");
        }
        if plan.canonical_transcript().is_empty()
            || plan.canonical_transcript().len() as u64 > HARD_MAX_CANONICAL_BYTES_V1
            || frontend.canonical_bytes().is_empty()
            || frontend.canonical_bytes().len() as u64 > HARD_MAX_CANONICAL_BYTES_V1
            || frontend
                .resource_canonical_bytes()
                .is_some_and(|bytes| bytes.len() as u64 > HARD_MAX_CANONICAL_BYTES_V1)
        {
            return Err("ordered program canonical compiler input is absent or too large");
        }
        let terminal = plan
            .terminal_producers()
            .get(recipe.terminal as usize)
            .ok_or("ordered program terminal producer missing")?;
        if terminal.instance != recipe.instance
            || terminal.identities != recipe.identities
            || terminal.expansion != recipe.expansion
            || recipe.arguments != 8
        {
            return Err("ordered program authenticated terminal recipe mismatch");
        }
        let retained_program = parse_program_consts(tcx, terminal.instance)?;
        let body = tcx.instance_mir(root.instance.def);
        require_unconditional_single_execution(body, recipe.block)?;
        let raw = body
            .basic_blocks
            .get(BasicBlock::from_u32(recipe.block))
            .ok_or("ordered program actual MIR block missing")?;
        let TerminatorKind::Call {
            func,
            args,
            destination,
            target: Some(_),
            unwind: UnwindAction::Continue | UnwindAction::Unreachable,
            ..
        } = &raw.terminator().kind
        else {
            return Err("ordered program requires a returning nonunwinding direct call");
        };
        if args.len() != 8 || !destination.projection.is_empty() {
            return Err(
                "ordered program actual call requires eight operands and an unprojected destination",
            );
        }
        let observed = observe_call(
            tcx,
            root.instance,
            body,
            func,
            [
                &args[0].node,
                &args[1].node,
                &args[2].node,
                &args[3].node,
                &args[4].node,
                &args[5].node,
                &args[6].node,
                &args[7].node,
            ],
        )?;
        if observed.instance() != terminal.instance
            || observed.program() != retained_program
            || canonical_function_identities_v1(tcx, observed.instance()) != terminal.identities
        {
            return Err(
                "ordered program live MIR Instance or program differs from its authenticated recipe",
            );
        }
        let [retained_body] = plan.body_producers() else {
            return Err("ordered program retained root body missing");
        };
        if retained_body.function != *root_id
            || retained_body.blocks.len() > MAX_PROFILE_BLOCKS
            || retained_body.raw_to_semantic_blocks.len() > MAX_PROFILE_BLOCKS
        {
            return Err("ordered program retained root body is inconsistent or too large");
        }
        let semantic_block = retained_body
            .raw_to_semantic_blocks
            .get(recipe.block as usize)
            .ok_or("ordered program raw-to-semantic block binding missing")?;
        let block = retained_body
            .blocks
            .get(semantic_block.index() as usize)
            .ok_or("ordered program retained block missing")?;
        if block.rustc_block != recipe.block {
            return Err("ordered program raw-to-semantic block binding differs");
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
        let registers = observed.registers();
        let [input0, input1, input2] = registers.inputs();
        let physical = [
            registers.scratch(),
            registers.output(),
            input0,
            input1,
            input2,
        ];
        let packed = observed.program().packed_words();
        let mut packed_bytes = [0_u8; 32];
        for (index, word) in packed.iter().enumerate() {
            packed_bytes[index * 8..index * 8 + 8].copy_from_slice(&word.to_le_bytes());
        }
        let statement = domain_digest(
            STATEMENT_DOMAIN,
            &[
                &unit,
                &contract,
                root.identities.function().as_bytes(),
                &recipe.block.to_le_bytes(),
                block.identity.as_bytes(),
                &[134, 0],
                terminal.identities.function().as_bytes(),
                terminal.identities.item_definition().as_bytes(),
                terminal.identities.monomorphization().as_bytes(),
                terminal.identities.generic_type_arguments().as_bytes(),
                terminal.identities.const_generic_arguments().as_bytes(),
                &8_u32.to_le_bytes(),
                &physical,
                &[observed.program().count()],
                &packed_bytes,
                b"gfx942:xnack-",
                &[64],
                &[64, 1, 1],
            ],
        );
        let source = SemanticOrderedProgramSourceV32::new(
            unit,
            root.identities.function(),
            contract,
            statement,
        )
        .map_err(|_| "ordered program source identity is incomplete")?;
        let callable = functions
            .len()
            .checked_add(recipe.terminal as usize)
            .and_then(|index| u32::try_from(index).ok())
            .ok_or("ordered program semantic callable index overflow")?;
        Ok(Self {
            slot: Slot {
                pending: Some(Pending {
                    key: (recipe.caller, recipe.block),
                    binding: Binding {
                        callee: SemanticCallableIdV1::from_index(callable),
                        instance: observed.instance(),
                        program: observed.program(),
                        registers,
                    },
                    source,
                }),
                consumed: None,
            },
        })
    }

    /// The body producer passes a fresh observation of its actual MIR call, not
    /// imported semantic metadata or a detached source-identity transcript.
    pub(crate) fn take(
        &mut self,
        caller: SemanticFunctionIdV1,
        block: u32,
        callee: SemanticCallableIdV1,
        observed: Option<ActualOrderedProgramCallV32<'tcx>>,
    ) -> Result<Option<SemanticOrderedProgramSourceV32>, &'static str> {
        self.slot.take(
            (caller, block),
            observed.map(|value| Binding {
                callee,
                instance: value.instance(),
                program: value.program(),
                registers: value.registers(),
            }),
        )
    }

    pub(crate) fn require_drained(&self) -> Result<(), &'static str> {
        self.slot.require_drained()
    }
}

#[cfg(test)]
#[path = "production_ordered_program_source_occurrences_v32_tests.rs"]
mod tests;
