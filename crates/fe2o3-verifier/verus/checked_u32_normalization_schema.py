"""Bind the real typed AST reads to the proof's explicit structural erasure."""

from pathlib import Path
from functools import lru_cache

MODEL = Path("crates/fe2o3-mir-model/src/semantic_mir_v1.rs")
CORRESPONDENCE = Path("crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1.rs")
CAPTURE = CORRESPONDENCE.with_name("production_checked_u32_add_capture_v1.rs")
DECLARATIONS = {
    "enum": ("SemanticScalarTypeV1", "SemanticTypeShapeV1", "SemanticConstantValueV1",
             "SemanticOperandV1", "SemanticRvalueKindV1", "SemanticStatementKindV1"),
    "struct": ("SemanticTypeDeclV1", "SemanticLocalDeclV1", "SemanticPlaceV1",
               "SemanticScalarValueV1", "SemanticConstantV1", "SemanticRvalueV1",
               "SemanticAssignmentV1", "SemanticStatementV1"),
}
ERASED = frozenset("""
SemanticTypeIdentityV1 SemanticLayoutIdentityV1 SemanticTypeLayoutV1
SemanticTypeAbiPropertiesV1 SemanticRustTypeKindV1 SemanticLocalIdentityV1
SemanticLocalRoleV1 SemanticSourceProvenanceV1 SemanticProjectionV1
SemanticValidityScalarTypeV1 SemanticPointerTypeV1 SemanticAggregateTypeV1
SemanticEnumVariantV1 SemanticFunctionSafetyV1 SemanticExternAbiV1
SemanticConstantBytesV1 SemanticPointerValueV1 SemanticCallableIdV1
SemanticUnaryOpV1 SemanticBinaryOpV1 SemanticCheckedBinaryRvalueV1
SemanticUncheckedBinaryRvalueV1 SemanticCastKindV1 SemanticBorrowKindV1
SemanticMutabilityV1 SemanticAggregateRvalueV1 SemanticMemoryLoadV1
SemanticMemoryStoreV1 SemanticAtomicRmwV1 SemanticAtomicCompareExchangeV1
""".split())
GETTERS = (
    ("SemanticTypeDeclV1", "pub const fn shape(&self) -> &SemanticTypeShapeV1", "&self.shape"),
    ("SemanticLocalDeclV1", "pub const fn ty(&self) -> SemanticTypeIdV1", "self.ty"),
    ("SemanticPlaceV1", "pub const fn local(&self) -> SemanticLocalIdV1", "self.local"),
    ("SemanticPlaceV1", "pub fn projections(&self) -> &[SemanticProjectionV1]", "&self.projections"),
    ("SemanticPlaceV1", "pub const fn ty(&self) -> SemanticTypeIdV1", "self.ty"),
    ("SemanticScalarValueV1", "pub const fn bits(self) -> u128", "self.bits"),
    ("SemanticScalarValueV1", "pub const fn size_bytes(self) -> u8", "self.size_bytes"),
    ("SemanticConstantV1", "pub const fn ty(&self) -> SemanticTypeIdV1", "self.ty"),
    ("SemanticConstantV1", "pub const fn value(&self) -> &SemanticConstantValueV1", "&self.value"),
    ("SemanticRvalueV1", "pub const fn result_type(&self) -> SemanticTypeIdV1", "self.result_type"),
    ("SemanticRvalueV1", "pub const fn kind(&self) -> &SemanticRvalueKindV1", "&self.kind"),
    ("SemanticAssignmentV1", "pub const fn destination(&self) -> &SemanticPlaceV1", "&self.destination"),
    ("SemanticAssignmentV1", "pub const fn value(&self) -> &SemanticRvalueV1", "&self.value"),
    ("SemanticStatementV1", "pub const fn kind(&self) -> &SemanticStatementKindV1", "&self.kind"),
)


