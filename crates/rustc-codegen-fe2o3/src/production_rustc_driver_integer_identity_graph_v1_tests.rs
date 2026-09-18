//! Observations of genuine source N, retained O and final I, never substitutes.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, Constant, Function, FunctionRole, LaunchDomain,
    LaunchExtent, Module, OperationKind as Kind, Type, ValueId, WorkgroupSize,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Root {
    pub(super) root: String,
    source_function: [u8; 32],
    source_body: [u8; 32],
    source_binding: [u8; 32],
    original_entry: String,
    original_owner: String,
    output_entry: String,
    output_owner: String,
    native_symbol: String,
    pub(super) original_binary_count: usize,
    pub(super) before_binary_count: usize,
    pub(super) after_binary_count: usize,
}

pub(super) fn census_roots(roots: &[Root]) -> Vec<census::SourceRoot> {
    roots
        .iter()
        .map(|root| census::SourceRoot {
            name: root.root.clone(),
            function: root.source_function,
            body: root.source_body,
        })
        .collect()
}

fn operations(function: &Function) -> impl Iterator<Item = &fe2o3_kernel_ir::Operation> {
    function
        .body
        .iter()
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
}

pub(super) fn exact_roster(roots: &[String]) -> Result<(), String> {
    let unique = roots.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if roots.len() != ROOTS.len() || unique != ROOTS.into_iter().collect() {
        return Err("integer identity missing, duplicate, extra or foreign root".into());
    }
    Ok(())
}

pub(super) fn root_order(module: &Module) -> Vec<String> {
    module
        .kernels
        .iter()
        .map(|kernel| kernel.id.as_str().into())
        .collect()
}

pub(super) fn validate_roots(case: Case, roots: &[Root]) -> Result<(), String> {
    exact_roster(
        &roots
            .iter()
            .map(|root| root.root.clone())
            .collect::<Vec<_>>(),
    )?;
    for root in roots {
        let symbol = if case.batch.retained {
            &root.output_owner
        } else {
            &root.root
        };
        if root.source_function == [0; 32]
            || root.source_body == [0; 32]
            || root.source_binding == [0; 32]
            || root.original_entry.is_empty()
            || root.output_entry != root.original_entry
            || root.original_owner.is_empty()
            || root.output_owner != root.original_owner
            || &root.native_symbol != symbol
        {
            return Err("identity root source/body/binding/native-owner evidence changed".into());
        }
    }
    Ok(())
}

