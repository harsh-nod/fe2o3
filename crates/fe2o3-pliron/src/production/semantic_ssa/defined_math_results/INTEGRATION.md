# Defined Math Result Checkpoint

The result relation, SSA hooks and seven regressions are mounted. The source
fixture was independently admitted and call-expanded with cached MIR code using
standalone rustc. Parsing and scoped diff checks passed. Central compilation and
the new integrated tests have not run at this checkpoint. No Cargo/SSH/network
was run by this worker.

## Closed Result, Not Issuance

`ProductionSemanticSsaFunctionPlanV1::defined_math_results()` exposes immutable
`ProductionSemanticMathBridgeResultV1` rows. Only replay of the exact retained
KernelMathDerive getter, its unique bridge child and that bridge's original
Current call can construct them. Getter/bridge call instances, both return sites,
original return local, getter return destination, context receiver and unbranded
Current result remain distinct. No public row constructor exists.

Immediately before the original bridge ReturnTransfer, the adapter appends
`Use(receiver), Use(current), Define(bridge_return)`. The original
`Use(bridge_return), Kill(bridge_return), Define(getter_return)` remains intact,
as do frame storage kills. No original MIR body is rewritten, and no ignored
return or aggregate ZST is initialized merely because of its type/layout.
Ordinary nonannotated bridges still fail the undefined-use check.

The execution plan commits every row and its exact view identity, adds the
relation's bounded logical work/storage accounting, and compares it on full
source replay. Empty relations add no hash payload. Source MIR versions/bytes,
epoch metadata and ambient Workgroup initialization rules are unchanged.

## Parent Tests

Run Pliron filters `defined_math_results::tests` (six tests) and
`execution_replay_rejects_omitted_defined_math_result_relation` (one test).
They cover distinct calls, exact event ordering, absence of metadata, foreign
views, changed coordinates, identity commitment and inclusive resource limits.

Two separate ignored compiler tests extend the actual cached-AMD callback:

- `ssa_results::policy_math_all13_source_ssa_gfx942`
- `ssa_results::policy_math_all13_source_ssa_gfx950`

Use the same FE2O3_CORE_TRY_DEVICE_RMETA, FE2O3_CORE_TRY_HOST_DEPS,
FE2O3_CORE_TRY_AMDGPU_CORE and FE2O3_CORE_TRY_AMDGPU_BUILTINS settings as the
existing MIR-only import tests. The old two import tests remain MIR-only.
The new callbacks verify the actual getter/bridge SSA definition, all thirteen
retained F32 calls, source bytes and replay, then remove only getter metadata and
require the ordinary ReturnTransfer undefined-use rejection. No synthetic
ranked-root inputs or production qualification are manufactured.

## Downstream Boundary

The lowerer must consume a checked bridge-result receipt before emitting the
getter's branded MathDerive. It must not treat the synthetic SSA return
definition as an ordinary source assignment, a Unit constant, an independent
MathContextCurrent issuer, or an ambient capability. That lowerer consumption
and closed Bind/reference handling are not completed by these SSA hooks.

Mixed20 reported ReturnTransfer failures at root14/instance36/function3/local0
and root3/instance31/function5/local0. The report does not retain a function-name
or type roster, so attribution of those exact coordinates to Math remains a
hypothesis until the real source callback/export rerun confirms it.
