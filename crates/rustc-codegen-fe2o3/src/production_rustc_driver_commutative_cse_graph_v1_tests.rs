//! Actual-source observations supplement, but do not replace, the pair checker.
use super::*;
use fe2o3_kernel_ir::{
    AddressSpace, AmdGpuDiagnosticOperation as Diagnostic, BinaryOp,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionDescendantKindV1 as Descendant,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirOperationCoordinateV1 as Coordinate, CanonicalKirOperationOriginV1 as Origin,
    CanonicalKirTransitionCandidateV1 as Candidate, Function, FunctionRole, Module, Operation,
    OperationKind as Kind, ScalarType, Type, ValueId,
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
    source_divide_count: usize,
    original_binary_count: usize,
    component_binary_count: usize,
    anchor: [u32; 3],
    removed: [u32; 3],
    descendant: [u32; 3],
    preserved_dynamic_divides: usize,
    preserved_traps: usize,
    preserved_global_stores: usize,
}

fn at(module: &Module, c: Coordinate) -> Result<&Operation, String> {
    module
        .functions
        .get(c.block.function.0 as usize)
        .and_then(|f| f.body.as_ref())
        .and_then(|b| b.blocks.get(c.block.block as usize))
        .and_then(|b| b.operations.get(c.operation as usize))
        .ok_or_else(|| "foreign operation coordinate".into())
}
fn coordinate(fi: usize, bi: usize, oi: usize) -> Coordinate {
    Coordinate {
        block: Block {
            function: FunctionCoordinate(u32::try_from(fi).unwrap()),
            block: u32::try_from(bi).unwrap(),
        },
        operation: u32::try_from(oi).unwrap(),
    }
}
fn binaries(
    function: &Function,
    fi: usize,
    case: Case,
    op: BinaryOp,
) -> Result<Vec<graph::Binary>, String> {
    let body = function.body.as_ref().ok_or("missing actual body")?;
    let [_, lhs, rhs, _] = body.parameters.as_slice() else {
        return Err("four actual parameters".into());
    };
    if lhs == rhs {
        return Err("distinct dynamic operand atoms required".into());
    }
    let mut rows = Vec::new();
    for (bi, block) in body.blocks.iter().enumerate() {
        for (oi, operation) in block.operations.iter().enumerate() {
            let Kind::Binary {
                op: actual,
                lhs: a,
                rhs: b,
            } = operation.kind
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
                || !((a == *lhs && b == *rhs) || (a == *rhs && b == *lhs))
                || !matches!(operation.results.as_slice(), [v] if v.ty == Type::Scalar(case.integer.scalar()))
            {
                return Err("commutative exact typed dynamic argument atoms".into());
            }
            rows.push(graph::Binary {
                coordinate: coordinate(fi, bi, oi),
                result: operation.results[0].id,
            });
        }
    }
    Ok(rows)
}
fn reversed_pair(
    function: &Function,
    pair: &[graph::Binary],
) -> Result<(graph::Binary, graph::Binary), String> {
    let (anchor, removed) = graph::dominated_pair(function, pair)?;
    let body = function.body.as_ref().unwrap();
    let [_, lhs, rhs, _] = body.parameters.as_slice() else {
        return Err("four actual parameters".into());
    };
    let operation =
        |c: Coordinate| &body.blocks[c.block.block as usize].operations[c.operation as usize];
    if !matches!(operation(anchor.coordinate).kind, Kind::Binary { lhs: a, rhs: b, .. } if a == *lhs && b == *rhs)
        || !matches!(operation(removed.coordinate).kind, Kind::Binary { lhs: a, rhs: b, .. } if a == *rhs && b == *lhs)
    {
        return Err("must require commutation, not old positional CSE".into());
    }
    Ok((anchor, removed))
}
fn divisions(function: &Function, fi: usize) -> Result<Vec<Coordinate>, String> {
    let body = function.body.as_ref().ok_or("actual body")?;
    let [_, _, _, choose] = body.parameters.as_slice() else {
        return Err("actual choose argument".into());
    };
    let mut rows = Vec::new();
    for (bi, block) in body.blocks.iter().enumerate() {
        for (oi, operation) in block.operations.iter().enumerate() {
            if let Kind::Binary {
                op: BinaryOp::Divide,
                lhs,
                rhs,
            } = operation.kind
            {
                if lhs != *choose
                    || rhs != *choose
                    || !matches!(operation.results.as_slice(), [v] if v.ty == Type::Scalar(ScalarType::U32))
                {
                    return Err(
                        "retained Divide must consume actual dynamic U32 choose twice".into(),
                    );
                }
                rows.push(coordinate(fi, bi, oi));
            }
        }
    }
    if rows.len() != 1 {
        return Err("nonvacuous actual dynamic U32 Divide required".into());
    }
    Ok(rows)
}
fn is_trap(op: &Operation) -> bool {
    matches!(&op.kind, Kind::Call { callee, arguments }
        if matches!(Diagnostic::from_intrinsic_call(callee, arguments), Some(Diagnostic::Trap | Diagnostic::AssertFail { .. })))
}

