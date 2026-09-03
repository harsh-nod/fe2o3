use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use fe2o3_kernel_ir::{
    SemanticArgumentStorageV1, SemanticKernelStorageV1, SemanticKirStorageRepresentationV1,
    SemanticStorageBindingV1, SemanticStorageMapV1, Type,
};
use fe2o3_kir_sim::{IndexWidthV1, SimulationArgumentV1};
use fe2o3_kir_sim_cli::{AdmittedSimulationBundleInputV3, load_debug_simulation_bundle_v3};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticEnumEncodingV1, SemanticFieldsShapeV1, SemanticLocalRoleV1,
    SemanticMirLimitsV1, SemanticRustcVariantsV1, SemanticTypeLayoutDetailsV1, SemanticTypeShapeV1,
};
use serde::Serialize;
use serde_json::{Value, json};

const SCHEMA: &str = "fe2o3-debug-typed-layout-v1";

#[derive(Debug)]
struct OptionsV1 {
    bundle: PathBuf,
    request: PathBuf,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ResponseV1 {
    schema: &'static str,
    status: &'static str,
    bundle_identity: String,
    bundle_subject_identity: String,
    semantic_mir_identity: String,
    storage_map_identity: String,
    target_layout_identity: String,
    types: Vec<TypeViewV1>,
    kernels: Vec<KernelViewV1>,
    arguments: Vec<ArgumentViewV1>,
    authority: AuthorityViewV1,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct TypeViewV1 {
    ordinal: u32,
    type_identity: String,
    layout_identity: String,
    rustc_size_bytes: u64,
    size_bytes: Option<u64>,
    alignment_bytes: u64,
    uninhabited: bool,
    fields: Value,
    explicit_padding: Vec<PaddingViewV1>,
    variants: Value,
    shape: Value,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct PaddingViewV1 {
    offset_bytes: u64,
    size_bytes: u64,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct KernelViewV1 {
    semantic_root: u32,
    semantic_body: u32,
    kir_function_ordinal: u32,
    arguments: Vec<SemanticArgumentStorageV1>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ArgumentViewV1 {
    ordinal: u32,
    semantic_type: Option<u32>,
    storage: Option<SemanticStorageBindingV1>,
    observation: Value,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct AuthorityViewV1 {
    compiler: bool,
    proof: bool,
    artifact: bool,
    hardware: bool,
    load: bool,
    launch: bool,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ErrorV1<'a> {
    schema: &'static str,
    status: &'static str,
    stage: &'a str,
    code: &'a str,
    message: &'a str,
}

pub(super) fn run(arguments: Vec<OsString>) -> ExitCode {
    let options = match parse(arguments) {
        Ok(options) => options,
        Err(message) => return fail("arguments", "invalid_command_line", &message),
    };
    let admitted = match load_debug_simulation_bundle_v3(&options.bundle, &options.request) {
        Ok(admitted) => admitted,
        Err(error) => return fail("input", "bundle_or_request_rejected", &error.message),
    };
    let response = match inspect(&admitted) {
        Ok(response) => response,
        Err(message) => return fail("typed_layout", "semantic_correspondence_rejected", &message),
    };
    let mut output = std::io::BufWriter::new(std::io::stdout().lock());
    match serde_json::to_writer(&mut output, &response)
        .and_then(|()| output.write_all(b"\n").map_err(serde_json::Error::io))
        .and_then(|()| output.flush().map_err(serde_json::Error::io))
    {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn parse(arguments: Vec<OsString>) -> Result<OptionsV1, String> {
    let mut arguments = arguments.into_iter();
    if arguments.next().as_deref() != Some(OsStr::new("typed-layout")) {
        return Err("typed-layout subcommand is required".into());
    }
    let mut bundle = None;
    let mut request = None;
    while let Some(option) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("option {option:?} requires a value"))?;
        match option.to_str() {
            Some("--bundle-v3") if bundle.is_none() => bundle = Some(PathBuf::from(value)),
            Some("--request") if request.is_none() => request = Some(PathBuf::from(value)),
            Some("--bundle-v3" | "--request") => {
                return Err(format!("option {option:?} may appear only once"));
            }
            _ => return Err(format!("unknown option {option:?}")),
        }
    }
    Ok(OptionsV1 {
        bundle: bundle.ok_or_else(|| "--bundle-v3 is required".to_owned())?,
        request: request.ok_or_else(|| "--request is required".to_owned())?,
    })
}

fn inspect(admitted: &AdmittedSimulationBundleInputV3) -> Result<ResponseV1, String> {
    let bundle = admitted.bundle();
    let semantic = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        bundle.semantic_mir(),
        SemanticMirLimitsV1::default(),
    )
    .map_err(|error| format!("semantic MIR is not current production canonical: {error}"))?;
    let storage = SemanticStorageMapV1::from_canonical_json_bytes(bundle.storage_map())
        .map_err(|error| format!("storage map is not canonical: {error}"))?;
    validate_correspondence(admitted, &semantic, &storage)?;

    let types = semantic
        .types()
        .iter()
        .enumerate()
        .map(|(ordinal, declaration)| {
            let layout = declaration.layout();
            let explicit_padding = match layout.details() {
                SemanticTypeLayoutDetailsV1::Aggregate(aggregate) => aggregate
                    .padding()
                    .iter()
                    .map(|padding| PaddingViewV1 {
                        offset_bytes: padding.offset_bytes(),
                        size_bytes: padding.size_bytes(),
                    })
                    .collect(),
                SemanticTypeLayoutDetailsV1::None => Vec::new(),
            };
            TypeViewV1 {
                ordinal: ordinal as u32,
                type_identity: hex(declaration.identity().as_bytes()),
                layout_identity: hex(declaration.layout_identity().as_bytes()),
                rustc_size_bytes: layout.rustc_size_bytes(),
                size_bytes: layout.size_bytes(),
                alignment_bytes: layout.alignment_bytes(),
                uninhabited: layout.is_uninhabited(),
                fields: fields(layout.fields()),
                explicit_padding,
                variants: variants(layout.variants()),
                shape: shape(declaration.shape()),
            }
        })
        .collect();
    let kernels = storage
        .kernels()
        .iter()
        .map(|kernel| KernelViewV1 {
            semantic_root: kernel.semantic_root(),
            semantic_body: kernel.semantic_body(),
            kir_function_ordinal: kernel.kir_function_ordinal(),
            arguments: kernel.arguments().to_vec(),
        })
        .collect();
    let selected = selected_storage_kernel(admitted, &storage)?;
    let arguments = admitted
        .input()
        .request
        .arguments
        .iter()
        .enumerate()
        .map(|(ordinal, argument)| {
            let matching = selected
                .arguments()
                .iter()
                .filter(|binding| {
                    matches!(
                        binding.storage(),
                        SemanticStorageBindingV1::ExactKirParameter {
                            kir_parameter_ordinal,
                            ..
                        } if *kir_parameter_ordinal as usize == ordinal
                    )
                })
                .collect::<Vec<_>>();
            let binding = match matching.as_slice() {
                [] => None,
                [binding] => Some(*binding),
                _ => return Err("multiple source arguments name one KIR parameter".to_owned()),
            };
            Ok(ArgumentViewV1 {
                ordinal: ordinal as u32,
                semantic_type: binding.map(SemanticArgumentStorageV1::semantic_type),
                storage: binding.map(|binding| binding.storage().clone()),
                observation: observe_argument(admitted, ordinal, argument)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(ResponseV1 {
        schema: SCHEMA,
        status: "ok",
        bundle_identity: hex(bundle.identity().as_bytes()),
        bundle_subject_identity: hex(bundle.subject_identity()),
        semantic_mir_identity: hex(bundle.semantic_mir_identity()),
        storage_map_identity: hex(bundle.storage_map_identity()),
        target_layout_identity: hex(semantic.target_layout_identity().as_bytes()),
        types,
        kernels,
        arguments,
        authority: AuthorityViewV1 {
            compiler: false,
            proof: false,
            artifact: false,
            hardware: false,
            load: false,
            launch: false,
        },
    })
}

fn validate_correspondence(
    admitted: &AdmittedSimulationBundleInputV3,
    semantic: &AdmittedInertSemanticMirV1,
    storage: &SemanticStorageMapV1,
) -> Result<(), String> {
    if semantic.wire_version().as_u16() != storage.semantic_mir_version()
        || semantic.target_layout_identity().as_bytes() != storage.target_layout_identity()
    {
        return Err("semantic MIR version or target layout does not match the storage map".into());
    }
    let module = admitted.input().module.module();
    for kernel in storage.kernels() {
        let root = semantic
            .functions()
            .get(kernel.semantic_root() as usize)
            .ok_or_else(|| "storage map semantic root is out of range".to_owned())?;
        let body = semantic
            .functions()
            .get(kernel.semantic_body() as usize)
            .ok_or_else(|| "storage map semantic body is out of range".to_owned())?;
        let selected = semantic
            .select_kernel_body_for_root_v1(
                fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1::from_index(
                    kernel.semantic_root(),
                ),
            )
            .ok_or_else(|| "storage map root has no admitted kernel body".to_owned())?;
        if selected.body().index() != kernel.semantic_body() {
            return Err("storage map selected semantic body was substituted".into());
        }
        let function = module
            .functions
            .get(kernel.kir_function_ordinal() as usize)
            .ok_or_else(|| "storage map KIR function is out of range".to_owned())?;
        if function.role != fe2o3_kernel_ir::FunctionRole::KernelEntry
            || !module.kernels.iter().any(|entry| entry.entry == function.id)
        {
            return Err("storage map KIR function is not a declared kernel entry".into());
        }
        let kir_body = function
            .body
            .as_ref()
            .ok_or_else(|| "storage map KIR function has no body".to_owned())?;
        if kernel.arguments().len() != root.abi().source_input_types().len() {
            return Err("storage map source argument roster differs from semantic ABI".into());
        }
        for argument in kernel.arguments() {
            let source = argument.source_ordinal() as usize;
            if root.abi().source_input_types()[source].index() != argument.semantic_type() {
                return Err("storage map source argument type was substituted".into());
            }
            let local = body
                .locals()
                .get(argument.semantic_local() as usize)
                .ok_or_else(|| "storage map semantic local is out of range".to_owned())?;
            if local.ty().index() != argument.semantic_type()
                || local.role() != SemanticLocalRoleV1::Argument(argument.source_ordinal())
            {
                return Err("storage map semantic argument local was substituted".into());
            }
            validate_binding(argument.storage(), function, kir_body)?;
        }
    }
    for variable in storage.variables() {
        let function = semantic
            .functions()
            .get(variable.semantic_function() as usize)
            .ok_or_else(|| "storage map variable function is out of range".to_owned())?;
        match (variable.semantic_local(), variable.semantic_type()) {
            (Some(local), Some(ty)) => {
                let local = function
                    .locals()
                    .get(local as usize)
                    .ok_or_else(|| "storage map variable local is out of range".to_owned())?;
                if local.ty().index() != ty {
                    return Err("storage map variable type was substituted".into());
                }
            }
            (None, None) => {}
            _ => return Err("storage map variable local/type presence differs".into()),
        }
    }
    Ok(())
}

fn selected_storage_kernel<'a>(
    admitted: &AdmittedSimulationBundleInputV3,
    storage: &'a SemanticStorageMapV1,
) -> Result<&'a SemanticKernelStorageV1, String> {
    let module = admitted.input().module.module();
    let request_kernel = module
        .kernels
        .iter()
        .find(|kernel| kernel.id == admitted.input().request.kernel)
        .ok_or_else(|| "simulation request kernel is absent after admission".to_owned())?;
    let function_ordinal = module
        .functions
        .iter()
        .position(|function| function.id == request_kernel.entry)
        .ok_or_else(|| "simulation request entry is absent after admission".to_owned())?;
    let matching = storage
        .kernels()
        .iter()
        .filter(|kernel| kernel.kir_function_ordinal() as usize == function_ordinal)
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [kernel] => Ok(*kernel),
        [] => Err("storage map has no correspondence for the requested kernel".into()),
        _ => Err("storage map has ambiguous correspondence for the requested kernel".into()),
    }
}

fn validate_binding(
    binding: &SemanticStorageBindingV1,
    function: &fe2o3_kernel_ir::Function,
    body: &fe2o3_kernel_ir::FunctionBody,
) -> Result<(), String> {
    let SemanticStorageBindingV1::ExactKirParameter {
        kir_parameter_ordinal,
        kir_value_ordinal,
        representation,
    } = binding
    else {
        return Ok(());
    };
    let ordinal = *kir_parameter_ordinal as usize;
    if body.parameters.get(ordinal).map(|value| value.0) != Some(*kir_value_ordinal) {
        return Err("storage map KIR value identity was substituted".into());
    }
    let ty = function
        .signature
        .parameters
        .get(ordinal)
        .ok_or_else(|| "storage map KIR parameter is out of range".to_owned())?;
    let compatible = matches!(
        (representation, ty),
        (SemanticKirStorageRepresentationV1::Scalar, Type::Scalar(_))
            | (
                SemanticKirStorageRepresentationV1::RegionPointer,
                Type::Pointer(_)
            )
            | (
                SemanticKirStorageRepresentationV1::RegionSlice,
                Type::Slice(_)
            )
            | (SemanticKirStorageRepresentationV1::OpaqueFlattened, _)
    );
    if !compatible {
        return Err("storage map representation does not match the KIR parameter type".into());
    }
    Ok(())
}

fn observe_argument(
    admitted: &AdmittedSimulationBundleInputV3,
    ordinal: usize,
    argument: &SimulationArgumentV1,
) -> Result<Value, String> {
    Ok(match argument {
        SimulationArgumentV1::Scalar(value) => json!({
            "status": "exact_scalar",
            "scalar_type": format!("{:?}", value.ty()).to_ascii_lowercase(),
            "bits_hex": format!("0x{:x}", value.bits()),
        }),
        SimulationArgumentV1::Buffer(buffer) => json!({
            "status": "exact_region",
            "provenance": {"kind": "owned_argument", "argument": ordinal},
            "byte_offset": 0,
            "byte_length": buffer.bytes().len(),
            "element_type": format!("{:?}", buffer.element()).to_ascii_lowercase(),
            "access": format!("{:?}", buffer.access()).to_ascii_lowercase(),
            "alignment": buffer.alignment(),
            "initialized_ranges": initialized_ranges(buffer.initialized()),
            "same_backing": Vec::<Value>::new(),
        }),
        SimulationArgumentV1::BufferView(view) => {
            let element_bytes = element_bytes(admitted, view.element())?;
            let length = view
                .elements()
                .checked_mul(element_bytes)
                .ok_or_else(|| "admitted buffer-view byte length overflowed".to_owned())?;
            let end = view
                .byte_offset()
                .checked_add(length)
                .ok_or_else(|| "admitted buffer-view byte range overflowed".to_owned())?;
            let backing = admitted
                .input()
                .request
                .shared_buffers
                .iter()
                .find(|backing| backing.id == view.backing());
            let initialized = backing
                .and_then(|backing| {
                    backing
                        .buffer
                        .initialized()
                        .get(view.byte_offset()..end)
                })
                .map(initialized_ranges)
                .unwrap_or_default();
            let same_backing = admitted
                .input()
                .request
                .arguments
                .iter()
                .enumerate()
                .filter_map(|(other, argument)| match argument {
                    SimulationArgumentV1::BufferView(other_view)
                        if other != ordinal && other_view.backing() == view.backing() =>
                    {
                        Some((other, other_view))
                    }
                    _ => None,
                })
                .map(|(other, other_view)| {
                    let other_length = other_view
                        .elements()
                        .checked_mul(element_bytes(admitted, other_view.element())?)
                        .ok_or_else(|| {
                            "admitted same-backing view byte length overflowed".to_owned()
                        })?;
                    let other_end = other_view
                        .byte_offset()
                        .checked_add(other_length)
                        .ok_or_else(|| {
                            "admitted same-backing view byte range overflowed".to_owned()
                        })?;
                    let overlap_start = view.byte_offset().max(other_view.byte_offset());
                    let overlap_end = end.min(other_end);
                    let overlap = (overlap_start < overlap_end)
                        .then_some([overlap_start, overlap_end]);
                    Ok(json!({
                        "argument": other,
                        "overlap_byte_range": overlap,
                    }))
                })
                .collect::<Result<Vec<_>, String>>()?;
            json!({
                "status": "exact_region",
                "provenance": {"kind": "shared_backing", "backing": view.backing().0},
                "byte_offset": view.byte_offset(),
                "byte_length": length,
                "element_type": format!("{:?}", view.element()).to_ascii_lowercase(),
                "access": format!("{:?}", view.access()).to_ascii_lowercase(),
                "alignment": view.alignment(),
                "initialized_ranges": initialized,
                "same_backing": same_backing,
            })
        }
    })
}

fn element_bytes(
    admitted: &AdmittedSimulationBundleInputV3,
    scalar: fe2o3_kernel_ir::ScalarType,
) -> Result<usize, String> {
    match scalar {
        fe2o3_kernel_ir::ScalarType::Index => Ok(match admitted
            .input()
            .simulation_target()
            .index_width()
        {
            IndexWidthV1::Bits32 => 4,
            IndexWidthV1::Bits64 => 8,
        }),
        _ => scalar
            .bit_width()
            .map(|bits| usize::from(bits / 8))
            .ok_or_else(|| "admitted scalar has no exact byte width".to_owned()),
    }
}

fn initialized_ranges(initialized: &[bool]) -> Vec<[usize; 2]> {
    let mut ranges = Vec::new();
    let mut start = None;
    for (offset, value) in initialized.iter().copied().chain([false]).enumerate() {
        match (start, value) {
            (None, true) => start = Some(offset),
            (Some(begin), false) => {
                ranges.push([begin, offset]);
                start = None;
            }
            _ => {}
        }
    }
    ranges
}

fn fields(fields: &SemanticFieldsShapeV1) -> Value {
    match fields {
        SemanticFieldsShapeV1::Primitive => json!({"kind": "primitive", "field_count": 0}),
        SemanticFieldsShapeV1::Union { field_count } => {
            json!({"kind": "union", "field_count": field_count})
        }
        SemanticFieldsShapeV1::Array {
            stride_bytes,
            count,
        } => json!({
            "kind": "array",
            "stride_bytes": stride_bytes,
            "field_count": count,
        }),
        SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes,
            memory_order_source_indices,
        } => json!({
            "kind": "arbitrary",
            "source_order_offsets_bytes": source_order_offsets_bytes,
            "memory_order_source_indices": memory_order_source_indices,
        }),
    }
}

fn variants(variants: &SemanticRustcVariantsV1) -> Value {
    match variants {
        SemanticRustcVariantsV1::Empty => json!({"kind": "empty"}),
        SemanticRustcVariantsV1::Single { index } => json!({"kind": "single", "index": index}),
        SemanticRustcVariantsV1::Multiple(layout) => {
            let encoding = match layout.encoding() {
                SemanticEnumEncodingV1::Direct(direct) => json!({
                    "kind": "direct",
                    "tag_field": direct.tag_field(),
                    "tag_offset_bytes": direct.tag_offset_bytes(),
                    "tag": format!("{:?}", direct.tag()),
                }),
                SemanticEnumEncodingV1::Niche(niche) => json!({
                    "kind": "niche",
                    "tag_field": niche.tag_field(),
                    "source_path": format!("{:?}", niche.source().path()),
                    "source_offset_bytes": niche.source().expected_offset_bytes(),
                    "untagged_variant": niche.untagged_variant(),
                    "niche_variant_range": niche.niche_variant_range(),
                    "niche_start": niche.niche_start().to_string(),
                    "tag": format!("{:?}", niche.tag()),
                }),
            };
            json!({
                "kind": "multiple",
                "encoding": encoding,
                "layouts": layout.variants().iter().map(|variant| json!({
                    "index": variant.variant_index(),
                    "rustc_size_bytes": variant.rustc_size_bytes(),
                    "alignment_bytes": variant.alignment_bytes(),
                    "fields": fields(variant.fields()),
                    "field_offsets_bytes": variant.aggregate().field_offsets(),
                    "explicit_padding": variant.aggregate().padding().iter().map(|padding| json!({
                        "offset_bytes": padding.offset_bytes(),
                        "size_bytes": padding.size_bytes(),
                    })).collect::<Vec<_>>(),
                    "uninhabited": variant.is_uninhabited(),
                })).collect::<Vec<_>>(),
            })
        }
    }
}

fn shape(shape: &SemanticTypeShapeV1) -> Value {
    match shape {
        SemanticTypeShapeV1::Unit => json!({"kind": "unit"}),
        SemanticTypeShapeV1::Never => json!({"kind": "never"}),
        SemanticTypeShapeV1::Scalar(scalar) => {
            json!({"kind": "scalar", "scalar": format!("{scalar:?}")})
        }
        SemanticTypeShapeV1::ValidityScalar(scalar) => json!({
            "kind": "validity_scalar",
            "scalar": format!("{:?}", scalar.scalar()),
            "valid_ranges": scalar.valid_ranges().iter().map(|range| [range.start().to_string(), range.end().to_string()]).collect::<Vec<_>>(),
        }),
        SemanticTypeShapeV1::Pointer(pointer) => json!({
            "kind": "pointer",
            "pointee": pointer.pointee().index(),
            "pointer_kind": format!("{:?}", pointer.kind()).to_ascii_lowercase(),
            "mutability": format!("{:?}", pointer.mutability()).to_ascii_lowercase(),
            "address_space": pointer.address_space(),
            "pointer_width_bits": pointer.pointer_width_bits(),
            "metadata": format!("{:?}", pointer.metadata()).to_ascii_lowercase(),
        }),
        SemanticTypeShapeV1::Array { element, length } => {
            json!({"kind": "array", "element": element.index(), "length": length})
        }
        SemanticTypeShapeV1::Slice { element } => {
            json!({"kind": "slice", "element": element.index()})
        }
        SemanticTypeShapeV1::Tuple(aggregate) => json!({
            "kind": "tuple",
            "fields": aggregate.fields().iter().map(|field| field.index()).collect::<Vec<_>>(),
        }),
        SemanticTypeShapeV1::Aggregate(aggregate) => json!({
            "kind": "struct",
            "fields": aggregate.fields().iter().map(|field| field.index()).collect::<Vec<_>>(),
        }),
        SemanticTypeShapeV1::Union(aggregate) => json!({
            "kind": "union",
            "fields": aggregate.fields().iter().map(|field| field.index()).collect::<Vec<_>>(),
        }),
        SemanticTypeShapeV1::Enum {
            discriminant,
            variants,
        } => json!({
            "kind": "enum",
            "discriminant_type": discriminant.index(),
            "variants": variants.iter().map(|variant| json!({
                "discriminant": variant.discriminant().to_string(),
                "fields": variant.fields().fields().iter().map(|field| field.index()).collect::<Vec<_>>(),
                "uninhabited": variant.is_uninhabited(),
            })).collect::<Vec<_>>(),
        }),
        SemanticTypeShapeV1::FunctionPointer {
            arguments,
            return_type,
            ..
        } => json!({
            "kind": "function_pointer",
            "arguments": arguments.fields().iter().map(|field| field.index()).collect::<Vec<_>>(),
            "return_type": return_type.index(),
        }),
        SemanticTypeShapeV1::Opaque => json!({"kind": "opaque"}),
    }
}

fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn fail(stage: &str, code: &str, message: &str) -> ExitCode {
    let error = ErrorV1 {
        schema: SCHEMA,
        status: "error",
        stage,
        code,
        message,
    };
    let mut output = std::io::BufWriter::new(std::io::stderr().lock());
    let _ = serde_json::to_writer(&mut output, &error);
    let _ = output.write_all(b"\n");
    let _ = output.flush();
    ExitCode::FAILURE
}
