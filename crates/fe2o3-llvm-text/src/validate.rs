use alloc::{vec, vec::Vec};

use fe2o3_llvm_handoff::{
    BlockIdV2, CallTargetV2, FunctionKindV2, FunctionV2, Gfx942HandoffV2, InstructionKindV2,
    NamedMetadataV1, TerminatorV2, ValueIdV2, ValueTypeV2,
};

use crate::{LlvmNamedMetadataV2, SerializeErrorV2};

pub(crate) fn admit(handoff: &Gfx942HandoffV2) -> Result<(), SerializeErrorV2> {
    let module = handoff.module();
    if module
        .globals()
        .iter()
        .any(|global| reserved_symbol(global.symbol()))
        || module
            .functions()
            .iter()
            .any(|function| reserved_symbol(function.symbol()))
    {
        return Err(SerializeErrorV2::ReservedLlvmSymbol);
    }
    validate_named_metadata(module.named_metadata())?;

    for function in module.functions() {
        validate_emitted_local_names(function)?;
        let cfg = FunctionCfg::new(function)?;
        validate_function_ssa(function, &cfg)?;
        for block in function.blocks() {
            for instruction in block.instructions() {
                match instruction.kind() {
                    InstructionKindV2::GetElementPtr { base, indices }
                        if indices.len() != 1
                            && !(indices.len() == 2
                                && matches!(
                                    function_value_type(function, *base),
                                    Some(ValueTypeV2::ArrayPointer { .. })
                                )) =>
                    {
                        return Err(SerializeErrorV2::UnsupportedGetElementPtr {
                            function: function.id(),
                            indices: indices.len(),
                        });
                    }
                    InstructionKindV2::Call {
                        target: CallTargetV2::Function(callee),
                        ..
                    } if module.functions().iter().any(|target| {
                        target.id() == *callee && target.kind() == FunctionKindV2::Kernel
                    }) =>
                    {
                        return Err(SerializeErrorV2::KernelCall {
                            caller: function.id(),
                            callee: *callee,
                        });
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn function_value_type(function: &FunctionV2, value: ValueIdV2) -> Option<ValueTypeV2> {
    function
        .parameters()
        .iter()
        .find_map(|parameter| {
            (parameter.value().id() == value).then_some(parameter.value().value_type())
        })
        .or_else(|| {
            function.blocks().iter().find_map(|block| {
                block.instructions().iter().find_map(|instruction| {
                    instruction
                        .result()
                        .filter(|result| result.id() == value)
                        .map(|result| result.value_type())
                })
            })
        })
}

fn reserved_symbol(symbol: &str) -> bool {
    symbol.starts_with("llvm.")
}

fn validate_emitted_local_names(function: &FunctionV2) -> Result<(), SerializeErrorV2> {
    for parameter in function.parameters() {
        let name = parameter.name();
        let conflicts_with_block = function
            .blocks()
            .iter()
            .any(|block| generated_name_matches(name, "bb", block.id().get()));
        let conflicts_with_value = function.blocks().iter().any(|block| {
            block.instructions().iter().any(|instruction| {
                instruction.result().is_some_and(|result| {
                    !matches!(
                        instruction.kind(),
                        InstructionKindV2::Constant(_)
                            | InstructionKindV2::VectorZero { .. }
                            | InstructionKindV2::GlobalAddress(_)
                    ) && generated_name_matches(name, "v", result.id().get())
                })
            })
        });
        if conflicts_with_block || conflicts_with_value {
            return Err(SerializeErrorV2::ConflictingEmittedLocalName {
                function: function.id(),
            });
        }
    }
    Ok(())
}

fn generated_name_matches(name: &str, prefix: &str, id: u32) -> bool {
    let Some(decimal) = name.strip_prefix(prefix) else {
        return false;
    };
    if decimal.is_empty() || (decimal.len() > 1 && decimal.starts_with('0')) {
        return false;
    }
    decimal.parse::<u32>() == Ok(id)
}

fn validate_named_metadata(metadata: &[NamedMetadataV1]) -> Result<(), SerializeErrorV2> {
    let mut emitted = metadata
        .iter()
        .copied()
        .map(emitted_metadata_name)
        .collect::<Vec<_>>();
    emitted.sort_unstable();
    if let Some(pair) = emitted.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(SerializeErrorV2::DuplicateEmittedNamedMetadata { metadata: pair[0] });
    }
    Ok(())
}

const fn emitted_metadata_name(metadata: NamedMetadataV1) -> LlvmNamedMetadataV2 {
    match metadata {
        NamedMetadataV1::OpenClVersion2_0 => LlvmNamedMetadataV2::OpenClOclVersion,
        NamedMetadataV1::OpenClSpirVersion2_0 => LlvmNamedMetadataV2::OpenClSpirVersion,
        NamedMetadataV1::ProducerIdentity(_) => LlvmNamedMetadataV2::LlvmIdent,
    }
}

struct FunctionCfg {
    block_ids: Vec<BlockIdV2>,
    dominators: Vec<Vec<u64>>,
    predecessors: Vec<Vec<usize>>,
}

impl FunctionCfg {
    fn new(function: &FunctionV2) -> Result<Self, SerializeErrorV2> {
        let block_ids = function
            .blocks()
            .iter()
            .map(|block| block.id())
            .collect::<Vec<_>>();
        let entry = block_index(&block_ids, function.entry())?;
        let mut successors = vec![Vec::new(); block_ids.len()];
        for (index, block) in function.blocks().iter().enumerate() {
            match block.terminator() {
                TerminatorV2::Branch(target) => {
                    successors[index].push(block_index(&block_ids, *target)?);
                }
                TerminatorV2::ConditionalBranch {
                    then_block,
                    else_block,
                    ..
                } => {
                    successors[index].push(block_index(&block_ids, *then_block)?);
                    let else_index = block_index(&block_ids, *else_block)?;
                    if !successors[index].contains(&else_index) {
                        successors[index].push(else_index);
                    }
                }
                TerminatorV2::Return(_) | TerminatorV2::Unreachable => {}
            }
        }
        if let Some((predecessor, _)) = successors
            .iter()
            .enumerate()
            .find(|(_, targets)| targets.contains(&entry))
        {
            return Err(SerializeErrorV2::EntryBlockHasPredecessor {
                function: function.id(),
                predecessor: block_ids[predecessor],
            });
        }

        let mut reachable = vec![false; block_ids.len()];
        let mut worklist = Vec::with_capacity(block_ids.len());
        reachable[entry] = true;
        worklist.push(entry);
        let mut cursor = 0;
        while cursor < worklist.len() {
            let block = worklist[cursor];
            cursor += 1;
            for successor in &successors[block] {
                if !reachable[*successor] {
                    reachable[*successor] = true;
                    worklist.push(*successor);
                }
            }
        }
        if let Some(index) = reachable.iter().position(|reachable| !reachable) {
            return Err(SerializeErrorV2::UnreachableBlock {
                function: function.id(),
                block: block_ids[index],
            });
        }

        let mut predecessors = vec![Vec::new(); block_ids.len()];
        for (block, targets) in successors.iter().enumerate() {
            for target in targets {
                predecessors[*target].push(block);
            }
        }
        let words = block_ids.len().div_ceil(u64::BITS as usize);
        let mut dominators = vec![vec![u64::MAX; words]; block_ids.len()];
        dominators[entry].fill(0);
        set_bit(&mut dominators[entry], entry);
        loop {
            let mut changed = false;
            for block in 0..block_ids.len() {
                if block == entry {
                    continue;
                }
                let mut updated = vec![u64::MAX; words];
                let mut predecessor_iter = predecessors[block].iter();
                let first = predecessor_iter
                    .next()
                    .ok_or(SerializeErrorV2::InconsistentValidatedModel)?;
                updated.copy_from_slice(&dominators[*first]);
                for predecessor in predecessor_iter {
                    for (word, predecessor_word) in
                        updated.iter_mut().zip(&dominators[*predecessor])
                    {
                        *word &= predecessor_word;
                    }
                }
                set_bit(&mut updated, block);
                if updated != dominators[block] {
                    dominators[block] = updated;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        Ok(Self {
            block_ids,
            dominators,
            predecessors,
        })
    }

    fn block_index(&self, block: BlockIdV2) -> Result<usize, SerializeErrorV2> {
        block_index(&self.block_ids, block)
    }

    fn dominates(&self, definition: usize, use_block: usize) -> bool {
        let word = definition / u64::BITS as usize;
        let bit = definition % u64::BITS as usize;
        self.dominators[use_block][word] & (1_u64 << bit) != 0
    }
}

fn block_index(blocks: &[BlockIdV2], block: BlockIdV2) -> Result<usize, SerializeErrorV2> {
    blocks
        .binary_search(&block)
        .map_err(|_| SerializeErrorV2::InconsistentValidatedModel)
}

fn set_bit(words: &mut [u64], bit: usize) {
    words[bit / u64::BITS as usize] |= 1_u64 << (bit % u64::BITS as usize);
}

#[derive(Clone, Copy)]
enum DefinitionSite {
    Parameter,
    Instruction { block: usize, instruction: usize },
}

#[derive(Clone, Copy)]
struct Definition {
    value: ValueIdV2,
    site: DefinitionSite,
}

fn validate_function_ssa(function: &FunctionV2, cfg: &FunctionCfg) -> Result<(), SerializeErrorV2> {
    let mut definitions = function
        .parameters()
        .iter()
        .map(|parameter| Definition {
            value: parameter.value().id(),
            site: DefinitionSite::Parameter,
        })
        .collect::<Vec<_>>();
    for block in function.blocks() {
        let block_index = cfg.block_index(block.id())?;
        for (instruction_index, instruction) in block.instructions().iter().enumerate() {
            if let Some(result) = instruction.result() {
                definitions.push(Definition {
                    value: result.id(),
                    site: DefinitionSite::Instruction {
                        block: block_index,
                        instruction: instruction_index,
                    },
                });
            }
        }
    }
    definitions.sort_unstable_by_key(|definition| definition.value);
    if definitions
        .windows(2)
        .any(|pair| pair[0].value == pair[1].value)
    {
        return Err(SerializeErrorV2::InconsistentValidatedModel);
    }

    for block in function.blocks() {
        let use_block = cfg.block_index(block.id())?;
        let mut saw_non_phi = false;
        for (instruction_index, instruction) in block.instructions().iter().enumerate() {
            if matches!(instruction.kind(), InstructionKindV2::Phi { .. }) {
                if saw_non_phi {
                    return Err(SerializeErrorV2::InconsistentValidatedModel);
                }
            } else {
                saw_non_phi = true;
            }
            validate_instruction_uses(
                function,
                cfg,
                &definitions,
                instruction.kind(),
                use_block,
                instruction_index,
            )?;
        }
        let terminator_position = block.instructions().len();
        match block.terminator() {
            TerminatorV2::Return(Some(value)) => validate_use(
                function,
                cfg,
                &definitions,
                *value,
                use_block,
                terminator_position,
            )?,
            TerminatorV2::ConditionalBranch { condition, .. } => validate_use(
                function,
                cfg,
                &definitions,
                *condition,
                use_block,
                terminator_position,
            )?,
            TerminatorV2::Return(None) | TerminatorV2::Branch(_) | TerminatorV2::Unreachable => {}
        }
    }
    Ok(())
}

fn validate_instruction_uses(
    function: &FunctionV2,
    cfg: &FunctionCfg,
    definitions: &[Definition],
    instruction: &InstructionKindV2,
    use_block: usize,
    use_position: usize,
) -> Result<(), SerializeErrorV2> {
    let validate = |value| validate_use(function, cfg, definitions, value, use_block, use_position);
    match instruction {
        InstructionKindV2::Constant(_)
        | InstructionKindV2::VectorZero { .. }
        | InstructionKindV2::GlobalAddress(_) => Ok(()),
        InstructionKindV2::Binary { left, right, .. }
        | InstructionKindV2::Compare { left, right, .. } => {
            validate(*left)?;
            validate(*right)
        }
        InstructionKindV2::Cast { value, .. } => validate(*value),
        InstructionKindV2::GetElementPtr { base, indices } => {
            validate(*base)?;
            for index in indices {
                validate(*index)?;
            }
            Ok(())
        }
        InstructionKindV2::Load { pointer, .. } => validate(*pointer),
        InstructionKindV2::VectorLoad4 { pointer, .. } => validate(*pointer),
        InstructionKindV2::Store { pointer, value, .. } => {
            validate(*pointer)?;
            validate(*value)
        }
        InstructionKindV2::Call { arguments, .. } => {
            for argument in arguments {
                validate(*argument)?;
            }
            Ok(())
        }
        InstructionKindV2::Phi { incoming } => {
            let mut incoming_blocks = incoming
                .iter()
                .map(|(_, block)| cfg.block_index(*block))
                .collect::<Result<Vec<_>, _>>()?;
            incoming_blocks.sort_unstable();
            incoming_blocks.dedup();
            let mut expected = cfg.predecessors[use_block].clone();
            expected.sort_unstable();
            if incoming_blocks != expected || incoming.len() != expected.len() {
                return Err(SerializeErrorV2::InconsistentValidatedModel);
            }
            for (value, predecessor) in incoming {
                validate_phi_use(cfg, definitions, *value, cfg.block_index(*predecessor)?)?;
            }
            Ok(())
        }
        InstructionKindV2::InsertElement {
            vector,
            element,
            index,
        } => {
            validate(*vector)?;
            validate(*element)?;
            validate(*index)
        }
        InstructionKindV2::ExtractElement { vector, index } => {
            validate(*vector)?;
            validate(*index)
        }
    }
}

fn validate_phi_use(
    cfg: &FunctionCfg,
    definitions: &[Definition],
    value: ValueIdV2,
    predecessor: usize,
) -> Result<(), SerializeErrorV2> {
    let definition = definitions
        .binary_search_by_key(&value, |definition| definition.value)
        .ok()
        .map(|index| definitions[index])
        .ok_or(SerializeErrorV2::InconsistentValidatedModel)?;
    match definition.site {
        DefinitionSite::Parameter => Ok(()),
        DefinitionSite::Instruction { block, .. }
            if block == predecessor || cfg.dominates(block, predecessor) =>
        {
            Ok(())
        }
        DefinitionSite::Instruction { .. } => Err(SerializeErrorV2::InconsistentValidatedModel),
    }
}

fn validate_use(
    function: &FunctionV2,
    cfg: &FunctionCfg,
    definitions: &[Definition],
    value: ValueIdV2,
    use_block: usize,
    use_position: usize,
) -> Result<(), SerializeErrorV2> {
    let definition = definitions
        .binary_search_by_key(&value, |definition| definition.value)
        .ok()
        .map(|index| definitions[index])
        .ok_or(SerializeErrorV2::MissingSsaDefinition {
            function: function.id(),
            value,
            use_block: cfg.block_ids[use_block],
        })?;
    match definition.site {
        DefinitionSite::Parameter => Ok(()),
        DefinitionSite::Instruction { block, instruction }
            if block == use_block && instruction >= use_position =>
        {
            Err(SerializeErrorV2::SsaUseBeforeDefinition {
                function: function.id(),
                value,
                block: cfg.block_ids[use_block],
            })
        }
        DefinitionSite::Instruction { block, .. } if block != use_block => {
            if cfg.dominates(block, use_block) {
                Ok(())
            } else {
                Err(SerializeErrorV2::SsaDefinitionDoesNotDominate {
                    function: function.id(),
                    value,
                    definition_block: cfg.block_ids[block],
                    use_block: cfg.block_ids[use_block],
                })
            }
        }
        DefinitionSite::Instruction { .. } => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use fe2o3_llvm_handoff::{
        BasicBlockV2, BinaryOperationV2, CallingConventionV2, EvidenceV2, FunctionAttributeV2,
        FunctionIdV2, FunctionKindV2, FunctionParameterV2, IdentityV1, InstructionV2,
        IntegerBinaryOperationV2, IntrinsicV2, NamedMetadataV1, OriginKindV1, OriginV1,
        ReturnTypeV2, ScalarConstantV2, ScalarTypeV1, TypedValueV2, ValueTypeV2,
    };

    use super::*;

    fn evidence() -> EvidenceV2 {
        let origin = OriginV1::new(
            OriginKindV1::AmdgcnIr,
            IdentityV1::new([0x31; 32]).unwrap(),
            None,
        );
        EvidenceV2::new(origin.identity(), vec![]).unwrap()
    }

    fn instruction(result: Option<TypedValueV2>, kind: InstructionKindV2) -> InstructionV2 {
        InstructionV2::new(result, kind, evidence()).unwrap()
    }

    fn function(parameters: Vec<FunctionParameterV2>, blocks: Vec<BasicBlockV2>) -> FunctionV2 {
        FunctionV2::new(
            FunctionIdV2::new(7),
            "helper",
            FunctionKindV2::Helper,
            CallingConventionV2::C,
            ReturnTypeV2::Void,
            parameters,
            vec![FunctionAttributeV2::NoUnwind],
            BlockIdV2::new(0),
            blocks,
            evidence(),
        )
        .unwrap()
    }

    fn i32_value(id: u32) -> TypedValueV2 {
        TypedValueV2::new(ValueIdV2::new(id), ValueTypeV2::Scalar(ScalarTypeV1::I32))
    }

    fn constant_pointer(id: u32) -> TypedValueV2 {
        TypedValueV2::new(
            ValueIdV2::new(id),
            ValueTypeV2::Pointer {
                pointee: ScalarTypeV1::F32,
                address_space: fe2o3_llvm_handoff::AddressSpaceV1::Constant,
            },
        )
    }

    fn i1_parameter(id: u32) -> FunctionParameterV2 {
        FunctionParameterV2::new(
            TypedValueV2::new(ValueIdV2::new(id), ValueTypeV2::Scalar(ScalarTypeV1::I1)),
            "condition",
            vec![],
        )
        .unwrap()
    }

    #[test]
    fn sibling_branch_alias_definition_does_not_dominate_use() {
        let function = function(
            vec![i1_parameter(1)],
            vec![
                BasicBlockV2::new(
                    BlockIdV2::new(0),
                    vec![],
                    TerminatorV2::ConditionalBranch {
                        condition: ValueIdV2::new(1),
                        then_block: BlockIdV2::new(1),
                        else_block: BlockIdV2::new(2),
                    },
                ),
                BasicBlockV2::new(
                    BlockIdV2::new(1),
                    vec![instruction(
                        Some(i32_value(2)),
                        InstructionKindV2::Constant(
                            ScalarConstantV2::new(ScalarTypeV1::I32, 1).unwrap(),
                        ),
                    )],
                    TerminatorV2::Branch(BlockIdV2::new(3)),
                ),
                BasicBlockV2::new(
                    BlockIdV2::new(2),
                    vec![instruction(
                        Some(i32_value(3)),
                        InstructionKindV2::Binary {
                            operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
                            left: ValueIdV2::new(2),
                            right: ValueIdV2::new(2),
                        },
                    )],
                    TerminatorV2::Branch(BlockIdV2::new(3)),
                ),
                BasicBlockV2::new(BlockIdV2::new(3), vec![], TerminatorV2::Return(None)),
            ],
        );
        let cfg = FunctionCfg::new(&function).unwrap();

        assert_eq!(
            validate_function_ssa(&function, &cfg),
            Err(SerializeErrorV2::SsaDefinitionDoesNotDominate {
                function: FunctionIdV2::new(7),
                value: ValueIdV2::new(2),
                definition_block: BlockIdV2::new(1),
                use_block: BlockIdV2::new(2),
            })
        );
    }

    #[test]
    fn entry_alias_use_before_definition_is_rejected() {
        let function = function(
            vec![],
            vec![BasicBlockV2::new(
                BlockIdV2::new(0),
                vec![
                    instruction(
                        Some(i32_value(3)),
                        InstructionKindV2::Binary {
                            operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
                            left: ValueIdV2::new(2),
                            right: ValueIdV2::new(2),
                        },
                    ),
                    instruction(
                        Some(i32_value(2)),
                        InstructionKindV2::Constant(
                            ScalarConstantV2::new(ScalarTypeV1::I32, 1).unwrap(),
                        ),
                    ),
                ],
                TerminatorV2::Return(None),
            )],
        );
        let cfg = FunctionCfg::new(&function).unwrap();

        assert_eq!(
            validate_function_ssa(&function, &cfg),
            Err(SerializeErrorV2::SsaUseBeforeDefinition {
                function: FunctionIdV2::new(7),
                value: ValueIdV2::new(2),
                block: BlockIdV2::new(0),
            })
        );
    }

    #[test]
    fn sibling_global_address_alias_does_not_dominate_load() {
        let function = function(
            vec![i1_parameter(1)],
            vec![
                BasicBlockV2::new(
                    BlockIdV2::new(0),
                    vec![],
                    TerminatorV2::ConditionalBranch {
                        condition: ValueIdV2::new(1),
                        then_block: BlockIdV2::new(1),
                        else_block: BlockIdV2::new(2),
                    },
                ),
                BasicBlockV2::new(
                    BlockIdV2::new(1),
                    vec![instruction(
                        Some(constant_pointer(2)),
                        InstructionKindV2::GlobalAddress(fe2o3_llvm_handoff::GlobalIdV2::new(9)),
                    )],
                    TerminatorV2::Branch(BlockIdV2::new(3)),
                ),
                BasicBlockV2::new(
                    BlockIdV2::new(2),
                    vec![instruction(
                        Some(TypedValueV2::new(
                            ValueIdV2::new(3),
                            ValueTypeV2::Scalar(ScalarTypeV1::F32),
                        )),
                        InstructionKindV2::Load {
                            pointer: ValueIdV2::new(2),
                            value_type: ScalarTypeV1::F32,
                            alignment: 4,
                        },
                    )],
                    TerminatorV2::Branch(BlockIdV2::new(3)),
                ),
                BasicBlockV2::new(BlockIdV2::new(3), vec![], TerminatorV2::Return(None)),
            ],
        );
        let cfg = FunctionCfg::new(&function).unwrap();

        assert_eq!(
            validate_function_ssa(&function, &cfg),
            Err(SerializeErrorV2::SsaDefinitionDoesNotDominate {
                function: FunctionIdV2::new(7),
                value: ValueIdV2::new(2),
                definition_block: BlockIdV2::new(1),
                use_block: BlockIdV2::new(2),
            })
        );
    }

    #[test]
    fn missing_ssa_definition_has_a_typed_error() {
        let function = function(
            vec![],
            vec![BasicBlockV2::new(
                BlockIdV2::new(0),
                vec![instruction(
                    Some(i32_value(3)),
                    InstructionKindV2::Call {
                        target: CallTargetV2::Intrinsic(IntrinsicV2::SqrtF32),
                        arguments: vec![ValueIdV2::new(99)],
                    },
                )],
                TerminatorV2::Return(None),
            )],
        );
        let cfg = FunctionCfg::new(&function).unwrap();

        assert_eq!(
            validate_function_ssa(&function, &cfg),
            Err(SerializeErrorV2::MissingSsaDefinition {
                function: FunctionIdV2::new(7),
                value: ValueIdV2::new(99),
                use_block: BlockIdV2::new(0),
            })
        );
    }

    #[test]
    fn duplicate_producer_identities_collide_on_llvm_ident() {
        let first = IdentityV1::new([1; 32]).unwrap();
        let second = IdentityV1::new([2; 32]).unwrap();
        assert_eq!(
            validate_named_metadata(&[
                NamedMetadataV1::ProducerIdentity(first),
                NamedMetadataV1::ProducerIdentity(second),
            ]),
            Err(SerializeErrorV2::DuplicateEmittedNamedMetadata {
                metadata: LlvmNamedMetadataV2::LlvmIdent,
            })
        );
    }

    #[test]
    fn parameter_name_cannot_collide_with_generated_ssa_name() {
        let parameter = FunctionParameterV2::new(i32_value(1), "v2", vec![]).unwrap();
        let function = function(
            vec![parameter],
            vec![BasicBlockV2::new(
                BlockIdV2::new(0),
                vec![instruction(
                    Some(i32_value(2)),
                    InstructionKindV2::Binary {
                        operation: BinaryOperationV2::Integer(IntegerBinaryOperationV2::Add),
                        left: ValueIdV2::new(1),
                        right: ValueIdV2::new(1),
                    },
                )],
                TerminatorV2::Return(None),
            )],
        );

        assert_eq!(
            validate_emitted_local_names(&function),
            Err(SerializeErrorV2::ConflictingEmittedLocalName {
                function: FunctionIdV2::new(7),
            })
        );
    }
}
