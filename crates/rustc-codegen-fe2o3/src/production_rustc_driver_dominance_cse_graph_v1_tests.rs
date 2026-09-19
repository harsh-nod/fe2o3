//! Actual B/C/I observations and existing independent whole-prefix replay.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantKindV1 as Descendant,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirOperationCoordinateV1 as Coordinate, CanonicalKirOperationOriginV1 as Origin,
    CanonicalKirTransitionCandidateV1 as Candidate, Function, FunctionRole, LaunchDomain,
    LaunchExtent, Module, OperationKind as Kind, ScalarType, Type, ValueId, WorkgroupSize,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Root {
    pub(super) name: String,
    pub(super) source_function: [u8; 32],
    pub(super) source_body: [u8; 32],
    source_binding: [u8; 32],
    function: String,
    source_binary_count: usize,
    original_binary_count: usize,
    bound_binary_count: usize,
    cse_binary_count: usize,
    final_binary_count: usize,
    anchor: [u32; 3],
    removed: [u32; 3],
    descendant: [u32; 3],
    ordered_global_stores: usize,
}

pub(super) fn roster<'a>(names: impl IntoIterator<Item = &'a str>) -> Result<(), String> {
    let names = names.into_iter().collect::<Vec<_>>();
    if names.len() != ROOTS.len()
        || names.iter().copied().collect::<BTreeSet<_>>() != ROOTS.into_iter().collect()
    {
        return Err("dominance exact unique three-root roster".into());
    }
    Ok(())
}
pub(super) fn order(module: &Module) -> Vec<String> {
    module
        .kernels
        .iter()
        .map(|k| k.id.as_str().to_owned())
        .collect()
}
pub(super) fn operation(ordinal: usize) -> BinaryOp {
    [BinaryOp::BitAnd, BinaryOp::BitOr, BinaryOp::BitXor][ordinal]
}
pub(super) fn kernel<'a>(
    module: &'a Module,
    case: Case,
    root: &str,
) -> Result<(&'a fe2o3_kernel_ir::Kernel, &'a Function, usize), String> {
    let matching = module
        .kernels
        .iter()
        .filter(|k| k.id.as_str() == root)
        .collect::<Vec<_>>();
    let [kernel] = matching.as_slice() else {
        return Err("missing/duplicate dominance kernel".into());
    };
    let matching = module
        .functions
        .iter()
        .enumerate()
        .filter(|(_, f)| f.id == kernel.entry)
        .collect::<Vec<_>>();
    let [(index, function)] = matching.as_slice() else {
        return Err("missing/duplicate physical dominance entry".into());
    };
    let [Type::Slice(slice), lhs, rhs, choose] = function.signature.parameters.as_slice() else {
        return Err("dominance four-argument ABI".into());
    };
    let ty = Type::Scalar(case.integer.scalar());
    if function.role != FunctionRole::KernelEntry
        || function.body.is_none()
        || !function.signature.results.is_empty()
        || slice.address_space != AddressSpace::Global
        || slice.access != AccessMode::ReadWrite
        || slice.element.as_ref() != &ty
        || lhs != &ty
        || rhs != &ty
        || choose != &Type::Scalar(ScalarType::U32)
        || !matches!(
            kernel.domain,
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic
            }
        )
        || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
    {
        return Err("dominance scalar/width/output/dynamic64 contract changed".into());
    }
    Ok((kernel, function, *index))
}

