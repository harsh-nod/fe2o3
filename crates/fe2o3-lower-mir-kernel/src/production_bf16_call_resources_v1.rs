// A logical prepayment for the CLOSED source/output profile, not RSS. Source,
// current SSA and occurrence storage remain borrowed/caller-reserved. These
// counters are obtained without constructing a planner, graph or analysis map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Bf16EmissionEnvelopeV1 {
    storage: usize,
    work: usize,
}

fn bf16_envelope_arithmetic_v1(
    locals: usize,
    statements: usize,
    transported: usize,
    events: usize,
    types: usize,
) -> Result<Bf16EmissionEnvelopeV1, ArgumentResourceV1> {
    // Four simultaneously live component tables: source-definition bindings,
    // local bindings, block-parameter transport and pending edge definitions.
    // Each uses the existing 256-component ceiling; each KIR type spine is
    // limited below to eight nodes. BTree bookkeeping gets eight machine words.
    let component = argument_sum_v1(&[
        std::mem::size_of::<SemanticValueBindingV1>(),
        std::mem::size_of::<ValueDef>(),
        8 * std::mem::size_of::<Type>(),
        8 * std::mem::size_of::<usize>(),
    ])?;
    let rows = argument_sum_v1(&[locals, statements, transported, events, 32, 1024])?;
    let component_storage = argument_product_v1(
        argument_product_v1(argument_product_v1(rows, 256)?, component)?,
        4,
    )?;
    // Pending and completed operation/header/correspondence copies coexist.
    // Symbols are bounded128 and operation operands/results256 in this profile.
    let operation = argument_sum_v1(&[
        std::mem::size_of::<Operation>(),
        256 * std::mem::size_of::<ValueDef>(),
        256 * std::mem::size_of::<ValueId>(),
        128,
        512,
    ])?;
    let output = argument_product_v1(argument_product_v1(1024, operation)?, 4)?;
    let storage = argument_sum_v1(&[
        component_storage,
        output,
        argument_product_v1(types, 4096)?,
        65536,
        std::mem::size_of::<Bf16CallEmissionStateV1<'static>>(),
        std::mem::size_of::<SealedBf16CallRelationV1>(),
    ])?;
    // Existing finite analysis walks may revisit a source/transport pair.
    // The added phase prepays their conservative product before the builder.
    // Later ledger-aware emission/canonical/replay scans charge additionally.
    let work = argument_product_v1(argument_product_v1(rows, rows)?, 256)?;
    Ok(Bf16EmissionEnvelopeV1 { storage, work })
}