// Full non-CSE operation and CFG preservation, including unreachable trap blocks.
// This is an additional closed-fixture check, not a substitute relation authority.
fn preserve(
    input: &Module,
    output: &Module,
    candidate: Candidate<'_>,
    replacements: &BTreeMap<Coordinate, (ValueId, ValueId)>,
) -> Result<BTreeMap<u32, usize>, String> {
    if input.id != output.id
        || input.kernels != output.kernels
        || input.functions.len() != output.functions.len()
    {
        return Err("component module/function/kernel custody changed".into());
    }
    let mut origins = BTreeMap::new();
    let mut outputs = BTreeSet::new();
    for row in candidate.operations {
        let Origin::Retained(source) = row.origin else {
            return Err("commutative synthesized operation".into());
        };
        if origins.insert(source, row.output).is_some() || !outputs.insert(row.output) {
            return Err("duplicate retained source operation".into());
        }
    }
    let mut traps = BTreeMap::new();
    let mut retained = 0;
    for (fi, (before, after)) in input.functions.iter().zip(&output.functions).enumerate() {
        if before.id != after.id
            || before.signature != after.signature
            || before.role != after.role
            || before.required_capabilities != after.required_capabilities
        {
            return Err("component function/header/declaration changed".into());
        }
        let (a, b) = match (&before.body, &after.body) {
            (None, None) => continue,
            (Some(a), Some(b)) => (a, b),
            _ => return Err("component body presence changed".into()),
        };
        if a.parameters != b.parameters || a.blocks.len() != b.blocks.len() {
            return Err("component block/parameter roster changed".into());
        }
        for (bi, (old_block, new_block)) in a.blocks.iter().zip(&b.blocks).enumerate() {
            if old_block.id != new_block.id
                || old_block.parameters != new_block.parameters
                || old_block.terminator != new_block.terminator
            {
                return Err("component changed exact assertion/control-flow edge".into());
            }
            for (oi, old) in old_block.operations.iter().enumerate() {
                let c = coordinate(fi, bi, oi);
                if replacements.contains_key(&c) {
                    if origins.contains_key(&c) {
                        return Err("claimed omitted duplicate remains".into());
                    }
                    continue;
                }
                let mapped = *origins
                    .get(&c)
                    .ok_or("missing retained operation/effect/trap")?;
                if mapped.block != c.block {
                    return Err("operation moved across assertion/control block".into());
                }
                let new = at(output, mapped)?;
                retained += 1;
                if old.results != new.results {
                    return Err("retained result changed".into());
                }
                if let Kind::Store {
                    pointer,
                    value,
                    access,
                } = &old.kind
                {
                    let expected = replacements
                        .iter()
                        .find_map(|(site, (from, to))| {
                            (site.block.function.0 as usize == fi && value == from).then_some(*to)
                        })
                        .unwrap_or(*value);
                    if new.kind
                        != (Kind::Store {
                            pointer: *pointer,
                            value: expected,
                            access: *access,
                        })
                    {
                        return Err("Store pointer/access/order/value substitution changed".into());
                    }
                } else if new != old {
                    return Err("retained non-CSE producer/trap/effect operation changed".into());
                }
                if is_trap(old) {
                    *traps.entry(fi as u32).or_insert(0) += 1;
                }
            }
        }
    }
    let output_operations = output
        .functions
        .iter()
        .flat_map(|f| &f.body)
        .flat_map(|b| &b.blocks)
        .map(|b| b.operations.len())
        .sum::<usize>();
    if retained != candidate.operations.len() || retained != output_operations {
        return Err("extra output operation/origin".into());
    }
    Ok(traps)
}