#[derive(Clone, Copy)]
struct Binary {
    coordinate: Coordinate,
    result: ValueId,
}
fn binaries(
    function: &Function,
    index: usize,
    case: Case,
    op: BinaryOp,
) -> Result<Vec<Binary>, String> {
    let body = function.body.as_ref().ok_or("missing defined function")?;
    let [_, lhs, rhs, _] = body.parameters.as_slice() else {
        return Err("four actual function parameter values".into());
    };
    let mut result = Vec::new();
    for (block, value) in body.blocks.iter().enumerate() {
        for (operation, value) in value.operations.iter().enumerate() {
            let Kind::Binary {
                op: actual,
                lhs: a,
                rhs: b,
            } = value.kind
            else {
                continue;
            };
            if !matches!(
                actual,
                BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor
            ) {
                continue;
            }
            if actual != op
                || a != *lhs
                || b != *rhs
                || !matches!(value.results.as_slice(), [v] if v.ty == Type::Scalar(case.integer.scalar()))
            {
                return Err(
                    "dominance requires exact typed ordered dynamic-argument expression".into(),
                );
            }
            result.push(Binary {
                coordinate: Coordinate {
                    block: Block {
                        function: FunctionCoordinate(
                            u32::try_from(index).map_err(|e| e.to_string())?,
                        ),
                        block: u32::try_from(block).map_err(|e| e.to_string())?,
                    },
                    operation: u32::try_from(operation).map_err(|e| e.to_string())?,
                },
                result: value.results[0].id,
            });
        }
    }
    Ok(result)
}
fn stores(function: &Function, values: &[ValueId]) -> Result<usize, String> {
    let mut seen = Vec::new();
    for operation in function
        .body
        .iter()
        .flat_map(|body| &body.blocks)
        .flat_map(|b| &b.operations)
    {
        if let Kind::Store { value, access, .. } = operation.kind
            && access.address_space == AddressSpace::Global
        {
            seen.push(value);
        }
    }
    if seen.len() != 2
        || !seen.iter().all(|id| values.contains(id))
        || values.iter().any(|id| !seen.contains(id))
    {
        return Err("both ordered global Stores must keep the observed expressions live".into());
    }
    Ok(seen.len())
}
fn dominated_pair(function: &Function, pair: &[Binary]) -> Result<(Binary, Binary), String> {
    let [a, b] = pair else {
        return Err("expected two actual B expressions".into());
    };
    if a.coordinate.block == b.coordinate.block {
        return Err("same-block CSE is not cross-block coverage".into());
    }
    let body = function.body.as_ref().ok_or("actual body")?;
    let cfg = fe2o3_kernel_ir::analyze_control_flow(function).map_err(|e| format!("{e:?}"))?;
    let id = |x: Binary| body.blocks[x.coordinate.block.block as usize].id;
    if !cfg.is_reachable(id(*a)) || !cfg.is_reachable(id(*b)) {
        return Err("unreachable CSE candidate".into());
    }
    if cfg.dominates(id(*a), id(*b)) {
        Ok((*a, *b))
    } else if cfg.dominates(id(*b), id(*a)) {
        Ok((*b, *a))
    } else {
        Err("candidate expressions do not dominate one another".into())
    }
}
fn descendant(
    candidate: Candidate<'_>,
    input: Coordinate,
    output: Coordinate,
    expected: Descendant,
) -> Result<(), String> {
    let coordinate = Definition::Result {
        operation: input,
        result: 0,
    };
    let rows = candidate
        .definitions
        .iter()
        .filter(|row| row.input == coordinate)
        .collect::<Vec<_>>();
    let [row] = rows.as_slice() else {
        return Err("exact input definition occurrence".into());
    };
    let start = row.outputs.start as usize;
    let end = start
        .checked_add(row.outputs.len as usize)
        .ok_or("descendant range overflow")?;
    let actual = candidate
        .definition_outputs
        .get(start..end)
        .ok_or("descendant range")?;
    if !matches!(actual, [value] if value.output == (Definition::Result { operation: output, result: 0 }) && value.kind == expected)
    {
        return Err(
            "duplicate and anchor must join the same actual C result with exact descendant kinds"
                .into(),
        );
    }
    Ok(())
}
fn triple(c: Coordinate) -> [u32; 3] {
    [c.block.function.0, c.block.block, c.operation]
}

