# Checked local ordering of u32 computations

The later [current-source fixed-prefix continuation](evidence/source-local-order-continuation-20260923.md)
emits actual checked L LLVM/descriptors. Its qualification and remaining
recipe-workflow gaps are separate from the historical experiment below.

The `fe2o3-kernel-opt` service `schedule_checked_u32_local_order_v1` executes a
small, dependency-preserving permutation on an actual immutable canonical V12
owner. It returns a new immutable owner together with the existing independently
checked transition receipt. It is a foundation for schedule recipes, not a
shipping source recipe or a new compiler policy.

## Supported operation profile

Select 2–64 contiguous scalar-u32 `BitAnd`, `BitOr` or `BitXor` operations in one
block. Every selected operation must have exactly two u32 inputs, one u32 result,
no memory/effect flags and no compiler-ordering flags. The input owner must have
already passed its ordinary canonical validation. Two fixed preferences exist:

- `SourceOrder` keeps the original dependency-valid order.
- `ReverseReady` repeatedly chooses the greatest original region position among
  the operations whose dependencies have already been scheduled.

For the computation `(a ^ b) & (c | d)`, the first two operations are independent:

```text
SourceOrder:   t0 = a ^ b; t1 = c | d; result = t0 & t1
ReverseReady:  t1 = c | d; t0 = a ^ b; result = t0 & t1
```

SSA IDs, operands, source references and every unselected operation remain
identical. Live-ins can be function or block parameters, earlier definitions, or
other dominating values. Uses after the region and on successor edges remain
unchanged. The service never moves a selected operation across the region's
boundary, rewrites an expression or changes the CFG.

Assembly, memory accesses, calls, control operations, floating-point operations,
casts, shifts, arithmetic and operations with implicit ordered state are outside
this profile. In particular, it cannot reorder an exact assembly region.

## Identity, checking and resource accounting

`U32LocalOrderRegionV1` binds an exact expected input identity, block coordinate,
first operation and count. These coordinates are valid only for that invocation.
They are not persistent source anchors and must not be saved as a cross-build
recipe.

The service derives an inventory from the original owner, computes the actual
permutation, copies and transforms the module, and admits a fresh output owner.
It inversely permutes a copy of that output and compares the complete module with
the original. Complete occurrence/definition/use/edge transition rows are then
checked by the existing independent transition checker. No equality-of-values
heuristic substitutes for exact operand or source identity.

The result `CheckedU32LocalOrderOutputV1` borrows the original owner; its fields
cannot be constructed or mutated by a caller. `replay` recomputes the selected
preference and independently checks the exact output and transition again. A
retained receipt alone cannot manufacture that result or detach it from the
original owner's lifetime.

One cumulative work/storage ledger is charged before controlled work and
allocation. Schedule and replay calls restore their entry storage floor on all
normal `Result` exits; neither resets consumed work. The caller keeps the input
reservation live and reserves the returned result's `retained_storage()` before
further controlled allocation or replay. The accounting is logical retained
payload accounting, not a measurement of allocator rounding or process RSS.

## Reproduce the checks

Use the repository's installed pinned Rust toolchain and normal serialized build
environment:

```sh
cargo test --offline --locked -p fe2o3-kernel-opt --lib checked_u32_local_order_v1
cargo test --offline --locked -p fe2o3-kernel-opt --doc
```

The 19 focused tests cover both real orders, deterministic replay, unchanged
inputs, a non-self-inverse three-cycle permutation, block parameters and
successor-edge uses, malformed/stale selection, excluded operations, changed
output/receipt rows and exact/one-short cumulative work and storage limits. Two
compile-fail documentation tests protect ownership and mutation boundaries.

The existing V12 CPU simulator executes both actual output owners for five
independent input cases. Every output equals the host bitwise oracle, and both
canary words remain unchanged. These are model-owner executions, not source
compilation, a universal proof or a hardware benchmark. Full kernel-opt library
and documentation suites passed 56 and 15 tests respectively in the initial
working-tree run. Strict Clippy still has two unrelated existing forwarding
findings; no warning-clean workspace claim is made.

## Remaining source-recipe integration

Fixed Policy3 and its eight-pass identity are unchanged. The local service does
not select production passes, emit a source recipe, qualify a source-to-output
relation, enter the protected finalizer, or promise the final LLVM machine order.

Issue #282 U3 still requires source/kernel/specialization/target/policy-bound
semantic anchors and preconditions, checked resolution before the relevant
canonicalization, a separately versioned fixed composition, actual source output
admission, source-edit replay and explicit rebind with fresh evidence. An
ambiguous or stale source match must reject; these operation ordinals are not a
shortcut around that requirement. Two different local orders of a model are not
the required two persistent schedules of one actual source algorithm.
