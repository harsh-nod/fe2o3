//! Bounded projection of the one actual verified CFG; no packed program input.
use super::*;
use crate::{
    Gfx942CompleteBodyBlockV1, Gfx942CompleteBodyBoundaryV1, Gfx942CompleteBodyLabelV1,
    Gfx942CompleteBodyResourcesV1, Gfx942CompleteBodyTerminatorV1 as PlanTerminator,
};
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAME_V19,
    AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAMESPACE_V19,
    AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME, ComparePredicate, Constant, FunctionRole,
    Gfx942OrderedProgramRegistersV1 as Registers, Gfx942ProgramDestinationV1 as Destination,
    Gfx942ProgramInstructionV1 as Instruction, Gfx942ProgramRoleV1 as Role, IntrinsicOperation,
    Module, OperationKind, TargetCapability, Terminator, WaveWidth, WorkgroupSize,
};
use std::collections::BTreeSet;

type Failure = Gfx942CompleteBodyCanonicalEmissionErrorV19;
type OperationCorrespondences = [Option<Gfx942CompleteBodyCanonicalOperationCorrespondenceV19>; 25];
type BlockCorrespondences = [Option<Gfx942CompleteBodyCanonicalBlockCorrespondenceV19>; 4];
fn require(value: bool, reason: &'static str) -> Result<()> {
    if value {
        Ok(())
    } else {
        Err(Failure::Profile(reason))
    }
}
#[derive(Clone, Copy)]
struct Block {
    label: u8,
    first: u8,
    count: u8,
    terminator: PlanTerminator,
}
#[derive(Clone, Copy)]
enum Mapping {
    Declaration,
    Instruction(u8),
    Zero,
    Selector,
    Index,
    Length,
    Bounds,
    Base,
    Address,
    Store,
}
#[derive(Clone, Copy)]
struct Operation {
    block: BlockId,
    ordinal: u8,
    result: Option<ValueId>,
    mapping: Mapping,
}
pub(super) struct Extracted<'a> {
    pub(super) symbol: &'a str,
    registers: Registers,
    instructions: [Instruction; 16],
    blocks: [Block; 4],
    block_count: usize,
    operations: [Option<Operation>; 25],
}
pub(super) fn scratch_storage() -> usize {
    std::mem::size_of::<Extracted<'_>>() + std::mem::size_of::<[Gfx942CompleteBodyBlockV1<'_>; 4]>()
}
fn capabilities(capabilities: &BTreeSet<TargetCapability>) -> Result<()> {
    require(
        capabilities.len() == 3,
        "exact complete-body capability roster",
    )?;
    let mut found = [false; 3];
    for capability in capabilities {
        let index = match capability {
            TargetCapability::WaveWidth(WaveWidth::Wave64) => 0,
            TargetCapability::Extension { namespace, name }
                if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE
                    && name == AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME =>
            {
                1
            }
            TargetCapability::Extension { namespace, name }
                if namespace == AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAMESPACE_V19
                    && name == AMDGPU_GFX942_COMPLETE_BODY_CAPABILITY_NAME_V19 =>
            {
                2
            }
            _ => {
                return Err(Failure::Profile(
                    "unsupported or conflicting complete-body capability",
                ));
            }
        };
        require(!found[index], "duplicate profile capability")?;
        found[index] = true;
    }
    require(found == [true; 3], "missing complete-body capability")
}
pub(super) fn extract(module: &Module) -> Result<Extracted<'_>> {
    let [function] = module.functions.as_slice() else {
        return Err(Failure::Profile("single function"));
    };
    let [kernel] = module.kernels.as_slice() else {
        return Err(Failure::Profile("single kernel"));
    };
    require(
        function.role == FunctionRole::KernelEntry && kernel.entry == function.id,
        "actual kernel entry",
    )?;
    require(
        kernel.domain.rank() == 1 && kernel.workgroup_size == Some(WorkgroupSize::new(64, 1, 1)),
        "rank-one workgroup64x1x1 launch",
    )?;
    for set in [
        &module.required_capabilities,
        &kernel.required_capabilities,
        &function.required_capabilities,
    ] {
        capabilities(set)?;
    }
    let body = function
        .body
        .as_ref()
        .ok_or(Failure::Profile("missing body"))?;
    require(
        matches!(body.blocks.len(), 1 | 4),
        "one block or selector diamond",
    )?;
    let declaration_operation = body.blocks[0]
        .operations
        .first()
        .ok_or(Failure::Profile("missing declaration"))?;
    let OperationKind::Gfx942CompleteBodyDeclaration(declaration) = &declaration_operation.kind
    else {
        return Err(Failure::Profile("first operation declaration"));
    };
    require(
        usize::from(declaration.block_count) == body.blocks.len(),
        "actual block census",
    )?;
    declaration
        .validate_shape()
        .map_err(|_| Failure::Profile("declaration shape"))?;
    let mut output = Extracted {
        symbol: function.id.as_str(),
        registers: declaration.registers,
        instructions: [Instruction::Move {
            destination: Destination::Scratch,
            source: Role::Input0,
        }; 16],
        blocks: [Block {
            label: 0,
            first: 0,
            count: 0,
            terminator: PlanTerminator::GuardedStoreOutputAndEnd,
        }; 4],
        block_count: body.blocks.len(),
        operations: [None; 25],
    };
    let mut authored = 0_usize;
    let mut operations = 0_usize;
    for (ordinal, block) in body.blocks.iter().enumerate() {
        require(
            block.id == BlockId(ordinal as u32),
            "actual ordinal block identity",
        )?;
        require(block.operations.len() <= 25, "per-block operation bound")?;
        let first = authored;
        let mut ended_steps = false;
        for (local, operation) in block.operations.iter().enumerate() {
            require(
                operations < 25 && operation.results.len() <= 1,
                "operation/result bound",
            )?;
            let mapping = match &operation.kind {
                OperationKind::Gfx942CompleteBodyDeclaration(_) if ordinal == 0 && local == 0 => {
                    Mapping::Declaration
                }
                OperationKind::Gfx942CompleteBodyStep(step) => {
                    require(
                        !ended_steps
                            && authored < 16
                            && usize::from(step.authored_block) == ordinal
                            && usize::from(step.authored_instruction) == authored,
                        "actual authored operation order",
                    )?;
                    output.instructions[authored] = step.instruction;
                    let mapping = Mapping::Instruction(authored as u8);
                    authored += 1;
                    mapping
                }
                kind => {
                    ended_steps = true;
                    match kind {
                        OperationKind::Constant(Constant::U32(0))
                            if ordinal == 0 && body.blocks.len() == 4 =>
                        {
                            Mapping::Zero
                        }
                        OperationKind::Compare {
                            predicate: ComparePredicate::Equal,
                            ..
                        } if ordinal == 0 && body.blocks.len() == 4 => Mapping::Selector,
                        OperationKind::Intrinsic(intrinsic)
                            if *intrinsic == IntrinsicOperation::global_id_1d() =>
                        {
                            Mapping::Index
                        }
                        OperationKind::SliceLength { .. } => Mapping::Length,
                        OperationKind::Compare {
                            predicate: ComparePredicate::LessThan,
                            ..
                        } => Mapping::Bounds,
                        OperationKind::SliceData { .. } => Mapping::Base,
                        OperationKind::GetElementPointer { .. } => Mapping::Address,
                        OperationKind::GuardedStore { .. } => Mapping::Store,
                        _ => return Err(Failure::Profile("unexpected complete-body operation")),
                    }
                }
            };
            output.operations[operations] = Some(Operation {
                block: block.id,
                ordinal: local as u8,
                result: operation.results.first().map(|result| result.id),
                mapping,
            });
            operations += 1;
        }
        let label = |target: BlockId| -> Result<Gfx942CompleteBodyLabelV1> {
            let index = target.0 as usize;
            require(
                index < body.blocks.len() && body.blocks[index].id == target,
                "actual successor membership",
            )?;
            Ok(Gfx942CompleteBodyLabelV1(declaration.labels[index]))
        };
        let terminator = match block.terminator.as_ref() {
            Some(Terminator::Branch { target, .. }) => PlanTerminator::Jump(label(*target)?),
            Some(Terminator::ConditionalBranch {
                then_target,
                else_target,
                ..
            }) => {
                require(
                    ordinal == 0 && body.blocks.len() == 4,
                    "uniform entry selector",
                )?;
                PlanTerminator::BranchSelectorZero {
                    zero: label(*then_target)?,
                    nonzero: label(*else_target)?,
                }
            }
            Some(Terminator::Return { values }) if values.is_empty() => {
                PlanTerminator::GuardedStoreOutputAndEnd
            }
            _ => return Err(Failure::Profile("unexpected complete-body terminator")),
        };
        output.blocks[ordinal] = Block {
            label: declaration.labels[ordinal],
            first: first as u8,
            count: (authored - first) as u8,
            terminator,
        };
    }
    require(
        authored == usize::from(declaration.instruction_count),
        "actual instruction census",
    )?;
    Ok(output)
}
impl Extracted<'_> {
    pub(super) fn checked_plan(&self) -> Result<Gfx942CompleteBodyPlanV1> {
        let blocks = self.blocks.map(|block| Gfx942CompleteBodyBlockV1 {
            label: Gfx942CompleteBodyLabelV1(block.label),
            instructions: &self.instructions
                [usize::from(block.first)..usize::from(block.first + block.count)],
            terminator: block.terminator,
        });
        Gfx942CompleteBodyPlanV1::check_prepaid_v19(
            Gfx942CompleteBodyBoundaryV1::PROFILE,
            self.registers,
            Gfx942CompleteBodyResourcesV1::required(self.registers),
            &blocks[..self.block_count],
        )
        .map_err(Failure::Plan)
    }
    pub(super) fn correspondence(
        &self,
        emission: &Gfx942CompleteBodyEmissionV1,
    ) -> Result<(OperationCorrespondences, BlockCorrespondences)> {
        use Gfx942CompleteBodyCanonicalOperationLoweringV19 as Lowering;
        let mut operations = [None; 25];
        let tail = emission.compiler_tail().first_instruction_line;
        for (slot, operation) in operations.iter_mut().zip(self.operations.iter().flatten()) {
            let lowering = match operation.mapping {
                Mapping::Declaration => Lowering::Declaration,
                Mapping::Instruction(ordinal) => {
                    let line = emission
                        .instructions()
                        .nth(usize::from(ordinal))
                        .ok_or(Failure::Profile("instruction correspondence"))?;
                    require(
                        line.ordinal == ordinal
                            && line.block_ordinal == operation.block.0 as u8
                            && line.descriptor
                                == self.instructions[usize::from(ordinal)].descriptor(),
                        "exact emitted descriptor/block correspondence",
                    )?;
                    Lowering::AuthoredInstruction {
                        descriptor: line.descriptor,
                        assembly_line: line.assembly_line,
                    }
                }
                Mapping::Zero | Mapping::Selector => {
                    let block = emission
                        .blocks()
                        .next()
                        .ok_or(Failure::Profile("selector correspondence"))?;
                    if matches!(operation.mapping, Mapping::Zero) {
                        Lowering::SelectorZero {
                            assembly_compare_line: block.terminator_line,
                        }
                    } else {
                        Lowering::SelectorCompare {
                            assembly_compare_line: block.terminator_line,
                        }
                    }
                }
                Mapping::Index => Lowering::GlobalIndexInLlvmShell,
                Mapping::Length => Lowering::OutputLengthAbiParameter,
                Mapping::Bounds => Lowering::BoundsCompare {
                    assembly_line: tail + 3,
                },
                Mapping::Base => Lowering::OutputPointerAbiParameter,
                Mapping::Address => Lowering::OutputElementAddress {
                    first_assembly_line: tail,
                    assembly_line_count: 3,
                },
                Mapping::Store => Lowering::GuardedOutputStore {
                    assembly_line: tail + 5,
                },
            };
            *slot = Some(Gfx942CompleteBodyCanonicalOperationCorrespondenceV19 {
                block: operation.block,
                operation_ordinal: operation.ordinal,
                result: operation.result,
                lowering,
            });
        }
        let mut blocks = [None; 4];
        for (ordinal, block) in emission.blocks().enumerate() {
            require(
                ordinal < self.block_count && block.label.0 == self.blocks[ordinal].label,
                "exact emitted block correspondence",
            )?;
            let terminal =
                self.blocks[ordinal].terminator == PlanTerminator::GuardedStoreOutputAndEnd;
            blocks[ordinal] = Some(Gfx942CompleteBodyCanonicalBlockCorrespondenceV19 {
                block: BlockId(ordinal as u32),
                authored_label: block.label.0,
                assembly_label_line: block.label_line,
                terminator_first_assembly_line: if terminal {
                    tail + 8
                } else {
                    block.terminator_line
                },
                terminator_assembly_line_count: if terminal {
                    1
                } else {
                    block.terminator_line_count
                },
            });
        }
        require(
            blocks.iter().flatten().count() == self.block_count,
            "emitted block census",
        )?;
        Ok((operations, blocks))
    }
}
