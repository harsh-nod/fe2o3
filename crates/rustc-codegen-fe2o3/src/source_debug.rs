//! Bounded source-debug metadata for the S09 alpha O0 pilot.

use crate::AmdGpuTarget;
use crate::collector::{CollectionResult, TypedKernelProfile};
use rustc_middle::mir::{Body, VarDebugInfoContents};
use rustc_middle::ty::{FloatTy, TyCtxt, TyKind, UintTy};
use rustc_span::Span;
use sha2::{Digest, Sha256};
use std::env;
use std::error::Error;
use std::fmt::{self, Write};

pub(crate) const SOURCE_DEBUG_PROFILE_ENV: &str = "FE2O3_WORKER_V2_SOURCE_DEBUG_PROFILE_V1";
const S09_ALPHA_PROFILE: &str = "s09-alpha-gfx942-o0-v1";
const S09_CRATE_NAME: &str = "fe2o3_typed_alias_spoof";
const S09_DEF_PATH: &str = "general_genuine::__fe2o3_host_kernel_v1_2d2a566a37ac0eca1d21361c2da8616f124a89c9c4b5b02365e24633c914de06";
const S09_SOURCE_PATH: &str =
    "crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs";
const S09_SOURCE_SHA256: [u8; 32] = [
    0xa0, 0x2f, 0x62, 0xa7, 0x31, 0x98, 0xb4, 0x93, 0x25, 0x82, 0x24, 0x70, 0x1c, 0x4f, 0x29, 0xe2,
    0x5b, 0x3e, 0xca, 0x02, 0xa7, 0x38, 0xbf, 0x02, 0xc0, 0x39, 0x89, 0xd4, 0x5b, 0x77, 0x09, 0x9e,
];
const S09_FUNCTION_LINE: usize = 68;
const S09_INDEX_LINE: usize = 69;
const S09_LOCAL_LINE: usize = 70;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AlphaSourceDebugV1 {
    source_file: String,
    source_directory: String,
    function_line: usize,
    index_line: usize,
    local_line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SourceDebugError(String);

impl SourceDebugError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for SourceDebugError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for SourceDebugError {}

pub(crate) fn collect_requested_profile<'tcx>(
    tcx: TyCtxt<'tcx>,
    collection: &CollectionResult<'tcx>,
    mir_module: &crate::mir_import::MirModule,
    target: &AmdGpuTarget,
) -> Result<Option<AlphaSourceDebugV1>, SourceDebugError> {
    let requested = match env::var(SOURCE_DEBUG_PROFILE_ENV) {
        Err(env::VarError::NotPresent) => return Ok(None),
        Err(env::VarError::NotUnicode(_)) => {
            return Err(SourceDebugError::new(format!(
                "{SOURCE_DEBUG_PROFILE_ENV} is not valid UTF-8"
            )));
        }
        Ok(value) if value == S09_ALPHA_PROFILE => value,
        Ok(value) => {
            return Err(SourceDebugError::new(format!(
                "{SOURCE_DEBUG_PROFILE_ENV} must be exactly {S09_ALPHA_PROFILE:?}; found {value:?}"
            )));
        }
    };
    if target.as_str() != "gfx942:xnack-" {
        return Err(SourceDebugError::new(format!(
            "{requested} requires exact target gfx942:xnack-; found {target}"
        )));
    }

    let matches = collection
        .functions
        .iter()
        .filter(|function| {
            function.is_kernel_entry()
                && function.export_name == "alpha"
                && matches!(
                    function.typed_profile,
                    Some(TypedKernelProfile::GeneralScalarSliceRustcLayoutV3 { .. })
                )
        })
        .collect::<Vec<_>>();
    let [alpha] = matches.as_slice() else {
        return Err(SourceDebugError::new(format!(
            "{requested} requires exactly one authenticated General V3 alpha kernel; found {}",
            matches.len()
        )));
    };
    let def_id = alpha.instance.def_id();
    let crate_name = tcx.crate_name(def_id.krate);
    let def_path = tcx.def_path_str(def_id);
    let mir_matches = mir_module
        .functions
        .iter()
        .filter(|function| function.export_name == "alpha")
        .collect::<Vec<_>>();
    let [mir_alpha] = mir_matches.as_slice() else {
        return Err(SourceDebugError::new(format!(
            "S09 alpha requires one imported MIR body; found {}",
            mir_matches.len()
        )));
    };
    validate_alpha_mir_body(mir_alpha)?;
    let body = tcx.instance_mir(alpha.instance.def);
    validate_alpha_arguments(body)?;
    validate_debug_names(body)?;

    let function = source_location(tcx, body.span)?;
    validate_source_identity(
        crate_name.as_str(),
        &def_path,
        &function.file,
        function.source_sha256,
    )?;
    let local = body
        .var_debug_info
        .iter()
        .find(|variable| variable.name.as_str() == "i" && variable.argument_index.is_none())
        .ok_or_else(|| SourceDebugError::new("S09 alpha has no source local named `i`"))?;
    let index = body
        .var_debug_info
        .iter()
        .find(|variable| variable.name.as_str() == "index" && variable.argument_index.is_none())
        .ok_or_else(|| SourceDebugError::new("S09 alpha has no source local named `index`"))?;
    let VarDebugInfoContents::Place(place) = local.value else {
        return Err(SourceDebugError::new(
            "S09 alpha local `i` is not represented by a MIR place",
        ));
    };
    if !place.projection.is_empty()
        || !matches!(
            body.local_decls[place.local].ty.kind(),
            TyKind::Uint(UintTy::Usize)
        )
    {
        return Err(SourceDebugError::new(
            "S09 alpha local `i` must be an unprojected usize place",
        ));
    }
    let local_location = source_location(tcx, local.source_info.span)?;
    let index_location = source_location(tcx, index.source_info.span)?;
    if function.file != local_location.file
        || function.file != index_location.file
        || function.source_sha256 != local_location.source_sha256
        || function.source_sha256 != index_location.source_sha256
        || function.line != S09_FUNCTION_LINE
        || index_location.line != S09_INDEX_LINE
        || local_location.line != S09_LOCAL_LINE
    {
        return Err(SourceDebugError::new(format!(
            "S09 alpha source spans changed: expected canonical line {S09_FUNCTION_LINE} with `index` at line {S09_INDEX_LINE} and `i` at line {S09_LOCAL_LINE}; found function line {}, index line {}, and local line {}",
            function.line, index_location.line, local_location.line
        )));
    }
    let (source_directory, source_file) = S09_SOURCE_PATH
        .rsplit_once('/')
        .expect("S09 source path has a directory");
    validate_metadata_string(&source_directory, "source directory")?;
    validate_metadata_string(&source_file, "source file")?;
    Ok(Some(AlphaSourceDebugV1 {
        source_file: source_file.to_owned(),
        source_directory: source_directory.to_owned(),
        function_line: function.line,
        index_line: index_location.line,
        local_line: local_location.line,
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AlphaMirShape {
    blocks: usize,
    locals: usize,
    arguments: usize,
    calls: usize,
    thread_index_calls: usize,
    output_guard_calls: usize,
    index_get_calls: usize,
    switches: usize,
    asserts: usize,
    returns: usize,
    unreachables: usize,
    multiplies: usize,
    less_than: usize,
}

const S09_ALPHA_MIR_SHAPE: AlphaMirShape = AlphaMirShape {
    blocks: 8,
    locals: 14,
    arguments: 3,
    calls: 3,
    thread_index_calls: 1,
    output_guard_calls: 1,
    index_get_calls: 1,
    switches: 1,
    asserts: 1,
    returns: 1,
    unreachables: 1,
    multiplies: 1,
    less_than: 1,
};

fn validate_alpha_mir_body(
    function: &crate::mir_import::MirFunction,
) -> Result<(), SourceDebugError> {
    use crate::mir_import::{
        MirBinaryOp, MirFunctionKind, MirKernelProfile, MirOperandRef, MirProjectionElem,
        MirRvalueKind, MirTerminatorKind,
    };
    use crate::trusted_device_items::TrustedDeviceItem;

    let expected_path = format!("{S09_CRATE_NAME}::{S09_DEF_PATH}");
    if function.rust_path != expected_path
        || function.kind != MirFunctionKind::KernelEntry
        || function.typed_profile != Some(MirKernelProfile::GeneralScalarSliceRustcLayoutV3)
    {
        return Err(SourceDebugError::new(
            "S09 alpha imported MIR owner or profile identity changed",
        ));
    }

    let mut shape = AlphaMirShape {
        blocks: function.blocks.len(),
        locals: function.local_count,
        arguments: function.arg_count,
        calls: 0,
        thread_index_calls: 0,
        output_guard_calls: 0,
        index_get_calls: 0,
        switches: 0,
        asserts: 0,
        returns: 0,
        unreachables: 0,
        multiplies: 0,
        less_than: 0,
    };
    let mut scalar_index = None;

    for block in &function.blocks {
        for statement in &block.statements {
            match statement.rvalue {
                Some(MirRvalueKind::Binary(MirBinaryOp::Mul)) => shape.multiplies += 1,
                Some(MirRvalueKind::Binary(MirBinaryOp::Lt)) => shape.less_than += 1,
                _ => {}
            }
        }
        let Some(terminator) = &block.terminator else {
            return Err(SourceDebugError::new(
                "S09 alpha imported MIR contains a block without a terminator",
            ));
        };
        match &terminator.kind {
            MirTerminatorKind::Call {
                callee: Some(callee),
                destination: Some(destination),
                operands,
                ..
            } => {
                shape.calls += 1;
                match callee.trusted_item() {
                    Some(TrustedDeviceItem::ThreadIndex1d) if operands.is_empty() => {
                        shape.thread_index_calls += 1;
                    }
                    Some(TrustedDeviceItem::DisjointSliceGetMut) => {
                        shape.output_guard_calls += 1;
                    }
                    Some(TrustedDeviceItem::ThreadIndexGet) => {
                        shape.index_get_calls += 1;
                        scalar_index = Some(destination.local);
                    }
                    _ => {}
                }
            }
            MirTerminatorKind::Call { .. } => shape.calls += 1,
            MirTerminatorKind::SwitchInt { .. } => shape.switches += 1,
            MirTerminatorKind::Assert { .. } => shape.asserts += 1,
            MirTerminatorKind::Return => shape.returns += 1,
            MirTerminatorKind::Unreachable => shape.unreachables += 1,
            _ => {}
        }
    }
    validate_alpha_mir_shape(shape)?;

    let scalar_index = scalar_index.ok_or_else(|| {
        SourceDebugError::new("S09 alpha MIR has no authenticated scalar index destination")
    })?;
    let exact_place = |operand: &MirOperandRef, local: usize| matches!(operand, MirOperandRef::Place(place) if place.local == local && place.projection.is_empty());

    let statements = function
        .blocks
        .iter()
        .flat_map(|block| &block.statements)
        .collect::<Vec<_>>();
    let loads = statements
        .iter()
        .filter_map(|statement| {
            let destination = statement.destination.as_ref()?;
            let [MirOperandRef::Place(input)] = statement.operands.as_slice() else {
                return None;
            };
            matches!(
                input.projection.as_slice(),
                [MirProjectionElem::Deref, MirProjectionElem::Index { local }]
                    if input.local == 2 && *local == scalar_index
            )
            .then_some(destination.local)
        })
        .collect::<Vec<_>>();
    let [loaded_value] = loads.as_slice() else {
        return Err(SourceDebugError::new(format!(
            "S09 alpha MIR requires one exact guarded input load; found {}",
            loads.len()
        )));
    };
    let products = statements
        .iter()
        .filter_map(|statement| {
            if statement.rvalue != Some(MirRvalueKind::Binary(MirBinaryOp::Mul)) {
                return None;
            }
            let destination = statement.destination.as_ref()?;
            (destination.projection.as_slice() == [MirProjectionElem::Deref]
                && matches!(
                    statement.operands.as_slice(),
                    [lhs, rhs] if exact_place(lhs, *loaded_value) && exact_place(rhs, 1)
                ))
            .then_some(())
        })
        .collect::<Vec<_>>();
    if products.len() != 1 {
        return Err(SourceDebugError::new(
            "S09 alpha MIR store is not the exact guarded input[i] times scale dataflow",
        ));
    }
    Ok(())
}

fn validate_alpha_mir_shape(shape: AlphaMirShape) -> Result<(), SourceDebugError> {
    if shape != S09_ALPHA_MIR_SHAPE {
        return Err(SourceDebugError::new(format!(
            "S09 alpha imported MIR shape changed: {shape:?}"
        )));
    }
    Ok(())
}

fn validate_alpha_arguments(body: &Body<'_>) -> Result<(), SourceDebugError> {
    let arguments = body
        .args_iter()
        .map(|local| body.local_decls[local].ty)
        .collect::<Vec<_>>();
    let [scale, input, output] = arguments.as_slice() else {
        return Err(SourceDebugError::new(
            "S09 alpha requires exactly scale, input, and output arguments",
        ));
    };
    if !matches!(scale.kind(), TyKind::Float(FloatTy::F32))
        || !matches!(input.kind(), TyKind::Ref(_, pointee, _) if matches!(pointee.kind(), TyKind::Slice(element) if matches!(element.kind(), TyKind::Float(FloatTy::F32))))
        || !matches!(output.kind(), TyKind::Adt(_, _))
    {
        return Err(SourceDebugError::new(
            "S09 alpha argument layout is not the exact f32/read-only-f32-slice/DisjointSlice profile",
        ));
    }
    Ok(())
}

fn validate_debug_names(body: &Body<'_>) -> Result<(), SourceDebugError> {
    let mut names = vec![None; body.arg_count];
    for variable in &body.var_debug_info {
        if let Some(argument) = variable.argument_index {
            let index = usize::from(argument.saturating_sub(1));
            if index < names.len() && names[index].is_none() {
                names[index] = Some(variable.name.as_str());
            }
        }
    }
    if names != [Some("scale"), Some("input"), Some("output")] {
        return Err(SourceDebugError::new(format!(
            "S09 alpha argument debug names changed: found {names:?}"
        )));
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceLocation {
    file: String,
    line: usize,
    source_sha256: [u8; 32],
}

fn source_location(tcx: TyCtxt<'_>, span: Span) -> Result<SourceLocation, SourceDebugError> {
    let location = tcx.sess.source_map().lookup_char_pos(span.lo());
    let file = location
        .file
        .name
        .prefer_remapped_unconditionally()
        .to_string_lossy()
        .into_owned();
    if file.is_empty() || location.line == 0 {
        return Err(SourceDebugError::new(
            "S09 alpha source span has no file or line",
        ));
    }
    let source = location.file.src.as_ref().ok_or_else(|| {
        SourceDebugError::new("S09 alpha source text is unavailable for identity binding")
    })?;
    Ok(SourceLocation {
        file,
        line: location.line,
        source_sha256: Sha256::digest(source.as_bytes()).into(),
    })
}

fn validate_source_identity(
    crate_name: &str,
    def_path: &str,
    source_path: &str,
    source_sha256: [u8; 32],
) -> Result<(), SourceDebugError> {
    let canonical_path = !source_path.starts_with('/')
        && !source_path.contains('\\')
        && source_path
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..");
    if !canonical_path || source_path != S09_SOURCE_PATH {
        return Err(SourceDebugError::new(
            "S09 alpha source path is not the exact canonical remapped path",
        ));
    }
    if crate_name != S09_CRATE_NAME || def_path != S09_DEF_PATH {
        return Err(SourceDebugError::new(
            "S09 alpha crate or DefPath identity changed",
        ));
    }
    if source_sha256 != S09_SOURCE_SHA256 {
        return Err(SourceDebugError::new(
            "S09 alpha whole-source SHA-256 identity changed",
        ));
    }
    Ok(())
}

fn validate_metadata_string(value: &str, field: &str) -> Result<(), SourceDebugError> {
    if value.is_empty()
        || value.len() > 4096
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-' | b'.'))
    {
        return Err(SourceDebugError::new(format!(
            "S09 alpha {field} is not safe deterministic LLVM metadata"
        )));
    }
    Ok(())
}

pub(crate) fn inject_alpha_dwarf_v1(
    llvm: &str,
    profile: &AlphaSourceDebugV1,
) -> Result<String, SourceDebugError> {
    for forbidden in [
        "!llvm.dbg.cu",
        "!llvm.module.flags",
        "@llvm.dbg.",
        "!DICompileUnit",
        "!DISubprogram",
    ] {
        if llvm.contains(forbidden) {
            return Err(SourceDebugError::new(format!(
                "S09 alpha refuses pre-existing debug construct {forbidden:?}"
            )));
        }
    }
    if llvm.contains(" asm ") {
        return Err(SourceDebugError::new(
            "S09 alpha refuses pre-existing inline assembly",
        ));
    }
    let signature = "define amdgpu_kernel void @alpha(float %arg0, ptr addrspace(1) %arg1.data, i64 %arg1.len, ptr addrspace(1) %arg2.data, i64 %arg2.len)";
    if llvm.matches(signature).count() != 1 {
        return Err(SourceDebugError::new(
            "S09 alpha LLVM signature is absent or ambiguous",
        ));
    }
    let function_start = llvm.find(signature).expect("count checked");
    let function_end = llvm[function_start..]
        .find("\n}\n")
        .map(|offset| function_start + offset + 3)
        .ok_or_else(|| SourceDebugError::new("S09 alpha LLVM definition is unterminated"))?;
    let function = &llvm[function_start..function_end];
    if function.matches("bb0:\n").count() != 1 {
        return Err(SourceDebugError::new(
            "S09 alpha requires exactly one entry block label",
        ));
    }
    let local_value = find_global_index_value(function)?;
    let first_metadata = next_metadata_id(llvm)?;
    let ids = DebugIds::new(first_metadata)?;

    let mut rewritten_function = function.replacen(
        " !reqd_work_group_size ",
        &format!(" !dbg !{} !reqd_work_group_size ", ids.subprogram),
        1,
    );
    let argument_records = format!("bb0:\n{}", argument_debug_records(ids));
    rewritten_function = rewritten_function.replacen("bb0:\n", &argument_records, 1);
    let local_definition =
        format!("  {local_value} = add i64 {local_value}.base, {local_value}.local");
    let local_record = format!(
        "{local_definition}\n  call void @llvm.dbg.value(metadata i64 {local_value}, metadata !{}, metadata !DIExpression()), !dbg !{}\n  call void asm sideeffect \"s_nop 0\", \"v,~{{memory}}\"(i64 {local_value}), !dbg !{}",
        ids.local, ids.local_location, ids.local_location,
    );
    if rewritten_function.matches(&local_definition).count() != 1 {
        return Err(SourceDebugError::new(
            "S09 alpha global-index definition changed before local binding",
        ));
    }
    rewritten_function = rewritten_function.replacen(&local_definition, &local_record, 1);

    let mut output = String::with_capacity(llvm.len() + 4096);
    let declaration_point = llvm
        .find("\n\n")
        .map(|index| index + 2)
        .ok_or_else(|| SourceDebugError::new("LLVM module has no declaration boundary"))?;
    output.push_str(&llvm[..declaration_point]);
    output.push_str("declare void @llvm.dbg.value(metadata, metadata, metadata)\n\n");
    output.push_str(&llvm[declaration_point..function_start]);
    output.push_str(&rewritten_function);
    output.push_str(&llvm[function_end..]);
    write_debug_metadata(&mut output, profile, ids)?;
    Ok(output)
}

fn argument_debug_records(ids: DebugIds) -> String {
    format!(
        "  call void @llvm.dbg.value(metadata float %arg0, metadata !{}, metadata !DIExpression()), !dbg !{}\n  call void @llvm.dbg.value(metadata ptr addrspace(1) %arg1.data, metadata !{}, metadata !DIExpression()), !dbg !{}\n  call void @llvm.dbg.value(metadata i64 %arg1.len, metadata !{}, metadata !DIExpression()), !dbg !{}\n  call void @llvm.dbg.value(metadata ptr addrspace(1) %arg2.data, metadata !{}, metadata !DIExpression()), !dbg !{}\n  call void @llvm.dbg.value(metadata i64 %arg2.len, metadata !{}, metadata !DIExpression()), !dbg !{}\n  call void asm sideeffect \"s_nop 0\", \"v,v,v,v,v,~{{memory}}\"(float %arg0, ptr addrspace(1) %arg1.data, i64 %arg1.len, ptr addrspace(1) %arg2.data, i64 %arg2.len), !dbg !{}\n  call void asm sideeffect \"s_nop 0\", \"v,v,v,v,v,~{{memory}}\"(float %arg0, ptr addrspace(1) %arg1.data, i64 %arg1.len, ptr addrspace(1) %arg2.data, i64 %arg2.len), !dbg !{}\n",
        ids.scale,
        ids.function_location,
        ids.input_data,
        ids.function_location,
        ids.input_len,
        ids.function_location,
        ids.output_data,
        ids.function_location,
        ids.output_len,
        ids.function_location,
        ids.function_location,
        ids.index_location,
    )
}

fn find_global_index_value(function: &str) -> Result<&str, SourceDebugError> {
    let matches = function
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            let (value, rhs) = line.split_once(" = add i64 ")?;
            let id = value.strip_prefix("%v")?;
            let rhs = rhs.strip_prefix("%v")?;
            let (base, local) = rhs.split_once(".base, %v")?;
            let local = local.strip_suffix(".local")?;
            (id == base && id == local && !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
                .then_some(value)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [value] => Ok(value),
        _ => Err(SourceDebugError::new(format!(
            "S09 alpha requires one global-index SSA value; found {}",
            matches.len()
        ))),
    }
}

fn next_metadata_id(llvm: &str) -> Result<usize, SourceDebugError> {
    let bytes = llvm.as_bytes();
    let mut max = None;
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'!' || index + 1 == bytes.len() || !bytes[index + 1].is_ascii_digit() {
            index += 1;
            continue;
        }
        let start = index + 1;
        let mut end = start;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        let value = llvm[start..end]
            .parse::<usize>()
            .map_err(|_| SourceDebugError::new("LLVM metadata identifier overflow"))?;
        max = Some(max.map_or(value, |previous: usize| previous.max(value)));
        index = end;
    }
    max.unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| SourceDebugError::new("LLVM metadata identifier space exhausted"))
}

#[derive(Clone, Copy)]
struct DebugIds {
    compile_unit: usize,
    file: usize,
    dwarf_flag: usize,
    debug_flag: usize,
    ident: usize,
    subprogram: usize,
    subroutine_type: usize,
    subroutine_types: usize,
    f32_type: usize,
    pointer_type: usize,
    usize_type: usize,
    retained: usize,
    scale: usize,
    input_data: usize,
    input_len: usize,
    output_data: usize,
    output_len: usize,
    local: usize,
    function_location: usize,
    index_location: usize,
    local_location: usize,
}

impl DebugIds {
    fn new(first: usize) -> Result<Self, SourceDebugError> {
        let id = |offset| {
            first
                .checked_add(offset)
                .ok_or_else(|| SourceDebugError::new("LLVM metadata identifier space exhausted"))
        };
        Ok(Self {
            compile_unit: id(0)?,
            file: id(1)?,
            dwarf_flag: id(2)?,
            debug_flag: id(3)?,
            ident: id(4)?,
            subprogram: id(5)?,
            subroutine_type: id(6)?,
            subroutine_types: id(7)?,
            f32_type: id(8)?,
            pointer_type: id(9)?,
            usize_type: id(10)?,
            retained: id(11)?,
            scale: id(12)?,
            input_data: id(13)?,
            input_len: id(14)?,
            output_data: id(15)?,
            output_len: id(16)?,
            local: id(17)?,
            function_location: id(18)?,
            index_location: id(19)?,
            local_location: id(20)?,
        })
    }
}

fn write_debug_metadata(
    output: &mut String,
    profile: &AlphaSourceDebugV1,
    ids: DebugIds,
) -> Result<(), SourceDebugError> {
    writeln!(output, "\n!llvm.dbg.cu = !{{!{}}}", ids.compile_unit).unwrap();
    writeln!(
        output,
        "!llvm.module.flags = !{{!{}, !{}}}",
        ids.dwarf_flag, ids.debug_flag
    )
    .unwrap();
    writeln!(output, "!llvm.ident = !{{!{}}}", ids.ident).unwrap();
    writeln!(output, "!{} = distinct !DICompileUnit(language: DW_LANG_Rust, file: !{}, producer: \"fe2o3 S09 alpha gfx942 O0 v1\", isOptimized: false, runtimeVersion: 0, emissionKind: FullDebug)", ids.compile_unit, ids.file).unwrap();
    writeln!(
        output,
        "!{} = !DIFile(filename: \"{}\", directory: \"{}\")",
        ids.file, profile.source_file, profile.source_directory
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{i32 7, !\"Dwarf Version\", i32 5}}",
        ids.dwarf_flag
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{i32 2, !\"Debug Info Version\", i32 3}}",
        ids.debug_flag
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{!\"fe2o3 S09 alpha gfx942 O0 v1\"}}",
        ids.ident
    )
    .unwrap();
    writeln!(output, "!{} = distinct !DISubprogram(name: \"alpha\", linkageName: \"alpha\", scope: !{}, file: !{}, line: {}, type: !{}, scopeLine: {}, spFlags: DISPFlagDefinition, unit: !{}, retainedNodes: !{})", ids.subprogram, ids.file, ids.file, profile.function_line, ids.subroutine_type, profile.function_line, ids.compile_unit, ids.retained).unwrap();
    writeln!(
        output,
        "!{} = !DISubroutineType(types: !{})",
        ids.subroutine_type, ids.subroutine_types
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{null, !{}, !{}, !{}, !{}, !{}}}",
        ids.subroutine_types,
        ids.f32_type,
        ids.pointer_type,
        ids.usize_type,
        ids.pointer_type,
        ids.usize_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DIBasicType(name: \"f32\", size: 32, encoding: DW_ATE_float)",
        ids.f32_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !{}, size: 64)",
        ids.pointer_type, ids.f32_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DIBasicType(name: \"usize\", size: 64, encoding: DW_ATE_unsigned)",
        ids.usize_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !{{!{}, !{}, !{}, !{}, !{}, !{}}}",
        ids.retained,
        ids.scale,
        ids.input_data,
        ids.input_len,
        ids.output_data,
        ids.output_len,
        ids.local
    )
    .unwrap();
    writeln!(output, "!{} = !DILocalVariable(name: \"scale\", arg: 1, scope: !{}, file: !{}, line: {}, type: !{})", ids.scale, ids.subprogram, ids.file, profile.function_line, ids.f32_type).unwrap();
    writeln!(output, "!{} = !DILocalVariable(name: \"input_data\", arg: 2, scope: !{}, file: !{}, line: {}, type: !{})", ids.input_data, ids.subprogram, ids.file, profile.function_line, ids.pointer_type).unwrap();
    writeln!(output, "!{} = !DILocalVariable(name: \"input_len\", arg: 3, scope: !{}, file: !{}, line: {}, type: !{})", ids.input_len, ids.subprogram, ids.file, profile.function_line, ids.usize_type).unwrap();
    writeln!(output, "!{} = !DILocalVariable(name: \"output_data\", arg: 4, scope: !{}, file: !{}, line: {}, type: !{})", ids.output_data, ids.subprogram, ids.file, profile.function_line, ids.pointer_type).unwrap();
    writeln!(output, "!{} = !DILocalVariable(name: \"output_len\", arg: 5, scope: !{}, file: !{}, line: {}, type: !{})", ids.output_len, ids.subprogram, ids.file, profile.function_line, ids.usize_type).unwrap();
    writeln!(
        output,
        "!{} = !DILocalVariable(name: \"i\", scope: !{}, file: !{}, line: {}, type: !{})",
        ids.local, ids.subprogram, ids.file, profile.local_line, ids.usize_type
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DILocation(line: {}, column: 5, scope: !{})",
        ids.function_location, profile.function_line, ids.subprogram
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DILocation(line: {}, column: 13, scope: !{})",
        ids.index_location, profile.index_line, ids.subprogram
    )
    .unwrap();
    writeln!(
        output,
        "!{} = !DILocation(line: {}, column: 13, scope: !{})",
        ids.local_location, profile.local_line, ids.subprogram
    )
    .unwrap();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> AlphaSourceDebugV1 {
        AlphaSourceDebugV1 {
            source_file: "main.rs".to_owned(),
            source_directory: "crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src"
                .to_owned(),
            function_line: S09_FUNCTION_LINE,
            index_line: S09_INDEX_LINE,
            local_line: S09_LOCAL_LINE,
        }
    }

    fn module() -> &'static str {
        r#"target triple = "amdgcn-amd-amdhsa"

define amdgpu_kernel void @alpha(float %arg0, ptr addrspace(1) %arg1.data, i64 %arg1.len, ptr addrspace(1) %arg2.data, i64 %arg2.len) #0 !reqd_work_group_size !0 {
bb0:
  %v3.local.i32 = call i32 @llvm.amdgcn.workitem.id.x()
  %v3.group.i32 = call i32 @llvm.amdgcn.workgroup.id.x()
  %v3.local = zext i32 %v3.local.i32 to i64
  %v3.group = zext i32 %v3.group.i32 to i64
  %v3.base = mul i64 %v3.group, 256
  %v3 = add i64 %v3.base, %v3.local
  ret void
}

attributes #0 = { nounwind }

!0 = !{i32 256, i32 1, i32 1}
"#
    }

    #[test]
    fn injects_exact_physical_arguments_and_source_local() {
        let first = inject_alpha_dwarf_v1(module(), &profile()).unwrap();
        let second = inject_alpha_dwarf_v1(module(), &profile()).unwrap();
        assert_eq!(first, second);
        for expected in [
            "!DICompileUnit(language: DW_LANG_Rust",
            "!DISubprogram(name: \"alpha\"",
            "!DILocalVariable(name: \"scale\", arg: 1",
            "!DILocalVariable(name: \"input_data\", arg: 2",
            "!DILocalVariable(name: \"input_len\", arg: 3",
            "!DILocalVariable(name: \"output_data\", arg: 4",
            "!DILocalVariable(name: \"output_len\", arg: 5",
            "!DILocalVariable(name: \"i\", scope:",
            "metadata i64 %v3",
            "line: 69",
            "line: 70",
        ] {
            assert!(first.contains(expected), "missing {expected:?}\n{first}");
        }
        assert_eq!(first.matches("@llvm.dbg.value(").count(), 7);
        assert!(first.contains("asm sideeffect \"s_nop 0\", \"v,v,v,v,v,~{memory}\""));
        assert_eq!(
            first
                .matches("asm sideeffect \"s_nop 0\", \"v,v,v,v,v,~{memory}\"")
                .count(),
            2
        );
        assert!(first.contains("asm sideeffect \"s_nop 0\", \"v,~{memory}\""));
    }

    #[test]
    fn rejects_mutated_or_predecorated_modules() {
        let wrong_signature = module().replace("float %arg0", "double %arg0");
        assert!(
            inject_alpha_dwarf_v1(&wrong_signature, &profile())
                .unwrap_err()
                .to_string()
                .contains("signature")
        );
        let no_index = module().replace(
            "  %v3 = add i64 %v3.base, %v3.local\n",
            "  %v3 = add i64 %v3.base, 0\n",
        );
        assert!(
            inject_alpha_dwarf_v1(&no_index, &profile())
                .unwrap_err()
                .to_string()
                .contains("global-index")
        );
        assert!(
            inject_alpha_dwarf_v1(&format!("{}!llvm.dbg.cu = !{{!9}}\n", module()), &profile())
                .unwrap_err()
                .to_string()
                .contains("pre-existing")
        );
        assert!(
            inject_alpha_dwarf_v1(
                &module().replace(
                    "ret void",
                    "call void asm sideeffect \"\", \"\"()\n  ret void"
                ),
                &profile(),
            )
            .unwrap_err()
            .to_string()
            .contains("inline assembly")
        );
    }

    #[test]
    fn source_identity_rejects_spoofs_and_checkout_paths() {
        validate_source_identity(
            S09_CRATE_NAME,
            S09_DEF_PATH,
            S09_SOURCE_PATH,
            S09_SOURCE_SHA256,
        )
        .unwrap();

        for (crate_name, def_path, source_path, digest) in [
            (
                "substitute",
                S09_DEF_PATH,
                S09_SOURCE_PATH,
                S09_SOURCE_SHA256,
            ),
            (
                S09_CRATE_NAME,
                "general_genuine::__fe2o3_host_kernel_v1_substitute",
                S09_SOURCE_PATH,
                S09_SOURCE_SHA256,
            ),
            (
                S09_CRATE_NAME,
                S09_DEF_PATH,
                "/checkout/crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/main.rs",
                S09_SOURCE_SHA256,
            ),
            (
                S09_CRATE_NAME,
                S09_DEF_PATH,
                "crates/rustc-codegen-fe2o3/tests/fixtures/typed-alias-spoof/src/../src/main.rs",
                S09_SOURCE_SHA256,
            ),
            (S09_CRATE_NAME, S09_DEF_PATH, S09_SOURCE_PATH, [0; 32]),
        ] {
            assert!(
                validate_source_identity(crate_name, def_path, source_path, digest).is_err(),
                "spoofed source identity was admitted"
            );
        }
    }

    #[test]
    fn exact_mir_shape_rejects_semantic_body_mutations() {
        validate_alpha_mir_shape(S09_ALPHA_MIR_SHAPE).unwrap();

        let mut changed = S09_ALPHA_MIR_SHAPE;
        changed.multiplies = 0;
        assert!(validate_alpha_mir_shape(changed).is_err());
        let mut changed = S09_ALPHA_MIR_SHAPE;
        changed.output_guard_calls = 0;
        assert!(validate_alpha_mir_shape(changed).is_err());
        let mut changed = S09_ALPHA_MIR_SHAPE;
        changed.asserts = 0;
        assert!(validate_alpha_mir_shape(changed).is_err());
        let mut changed = S09_ALPHA_MIR_SHAPE;
        changed.returns = 2;
        assert!(validate_alpha_mir_shape(changed).is_err());
    }
}
