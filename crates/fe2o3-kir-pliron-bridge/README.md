# fe2o3-kir-pliron-bridge

This crate is a bounded, lossless, authority-neutral envelope between the
frozen `fe2o3-kernel-ir` V1-V5 wire formats and the target-neutral
`dialect-kernel` and `dialect-gpu` Pliron shells.

Canonical KIR bytes are the only durable record. The bridge stores those bytes
unchanged on a builtin Pliron module, together with redundant wire-version and
KIR module-identity metadata. For each KIR kernel, it emits one
`kernel.algorithm_root` carrying the launch rank and one `gpu.hierarchy_id`
grid marker. This deterministic shell projection is an inert index, not a
second serialization of KIR and not a semantic lowering. Recovery decodes and
revalidates the canonical bytes, checks every redundant field, and requires an
exact shell projection before returning the original bytes.

The bridge rejects unknown wire versions, noncanonical or malformed KIR,
resource-limit violations, missing or type-confused metadata, duplicate or
conflicting KIR identities, unexpected or reordered shell operations, shell
mutation, and record substitution when an expected record is supplied. It
never derives KIR identity from a Pliron symbol or printed form.

This crate does not select a target or runtime, lower code, invoke a compiler,
read or write files, start processes, publish or create artifacts, load code,
or authorize execution or launch. It has no COMGR, LLVM-dialect, HSA, or HIP
integration. Successful bridging proves only bounded representation integrity;
it grants no proof, target, artifact, load, or runtime authority.