fn bf16_emission_profile_v1(
    source: &CheckedBf16CallInstanceV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Bf16EmissionEnvelopeV1, ProductionSemanticKirErrorV1> {
    budget.charge_work(8)?;
    let owner = source.owner();
    let semantic = owner.source_semantic();
    if semantic.functions().len() != 2
        || semantic.types().len() > 4096
        || semantic.callables().len() > 4096
    {
        return Err(bf16_emission_refusal_v1("BF16 finite emission header"));
    }
    let mut blocks = 0usize;
    let mut locals = 0usize;
    let mut statements = 0usize;
    let mut transported = 0usize;
    let mut events = 0usize;
    for (index, function) in semantic.functions().iter().enumerate() {
        budget.charge_work(1)?;
        blocks = argument_sum_v1(&[blocks, function.blocks().len()])?;
        locals = argument_sum_v1(&[locals, function.locals().len()])?;
        if blocks > 32 || locals > 4096 {
            return Err(bf16_emission_refusal_v1("BF16 finite source blocks/locals"));
        }
        let plan = owner
            .plan_for_function(SemanticFunctionIdV1::from_index(index as u32))
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        let occurrence = owner
            .occurrences_v1()
            .and_then(|r| r.function(SemanticFunctionIdV1::from_index(index as u32)))
            .ok_or_else(|| bf16_emission_refusal_v1("BF16 real occurrence capture required"))?;
        events = argument_sum_v1(&[events, occurrence.events().len()])?;
        if events > 65536 {
            return Err(bf16_emission_refusal_v1("BF16 occurrence ceiling"));
        }
        for block in function.blocks() {
            budget.charge_work(1)?;
            statements = argument_sum_v1(&[statements, block.statements().len()])?;
            if statements > 4096 || locals.checked_add(statements).is_none_or(|n| n > 4096) {
                return Err(bf16_emission_refusal_v1("BF16 finite source item ceiling"));
            }
            // No hidden assertion elision. Sealing also independently requires
            // empty exact source/canonical assertion coverage after emission.
            bf16_require_no_assert_v1(block.terminator().kind())?;
        }
        for block in plan.plan().reverse_postorder() {
            budget.charge_work(1)?;
            let count = plan
                .plan()
                .transport_variables(*block)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?
                .len();
            transported = argument_sum_v1(&[transported, count])?;
            if transported > 65536 {
                return Err(bf16_emission_refusal_v1("BF16 transport ceiling"));
            }
        }
    }
    bf16_bounded_types_v1(semantic.types(), budget)?;
    let entry = semantic.functions()[source.root().index() as usize]
        .kernel_entry()
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if entry.export_symbol().as_bytes().len() > 128 {
        return Err(bf16_emission_refusal_v1("BF16 symbol ceiling"));
    }
    bf16_envelope_arithmetic_v1(
        locals,
        statements,
        transported,
        events,
        semantic.types().len(),
    )
    .map_err(Into::into)
}

fn bf16_require_no_assert_v1(
    kind: &SemanticTerminatorKindV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if matches!(kind, SemanticTerminatorKindV1::Assert { .. }) {
        return Err(bf16_emission_refusal_v1(
            "BF16 semantic Assert is unavailable",
        ));
    }
    Ok(())
}

// Charge this allocation-free scan's two caches and logical stack independently;
// do not borrow a source/callback reservation or claim native RSS. Both complete
// height and expanded node count are memoized. Array multiplicity and repeated
// DAG children count in the expanded size, without exponential scan work. 255
// in the height cache marks a current ancestor, never a completed type.
const BF16_TYPE_SCAN_STORAGE_V1: usize =
    4096 * (std::mem::size_of::<u8>() + std::mem::size_of::<u16>()) + 8 * 4096;
fn bf16_bounded_types_v1(
    types: &[SemanticTypeDeclV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if types.len() > 4096 {
        return Err(bf16_emission_refusal_v1("BF16 type roster ceiling"));
    }
    budget.reserve_storage(BF16_TYPE_SCAN_STORAGE_V1)?;
    let result = {
        let mut heights = [0u8; 4096];
        let mut nodes = [0u16; 4096];
        (|| {
            for index in 0..types.len() {
                bf16_type_height_v1(
                    types,
                    SemanticTypeIdV1::from_index(index as u32),
                    0,
                    &mut heights,
                    &mut nodes,
                    budget,
                )?;
            }
            Ok(())
        })()
    };
    budget.release_storage(BF16_TYPE_SCAN_STORAGE_V1)?;
    result
}
fn bf16_type_height_v1(
    types: &[SemanticTypeDeclV1],
    id: SemanticTypeIdV1,
    depth: usize,
    heights: &mut [u8; 4096],
    nodes: &mut [u16; 4096],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(u8, u16), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if depth >= 8 || id.index() as usize >= types.len() {
        return Err(bf16_emission_refusal_v1("BF16 finite type depth"));
    }
    let index = id.index() as usize;
    let old = heights[index];
    if old == u8::MAX {
        return Err(bf16_emission_refusal_v1("BF16 cyclic type unavailable"));
    }
    if old != 0 {
        if depth + usize::from(old) > 8 {
            return Err(bf16_emission_refusal_v1("BF16 finite type depth"));
        }
        return Ok((old, nodes[index]));
    }
    let shape = types[index].shape();
    let count = match shape {
        SemanticTypeShapeV1::Tuple(fields)
        | SemanticTypeShapeV1::Aggregate(fields)
        | SemanticTypeShapeV1::Union(fields) => fields.fields().len(),
        SemanticTypeShapeV1::Enum { variants, .. } => {
            if variants.len() > 256 {
                return Err(bf16_emission_refusal_v1("BF16 type variant ceiling"));
            }
            let mut count = 1usize;
            for variant in variants {
                budget.charge_work(1)?;
                count = count
                    .checked_add(variant.fields().fields().len())
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
            }
            count
        }
        SemanticTypeShapeV1::FunctionPointer { arguments, .. } => arguments
            .fields()
            .len()
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?,
        SemanticTypeShapeV1::Array { length, .. } => {
            if *length > 256 {
                return Err(bf16_emission_refusal_v1("BF16 array component ceiling"));
            }
            1
        }
        SemanticTypeShapeV1::Pointer(_) | SemanticTypeShapeV1::Slice { .. } => 1,
        _ => 0,
    };
    if count > 256 {
        return Err(bf16_emission_refusal_v1("BF16 type child ceiling"));
    }
    heights[index] = u8::MAX;
    let mut maximum = 0u8;
    let mut expanded = 1usize;
    let mut child = |id, multiplicity: usize| -> Result<(), ProductionSemanticKirErrorV1> {
        let (height, count) = bf16_type_height_v1(types, id, depth + 1, heights, nodes, budget)?;
        maximum = maximum.max(height);
        expanded = expanded
            .checked_add(
                usize::from(count)
                    .checked_mul(multiplicity)
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
            )
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if expanded > 256 {
            return Err(bf16_emission_refusal_v1("BF16 expanded type node ceiling"));
        }
        Ok(())
    };
    match shape {
        SemanticTypeShapeV1::Pointer(pointer) => child(pointer.pointee(), 1)?,
        SemanticTypeShapeV1::Array { element, length } => child(
            *element,
            usize::try_from(*length).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        )?,
        SemanticTypeShapeV1::Slice { element } => child(*element, 1)?,
        SemanticTypeShapeV1::Tuple(fields)
        | SemanticTypeShapeV1::Aggregate(fields)
        | SemanticTypeShapeV1::Union(fields) => {
            for id in fields.fields() {
                child(*id, 1)?;
            }
        }
        SemanticTypeShapeV1::Enum {
            discriminant,
            variants,
        } => {
            child(*discriminant, 1)?;
            for variant in variants {
                for id in variant.fields().fields() {
                    child(*id, 1)?;
                }
            }
        }
        SemanticTypeShapeV1::FunctionPointer {
            arguments,
            return_type,
            ..
        } => {
            for id in arguments.fields() {
                child(*id, 1)?;
            }
            child(*return_type, 1)?;
        }
        _ => {}
    }
    let height = maximum + 1;
    heights[index] = height;
    nodes[index] = u16::try_from(expanded).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    Ok((height, nodes[index]))
}
