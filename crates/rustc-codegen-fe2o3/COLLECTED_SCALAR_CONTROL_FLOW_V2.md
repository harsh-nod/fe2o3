# Collected scalar control flow V2

`FE2O3_CODEGEN_PIPELINE=collected-executable-scalar-control-flow-v2` is an
admission-only compiler frontend slice. It recognizes the checked-in
`tests/fixtures/executable-scalar-control-flow-v1.rs` program on the exact
`gfx942:xnack-` target and then stops before executable lowering.

## Authority

The collector is authoritative for function roles. Admission requires exactly
one `KernelEntry` and one `InternalHelper`; `DeviceFfiExport` and every other
collected role reject. The kernel symbol retained by the sealed admission token
is the authenticated `KernelEntry.export_name`. V2 neither creates a role from
structural MIR nor obtains export authority from an executable-MIR function
name.

The helper is identified by its collected rustc `Instance`, pinned full MIR CFG
identity, exact safe Rust `fn(u32) -> u32` signature, and equality with the
root's one resolved direct-call target. Its collector display/export text is
not consulted, so leaf-name normalization cannot select or alias it. The root
is independently pinned to safe Rust `fn(u32) -> ()`, its full MIR CFG identity,
and the fixed untyped registration contract. Extra collected functions, calls,
roles, registrations, or metadata reject.

The active rustc full-MIR identity includes source-bearing data. Consequently,
the positive fixture is compiled from its checked-in relative path. A copied or
rewritten fixture is a different program and rejects. Statement, operation,
type, call, and CFG substitutions are therefore vetoed before any downstream
lowering. Malformed Rust/MIR rejected by rustc never reaches admission.

The selector also rejects a non-exact target, any `-Cllvm-args` or `-Cpasses`
pipeline, and unsupported collection shapes. Every rejection is fatal and has
no legacy or artifact fallback.

## Deliberate stop

Scalar V1 at predecessor `d63bc81d190f4a33a5e22b9b22da6df24c86b3a3`
cannot soundly consume this admission. Its reviewed contract can invent an
export role, derive symbols from colliding leaf names, charge resource budgets
after growth, and omit the `xnack-` binding in LLVM. V2 therefore emits no
Kernel IR, LLVM IR, LLD input, HSACO, or hardware claim after successful
admission.

The next handoff requires a repaired Scalar V1 API that:

1. consumes the authenticated `InternalHelper` identity without assigning an
   export role or deriving a symbol from a leaf name;
2. preserves the authenticated root export contract when composing Kernel IR;
3. charges every existing resource budget before allocations or CFG/value
   growth; and
4. binds exact `gfx942:xnack-` in direct compiler-module LLVM output.

After that repair is reviewed, the next dependency is direct COV6/LLD
production followed by execution on matching gfx942 hardware.

## Boundary limitation

`CollectionResult` authenticates the collected device closure, not arbitrary
uncollected Rust items in the crate. V2 rejects additional functions present in
that authenticated collection, including extra roots or reachable helpers. It
does not claim to detect an ordinary item that the collector never includes.
Extending authority to that shape requires a collector contract change; V2
does not infer it from source names or caller-provided lists.