pub(super) fn observe(stage: &Stage, case: Case) -> Result<Vec<Root>, String> {
    let semantic = stage.semantic();
    let mut sources = BTreeMap::new();
    for root in semantic.roots() {
        let function = &semantic.functions()[root.index() as usize];
        let entry = function.kernel_entry().ok_or("semantic kernel entry")?;
        let name =
            std::str::from_utf8(entry.export_symbol().as_bytes()).map_err(|e| e.to_string())?;
        let selected = semantic
            .select_kernel_body_for_root_v1(*root)
            .filter(|s| s.root() == *root)
            .ok_or("actual selected semantic body")?;
        let body = &semantic.functions()[selected.body().index() as usize];
        if sources.insert(name, (function, entry, body)).is_some() {
            return Err("duplicate semantic root".into());
        }
    }
    roster(sources.keys().copied())?;
    let b = stage.test_bound_owner_v1();
    let p3 = stage
        .checked_output()
        .intermediate_policy5()
        .intermediate_policy4()
        .intermediate_policy3();
    let c = p3.owner();
    if p3.native_input_audit_bytes() != b.canonical().canonical_bytes() {
        return Err("actual P3 input history differs from retained B".into());
    }
    let candidate = p3.occurrences().candidate();
    let original_order = order(stage.original_module());
    roster(original_order.iter().map(String::as_str))?;
    for module in [
        stage.original_module(),
        b.module(),
        c.module(),
        stage.output().module(),
    ] {
        if order(module) != original_order
            || module
                .functions
                .iter()
                .any(|f| f.role == FunctionRole::InternalHelper)
        {
            return Err("dominance direct N/B/C/I root order/helper custody changed".into());
        }
    }
    let mut report = Vec::new();
    for (ordinal, name) in ROOTS.into_iter().enumerate() {
        let (source, entry, body) = sources[name];
        let source_op = [
            SemanticBinaryOpV1::BitAnd,
            SemanticBinaryOpV1::BitOr,
            SemanticBinaryOpV1::BitXor,
        ][ordinal];
        let mut source_count = 0;
        let mut source_blocks = BTreeSet::new();
        for (block_index, block) in body.blocks().iter().enumerate() {
            for statement in block.statements() {
                if let SemanticStatementKindV1::Assign(value) = statement.kind()
                    && let SemanticRvalueKindV1::Binary {
                        operation,
                        left,
                        right,
                    } = value.value().kind()
                    && *operation == source_op
                {
                    let shape = semantic.types()[left.ty().index() as usize].shape();
                    if left.ty() != right.ty()
                        || !matches!(shape, SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }) if *signed == case.integer.signed() && u32::from(*bits) == case.integer.width())
                    {
                        return Err("actual selected source bitwise type mismatch".into());
                    }
                    source_count += 1;
                    source_blocks.insert(block_index);
                }
            }
        }
        if source_count != 2 || source_blocks.len() != 2 {
            return Err("opt0 source must retain two cross-block expressions".into());
        }
        let (_, n_function, ni) = kernel(stage.original_module(), case, name)?;
        let (_, b_function, bi) = kernel(b.module(), case, name)?;
        let (_, c_function, ci) = kernel(c.module(), case, name)?;
        let (_, i_function, ii) = kernel(stage.output().module(), case, name)?;
        if n_function.id != b_function.id
            || b_function.id != c_function.id
            || c_function.id != i_function.id
        {
            return Err("actual expression-owning function changed".into());
        }
        let n_values = binaries(n_function, ni, case, operation(ordinal))?;
        let b_values = binaries(b_function, bi, case, operation(ordinal))?;
        let c_values = binaries(c_function, ci, case, operation(ordinal))?;
        let i_values = binaries(i_function, ii, case, operation(ordinal))?;
        let (anchor, removed) = dominated_pair(b_function, &b_values)?;
        let ([c_value], [i_value]) = (c_values.as_slice(), i_values.as_slice()) else {
            return Err("actual C/I must retain exactly one expression".into());
        };
        if n_values.len() != 2 {
            return Err("actual original N lost the source pair".into());
        }
        descendant(
            candidate,
            anchor.coordinate,
            c_value.coordinate,
            Descendant::Retained,
        )?;
        descendant(
            candidate,
            removed.coordinate,
            c_value.coordinate,
            Descendant::Substituted,
        )?;
        let origin = candidate
            .operations
            .iter()
            .filter(|r| r.output == c_value.coordinate)
            .collect::<Vec<_>>();
        if !matches!(origin.as_slice(), [row] if row.origin == Origin::Retained(anchor.coordinate))
            || candidate
                .operations
                .iter()
                .any(|r| r.origin == Origin::Retained(removed.coordinate))
        {
            return Err("actual C operation was not the retained dominating B expression".into());
        }
        stores(b_function, &[anchor.result, removed.result])?;
        stores(c_function, &[c_value.result])?;
        let ordered_global_stores = stores(i_function, &[i_value.result])?;
        report.push(Root {
            name: name.into(),
            source_function: *source.identity().as_bytes(),
            source_body: *body.identity().as_bytes(),
            source_binding: *entry.kernel_binding_identity().as_bytes(),
            function: i_function.id.as_str().into(),
            source_binary_count: source_count,
            original_binary_count: n_values.len(),
            bound_binary_count: b_values.len(),
            cse_binary_count: c_values.len(),
            final_binary_count: i_values.len(),
            anchor: triple(anchor.coordinate),
            removed: triple(removed.coordinate),
            descendant: triple(c_value.coordinate),
            ordered_global_stores,
        });
    }
    for root in &report {
        validate_root(root)?;
    }
    Ok(report)
}

