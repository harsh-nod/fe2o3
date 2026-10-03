"""Source-check typed KIR erasure and original-operation/capture forwarding."""
from functools import lru_cache
from pathlib import Path

IR = Path("crates/fe2o3-kernel-ir/src/ir.rs")
TYPES = IR.with_name("types.rs")
ERASED = frozenset("""PointerType SliceType IntrinsicOperation MemoryIntrinsicOperation
UnaryOp ComparePredicate CastKind FunctionId AddressSpace MemoryAccess Barrier Atomic
Fence WorkgroupBarrier WorkgroupMemory MatrixOperation Gfx950LdsTransposeOperationV1
WaveOperation InlineAssembly""".split())


def validate(base, schema, sources, proof, wrapper, adapter):
    return validate_bytes(base, schema, *(sources[str(path)] for path in
        (IR, TYPES, proof, wrapper, adapter, schema.CAPTURE)))


@lru_cache(maxsize=8)
def validate_bytes(base, schema, ir_bytes, types_bytes, proof_bytes, wrapper_bytes, adapter_bytes, capture_bytes):
    tokens, block, need = base.tokens, base.one_block, base.need
    ir, types = tokens(ir_bytes.decode("ascii")), tokens(types_bytes.decode("ascii"))
    proof = block(tokens(proof_bytes.decode("ascii")), "verus!")[0]
    common = "Clone, Debug, Eq, PartialEq"
    ordered = "Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd"
    for source, kind, name, derive in (
        (ir, "struct", "ValueDef", common), (ir, "struct", "Operation", common),
        (ir, "enum", "OperationKind", common),
        (ir, "enum", "Constant", "Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd"),
        (ir, "enum", "BinaryOp", ordered), (ir, "enum", "CheckedBinaryOperator", ordered),
        (types, "enum", "Type", "Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd"),
        (types, "enum", "ScalarType", ordered),
    ):
        attrs = "#[derive(" + derive + ")]"
        if name == "OperationKind":
            attrs += '''#[allow(clippy::large_enum_variant,
                reason = "boxing a public IR operation would change its established ownership and API shape")]'''
        schema.top_level(base, source, "pub " + kind + " " + name, attrs)
        need(len(base.positions(source, [kind, name])) == 1 and not base.positions(source, ["type", name]), "unique KIR type: " + name)
        actual = block(source, "pub " + kind + " " + name)[0]
        projected = ["Ignored" if token in ERASED and actual[index + 1:index + 2] != ["("] else token
                     for index, token in enumerate(actual)]
        need(schema.canonical(projected) == schema.canonical(block(proof, kind + " " + name)[0]), "exact projected KIR schema: " + name)
    schema.top_level(base, ir, "pub struct ValueId(pub u32);", "#[derive(" + ordered + ")]")
    for name, fields in (
        ("Module", "pub id: ModuleId, pub functions: Vec<Function>, pub kernels: Vec<Kernel>, pub required_capabilities: BTreeSet<TargetCapability>,"),
        ("Function", "pub id: FunctionId, pub signature: Signature, pub role: FunctionRole, pub body: Option<FunctionBody>, pub required_capabilities: BTreeSet<TargetCapability>,"),
        ("FunctionBody", "pub parameters: Vec<ValueId>, pub blocks: Vec<BasicBlock>,"),
        ("BasicBlock", "pub id: BlockId, pub parameters: Vec<ValueDef>, pub operations: Vec<Operation>, pub terminator: Option<Terminator>,"),
        ("Signature", "pub parameters: Vec<Type>, pub results: Vec<Type>,"),
    ):
        schema.container(base, ir, "pub struct " + name, common, fields)
    capture = tokens(capture_bytes.decode("ascii"))
    implementation = "impl<'a> ProductionCheckedU32AddCaptureV1<'a>"
    schema.top_level(base, capture, implementation)
    methods = block(capture, implementation)[0]
    for name, kind, field in (("operand", "ValueId", "operand"), ("value", "ValueId", "value"),
                              ("overflow", "ValueId", "overflow"), ("literal", "u32", "source.literal"),
                              ("block", "BlockId", "block")):
        signature = "pub const fn " + name + "(self) -> " + kind
        schema.top_level(base, methods, signature)
        need(block(methods, signature)[0] == tokens("self.capture." + field), "exact KIR capture getter: " + name)
    wrapper = tokens(wrapper_bytes.decode("ascii"))
    base.includes(wrapper, ["kernel_body.rs"])
    need(block(wrapper, "macro_rules! ordinary_exec")[0] == tokens("($body:expr) => { $body };"), "identity KIR adapter")
    need(block(wrapper, """pub(super) fn kernel_prefix(arguments: &[CheckedU32PrefixArgumentV1], operations: &[Operation],
        capture: ProductionCheckedU32AddCaptureV1<'_>,) -> Result<Origin, CheckedU32PrefixErrorV1>""")[0] == tokens("""
        let operand = capture.operand().0; let value = capture.value().0;
        let overflow = capture.overflow().0; let literal = capture.literal();
        checked_u32_prefix_kernel_assemble_body_v1!(ordinary_exec, arguments, operations, operand, value, overflow, literal,
            origins, argument, [], [], index, [], [])"""), "actual KIR capture forwarding")
    caller = tokens(adapter_bytes.decode("ascii"))
    need(len(base.positions(caller, tokens("""let operations = entry.operations.get(..next_operation as usize).ok_or(E::Kernel)?;
        let kernel_origin = kernel::kernel_prefix(&arguments, operations, capture)?;
        let origin = source_state[source_local]; if kernel_origin != origin { return Err(E::ValueMismatch); }"""))) == 1,
        "original retained KIR slice and exact source-origin comparison")
