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

The fixed closure is pinned with the repository's portable MIR semantic
encoder after compiler collection. That identity excludes checkout paths,
source diagnostics, and build observations, but binds function roles, types,
statements, operations, operands, constants, CFG, and resolved internal calls.
The same checked source therefore authenticates in a different worktree while
a semantic rewrite rejects. Malformed Rust/MIR rejected by rustc never reaches
admission.

The selector also rejects a non-exact target, any `-Cllvm-args` or `-Cpasses`
pipeline, and unsupported collection shapes. Every rejection is fatal and has
no legacy or artifact fallback.

## Deliberate stop

Repaired Scalar V1 now supplies a sealed, role-preserving composition contract.
V2 constructs it only after authenticating the exact collected root, helper,
and direct-call edge. The contract retains the `KernelEntry` identity and exact
collected export symbol while deriving an identity- and role-bound symbol for
the `InternalHelper`; lowering that helper emits internal LLVM linkage. V1 also
precharges its CFG and operation budgets and binds the reviewed data layout,
`gfx942`, wave64, and `xnack-` in direct LLVM.

V2 still emits no Kernel IR, LLVM IR, LLD input, HSACO, or hardware claim after
successful admission. The remaining frontend dependency is an authenticated
executable-MIR capture/import for the exact collected helper. That importer
must preserve the helper identity consumed by the composition contract rather
than rebuilding authority from serialized MIR or a function name. Once that
bridge is reviewed, the next dependencies are kernel-root body composition,
direct COV6/LLD production, and execution on matching gfx942 hardware.

## Boundary limitation

`CollectionResult` authenticates the collected device closure, not arbitrary
uncollected Rust items in the crate. V2 rejects additional functions present in
that authenticated collection, including extra roots or reachable helpers. It
does not claim to detect an ordinary item that the collector never includes.
Extending authority to that shape requires a collector contract change; V2
does not infer it from source names or caller-provided lists.