fn owner<'a>(module: &'a Module, case: Case, root: &str) -> Result<&'a Function, String> {
    let kernel = simulation::integer_identity::kernel_for_root(module, root)
        .map_err(|e| format!("{e:?}"))?;
    let entry = module
        .function(&kernel.entry)
        .ok_or("missing actual root entry")?;
    let [Type::Slice(output), scalar] = entry.signature.parameters.as_slice() else {
        return Err("identity root output/scalar ABI".into());
    };
    let scalar_type = Type::Scalar(case.batch.integer.scalar());
    if entry.role != FunctionRole::KernelEntry
        || !entry.signature.results.is_empty()
        || entry.body.is_none()
        || output.address_space != AddressSpace::Global
        || output.access != AccessMode::ReadWrite
        || output.element.as_ref() != &scalar_type
        || scalar != &scalar_type
        || !matches!(
            kernel.domain,
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic
            }
        )
        || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
    {
        return Err("identity exact signed-width ABI or dynamic64 launch changed".into());
    }
    let calls = operations(entry)
        .filter_map(|operation| match &operation.kind {
            Kind::Call { callee, arguments } => module
                .function(callee)
                .filter(|function| function.role == FunctionRole::InternalHelper)
                .map(|function| (function, arguments, &operation.results)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !case.batch.retained {
        if !calls.is_empty() {
            return Err("direct identity unexpectedly retained a helper".into());
        }
        return Ok(entry);
    }
    let [(helper, arguments, results)] = calls.as_slice() else {
        return Err("identity requires one actual retained helper call per root".into());
    };
    if arguments.len() != 1
        || !matches!(results.as_slice(), [value] if value.ty == scalar_type)
        || helper.signature.parameters != [scalar_type.clone()]
        || helper.signature.results != [scalar_type]
        || helper.body.is_none()
    {
        return Err("identity exact retained-helper scalar ABI".into());
    }
    Ok(helper)
}

pub(super) fn check_descriptor_bindings(
    roots: &[Root],
    descriptor: &[fe2o3_kernel_descriptor::KernelDescriptorV1],
) -> Result<(), String> {
    exact_roster(
        &descriptor
            .iter()
            .map(|row| row.entry_name().as_str().to_owned())
            .collect::<Vec<_>>(),
    )?;
    let by_name = roots
        .iter()
        .map(|root| (root.root.as_str(), root))
        .collect::<BTreeMap<_, _>>();
    for row in descriptor {
        let root = by_name
            .get(row.entry_name().as_str())
            .ok_or("foreign descriptor root")?;
        if row.kernel_id().as_bytes() != &root.source_binding {
            return Err("descriptor replaced exact source kernel binding".into());
        }
    }
    Ok(())
}

fn constant_bits(value: &Constant) -> Option<u128> {
    Some(match value {
        Constant::I8(value) => *value as u8 as u128,
        Constant::U8(value) => *value as u128,
        Constant::I16(value) => *value as u16 as u128,
        Constant::U16(value) => *value as u128,
        Constant::I32(value) => *value as u32 as u128,
        Constant::U32(value) => *value as u128,
        Constant::I64(value) => *value as u64 as u128,
        Constant::U64(value) => *value as u128,
        _ => return None,
    })
}

fn literal(function: &Function, id: ValueId) -> Option<&Constant> {
    operations(function).find_map(|operation| match &operation.kind {
        Kind::Constant(value) if matches!(operation.results.as_slice(), [result] if result.id == id) => Some(value),
        _ => None,
    })
}

fn binary_count(
    function: &Function,
    case: Case,
    ordinal: usize,
    require_literal: bool,
) -> Result<usize, String> {
    let mask = (1_u128 << case.batch.integer.width()) - 1;
    let expected = match ordinal {
        0 => Some((BinaryOp::BitXor, 0)),
        1 => Some((BinaryOp::BitOr, 0)),
        2 => Some((BinaryOp::BitAnd, mask)),
        3 => Some((BinaryOp::BitXor, 3)),
        4 => None,
        _ => return Err("identity root ordinal".into()),
    };
    let scalar_type = Type::Scalar(case.batch.integer.scalar());
    let mut count = 0;
    for operation in operations(function) {
        let Kind::Binary { op, lhs, rhs } = &operation.kind else {
            continue;
        };
        if !matches!(op, BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor) {
            continue;
        }
        let Some((expected_op, bits)) = expected else {
            return Err("no-op root gained bitwise arithmetic".into());
        };
        let neutral = |constant: Option<&Constant>| {
            constant.is_some_and(|value| {
                value.ty() == scalar_type && constant_bits(value) == Some(bits)
            })
        };
        if *op != expected_op
            || !matches!(operation.results.as_slice(), [result] if result.ty == scalar_type)
            || (require_literal
                && !(neutral(literal(function, *rhs)) && literal(function, *lhs).is_none()
                    || neutral(literal(function, *lhs)) && literal(function, *rhs).is_none()))
        {
            return Err("actual identity/control operator, literal or width changed".into());
        }
        count += 1;
    }
    if count > 1 || (ordinal == 3 && count != 1) {
        return Err("identity/control exact operation count changed".into());
    }
    Ok(count)
}

pub(super) fn observe(stage: &Stage, case: Case) -> Result<Vec<Root>, String> {
    let semantic = stage.semantic();
    let mut source_roots = BTreeMap::new();
    for root in semantic.roots() {
        let function = &semantic.functions()[root.index() as usize];
        let entry = function
            .kernel_entry()
            .ok_or("semantic root entry missing")?;
        let selected = semantic
            .select_kernel_body_for_root_v1(*root)
            .filter(|selection| selection.root() == *root)
            .ok_or("exact semantic root body selection")?;
        let name =
            std::str::from_utf8(entry.export_symbol().as_bytes()).map_err(|e| e.to_string())?;
        if source_roots
            .insert(
                name,
                (
                    *function.identity().as_bytes(),
                    *semantic.functions()[selected.body().index() as usize]
                        .identity()
                        .as_bytes(),
                    *entry.kernel_binding_identity().as_bytes(),
                ),
            )
            .is_some()
        {
            return Err("duplicate exact semantic root".into());
        }
    }
    exact_roster(
        &source_roots
            .keys()
            .map(|root| (*root).to_owned())
            .collect::<Vec<_>>(),
    )?;
    let original = stage.original_module();
    let before = stage
        .checked_output()
        .intermediate_policy5()
        .owner()
        .module();
    let after = stage.output().module();
    let original_order = root_order(original);
    exact_roster(&original_order)?;
    if root_order(before) != original_order || root_order(after) != original_order {
        return Err("actual N/O/I root identities or original order changed".into());
    }
    for module in [original, before, after] {
        let helpers = module
            .functions
            .iter()
            .filter(|f| f.role == FunctionRole::InternalHelper)
            .count();
        if helpers != if case.batch.retained { ROOTS.len() } else { 0 } {
            return Err("identity exact helper roster changed".into());
        }
    }
    let mut result = Vec::new();
    let mut unique_helpers = BTreeSet::new();
    for (ordinal, root) in ROOTS.into_iter().enumerate() {
        let source_owner = owner(original, case, root)?;
        let before_owner = owner(before, case, root)?;
        let after_owner = owner(after, case, root)?;
        // N may still compute !0 explicitly; only actual O must expose the
        // typed neutral literal that the appended pass consumes.
        let original_count = binary_count(source_owner, case, ordinal, false)?;
        let before_count = binary_count(before_owner, case, ordinal, true)?;
        let after_count = binary_count(after_owner, case, ordinal, true)?;
        if before_owner.id != after_owner.id || source_owner.id != after_owner.id {
            return Err("actual N/O/I operation-owning function changed".into());
        }
        if ordinal < 3 && (after_count != 0 || case.opt0 && before_count != 1) {
            return Err("Policy6 failed actual-O identity survival/removal requirement".into());
        }
        if case.batch.retained && !unique_helpers.insert(after_owner.id.clone()) {
            return Err("retained roots share a substituted helper identity".into());
        }
        let original_kernel = simulation::integer_identity::kernel_for_root(original, root)
            .map_err(|e| format!("{e:?}"))?;
        let final_kernel = simulation::integer_identity::kernel_for_root(after, root)
            .map_err(|e| format!("{e:?}"))?;
        let (source_function, source_body, source_binding) = source_roots[root];
        result.push(Root {
            root: root.into(),
            source_function,
            source_body,
            source_binding,
            original_entry: original_kernel.entry.as_str().into(),
            original_owner: source_owner.id.as_str().into(),
            output_entry: final_kernel.entry.as_str().into(),
            output_owner: after_owner.id.as_str().into(),
            native_symbol: if case.batch.retained {
                after_owner.id.as_str()
            } else {
                root
            }
            .into(),
            original_binary_count: original_count,
            before_binary_count: before_count,
            after_binary_count: after_count,
        });
    }
    Ok(result)
}

fn native_body<'a>(llvm: &'a str, symbol: &str) -> Result<&'a str, String> {
    let marker = format!("@{symbol}(");
    let starts = llvm
        .match_indices("define ")
        .filter_map(|(start, _)| {
            llvm[start..]
                .lines()
                .next()
                .is_some_and(|line| line.contains(&marker))
                .then_some(start)
        })
        .collect::<Vec<_>>();
    let [start] = starts.as_slice() else {
        return Err("exact native owning function missing/duplicate".into());
    };
    let body = &llvm[*start..];
    let end = body
        .find("\n}")
        .ok_or("native owning function not closed")?;
    Ok(&body[..end])
}

