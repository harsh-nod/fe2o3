//! Test-only positional transport observations from the SAME live checked owner.
//! No bytes-to-owner constructor, machine interpretation or runtime authority.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, Function, FunctionRole, Module, Operation, OperationKind,
    OrderedProgramSiteV1, ScalarType, Terminator, Type, ValueId,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Ledger,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::ProductionOrderedCompositionCheckedKirOwnerV1 as Checked;

pub(super) const SCHEMA: &str = "fe2o3-test-composition-transport-roles-v1";
pub(super) const MODE: &str = "transport-roles";
pub(super) const FRAME_PREFIX: &str = "FE2O3_COMPOSITION_TRANSPORT_ROLES_V1 ";
const ROW_CAP: usize = 64;
const JSON_CAP: usize = 32 * 1024;
const FILE_CAP: usize = 4 * 1024 * 1024;
// Two fixed projections plus JSON tree/encoded copy/outer-frame role contribution.
// This is a logical payload envelope, not allocator overhead or whole compiler RSS.
const ROLE_STORAGE: usize = 2 * std::mem::size_of::<Rows>() + 3 * JSON_CAP;
const ROLE_WORK: usize = 65_536 + 3 * JSON_CAP;
const U32: Type = Type::Scalar(ScalarType::U32);
const NONE: u32 = u32::MAX;

