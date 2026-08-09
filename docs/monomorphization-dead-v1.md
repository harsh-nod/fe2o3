# Monomorphization-Dead V1 Foundation

Status: bounded compiler foundation. This is not parity evidence for row 23.

## Authority

`CONSTANT_FOLD_POLICY_VERSION_V1` is the only accepted policy version. Public
policy and evidence values in `fe2o3-rustc-front` are caller-constructible and
inert. They grant no authority to omit collection, panic rejection,
address-space analysis, verification, or lowering. The verifier can reconcile
an inert claim with an observation identity, but that binding is also inert.

The rustc backend derives a private observation from the active concrete
`Instance` and its `instance_mir`. No serialized or caller-authored claim enters
that path. MIR import recomputes the observation and requires exact equality
before consuming it.

## Policy V1

V1 admits known booleans and fixed-width signed or unsigned integers of 8, 16,
32, 64, or 128 bits. The portable evaluator defines checked add, subtract,
multiply, divide, remainder, bitwise operations, shifts, and comparisons.
Cross-type operands, unsupported widths, overflow, division by zero, and shifts
at least as wide as the value fail closed. Unknown, poison, and unevaluated
target-dependent values also fail closed; `isize` and `usize` are never policy
inputs.

The compiler integration is intentionally narrower than the portable policy.
It proves only a `SwitchInt` whose discriminant is directly an
`Operand::Constant` of a supported fixed-width type after concrete instance
substitution. Copy/move operands, locals, def-use reasoning, rvalues,
projections, predecessor-derived values, and runtime-check operands produce no
decision. This intentionally accepts false negatives rather than reasoning
through writes or aliases. A compiler-evaluated direct fixed-width constant may
reflect target properties; the target identity described below is therefore a
required part of every observation and evidence identity.

## Identity

Every canonical evidence record binds:

- policy and evidence format versions;
- a function identity over the rustc definition-path hash and concrete symbol;
- a structured rustc `HashStable` identity over the complete active MIR body,
  including declarations, statements, operands, discriminants, and
  terminators;
- a source identity over the remapped file and line/column coordinates of the
  definition, statements, and terminators;
- a compiler target identity over the LLVM target tuple, exact data-layout
  string, and pointer width; and
- sorted branch decisions containing the block, fixed-width discriminant,
  selected successor, and sorted dead successors.

The canonical byte domain is `FE2MDBE\0`; its SHA-256 digest is the evidence
identity. Function, MIR, source-location, target, decision, or version
substitution changes the identity or fails reconciliation. The source identity
does not archive or authenticate source-file bytes; no source-content proof is
claimed.

## Consumption

The backend computes entry reachability twice: once over the original MIR CFG
and once with each V1 switch restricted to its selected successor. Only
`original_reachable - policy_reachable` blocks are excluded. A shared join stays
live, and a block already structurally unreachable in the original body is not
made exempt by this policy.

Collection and device-export panic traversal skip only that exclusion set. A
reachable panic-only body is rejected rather than treated as an intrinsic stub;
only exact rustc-authenticated trusted diagnostic items may terminate traversal.
MIR import re-derives the observation, imports only the policy-reachable set,
and rewrites each proved switch to a `Goto`. Omitting structurally unreachable
blocks at import keeps every retained terminator target inside the imported
function, including the unreachable-to-policy-excluded edge case.

## Deliberate Gaps

- The observation is compiler-private collection state; it is not yet carried
  as authenticated semantic IR or wire evidence.
- The machine-effect address-space analyzer consumes caller-supplied
  straight-line mechanics and has no exact source-MIR CFG identity. It receives
  no dead-branch exemption. The G2 fixture verifies conservative rejection of
  local-backed raw-pointer branches, not an independent machine proof.
- Device FFI static-reference discovery still scans the entire body, so a
  policy-dead static relocation remains a conservative rejection.
- The backend consumes rustc's active `instance_mir`; optimizations that occur
  before that query are not independently attested by this evidence format.
- Remapped source locations are bound, but source bytes are not archived or
  independently authenticated.
- The G2 backend test is ignored by default and does not emit or execute a
  supported row-23 kernel. Configured compiler, finalized `gfx942` artifact, and
  hardware differential evidence are absent.

These gaps keep parity row 23 `Missing`.