def erased(tokens):
    result = []
    index = 0
    while index < len(tokens):
        if tokens[index:index + 3] == ["Box", "<", "["]:
            assert tokens[index + 4:index + 6] == ["]", ">"]
            element = tokens[index + 3]
            result.extend(["Vec", "<", "Ignored" if element in ERASED else element, ">"])
            index += 6
        else:
            result.append("Ignored" if tokens[index] in ERASED else tokens[index])
            index += 1
    return canonical(result)


def canonical(tokens):
    return [token for i, token in enumerate(tokens)
            if token != "," or i + 1 < len(tokens) and tokens[i + 1] not in ("}", ")")]


def top_level(base, source, prefix, attributes=""):
    starts = base.positions(source, base.tokens(prefix))
    base.need(len(starts) == 1, "one top-level item: " + prefix)
    start = starts[0]
    nesting = []
    closing = {"}": "{", ")": "(", "]": "["}
    for token in source[:start]:
        if token in ("{", "(", "["):
            nesting.append(token)
        elif token in closing:
            base.need(nesting and nesting.pop() == closing[token], "balanced item delimiters")
    base.need(not nesting, "not nested: " + prefix)
    attributes = base.tokens(attributes)
    begin = start - len(attributes)
    base.need(begin >= 0 and source[begin:start] == attributes
              and (begin == 0 or source[begin - 1] in ("}", ";")), "exact unconditional item attributes: " + prefix)


def container(base, source, prefix, derives, fields):
    top_level(base, source, prefix, "#[derive(" + derives + ")]")
    base.need(base.one_block(source, prefix)[0] == base.tokens(fields), "direct retained container: " + prefix)


def validate(base, sources, adapter, normalizer, proof, assembler):
    return validate_bytes(base, *(sources[str(path)] for path in (MODEL, CORRESPONDENCE, adapter, normalizer, proof, assembler, CAPTURE)))