pub(super) fn observe(
    stage: &Stage,
    actual: &fe2o3_pliron::CheckedCommutativeBitwiseOptimizationV1<'_>,
    case: Case,
) -> Result<Vec<Root>, String> {
    let semantic = stage.semantic();
    let input = actual.input().module();
    let output = actual.owner().module();
    if !std::ptr::eq(actual.input(), stage.test_original_owner_v1()) {
        return Err("foreign original owner".into());
    }
    observe_graphs(
        semantic,
        input,
        output,
        actual.occurrences().candidate(),
        case,
    )
}

// Reused only by strict test observers. Each caller separately proves actual
// input-owner custody; these shape observations never construct that authority.
pub(super) fn observe_graphs(
    semantic: &AdmittedInertSemanticMirV1,
    input: &Module,
    output: &Module,
    candidate: Candidate<'_>,
    case: Case,
) -> Result<Vec<Root>, String> {
    let original_order = graph::order(input);
    graph::roster(original_order.iter().map(String::as_str))?;
    for module in [input, output] {
        if graph::order(module) != original_order
            || module
                .functions
                .iter()
                .any(|f| f.role == FunctionRole::InternalHelper)
        {
            return Err("commutative direct source root/helper roster changed".into());
        }
    }
    let mut sources = BTreeMap::new();
    for root in semantic.roots() {
        let function = &semantic.functions()[root.index() as usize];
        let entry = function.kernel_entry().ok_or("semantic root entry")?;
        let name =
            std::str::from_utf8(entry.export_symbol().as_bytes()).map_err(|e| e.to_string())?;
        let selected = semantic
            .select_kernel_body_for_root_v1(*root)
            .filter(|s| s.root() == *root)
            .ok_or("selected actual source body")?;
        let body = &semantic.functions()[selected.body().index() as usize];
        if sources.insert(name, (function, entry, body)).is_some() {
            return Err("duplicate semantic root".into());
        }
    }
    graph::roster(sources.keys().copied())?;
    let mut replacements = BTreeMap::new();
    let mut report = Vec::new();
    for (ordinal, name) in ROOTS.into_iter().enumerate() {
        let (source, entry, body) = sources[name];
        let source_op = [
            SemanticBinaryOpV1::BitAnd,
            SemanticBinaryOpV1::BitOr,
            SemanticBinaryOpV1::BitXor,
        ][ordinal];
        let mut count = 0;
        let mut divides = 0;
        let mut blocks = BTreeSet::new();
        for (bi, block) in body.blocks().iter().enumerate() {
            for statement in block.statements() {
                if let SemanticStatementKindV1::Assign(value) = statement.kind()
                    && let SemanticRvalueKindV1::Binary {
                        operation,
                        left,
                        right,
                    } = value.value().kind()
                {
                    if *operation == source_op {
                        if left.ty() != right.ty()
                            || !matches!(semantic.types()[left.ty().index() as usize].shape(),
                            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }) if *signed == case.integer.signed() && u32::from(*bits) == case.integer.width())
                        {
                            return Err("actual semantic bitwise type changed".into());
                        }
                        count += 1;
                        blocks.insert(bi);
                    }
                    if *operation == SemanticBinaryOpV1::Divide {
                        if left.ty() != right.ty()
                            || !matches!(
                                semantic.types()[left.ty().index() as usize].shape(),
                                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                                    signed: false,
                                    bits: 32
                                })
                            )
                        {
                            return Err("actual semantic guard division type changed".into());
                        }
                        divides += 1;
                    }
                }
            }
        }
        if count != 2 || blocks.len() != 2 || divides != 1 {
            return Err("opt0 source lost actual bitwise pair or dynamic division".into());
        }
        let (_, n, ni) = graph::kernel(input, case, name)?;
        let (_, q, qi) = graph::kernel(output, case, name)?;
        if n.id != q.id || ni != qi {
            return Err("component function identity/order changed".into());
        }
        let pair = binaries(n, ni, case, graph::operation(ordinal))?;
        let (anchor, removed) = reversed_pair(n, &pair)?;
        let result = binaries(q, qi, case, graph::operation(ordinal))?;
        let [output_value] = result.as_slice() else {
            return Err("component must retain exactly one bitwise expression".into());
        };
        graph::descendant(
            candidate,
            anchor.coordinate,
            output_value.coordinate,
            Descendant::Retained,
        )?;
        graph::descendant(
            candidate,
            removed.coordinate,
            output_value.coordinate,
            Descendant::Substituted,
        )?;
        if !matches!(candidate.operations.iter().filter(|r| r.output == output_value.coordinate).collect::<Vec<_>>().as_slice(),
            [row] if row.origin == Origin::Retained(anchor.coordinate))
        {
            return Err("component output lost actual anchor origin".into());
        }
        graph::stores(n, &[anchor.result, removed.result])?;
        let stores = graph::stores(q, &[output_value.result])?;
        let divide = divisions(n, ni)?[0];
        let output_divide = divisions(q, qi)?[0];
        if at(input, divide)? != at(output, output_divide)?
            || !matches!(candidate.operations.iter().filter(|r| r.output == output_divide).collect::<Vec<_>>().as_slice(), [row] if row.origin == Origin::Retained(divide))
        {
            return Err("dynamic Divide operation/origin was not preserved".into());
        }
        replacements.insert(removed.coordinate, (removed.result, output_value.result));
        report.push(Root {
            name: name.into(),
            source_function: *source.identity().as_bytes(),
            source_body: *body.identity().as_bytes(),
            source_binding: *entry.kernel_binding_identity().as_bytes(),
            function: n.id.as_str().into(),
            source_binary_count: count,
            source_divide_count: divides,
            original_binary_count: pair.len(),
            component_binary_count: result.len(),
            anchor: graph::triple(anchor.coordinate),
            removed: graph::triple(removed.coordinate),
            descendant: graph::triple(output_value.coordinate),
            preserved_dynamic_divides: 1,
            preserved_traps: 0,
            preserved_global_stores: stores,
        });
    }
    let traps = preserve(input, output, candidate, &replacements)?;
    for root in &mut report {
        root.preserved_traps = traps.get(&root.anchor[0]).copied().unwrap_or(0);
        validate(root)?;
    }
    Ok(report)
}
pub(super) fn validate(root: &Root) -> Result<(), String> {
    if root.function.is_empty()
        || [root.source_function, root.source_body, root.source_binding].contains(&[0; 32])
        || (
            root.source_binary_count,
            root.source_divide_count,
            root.original_binary_count,
            root.component_binary_count,
            root.preserved_dynamic_divides,
            root.preserved_global_stores,
        ) != (2, 1, 2, 1, 1, 2)
        || root.anchor[0] != root.removed[0]
        || root.anchor[0] != root.descendant[0]
        || root.anchor[1] == root.removed[1]
        || root.anchor[1] != root.descendant[1]
    {
        return Err(
            "commutative root lost exact nonempty source/mutation/preservation evidence".into(),
        );
    }
    Ok(())
}
pub(super) fn descriptor(
    roots: &[Root],
    descriptors: &[fe2o3_kernel_descriptor::KernelDescriptorV1],
) -> Result<(), String> {
    graph::roster(descriptors.iter().map(|r| r.entry_name().as_str()))?;
    for row in descriptors {
        let source = roots
            .iter()
            .find(|r| r.name == row.entry_name().as_str())
            .ok_or("descriptor root")?;
        let fe2o3_kernel_descriptor::BlockSizeV1::Exact(size) = row.launch().block_size() else {
            return Err("exact baseline block size".into());
        };
        if row.kernel_id().as_bytes() != &source.source_binding
            || row.launch().rank() != 1
            || [size.x(), size.y(), size.z()] != [64, 1, 1]
            || row.launch().max_flat_workgroup_size() != 64
        {
            return Err("baseline descriptor source binding/dynamic64 changed".into());
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_rustc_driver_commutative_cse_graph_controls_v1_tests.rs"]
mod controls;