fn native_arithmetic(case: Case, ordinal: usize, body: &str) -> Result<(), String> {
    let instructions = body
        .lines()
        .filter_map(|line| line.split_once(" = ").map(|(_, rhs)| rhs))
        .filter(|rhs| {
            ["xor ", "or ", "and "]
                .iter()
                .any(|opcode| rhs.starts_with(opcode))
        })
        .collect::<Vec<_>>();
    if ordinal == 3 {
        let [instruction] = instructions.as_slice() else {
            return Err("native control XOR missing/duplicated".into());
        };
        if !instruction.starts_with(&format!("xor i{} ", case.batch.integer.width()))
            || !instruction.ends_with(", 3")
        {
            return Err("native control width/literal/flags changed".into());
        }
    } else if !instructions.is_empty() {
        return Err("native identity/no-op retained bitwise operation".into());
    }
    Ok(())
}

pub(super) fn check_native(case: Case, roots: &[Root], llvm: &str) -> Result<(), String> {
    exact_roster(
        &roots
            .iter()
            .map(|root| root.root.clone())
            .collect::<Vec<_>>(),
    )?;
    let by_name = roots
        .iter()
        .map(|root| (root.root.as_str(), root))
        .collect::<BTreeMap<_, _>>();
    for (ordinal, name) in ROOTS.into_iter().enumerate() {
        let root = by_name[name];
        native_arithmetic(case, ordinal, native_body(llvm, &root.native_symbol)?)?;
        if case.batch.retained {
            let entry = native_body(llvm, name)?;
            if !entry.lines().any(|line| {
                line.contains("call ") && line.contains(&format!("@{}(", root.native_symbol))
            }) {
                return Err("native root lost its exact final-I helper call".into());
            }
        }
    }
    Ok(())
}