@lru_cache(maxsize=8)
def validate_bytes(base, model_bytes, correspondence_bytes, adapter_bytes, normalizer_bytes, proof_bytes, assembler_bytes, capture_bytes):
    # Cache only completed checks keyed by every consulted byte, never paths.
    need, block, tokens = base.need, base.one_block, base.tokens
    source = tokens(model_bytes.decode("ascii"))
    harness = block(tokens(proof_bytes.decode("ascii")), "mod normalization")[0]
    need(source.count("cfg") == 1 and "cfg_attr" not in source
         and len(base.positions(source, tokens("#[cfg(test)] mod private_tests {"))) == 1
         and block(source, "mod private_tests")[1] == len(source), "only final private-test cfg")
    for name, fields in (
        ("SemanticBasicBlockV1", """identity: SemanticBlockIdentityV1, source: SemanticSourceProvenanceV1,
            statements: Box<[SemanticStatementV1]>, terminator: SemanticTerminatorV1,"""),
        ("SemanticFunctionDeclV1", """identity: SemanticFunctionIdentityV1, role: SemanticFunctionRoleV1,
            export: Option<SemanticFunctionExportV1>, item_definition_identity: SemanticItemDefinitionIdentityV1,
            monomorphization_identity: SemanticMonomorphizationIdentityV1, generic_type_arguments_identity: SemanticGenericTypeArgumentsIdentityV1,
            const_generic_arguments_identity: SemanticConstGenericArgumentsIdentityV1, source: SemanticSourceProvenanceV1,
            abi: SemanticFunctionAbiV1, locals: Box<[SemanticLocalDeclV1]>, entry: SemanticBlockIdV1, blocks: Box<[SemanticBasicBlockV1]>,"""),
        ("InertSemanticMirRequestV1", """target: SemanticTargetDataLayoutV1, types: Box<[SemanticTypeDeclV1]>,
            allocations: Box<[SemanticAllocationDeclV1]>, statics: Box<[SemanticStaticDeclV1]>, vtables: Box<[SemanticVTableDeclV1]>,
            functions: Box<[SemanticFunctionDeclV1]>, callables: Box<[SemanticCallableDeclV1]>, roots: Box<[SemanticFunctionIdV1]>,"""),
    ):
        container(base, source, "pub struct " + name, "Clone, Debug, Eq, PartialEq", fields)
    container(base, source, "pub struct AdmittedInertSemanticMirV1", "Debug", """
        request: InertSemanticMirRequestV1, wire_version: SemanticMirWireVersionV1,
        canonical: Vec<u8>, semantic_sha256: InertSemanticMirSha256V1,""")
    for kind, names in DECLARATIONS.items():
        for name in names:
            derives = ("Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd"
                       if name in ("SemanticScalarTypeV1", "SemanticScalarValueV1")
                       else "Clone, Debug, Eq, PartialEq")
            top_level(base, source, "pub " + kind + " " + name, "#[derive(" + derives + ")]")
            need(len(base.positions(source, [kind, name])) == 1
                 and not base.positions(source, ["type", name]), "unique AST type: " + name)
            actual = block(source, "pub " + kind + " " + name)[0]
            need("Vec" not in actual, "ordinary boxed AST containers")
            modeled = block(harness, kind + " " + name)[0]
            need(erased(actual) == canonical(modeled), "exact projected AST schema: " + name)
    id_macro = """($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u32);
        impl $name {
            pub const fn from_index(index: u32) -> Self { Self(index) }
            pub const fn index(self) -> u32 { self.0 }
        }
    };"""
    need(block(source, "macro_rules! index_id")[0] == tokens(id_macro), "exact typed index implementation")
    top_level(base, source, "macro_rules! index_id")
    for name in ("SemanticTypeIdV1", "SemanticLocalIdV1", "SemanticFunctionIdV1", "SemanticBlockIdV1"):
        top_level(base, source, "index_id!(" + name + ");")
        need(len(base.positions(source, tokens("index_id!(" + name + ");"))) == 1,
             "unique typed index invocation: " + name)
        need(len(base.positions(harness, tokens("struct " + name + "(u32);"))) == 1,
             "exact proof index: " + name)
        base.method(harness, name, "fn index(self) -> (value: u32) ensures value == self.0", "self.0")
    for owner, signature, body in GETTERS:
        top_level(base, source, "impl " + owner + " {")
        implementation = block(source, "impl " + owner)[0]
        need(not any(token in implementation for token in ("cfg", "cfg_attr", "include")), "unconditional AST getter")
        top_level(base, implementation, signature)
        base.method(source, owner, signature, body)
        name = tokens(signature)[tokens(signature).index("fn") + 1]
        model_impl = block(harness, "impl " + owner)[0]
        # The reviewed proof pin supplies each postcondition; this checks the
        # executable getter projection itself independently of that contract.
        starts = base.positions(model_impl, ["fn", name])
        need(len(starts) == 1, "unique proof getter")
        begin = starts[0]
        brace = model_impl.index("{", begin)
        need(block(model_impl, " ".join(model_impl[begin:brace]))[0] == tokens(body), "same getter projection")
    for owner, signature, body in (
        ("SemanticFunctionDeclV1", "pub fn locals(&self) -> &[SemanticLocalDeclV1]", "&self.locals"),
        ("SemanticFunctionDeclV1", "pub fn blocks(&self) -> &[SemanticBasicBlockV1]", "&self.blocks"),
        ("SemanticFunctionDeclV1", "pub const fn entry(&self) -> SemanticBlockIdV1", "self.entry"),
        ("SemanticBasicBlockV1", "pub fn statements(&self) -> &[SemanticStatementV1]", "&self.statements"),
        ("AdmittedInertSemanticMirV1", "pub fn types(&self) -> &[SemanticTypeDeclV1]", "&self.request.types"),
        ("AdmittedInertSemanticMirV1", "pub fn functions(&self) -> &[SemanticFunctionDeclV1]", "&self.request.functions"),
    ):
        top_level(base, source, "impl " + owner + " {")
        top_level(base, block(source, "impl " + owner)[0], signature)
        base.method(source, owner, signature, body)
    correspondence = tokens(correspondence_bytes.decode("ascii"))
    container(base, correspondence, "pub struct SemanticKirCorrespondenceV1", "Clone, Debug, Eq, PartialEq", """
        semantic_sha256: [u8; 32], function_count: usize, lowered_functions: Box<[SemanticKirFunctionCorrespondenceV1]>,
        blocks: Box<[SemanticKirBlockCorrespondenceV1]>, statement_operation_spans: Box<[SemanticKirStatementOperationSpanV1]>,
        terminator_operation_spans: Box<[SemanticKirTerminatorOperationSpanV1]>, generated_terminator_values: Box<[SemanticKirGeneratedTerminatorValuesV1]>,
        synthetic_operation_spans: Box<[SemanticKirSyntheticOperationSpanV1]>, parameter_bindings: Box<[SemanticKirParameterBindingV1]>,
        parameter_component_bindings: Box<[SemanticKirParameterComponentBindingV1]>, ignored_parameter_bindings: Box<[SemanticKirIgnoredParameterBindingV1]>,""")
    span = "SemanticKirStatementOperationSpanV1"
    top_level(base, correspondence, "pub struct " + span, "#[derive(Clone, Copy, Debug, Eq, PartialEq)]")
    top_level(base, correspondence, "impl " + span + " {")
    need(block(correspondence, "pub struct " + span)[0] == tokens("""
        correspondence_owner: SemanticFunctionIdV1, semantic_function: SemanticFunctionIdV1,
        semantic_block: SemanticBlockIdV1, statement_ordinal: u32, kernel_ir_block: BlockId,
        first_operation_ordinal: u32, operation_count: u32,
    """), "exact retained span schema")
    need(not any(token in block(correspondence, "impl " + span)[0]
                 for token in ("cfg", "cfg_attr", "include")), "unconditional span getter")
    need(canonical(block(correspondence, "pub struct " + span)[0])
         == canonical(block(harness, "struct " + span)[0]), "actual span fields match proof")
    for name, kind in (("correspondence_owner", "SemanticFunctionIdV1"),
                       ("semantic_function", "SemanticFunctionIdV1"), ("semantic_block", "SemanticBlockIdV1"),
                       ("statement_ordinal", "u32"), ("kernel_ir_block", "BlockId"),
                       ("first_operation_ordinal", "u32"), ("operation_count", "u32")):
        signature = "pub const fn " + name + "(self) -> " + kind
        top_level(base, block(correspondence, "impl " + span)[0], signature)
        base.method(correspondence, span, signature, "self." + name)
        base.method(harness, span, "fn " + name + "(self) -> (value: " + kind
                    + ") ensures value == self." + name, "self." + name)
    roster = "SemanticKirCorrespondenceV1"
    top_level(base, correspondence, "impl " + roster + " {")
    signature = "pub fn statement_operation_spans(&self) -> &[SemanticKirStatementOperationSpanV1]"
    top_level(base, block(correspondence, "impl " + roster)[0], signature)
    base.method(correspondence, roster, signature, "&self.statement_operation_spans")
    capture = tokens(capture_bytes.decode("ascii"))
    for prefix, fields in (
        ("pub struct ProductionCheckedU32AddCaptureRequestV1", """root: SemanticFunctionIdV1,
            function: SemanticFunctionIdV1, block: SemanticBlockIdV1, statement: u32,"""),
        ("struct Source", """request: ProductionCheckedU32AddCaptureRequestV1, lhs_local: SemanticLocalIdV1,
            tuple_local: SemanticLocalIdV1, lhs_ssa: SsaValueV1, tuple_ssa: SsaValueV1, use_event: u32, define_event: u32, literal: u32,"""),
        ("pub(super) struct Captured", """source: Source, block: BlockId, operation: u32, operand: ValueId, value: ValueId, overflow: ValueId,"""),
    ):
        container(base, capture, prefix, "Clone, Copy, Debug, Eq, PartialEq", fields)
    container(base, capture, "pub struct ProductionCheckedU32AddCaptureV1<'a>", "Clone, Copy, Debug", """
        owner: &'a ProductionSemanticKirOwnerV1, capture: &'a Captured,""")
    request = "ProductionCheckedU32AddCaptureRequestV1"
    top_level(base, capture, "impl " + request + " {")
    for name, kind in (("root", "SemanticFunctionIdV1"), ("function", "SemanticFunctionIdV1"), ("block", "SemanticBlockIdV1"), ("statement", "u32")):
        signature = "pub const fn " + name + "(self) -> " + kind
        top_level(base, block(capture, "impl " + request)[0], signature)
        base.method(capture, request, signature, "self." + name)
    implementation = "impl<'a> ProductionCheckedU32AddCaptureV1<'a>"
    top_level(base, capture, implementation)
    methods = block(capture, implementation)[0]
    for name, kind, body in (("request", request, "self.capture.source.request"),
                             ("operation", "u32", "self.capture.operation")):
        signature = "pub const fn " + name + "(self) -> " + kind
        top_level(base, methods, signature)
        need(block(methods, signature)[0] == tokens(body), "actual capture projection: " + name)
    caller = tokens(adapter_bytes.decode("ascii"))
    need(len(base.positions(caller, tokens("""let (source_steps, next_operation) = assemble::source_prefix(
        source, function, prefix, correspondence.statement_operation_spans(), capture, entry.id,)?;"""))) == 1,
        "actual retained source assembly forwarding")
    assembly = tokens(assembler_bytes.decode("ascii"))
    base.includes(assembly, ["assemble_body.rs"])
    need(block(assembly, "macro_rules! ordinary_exec")[0] == tokens("($body:expr) => { $body };"), "identity assembly adapter")
    need(block(assembly, """pub(super) fn source_prefix(source: &AdmittedSemanticMirV1,
        function: &SemanticFunctionDeclV1, prefix: &[SemanticStatementV1], spans: &[SemanticKirStatementOperationSpanV1],
        capture: ProductionCheckedU32AddCaptureV1<'_>, entry: BlockId,) -> Result<(Vec<PrefixStep>, u32), CheckedU32PrefixErrorV1>""")[0]
        == tokens("""let types = source.types(); let locals = function.locals();
            let root = capture.request().root().index(); let function_index = capture.request().function().index();
            let block = capture.request().block().index(); let kernel_block = entry.0; let operation = capture.operation();
            checked_u32_prefix_assemble_body_v1!(ordinary_exec, types, locals, prefix, spans, root, function_index, block,
                kernel_block, operation, selected, index, [], [], steps, next_operation, ordinal, [], [])"""),
        "direct actual source assembler wrapper")
    normal = tokens(normalizer_bytes.decode("ascii"))
    base.includes(normal, ["normalize_body.rs"])
    need(block(normal, "macro_rules! ordinary_exec")[0] == tokens("($body:expr) => { $body };"), "identity expression adapter")
    for name, signature, macro, arguments in (
        ("is_u32", "types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1", "type", "types, ty"),
        ("scalar_local", "types: &[SemanticTypeDeclV1], locals: &[SemanticLocalDeclV1], place: &SemanticPlaceV1,", "local", "types, locals, place"),
        ("scalar_constant", "types: &[SemanticTypeDeclV1], operand: &SemanticOperandV1,", "constant", "types, operand"),
        ("source_step", "types: &[SemanticTypeDeclV1], locals: &[SemanticLocalDeclV1], statement: &SemanticStatementV1, operations: u32,", "source_step", "types, locals, statement, operations"),
    ):
        returns = {"is_u32": "bool", "scalar_local": "Result<usize, CheckedU32PrefixErrorV1>",
                   "scalar_constant": "Option<u32>", "source_step": "Result<Option<PrefixStep>, CheckedU32PrefixErrorV1>"}[name]
        need(block(normal, "pub(super) fn " + name + "(" + signature + ") -> " + returns)[0]
             == tokens("checked_u32_prefix_" + macro + "_body_v1!(ordinary_exec, " + arguments + ")"),
             "direct normalization wrapper: " + name)