pub(super) fn replay_prefix(stage: &Stage) -> Result<usize, String> {
    let mut work = Work::new(crate::production_canonical_phase_policy_v1::WORK_LIMIT as usize);
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    let floor = stage.retained_storage_floor_v1();
    budget
        .reserve_storage(floor)
        .map_err(|e| format!("{e:?}"))?;
    let ledger = budget.work_ledger_identity_v1();
    stage
        .checked_output()
        .replay(stage.test_bound_owner_v1(), &mut budget)
        .map_err(|e| format!("whole actual B/C/S/O/I replay: {e:?}"))?;
    if budget.storage() != floor || budget.work_ledger_identity_v1() != ledger {
        return Err("prefix replay changed inherited accounting".into());
    }
    Ok(budget.work())
}
pub(super) fn validate_root(root: &Root) -> Result<(), String> {
    if root.function.is_empty()
        || [root.source_function, root.source_body, root.source_binding].contains(&[0; 32])
        || (
            root.source_binary_count,
            root.original_binary_count,
            root.bound_binary_count,
            root.cse_binary_count,
            root.final_binary_count,
        ) != (2, 2, 2, 1, 1)
        || root.anchor[0] != root.removed[0]
        || root.anchor[1] == root.removed[1]
        || root.ordered_global_stores != 2
    {
        return Err(
            "dominance root lost exact source/cross-block/nonempty substitution evidence".into(),
        );
    }
    Ok(())
}
pub(super) fn descriptor(
    roots: &[Root],
    descriptors: &[fe2o3_kernel_descriptor::KernelDescriptorV1],
) -> Result<(), String> {
    roster(descriptors.iter().map(|r| r.entry_name().as_str()))?;
    for row in descriptors {
        let source = roots
            .iter()
            .find(|r| r.name == row.entry_name().as_str())
            .ok_or("foreign descriptor root")?;
        let fe2o3_kernel_descriptor::BlockSizeV1::Exact(size) = row.launch().block_size() else {
            return Err("exact launch descriptor required".into());
        };
        if row.kernel_id().as_bytes() != &source.source_binding
            || row.launch().rank() != 1
            || [size.x(), size.y(), size.z()] != [64, 1, 1]
            || row.launch().max_flat_workgroup_size() != 64
        {
            return Err("descriptor source binding/launch changed".into());
        }
    }
    Ok(())
}
fn native_body<'a>(llvm: &'a str, symbol: &str) -> Result<&'a str, String> {
    let marker = format!("@{symbol}(");
    let starts = llvm
        .match_indices("define ")
        .filter_map(|(index, _)| {
            llvm[index..]
                .lines()
                .next()
                .is_some_and(|line| line.contains(&marker))
                .then_some(index)
        })
        .collect::<Vec<_>>();
    let [start] = starts.as_slice() else {
        return Err("missing/duplicate exact native entry".into());
    };
    let text = &llvm[*start..];
    Ok(&text[..text.find("\n}").ok_or("unclosed native entry")?])
}
pub(super) fn native(case: Case, llvm: &str) -> Result<(), String> {
    for (index, root) in ROOTS.into_iter().enumerate() {
        let body = native_body(llvm, root)?;
        let opcode = ["and", "or", "xor"][index];
        let prefix = format!("{opcode} i{} ", case.integer.width());
        let instructions = body
            .lines()
            .filter_map(|line| line.split_once(" = ").map(|(_, rhs)| rhs))
            .filter(|rhs| ["and ", "or ", "xor "].iter().any(|op| rhs.starts_with(op)))
            .collect::<Vec<_>>();
        let [instruction] = instructions.as_slice() else {
            return Err("native exact one bitwise instruction per owning entry".into());
        };
        let Some(operands) = instruction.strip_prefix(&prefix) else {
            return Err("native bitwise opcode/width/flags mismatch".into());
        };
        let Some((a, b)) = operands.split_once(", ") else {
            return Err("native binary operands".into());
        };
        if !a.starts_with('%') || !b.starts_with('%') || a == b {
            return Err("native dynamic distinct operands required".into());
        }
    }
    Ok(())
}