#[test]
fn identity_roster_preserves_actual_order_but_refuses_missing_duplicate_extra_foreign() {
    let mut roots = ROOTS.map(str::to_owned).to_vec();
    roots.reverse();
    exact_roster(&roots).unwrap();
    let mut missing = roots.clone();
    missing.pop();
    assert!(exact_roster(&missing).is_err());
    let mut duplicate = roots.clone();
    duplicate[1] = duplicate[0].clone();
    assert!(exact_roster(&duplicate).is_err());
    let mut extra = roots.clone();
    extra.push("foreign".into());
    assert!(exact_roster(&extra).is_err());
    roots[0] = "foreign".into();
    assert!(exact_roster(&roots).is_err());
}

#[test]
fn identity_native_oracle_refuses_foreign_owner_width_literal_and_leftover_identity() {
    for integer in Integer::ALL {
        let case = Case {
            batch: Batch {
                integer,
                retained: false,
            },
            opt0: true,
            target: Target::Gfx942,
        };
        let exact = format!("%x = xor i{} %value, 3", integer.width());
        native_arithmetic(case, 3, &exact).unwrap();
        native_arithmetic(case, 0, "ret void").unwrap();
        for bad in [
            exact.replace(", 3", ", 0"),
            exact.replace("xor ", "or "),
            exact.replace("xor ", "xor exact "),
            exact.replace(&format!("i{} ", integer.width()), "i128 "),
            String::new(),
            format!("{exact}\n{exact}"),
        ] {
            assert!(native_arithmetic(case, 3, &bad).is_err());
        }
        assert!(native_arithmetic(case, 0, &exact).is_err());
    }
    assert!(native_body("define void @foreign() {\n}\n", "expected").is_err());
    assert!(native_body("define void @x() {\n}\ndefine void @x() {\n}\n", "x").is_err());
}

#[test]
fn identity_graph_oracle_requires_typed_neutral_literal_and_keeps_nonidentity_control() {
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Operation, ScalarType, Signature, Terminator, ValueDef,
    };
    let case = Case {
        batch: Batch {
            integer: Integer::U32,
            retained: true,
        },
        opt0: true,
        target: Target::Gfx942,
    };
    let function = |op, literal: Constant, result_type| {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(1), literal.ty()),
                Kind::Constant(literal),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(2), result_type),
                Kind::Binary {
                    op,
                    lhs: ValueId(0),
                    rhs: ValueId(1),
                },
            ),
        ];
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(2)],
        });
        Function::internal_helper(
            "test-only-oracle",
            Signature::new(
                vec![Type::Scalar(ScalarType::U32)],
                vec![Type::Scalar(ScalarType::U32)],
            ),
            vec![ValueId(0)],
            vec![block],
        )
    };
    let exact = function(
        BinaryOp::BitXor,
        Constant::U32(0),
        Type::Scalar(ScalarType::U32),
    );
    assert_eq!(binary_count(&exact, case, 0, true).unwrap(), 1);
    assert!(binary_count(&exact, case, 3, true).is_err());
    assert!(binary_count(&exact, case, 4, true).is_err());
    let control = function(
        BinaryOp::BitXor,
        Constant::U32(3),
        Type::Scalar(ScalarType::U32),
    );
    assert_eq!(binary_count(&control, case, 3, true).unwrap(), 1);
    for bad in [
        function(
            BinaryOp::BitOr,
            Constant::U32(0),
            Type::Scalar(ScalarType::U32),
        ),
        function(
            BinaryOp::BitXor,
            Constant::U32(1),
            Type::Scalar(ScalarType::U32),
        ),
        function(
            BinaryOp::BitXor,
            Constant::U16(0),
            Type::Scalar(ScalarType::U32),
        ),
        function(
            BinaryOp::BitXor,
            Constant::U32(0),
            Type::Scalar(ScalarType::I32),
        ),
    ] {
        assert!(binary_count(&bad, case, 0, true).is_err());
    }
}