// Closed row tags; every row is four u32 words, exactly 16 logical bytes.
const ROOT: u32 = 1;
const ROOT_PARAMETER: u32 = 2;
const HELPER: u32 = 3;
const HELPER_PARAMETER: u32 = 4;
const SITE: u32 = 5;
const SITE_BLOCK: u32 = 6;
const CALL_RESULT: u32 = 7;
const CALL_ARGUMENT: u32 = 8;
const PROGRAM_INPUT: u32 = 9;
const PROGRAM_RESULT: u32 = 10;
const REGISTERS: u32 = 11;
const INPUT_REGISTER: u32 = 12;
const DESCRIPTOR: u32 = 13;
const RETURN_VALUE: u32 = 14;
const STORE_DATA: u32 = 15;
const IDENTITY: u32 = 16;
const ABI_POINTER: u32 = 17;
const ABI_LENGTH: u32 = 18;
const ABI_SCALAR: u32 = 19;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rows {
    values: [[u32; 4]; ROW_CAP],
    len: usize,
}
impl Rows {
    fn new() -> Self {
        Self {
            values: [[0; 4]; ROW_CAP],
            len: 0,
        }
    }
    fn push(&mut self, row: [u32; 4]) -> Result<(), &'static str> {
        if self.len == ROW_CAP {
            return Err("transport role row cap");
        }
        self.values[self.len] = row;
        self.len += 1;
        Ok(())
    }
    fn slice(&self) -> &[[u32; 4]] {
        &self.values[..self.len]
    }
    fn site(
        &mut self,
        role: u32,
        function: u32,
        block: usize,
        actual: &BasicBlock,
        operation: usize,
    ) -> Result<(), &'static str> {
        self.push([SITE, role, block as u32, operation as u32])?;
        self.push([SITE_BLOCK, role, actual.id.0, function])
    }
}
// An inert coordinate projection; it cannot construct any canonical/source owner.
#[derive(Clone, Copy)]
struct Site {
    function: u32,
    ordinal: u32,
    block: fe2o3_kernel_ir::BlockId,
    operation: u32,
}
impl From<OrderedProgramSiteV1> for Site {
    fn from(site: OrderedProgramSiteV1) -> Self {
        Self {
            function: site.function_ordinal(),
            ordinal: site.block_ordinal(),
            block: site.block(),
            operation: site.operation_ordinal(),
        }
    }
}
impl Site {
    fn function_ordinal(self) -> u32 {
        self.function
    }
    fn block_ordinal(self) -> u32 {
        self.ordinal
    }
    fn block(self) -> fe2o3_kernel_ir::BlockId {
        self.block
    }
    fn operation_ordinal(self) -> u32 {
        self.operation
    }
}
#[derive(Clone, Copy)]
struct Selection {
    root: u32,
    helper: u32,
    definition: Site,
    call: Site,
}
fn require(value: bool, error: &'static str) -> Result<(), &'static str> {
    if value { Ok(()) } else { Err(error) }
}
fn selected(checked: &Checked) -> Result<Selection, &'static str> {
    let owner = checked.composition();
    require(
        owner.definitions().len() == 1
            && owner.helpers().len() == 1
            && owner.calls().len() == 1
            && owner.occurrences().len() == 1,
        "transport requires exactly one helper, call, program and occurrence",
    )?;
    let helper = owner.helpers()[0];
    let call = owner.calls()[0];
    let definition = owner.definitions()[0];
    let occurrence = owner.occurrences()[0];
    require(
        call.callee() == helper.key()
            && call.site().function_ordinal() == owner.root_function_ordinal()
            && definition.site().function_ordinal() == helper.function_ordinal()
            && occurrence.definition() == definition.key()
            && occurrence.incoming_call() == Some(call.key())
            && occurrence.root_function_ordinal() == owner.root_function_ordinal(),
        "transport actual composition roster join",
    )?;
    Ok(Selection {
        root: owner.root_function_ordinal(),
        helper: helper.function_ordinal(),
        definition: definition.site().into(),
        call: call.site().into(),
    })
}
fn actual(module: &Module, site: Site) -> Result<(&BasicBlock, &Operation), &'static str> {
    let f = module
        .functions
        .get(site.function_ordinal() as usize)
        .ok_or("transport foreign function")?;
    let body = f.body.as_ref().ok_or("transport missing body")?;
    let block = body
        .blocks
        .get(site.block_ordinal() as usize)
        .ok_or("transport foreign block")?;
    require(block.id == site.block(), "transport mismatched block ID")?;
    let op = block
        .operations
        .get(site.operation_ordinal() as usize)
        .ok_or("transport foreign operation")?;
    Ok((block, op))
}
fn value_type(function: &Function, value: ValueId) -> Result<&Type, &'static str> {
    let body = function
        .body
        .as_ref()
        .ok_or("transport missing function body")?;
    let mut found = None;
    let mut retain = |id: ValueId, ty| -> Result<(), &'static str> {
        if id == value {
            require(found.is_none(), "transport repeated value definition")?;
            found = Some(ty);
        }
        Ok(())
    };
    for (&id, ty) in body.parameters.iter().zip(&function.signature.parameters) {
        retain(id, ty)?;
    }
    for block in &body.blocks {
        for parameter in &block.parameters {
            retain(parameter.id, &parameter.ty)?;
        }
        for operation in &block.operations {
            for result in &operation.results {
                retain(result.id, &result.ty)?;
            }
        }
    }
    found.ok_or("transport undefined value")
}
fn result(op: &Operation) -> Result<ValueId, &'static str> {
    let [value] = op.results.as_slice() else {
        return Err("transport one u32 result");
    };
    require(value.ty == U32, "transport result is not u32")?;
    Ok(value.id)
}
fn output_type(ty: &Type) -> Result<u32, &'static str> {
    let Type::Slice(slice) = ty else {
        return Err("transport output is not slice");
    };
    require(
        slice.element.as_ref() == &U32 && slice.address_space == AddressSpace::Global,
        "transport output element/space",
    )?;
    match slice.access {
        AccessMode::ReadWrite => Ok(2),
        AccessMode::WriteOnly => Ok(3),
        AccessMode::ReadOnly => Err("transport output is readonly"),
    }
}
fn signature(rows: &mut Rows, f: &Function, tag: u32, output: bool) -> Result<(), &'static str> {
    let body = f.body.as_ref().ok_or("transport body")?;
    let expected = if output { 4 } else { 3 };
    require(
        body.parameters.len() == expected && f.signature.parameters.len() == expected,
        "transport positional parameter count",
    )?;
    for (position, (&id, ty)) in body
        .parameters
        .iter()
        .zip(&f.signature.parameters)
        .enumerate()
    {
        let kind = if output && position == 0 {
            output_type(ty)?
        } else {
            require(ty == &U32, "transport positional parameter type")?;
            1
        };
        require(value_type(f, id)? == ty, "transport parameter definition")?;
        rows.push([tag, position as u32, id.0, kind])?;
    }
    require(
        if output {
            f.signature.results.is_empty()
        } else {
            f.signature.results == [U32]
        },
        "transport signature results",
    )
}
/// Pure bounded projection. Only selected() over a retained Checked owner calls
/// this in the actual route. Inert unit tests exercise refusal, not source custody.
fn project(module: &Module, s: Selection, edited: bool) -> Result<Rows, &'static str> {
    require(
        module.functions.len() == 2
            && module.kernels.len() == 1
            && module.kernels[0].id.as_str().len() <= 256
            && s.root != s.helper,
        "transport finite function roster",
    )?;
    let mut total_blocks = 0;
    let mut total_operations = 0;
    for function in &module.functions {
        require(function.id.as_str().len() <= 256, "transport symbol cap")?;
        let body = function
            .body
            .as_ref()
            .ok_or("transport external function")?;
        require(
            !body.blocks.is_empty() && body.blocks.len() <= 16,
            "transport block cap",
        )?;
        total_blocks += body.blocks.len();
        require(total_blocks <= 16, "transport aggregate block cap")?;
        for block in &body.blocks {
            require(
                block.operations.len() <= 128 && block.parameters.len() <= 4,
                "transport operation/parameter cap",
            )?;
            total_operations += block.operations.len();
            require(total_operations <= 128, "transport aggregate operation cap")?;
            for op in &block.operations {
                require(op.results.len() <= 4, "transport result cap")?;
            }
        }
    }
    let root = module
        .functions
        .get(s.root as usize)
        .ok_or("transport root ordinal")?;
    let helper = module
        .functions
        .get(s.helper as usize)
        .ok_or("transport helper ordinal")?;
    require(
        root.role == FunctionRole::KernelEntry
            && helper.role == FunctionRole::InternalHelper
            && module.kernels[0].entry == root.id,
        "transport actual root/helper role",
    )?;
    require(
        s.call.function_ordinal() == s.root && s.definition.function_ordinal() == s.helper,
        "transport foreign selected site",
    )?;
    let mut rows = Rows::new();
    rows.push([ROOT, s.root, 4, 0])?;
    signature(&mut rows, root, ROOT_PARAMETER, true)?;
    rows.push([HELPER, s.helper, 3, 1])?;
    signature(&mut rows, helper, HELPER_PARAMETER, false)?;
    let root_parameters = &root.body.as_ref().unwrap().parameters;
    let helper_parameters = &helper.body.as_ref().unwrap().parameters;
    let (call_block, call) = actual(module, s.call)?;
    let OperationKind::Call { callee, arguments } = &call.kind else {
        return Err("transport actual selected call");
    };
    require(
        callee == &helper.id && arguments.as_slice() == &root_parameters[1..],
        "transport positional call argument ancestry",
    )?;
    let call_result = result(call)?;
    rows.site(
        1,
        s.root,
        s.call.block_ordinal() as usize,
        call_block,
        s.call.operation_ordinal() as usize,
    )?;
    rows.push([CALL_RESULT, s.helper, call_result.0, 1])?;
    for (position, (&argument, &formal)) in arguments.iter().zip(helper_parameters).enumerate() {
        require(
            value_type(root, argument)? == &U32,
            "transport call argument type",
        )?;
        rows.push([CALL_ARGUMENT, position as u32, argument.0, formal.0])?;
        rows.push([
            IDENTITY,
            position as u32,
            root_parameters[position + 1].0,
            argument.0,
        ])?;
    }
    let (program_block, operation) = actual(module, s.definition)?;
    let OperationKind::Gfx942OrderedProgram(program) = &operation.kind else {
        return Err("transport actual selected ordered program");
    };
    require(
        program.inputs().as_slice() == helper_parameters.as_slice(),
        "transport positional formal/program ancestry",
    )?;
    let regs = program.registers();
    require(
        regs.scratch() == 8 && regs.output() == 9 && regs.inputs() == [10, 11, 12],
        "transport fixed authored register profile",
    )?;
    let expected: &[u16] = if edited { &[40] } else { &[133, 315] };
    require(
        program.program().instructions().count() == expected.len()
            && program
                .program()
                .instructions()
                .map(|i| i.descriptor())
                .eq(expected.iter().copied()),
        "transport exact authored descriptors",
    )?;
    let program_result = result(operation)?;
    rows.site(
        2,
        s.helper,
        s.definition.block_ordinal() as usize,
        program_block,
        s.definition.operation_ordinal() as usize,
    )?;
    rows.push([
        REGISTERS,
        u32::from(regs.scratch()),
        u32::from(regs.output()),
        expected.len() as u32,
    ])?;
    for (position, (input, register)) in program
        .inputs()
        .iter()
        .copied()
        .zip(regs.inputs())
        .enumerate()
    {
        require(
            value_type(helper, input)? == &U32,
            "transport program input type",
        )?;
        rows.push([PROGRAM_INPUT, position as u32, input.0, 1])?;
        rows.push([INPUT_REGISTER, position as u32, u32::from(register), 0])?;
    }
    for (position, descriptor) in program.program().instructions().enumerate() {
        rows.push([
            DESCRIPTOR,
            position as u32,
            u32::from(descriptor.descriptor()),
            0,
        ])?;
    }
    rows.push([PROGRAM_RESULT, program_result.0, 1, 0])?;
    let mut returns = 0;
    for (ordinal, block) in helper.body.as_ref().unwrap().blocks.iter().enumerate() {
        if let Some(Terminator::Return { values }) = &block.terminator {
            returns += 1;
            require(returns == 1, "transport unique helper return")?;
            require(
                values.as_slice() == [program_result],
                "transport program/return DATA ancestry",
            )?;
            require(
                value_type(helper, values[0])? == &U32,
                "transport return type",
            )?;
            rows.site(3, s.helper, ordinal, block, block.operations.len())?;
            rows.push([RETURN_VALUE, values[0].0, call_result.0, s.helper])?;
        }
    }
    require(returns == 1, "transport unique helper return")?;
    let mut stores = 0;
    for (ordinal, block) in root.body.as_ref().unwrap().blocks.iter().enumerate() {
        for (index, op) in block.operations.iter().enumerate() {
            let (pointer, data, predicate, access) = match &op.kind {
                OperationKind::Store {
                    pointer,
                    value,
                    access,
                } => (*pointer, *value, None, access),
                OperationKind::GuardedStore {
                    pointer,
                    value,
                    predicate,
                    access,
                } => (*pointer, *value, Some(*predicate), access),
                _ => continue,
            };
            stores += 1;
            require(stores == 1, "transport unique global store")?;
            require(
                data == call_result && value_type(root, data)? == &U32,
                "transport call/store DATA ancestry",
            )?;
            let Type::Pointer(pointer_ty) = value_type(root, pointer)? else {
                return Err("transport store pointer type");
            };
            require(
                pointer_ty.address_space == AddressSpace::Global
                    && pointer_ty.pointee.as_ref() == &U32
                    && access.address_space == AddressSpace::Global,
                "transport store space/pointee",
            )?;
            if let Some(predicate) = predicate {
                require(
                    value_type(root, predicate)? == &Type::BOOL,
                    "transport predicate type",
                )?;
            }
            rows.site(4, s.root, ordinal, block, index)?;
            rows.push([
                STORE_DATA,
                data.0,
                pointer.0,
                predicate.map_or(NONE, |v| v.0),
            ])?;
            rows.push([IDENTITY, 3, call_result.0, data.0])?;
        }
    }
    require(stores == 1, "transport unique global store")?;
    Ok(rows)
}
fn check_components(
    position: usize,
    actual: impl Iterator<
        Item = (
            fe2o3_kernel_descriptor::PhysicalAbiComponentKind,
            u32,
            u16,
            u16,
        ),
    >,
) -> Result<(), &'static str> {
    use fe2o3_kernel_descriptor::{PhysicalAbiComponentKind as Kind, ScalarTypeV1 as Scalar};
    let expected: &[(Kind, u32, u16, u16)] = match position {
        0 => &[
            (Kind::GlobalPointer, 0, 8, 8),
            (Kind::SliceLengthU64, 8, 8, 8),
        ],
        1 => &[(Kind::ScalarByValue(Scalar::U32), 16, 4, 4)],
        2 => &[(Kind::ScalarByValue(Scalar::U32), 20, 4, 4)],
        3 => &[(Kind::ScalarByValue(Scalar::U32), 24, 4, 4)],
        _ => return Err("transport descriptor extra logical argument"),
    };
    require(
        actual.eq(expected.iter().copied()),
        "transport actual component order/offset/width/alignment",
    )
}
fn append_abi(
    rows: &mut Rows,
    descriptor: &fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
    module: &Module,
) -> Result<(), &'static str> {
    use fe2o3_kernel_descriptor::{PhysicalAbiComponentKind as Kind, ScalarTypeV1 as Scalar};
    let [kernel] = descriptor.table().kernels() else {
        return Err("transport descriptor root count");
    };
    require(
        kernel.entry_name().as_str() == module.kernels[0].id.as_str()
            && kernel.arguments().len() == 4,
        "transport same source descriptor root",
    )?;
    for (position, argument) in kernel.arguments().iter().enumerate() {
        require(
            usize::from(argument.source_index()) == position,
            "transport descriptor logical parameter order",
        )?;
        check_components(position, argument.physical_components())?;
        for (kind, offset, size, alignment) in argument.physical_components() {
            let tag = match kind {
                Kind::GlobalPointer => ABI_POINTER,
                Kind::SliceLengthU64 => ABI_LENGTH,
                Kind::ScalarByValue(Scalar::U32) => ABI_SCALAR,
                _ => return Err("transport unsupported ABI component"),
            };
            rows.push([
                tag,
                position as u32,
                offset,
                (u32::from(size) << 16) | u32::from(alignment),
            ])?;
        }
    }
    Ok(())
}
fn prepay(budget: &mut Budget<'_>, floor: usize, llvm_bytes: usize) -> Result<(), Resource> {
    budget.charge_work(ROLE_WORK)?;
    if budget.storage() < floor || llvm_bytes > FILE_CAP {
        return Err(Resource::Accounting);
    }
    let reserve = ROLE_STORAGE
        .checked_add(llvm_bytes)
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(reserve)
}
fn matches_claim(actual: &Rows, claimed: &Rows) -> Result<(), &'static str> {
    require(
        actual == claimed,
        "transport role claim differs from same live owner",
    )
}
fn controls(actual: &Rows) -> usize {
    // This only tests immutable-observation substitution. It does not mint or
    // mutate a checked owner, and is not presented as a source replay test.
    for tag in [
        ROOT,
        ROOT_PARAMETER,
        HELPER,
        HELPER_PARAMETER,
        SITE,
        SITE_BLOCK,
        CALL_RESULT,
        CALL_ARGUMENT,
        PROGRAM_INPUT,
        PROGRAM_RESULT,
        DESCRIPTOR,
        RETURN_VALUE,
        STORE_DATA,
        ABI_POINTER,
        ABI_LENGTH,
        ABI_SCALAR,
    ] {
        let mut changed = actual.clone();
        let row = changed.values[..changed.len]
            .iter_mut()
            .find(|row| row[0] == tag)
            .unwrap();
        row[2] ^= 1;
        assert!(matches_claim(actual, &changed).is_err());
    }
    {
        let mut missing = actual.clone();
        missing.len -= 1;
        assert!(matches_claim(actual, &missing).is_err());
    }
    {
        let mut extra = actual.clone();
        extra.push([IDENTITY, 63, 0, 0]).unwrap();
        assert!(matches_claim(actual, &extra).is_err());
    }
    18
}
fn publication_fields(
    record: &Value,
    snapshot: &Value,
    request_sha256: &str,
    variant: &str,
) -> Result<(), &'static str> {
    require(
        record["case"] == variant
            && record["actual_extractor"] == true
            && record["exit"] == 0
            && record["result"]["stage"] == "actual_cli_published"
            && record["source_custody_from_files"] == false
            && record["hardware_observed"] == false
            && record["protected_authority"] == false,
        "transport closed publication metadata",
    )?;
    require(
        record["result"]["candidate"] == *snapshot,
        "transport published candidate snapshot",
    )?;
    let promotion = &record["result"]["report"]["source_promotion"];
    require(
        record["result"]["report"]["schema"] == "fe2o3-ordered-composition-diagnostic-v1"
            && promotion["created_new"] == true
            && promotion["original_overwritten"] == false
            && promotion["fresh_compilation_required"] == true
            && promotion["grants_artifact_or_launch_authority"] == false
            && promotion["source_custody_exported"] == false
            && promotion["program_edited"] == (variant == "edit")
            && promotion["request_sha256"] == request_sha256,
        "transport publication request/profile join",
    )?;
    for (left, right) in [
        ("bytes", "candidate_bytes"),
        ("sha256", "candidate_sha256"),
        ("device", "candidate_device"),
        ("inode", "candidate_inode"),
    ] {
        require(
            snapshot[left] == promotion[right] && !snapshot[left].is_null(),
            "transport publication byte/inode join",
        )?;
    }
    Ok(())
}
fn publication(root: &Path, variant: &str, budget: &mut Budget<'_>) -> Result<Value, String> {
    // Existing parent creates these through the real public CLI. Reading them
    // joins inert metadata only; source authority comes from the new rustc owner.
    const INPUT_CAP: usize = JSON_CAP + 8192 + 72 * 1024;
    budget
        .with_prepaid_scope::<_, Resource>(
            budget.storage(),
            1,
            4 * INPUT_CAP,
            3 * INPUT_CAP,
            |_| {
                Ok((|| -> Result<Value, String> {
                    let bytes =
                        read_bounded(&root.join(format!("cli-{variant}.accepted.json")), JSON_CAP)?;
                    let record: Value =
                        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                    let source = inputs::snapshot(&inputs::source(root, variant), 72 * 1024);
                    let snapshot = serde_json::to_value(&source).map_err(|e| e.to_string())?;
                    let request =
                        read_bounded(&root.join(format!("{variant}.request.json")), 8192)?;
                    let request_sha256 = digest(&request);
                    publication_fields(&record, &snapshot, &request_sha256, variant)?;
                    Ok(
                        json!({"report_bytes":bytes.len(),"report_sha256":digest(&bytes),
                    "request_bytes":request.len(),"request_sha256":request_sha256,
                    "candidate":snapshot,"inert_metadata_join_only":true}),
                    )
                })())
            },
        )
        .map_err(|e| e.to_string())?
}
pub(super) fn case(variant: &str) -> Result<String, &'static str> {
    require(
        ["copy", "preserve", "edit"].contains(&variant),
        "transport closed published variant",
    )?;
    Ok(format!("{variant}-{MODE}"))
}
pub(super) fn observe(
    mut target: crate::production_pipeline::ordered_composition_target_v1::AuthenticatedOrderedCompositionTargetModuleV1<'_>,
    variant: &str,
    output: &Path,
    started: std::time::Instant,
) -> Result<(Value, Ledger), String> {
    require(
        ["copy", "preserve", "edit"].contains(&variant),
        "transport closed published variant",
    )?;
    let edited = variant == "edit";
    let root = output.parent().ok_or("transport output parent")?;
    let (rows, llvm, published) =
        target.observe_transport_with_budget(|checked, llvm, budget| {
            prepay(budget, checked.retained_storage(), llvm.len()).map_err(|e| e.to_string())?;
            let rows = project(checked.executable().module(), selected(checked)?, edited)?;
            let published = publication(root, variant, budget)?;
            Ok::<_, String>((rows, llvm.to_owned(), published))
        })?;
    let cpu = publisher::publication::cpu(target.checked().executable(), edited, started);
    require(cpu["cases"] == 32, "transport exact independent CPU corpus")?;
    let prepared =
        crate::production_worker_handoff::prepare_ordered_composition_worker_handoff_v1(target)
            .map_err(|e| e.to_string())?;
    prepared.into_inert_transport_observation(|checked, handoff, descriptor, budget| {
        let current = project(checked.executable().module(), selected(checked)?, edited)?;
        matches_claim(&current, &rows)?;
        drop(rows);
        let mut roles = current;
        append_abi(&mut roles, descriptor, checked.executable().module())?;
        let refusals = controls(&roles);
        let module_bytes = handoff.module_bytes();
        require(!handoff.authenticates_compiler_origin() && !handoff.grants_compiler_authority()
            && !descriptor.authenticates_compiler_origin() && !descriptor.grants_launch_authority(),
            "transport inert extraction")?;
        require(module_bytes.len() <= FILE_CAP && descriptor.canonical_bytes().len() <= JSON_CAP,
            "transport descriptor relation input cap")?;
        let worker = std::str::from_utf8(module_bytes).map_err(|e|e.to_string())?;
        let scratch = (worker.len() + 1).checked_mul(4).and_then(|v|v.checked_add(1024))
            .ok_or("transport descriptor relation storage overflow")?;
        let related = budget.with_prepaid_scope::<_, Resource>(
            budget.storage(), 1, (worker.len() + 1) * 4, scratch, |_| {
                let relation = crate::kernel_ir_codegen::exact_ordered_composition_descriptor_extension_v1;
                Ok(relation(&llvm, worker, descriptor)
                    && !relation(&llvm, &format!("{worker}\n"), descriptor)
                    && !relation(&format!("{llvm}\n"), worker, descriptor)
                    && !relation(&llvm, &llvm, descriptor))
            }).map_err(|e|e.to_string())?;
        require(related, "transport exact canonical/worker/descriptor join")?;
        let canonical = checked.executable().canonical_bytes();
        require(canonical.len() <= FILE_CAP && module_bytes.len() <= FILE_CAP
            && handoff.canonical_bytes().len() <= FILE_CAP
            && descriptor.canonical_bytes().len() <= JSON_CAP, "transport output byte caps")?;
        budget.charge_work(canonical.len() + llvm.len() + module_bytes.len()
            + descriptor.canonical_bytes().len() + handoff.canonical_bytes().len()).map_err(|e|e.to_string())?;
        let executable = checked.executable();
        let owner = checked.composition();
        let module = executable.module();
        let s = selected(checked)?;
        let (_, definition) = actual(module, s.definition)?;
        let OperationKind::Gfx942OrderedProgram(program) = &definition.kind else { unreachable!() };
        let source = program.source();
        let conditions = checked.launch_envelope_requirements();
        require(checked.source_launch().roots().len() == 1
            && checked.source_launch().roots()[0].layout().global_extents() == [128,1,1]
            && conditions.len() == 1 && conditions[0].parameter_index() == 0
            && conditions[0].minimum_byte_len() == 512
            && !conditions[0].requires_initialized_read() && conditions[0].requires_write_permission(),
            "transport retained source launch/formal conditions")?;
        require(publication(root,variant,budget)? == published, "transport publication changed during continuation")?;
        let observed = json!({
            "schema":SCHEMA, "source_profile":variant, "publication":published,
            "semantic_identity":driver::lower_hex_v1(checked.semantic_ssa().source_semantic().semantic_sha256().as_bytes()),
            "canonical_identity":driver::lower_hex_v1(executable.identity().digest()),
            "canonical_bytes":canonical.len(), "canonical_sha256":digest(canonical),
            "llvm_bytes":llvm.len(), "llvm_sha256":digest(llvm.as_bytes()),
            "worker_bytes":module_bytes.len(), "worker_sha256":digest(module_bytes),
            "handoff_bytes":handoff.canonical_bytes().len(), "handoff_sha256":digest(handoff.canonical_bytes()),
            "descriptor_abi":{
                "explicit_argument_size":descriptor.table().kernels()[0].abi_layout().explicit_argument_size(),
                "kernarg_segment_size":descriptor.table().kernels()[0].abi_layout().kernarg_segment_size(),
                "kernarg_segment_alignment":descriptor.table().kernels()[0].abi_layout().kernarg_segment_alignment()
            },
            "descriptor_bytes":descriptor.canonical_bytes().len(),
            "descriptor_sha256":digest(descriptor.canonical_bytes()),
            "root_symbol":module.functions[s.root as usize].id.as_str(),
            "entry_symbol":module.kernels[0].id.as_str(),
            "helper_symbol":module.functions[s.helper as usize].id.as_str(),
            "definition_source":{
                "frontend_unit":driver::lower_hex_v1(&source.frontend_unit),
                "function":driver::lower_hex_v1(&source.function),
                "contract":driver::lower_hex_v1(&source.contract),
                "statement":driver::lower_hex_v1(&source.statement)
            },
            "definition_key":owner.definitions()[0].key().ordinal(),
            "call_key":owner.calls()[0].key().ordinal(),
            "occurrence_key":owner.occurrences()[0].key().ordinal(),
            "row_count":roles.len, "rows":roles.slice(), "cpu":cpu,
            "role_substitution_refusals":refusals, "descriptor_extension_refusals":3,
            "output_condition":{"parameter":0,"minimum_bytes":512,"initialized_read":false,"write_permission":true},
            "logical_role_storage_reserved":ROLE_STORAGE,
            "same_ledger_storage_at_observation":budget.storage(),
            "same_ledger_work_at_observation":budget.work(),
            "pointer_and_predicate_recorded_not_proved":true,
            "ancestry_language":"direct intra-function SSA plus actual positional call/return",
            "compiler_owner_replayed_by_normal_descriptor":true,
            "source_custody_exported":false, "machine_abi_transport_proved":false,
            "native_functional_equivalence":false, "runtime_conditions_discharged":false,
            "hardware_observed":false, "protected_authority":false
        });
        // Prepaid tree + encoded export + outer-frame contribution. Do not refund
        // when this Vec is dropped: the original ledger survives caller output.
        let encoded = serde_json::to_vec(&observed).map_err(|e|e.to_string())?;
        require(encoded.len() <= JSON_CAP, "transport role JSON cap")?;
        timely(started.elapsed(), 300)?;
        fs::create_dir(output).map_err(|e|e.to_string())?;
        for (name, bytes) in [
            ("canonical-v17.bin", canonical),
            ("canonical.ll", llvm.as_bytes()),
            ("worker.ll", module_bytes),
            ("handoff-v2.bin", handoff.canonical_bytes()),
            ("descriptor-v1.bin", descriptor.canonical_bytes()),
            ("transport-roles.json", encoded.as_slice()),
        ] {
            driver::publish_new_inert_output(&output.join(name), bytes, FILE_CAP, name)
                .map_err(|e|e.to_string())?;
        }
        timely(started.elapsed(), 300)?;
        Ok(observed)
    })
}

#[path = "gfx942_ordered_composition_transport_roles_controls_v1_tests.rs"]
mod controls_tests;