#[test]
fn dominance_roster_and_native_oracles_refuse_missing_duplicate_wrong_width_or_expression() {
    roster(ROOTS.into_iter().rev()).unwrap();
    assert!(roster([ROOTS[0], ROOTS[0], ROOTS[2]]).is_err());
    assert!(roster(ROOTS[..2].iter().copied()).is_err());
    assert!(roster([ROOTS[0], ROOTS[1], "foreign"]).is_err());
    for integer in Integer::ALL {
        let case = Case {
            integer,
            target: Target::Gfx942,
        };
        let text = ROOTS
            .into_iter()
            .zip(["and", "or", "xor"])
            .map(|(root, op)| {
                format!(
                    "define void @{root}() {{\n %r = {op} i{} %lhs, %rhs\n}}\n",
                    integer.width()
                )
            })
            .collect::<String>();
        native(case, &text).unwrap();
        for changed in [
            text.replace("%rhs", "0"),
            text.replace("%rhs", "%lhs"),
            text.replace("and i", "and exact i"),
            text.replace("or i", "xor i"),
            text.replace(&format!("i{} ", integer.width()), "i128 "),
            format!("{text}{text}"),
        ] {
            assert!(native(case, &changed).is_err());
        }
    }
}

#[test]
fn dominance_graph_oracle_requires_dynamic_ordered_pair_in_reachable_dominating_blocks() {
    use fe2o3_kernel_ir::{BasicBlock, BlockId, Operation, Signature, Terminator, ValueDef};
    for integer in Integer::ALL {
        let case = Case {
            integer,
            target: Target::Gfx942,
        };
        let binary = |id| {
            Operation::new(
                vec![ValueDef::new(ValueId(id), Type::Scalar(integer.scalar()))],
                Kind::Binary {
                    op: BinaryOp::BitXor,
                    lhs: ValueId(1),
                    rhs: ValueId(2),
                },
            )
        };
        let mut first = BasicBlock::new(BlockId(10));
        first.operations.push(binary(10));
        first.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(3),
            then_target: BlockId(20),
            then_arguments: vec![],
            else_target: BlockId(30),
            else_arguments: vec![],
        });
        let mut second = BasicBlock::new(BlockId(20));
        second.operations.push(binary(11));
        second.terminator = Some(Terminator::Branch {
            target: BlockId(30),
            arguments: vec![],
        });
        let mut exit = BasicBlock::new(BlockId(30));
        exit.terminator = Some(Terminator::Return { values: vec![] });
        // Only the bounded observation helper is tested here, not source or IR admission.
        let function = Function::kernel_entry(
            "test",
            Signature::new(vec![Type::Scalar(integer.scalar()); 4], vec![]),
            (0..4).map(ValueId).collect(),
            vec![first, second, exit],
        );
        let pair = binaries(&function, 0, case, BinaryOp::BitXor).unwrap();
        let (anchor, removed) = dominated_pair(&function, &pair).unwrap();
        assert_ne!(anchor.coordinate.block, removed.coordinate.block);
        let mut same = pair.clone();
        same[1].coordinate.block = same[0].coordinate.block;
        assert!(dominated_pair(&function, &same).is_err());
        let mut swapped = function.clone();
        if let Kind::Binary { lhs, rhs, .. } =
            &mut swapped.body.as_mut().unwrap().blocks[1].operations[0].kind
        {
            std::mem::swap(lhs, rhs);
        }
        assert!(binaries(&swapped, 0, case, BinaryOp::BitXor).is_err());
        let mut sibling = function.clone();
        let body = sibling.body.as_mut().unwrap();
        let anchor = body.blocks[0].operations.remove(0);
        body.blocks[2].operations.push(anchor);
        body.blocks[1].terminator = Some(Terminator::Return { values: vec![] });
        let pair = binaries(&sibling, 0, case, BinaryOp::BitXor).unwrap();
        assert!(dominated_pair(&sibling, &pair).is_err());
        let mut dead = function.clone();
        dead.body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
            target: BlockId(30),
            arguments: vec![],
        });
        let pair = binaries(&dead, 0, case, BinaryOp::BitXor).unwrap();
        assert!(dominated_pair(&dead, &pair).is_err());
    }
}

