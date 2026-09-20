//! Complete-shape recognition of a borrowed V17 owner, not source authority.
//! All operations and blocks are covered; no kernel/source/opcode-name shortcut.
use fe2o3_kernel_ir::*;
pub type Result<T> = std::result::Result<T, &'static str>;
pub struct Profile<'a> {
    pub symbol: &'a str,
    pub program: &'a Gfx942OrderedProgramV1,
}
fn one(operation: &Operation, ty: &Type) -> Result<ValueId> {
    match operation.results.as_slice() {
        [result] if &result.ty == ty => Ok(result.id),
        _ => Err("result type or arity"),
    }
}
fn block(body: &FunctionBody, id: BlockId) -> Result<&BasicBlock> {
    body.blocks
        .iter()
        .find(|block| block.id == id)
        .ok_or("missing block")
}
fn branch(block: &BasicBlock) -> Result<BlockId> {
    match &block.terminator {
        Some(Terminator::Branch { target, arguments }) if arguments.is_empty() => Ok(*target),
        _ => Err("expected empty-argument branch"),
    }
}
fn mark(body: &FunctionBody, id: BlockId, seen: &mut [bool; 32]) -> Result<()> {
    let ordinal = body
        .blocks
        .iter()
        .position(|b| b.id == id)
        .ok_or("missing block")?;
    if seen[ordinal] {
        return Err("cycle or repeated path block");
    }
    seen[ordinal] = true;
    Ok(())
}
fn tail(
    body: &FunctionBody,
    first: BlockId,
    store_block: BlockId,
    expect_store: bool,
    covered: &mut [bool; 32],
    prefix: &[bool; 32],
) -> Result<()> {
    let mut path = *prefix;
    let mut cursor = first;
    let mut stores = 0;
    for _ in 0..body.blocks.len() {
        mark(body, cursor, &mut path)?;
        let ordinal = body
            .blocks
            .iter()
            .position(|b| b.id == cursor)
            .ok_or("missing block")?;
        covered[ordinal] = true;
        let b = block(body, cursor)?;
        if cursor == store_block {
            if b.operations.len() != 1 {
                return Err("write branch has extra work");
            }
            stores += 1;
        } else if !b.operations.is_empty() {
            return Err("branch has extra work");
        }
        if let Some(Terminator::Return { values }) = &b.terminator {
            return if values.is_empty() && stores == usize::from(expect_store) {
                Ok(())
            } else {
                Err("wrong terminal store count or return")
            };
        }
        cursor = branch(b)?;
    }
    Err("tail did not terminate")
}
pub fn inspect(owner: &VerifiedCanonicalKernelIrModuleV17) -> Result<Profile<'_>> {
    let module = owner.module();
    let [kernel] = module.kernels.as_slice() else {
        return Err("one kernel required");
    };
    let [function] = module.functions.as_slice() else {
        return Err("one function required");
    };
    if kernel.entry != function.id
        || function.role != FunctionRole::KernelEntry
        || kernel.workgroup_size != Some(WorkgroupSize::new(64, 1, 1))
        || kernel.domain
            != (LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            })
        || !function.signature.results.is_empty()
    {
        return Err("entry, rank, launch, or result ABI");
    }
    // Restrict the diagnostic symbol grammar, not the source/kernel identity.
    let symbol = kernel.id.as_str();
    if symbol.is_empty()
        || symbol.len() > 128
        || !symbol
            .bytes()
            .enumerate()
            .all(|(i, c)| c.is_ascii_alphabetic() || c == b'_' || (i > 0 && c.is_ascii_digit()))
    {
        return Err("diagnostic symbol grammar");
    }
    let u32_ty = Type::Scalar(ScalarType::U32);
    let slice_ty = Type::slice(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    if function.signature.parameters != [slice_ty, u32_ty.clone(), u32_ty.clone(), u32_ty.clone()] {
        return Err("expected writable global u32 slice and three u32 arguments");
    }
    let body = function.body.as_ref().ok_or("missing body")?;
    let [out, a, b, c] = body.parameters.as_slice() else {
        return Err("parameter IDs");
    };
    if body.blocks.is_empty()
        || body.blocks.len() > 32
        || body.blocks.iter().any(|block| !block.parameters.is_empty())
        || body
            .blocks
            .iter()
            .map(|block| block.operations.len())
            .sum::<usize>()
            != 8
    {
        return Err("complete block/operation bound");
    }
    let mut program = None;
    let mut index = None;
    let mut length = None;
    let mut compare = None;
    let mut data = None;
    let mut address = None;
    let mut cast = None;
    let mut store = None;
    for block in &body.blocks {
        for operation in &block.operations {
            if !operation.has_complete_effect_summary() {
                return Err("unknown effect");
            }
            match &operation.kind {
                OperationKind::Gfx942OrderedProgram(value) => {
                    if program.replace((one(operation, &u32_ty)?, value)).is_some()
                        || value.inputs() != &[*a, *b, *c]
                    {
                        return Err("program boundary");
                    }
                    let r = value.registers();
                    let roles = [
                        r.scratch(),
                        r.output(),
                        r.inputs()[0],
                        r.inputs()[1],
                        r.inputs()[2],
                    ];
                    // v0:v1 = index, v2:v3 = address, v4 = pointer-high input;
                    // v5:v7 remain reserved.
                    if roles.iter().any(|r| *r < 8) {
                        return Err("program collides with diagnostic ABI scratch");
                    }
                }
                OperationKind::Intrinsic(value)
                    if value.kind
                        == (IntrinsicKind::InvocationIndex {
                            kind: IndexKind::Global,
                            axis: Axis::X,
                        })
                        && value.result_type == Type::INDEX =>
                {
                    if index.replace(one(operation, &Type::INDEX)?).is_some() {
                        return Err("duplicate index");
                    }
                }
                OperationKind::SliceLength { slice } if slice == out => {
                    if length.replace(one(operation, &Type::INDEX)?).is_some() {
                        return Err("duplicate length");
                    }
                }
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs,
                    rhs,
                } => {
                    if compare
                        .replace((one(operation, &Type::BOOL)?, *lhs, *rhs))
                        .is_some()
                    {
                        return Err("duplicate compare");
                    }
                }
                OperationKind::SliceData { slice } if slice == out => {
                    let ptr =
                        Type::pointer(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
                    if data.replace(one(operation, &ptr)?).is_some() {
                        return Err("duplicate slice data");
                    }
                }
                OperationKind::GetElementPointer { base, offset } => {
                    let ptr =
                        Type::pointer(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
                    if address
                        .replace((one(operation, &ptr)?, *base, *offset))
                        .is_some()
                    {
                        return Err("duplicate address");
                    }
                }
                OperationKind::Cast {
                    kind: CastKind::ZeroExtend,
                    value,
                    to,
                } if matches!(to, Type::Scalar(ScalarType::U64 | ScalarType::I64)) => {
                    // Rust's Option discriminant is isize, lowered as I64.
                    // Below we require the source to be this exact BOOL guard;
                    // zero extension therefore yields only 0 or 1, independent
                    // of the destination integer's signedness.
                    if cast.replace((one(operation, to)?, *value)).is_some() {
                        return Err("duplicate cast");
                    }
                }
                OperationKind::Store {
                    pointer,
                    value,
                    access,
                } if operation.results.is_empty()
                    && *access == MemoryAccess::new(AddressSpace::Global, 4) =>
                {
                    if store.replace((block.id, *pointer, *value)).is_some() {
                        return Err("duplicate store");
                    }
                }
                _ => return Err("unsupported operation or memory effect"),
            }
        }
    }
    let (result, program) = program.ok_or("missing program")?;
    let index = index.ok_or("missing global index")?;
    let length = length.ok_or("missing slice length")?;
    let (predicate, lhs, rhs) = compare.ok_or("missing guard")?;
    let data = data.ok_or("missing slice data")?;
    let (pointer, base, offset) = address.ok_or("missing address")?;
    let (selector, cast_value) = cast.ok_or("missing boolean extension")?;
    let (store_block, store_pointer, store_value) = store.ok_or("missing write")?;
    if (lhs, rhs) != (index, length)
        || (base, offset) != (data, index)
        || cast_value != predicate
        || (store_pointer, store_value) != (pointer, result)
    {
        return Err("changed dataflow or store value/address");
    }
    let mut prefix = [false; 32];
    let mut cursor = body.blocks[0].id;
    let mut prefix_operations = 0;
    let (yes, no, default) = loop {
        mark(body, cursor, &mut prefix)?;
        let current = block(body, cursor)?;
        if cursor == store_block {
            return Err("unguarded store");
        }
        prefix_operations += current.operations.len();
        match &current.terminator {
            Some(Terminator::Branch { target, arguments }) if arguments.is_empty() => {
                cursor = *target
            }
            Some(Terminator::Switch {
                selector: actual,
                cases,
                default_target,
                default_arguments,
            }) if *actual == selector
                && default_arguments.is_empty()
                && cases.len() == 2
                && cases.iter().all(|case| case.arguments.is_empty()) =>
            {
                let no = cases
                    .iter()
                    .find(|case| case.value == 0)
                    .ok_or("missing false case")?
                    .target;
                let yes = cases
                    .iter()
                    .find(|case| case.value == 1)
                    .ok_or("missing true case")?
                    .target;
                break (yes, no, *default_target);
            }
            Some(Terminator::IntegerSwitch {
                selector: actual,
                cases,
                default_target,
                default_arguments,
            }) if *actual == selector
                && default_arguments.is_empty()
                && cases.len() == 2
                && cases.iter().all(|case| case.arguments.is_empty()) =>
            {
                let bit = |v: &Constant, bit| matches!(v, Constant::U64(x) | Constant::Index(x) if *x == bit);
                let no = cases
                    .iter()
                    .find(|case| bit(&case.value, 0))
                    .ok_or("missing false case")?
                    .target;
                let yes = cases
                    .iter()
                    .find(|case| bit(&case.value, 1))
                    .ok_or("missing true case")?
                    .target;
                break (yes, no, *default_target);
            }
            _ => return Err("unsupported complete control flow"),
        }
    };
    if prefix_operations != 7 {
        return Err("guard prefix does not cover all value operations");
    }
    let mut covered = prefix;
    tail(body, yes, store_block, true, &mut covered, &prefix)?;
    tail(body, no, store_block, false, &mut covered, &prefix)?;
    let unreachable = block(body, default)?;
    if !unreachable.operations.is_empty()
        || !matches!(unreachable.terminator, Some(Terminator::Unreachable))
    {
        return Err("switch default is not the impossible boolean case");
    }
    mark(body, default, &mut covered)?;
    if covered[..body.blocks.len()].iter().any(|v| !v) {
        return Err("uncovered block");
    }
    // Reuse exact-owner profile/capability/target/launch checks; do not parse or
    // edit the ordinary LLVM text. Existing emitter has a separate 16 MiB cap.
    fe2o3_amdgcn_model::lower_canonical_v17_compiler_module_to_gfx942_xnack_minus_llvm_ir(owner)
        .map_err(|_| "existing exact-target owner lowering refused")?;
    Ok(Profile { symbol, program })
}
