"""Bind the real typed AST reads to the proof's explicit structural erasure."""

from pathlib import Path
from functools import lru_cache

MODEL = Path("crates/fe2o3-mir-model/src/semantic_mir_v1.rs")
CORRESPONDENCE = Path("crates/fe2o3-lower-mir-kernel/src/production_semantic_kir_v1.rs")
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


def validate(base, sources, adapter, normalizer, proof):
    return validate_bytes(base, *(sources[str(path)] for path in (MODEL, CORRESPONDENCE, adapter, normalizer, proof)))


@lru_cache(maxsize=8)
def validate_bytes(base, model_bytes, correspondence_bytes, adapter_bytes, normalizer_bytes, proof_bytes):
    # Cache only completed checks keyed by every consulted byte, never paths.
    need, block, tokens = base.need, base.one_block, base.tokens
    source = tokens(model_bytes.decode("ascii"))
    harness = block(tokens(proof_bytes.decode("ascii")), "mod normalization")[0]
    need(source.count("cfg") == 1 and "cfg_attr" not in source
         and len(base.positions(source, tokens("#[cfg(test)] mod private_tests {"))) == 1
         and block(source, "mod private_tests")[1] == len(source), "only final private-test cfg")
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
    for name in ("SemanticTypeIdV1", "SemanticLocalIdV1"):
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
        ("AdmittedInertSemanticMirV1", "pub fn types(&self) -> &[SemanticTypeDeclV1]", "&self.request.types"),
    ):
        top_level(base, source, "impl " + owner + " {")
        top_level(base, block(source, "impl " + owner)[0], signature)
        base.method(source, owner, signature, body)
    correspondence = tokens(correspondence_bytes.decode("ascii"))
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
    top_level(base, block(correspondence, "impl " + span)[0], "pub const fn operation_count(self) -> u32")
    base.method(correspondence, span, "pub const fn operation_count(self) -> u32", "self.operation_count")
    caller = tokens(adapter_bytes.decode("ascii"))
    need(block(caller, """fn source_step(source: &AdmittedSemanticMirV1,
        function: &SemanticFunctionDeclV1, statement: &SemanticStatementV1,
        span: &SemanticKirStatementOperationSpanV1,) -> Result<Option<PrefixStep>, CheckedU32PrefixErrorV1>""")[0]
        == tokens("normalize::source_step(source.types(), function.locals(), statement, span.operation_count(),)"),
        "actual retained AST/span forwarding")
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