#[test]
fn dominance_descendant_join_refuses_missing_duplicate_wrong_kind_or_foreign_output() {
    use fe2o3_kernel_ir::{
        CanonicalKirDefinitionDescendantV1 as Output, CanonicalKirDefinitionTransitionV1 as Row,
        CanonicalKirTransitionRangeV1 as Range,
    };
    let coordinate = |block| Coordinate {
        block: Block {
            function: FunctionCoordinate(0),
            block,
        },
        operation: 0,
    };
    let input = coordinate(0);
    let output = coordinate(1);
    let rows = [Row {
        input: Definition::Result {
            operation: input,
            result: 0,
        },
        outputs: Range { start: 0, len: 1 },
    }];
    let outputs = [Output {
        output: Definition::Result {
            operation: output,
            result: 0,
        },
        kind: Descendant::Substituted,
    }];
    let candidate = Candidate {
        functions: &[],
        blocks: &[],
        segments: &[],
        operations: &[],
        definitions: &rows,
        definition_outputs: &outputs,
        uses: &[],
        edges: &[],
        edge_arguments: &[],
    };
    descendant(candidate, input, output, Descendant::Substituted).unwrap();
    assert!(descendant(candidate, input, output, Descendant::Retained).is_err());
    assert!(descendant(candidate, input, coordinate(3), Descendant::Substituted).is_err());
    assert!(
        descendant(
            Candidate {
                definitions: &[],
                ..candidate
            },
            input,
            output,
            Descendant::Substituted
        )
        .is_err()
    );
    let duplicate = [rows[0], rows[0]];
    assert!(
        descendant(
            Candidate {
                definitions: &duplicate,
                ..candidate
            },
            input,
            output,
            Descendant::Substituted
        )
        .is_err()
    );
    let out_of_bounds = [Row {
        outputs: Range {
            start: u32::MAX,
            len: 1,
        },
        ..rows[0]
    }];
    assert!(
        descendant(
            Candidate {
                definitions: &out_of_bounds,
                ..candidate
            },
            input,
            output,
            Descendant::Substituted
        )
        .is_err()
    );
}
