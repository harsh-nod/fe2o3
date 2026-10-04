# Caller DATA, exact BF16 CPU execution and debugger startup — 2026-09-24

This later checkpoint extends the
[source/normal-continuation record](source-transport-tiled-debugger-qualification-20260924.md).
It adds finite static caller DATA analysis and restricted numerical CPU execution.
It does not close another broad issue exit: M1/V1/V2/U1/U2/U3 remain accepted
(6/18). A passing static/native build is not a GPU execution.

## Caller-to-helper DATA relation

The private, separately selected CallerToHelperO0 profile extends the existing
typed LLVM MC analysis. It joins the actual descriptor's kernarg enable order,
the exact tied lane1/2 pointer spill/reload, seven pending kernarg words and
their wait, one exact EXEC save/restore, and scalar-to-vector moves to the
three actual helper inputs. It joins the sole decoded direct call and its
return-register pair to the already checked complete helper DATA relation.

The profile is closed over the measured 45-row prefix of a 144-row, seven-block,
840-byte O0 entry. Instruction encodings, numeric opcode/class identities, ties
and register aliases are checked against the pinned LLVM tables; printed names
are not authority. Its fixed state uses the existing 65,536-byte reservation
and 280,576-work debit, without a heap, account reset or increased allowance.
Original whole-function, call-closure and effect/trace checks remain mandatory.

The root built and ran all 175 C++ controls (96 existing plus 79 caller controls).
A separate 120-control driver gate then completed three fresh ordinary-worker
copy/preserve/edit O0 cases. Every case retains byte-identical default-versus-typed
effect traces and passes late original-account and cumulative-account refusals.
The runs consume the exact retained source-provider R2 publications; they do
not rerun the frontend or reconstruct its live owner from JSON.

- Controls/build receipt: 130,521 bytes,
  a1cbb842cd33b9d95c465dc1ae705566a4f7cc5d7d8c8f732c108359f3b323a9.
- Observer build receipt: 158,473 bytes,
  426ebedf5b256d9f62d39daffb27a6e217d199d29acd1741f93d35732645a32f.
- Completed compiler-caller-three-actual-r1 receipt: 185,531 bytes,
  45bff009af719658babb6e76604c3e05a86cf28d832b84f6049d3e3b4741bd73.
- Three-case report: 13,347 bytes,
  57cee46e8d226aa2ecd224b21dac92d0b2a34ce18343644e6f5c08c8f53eb1c1.

All three HSACOs are 6,816 bytes. Copy and preserve are byte-identical; edit is
different. Those facts are artifact observations, not a semantic-equivalence
proof. The new profile proves no post-call AGPR-result-to-store path, address,
predicate, runtime memory validity, full ABI transport or whole-kernel behavior.
The ordinary production worker still excludes the private macro-gated checker.

The rebuilt existing six-case profile matrix also passed, as did a separately
rebuilt fourteen-case legacy matrix. Root compared every resulting HSACO with
its corresponding retained predecessor; all twenty matched. These historical
provider-based compatibility gates are not twenty fresh source executions:

- Six-case receipt: 170,645 bytes,
  10b77b31c0f197fd0343311c04275d320dd4434e1879f04ba7b47d85fadc2adf.
- Fourteen-case receipt: 388,660 bytes,
  892f3e0355ad981216013a8361ae6426ab00d44b7f94338cc48d8f339dbe5b55.

## Exact-integer BF16 matrix simulation

The existing CPU engine now executes the closed gfx942 BF16/F32 m16n16k16,
uniform Wave64 matrix contract. It checks the complete declared tensor layout,
including the eight admitted A/B storage-layout variants. The existing logical
lane/component mapping is retained; storage swizzles are not applied again to
register values.

This first numerical domain admits only integral BF16 A/B values in [-16,16]
and integral F32 accumulators of magnitude at most 2^20, with positive zero.
Negative zero, fractions, subnormals, nonfinite values and values outside the
range refuse with typed role/lane/component details, not raw operand bits.
Every product and partial sum in this domain is exactly representable in F32.
The evaluator uses the existing software round-to-nearest-even arithmetic,
not host floating point or the independent signed-integer test oracle.
It does not promise general BF16 rounding equivalence or gfx950 hardware support.

The existing scheduler must supply all 64 actual participants at the same
operation/site. All 768 operand components are validated before any of the
256 result bindings are committed. Results are staged before ordinary
collective completion. Partial/divergent/mismatched waves refuse. Existing
memory bounds, initialization, race tracking and event/debug lifecycle remain
in force; this is not whole-launch rollback on a later sink failure.

The new arrival work and checked 66*N+21,824 resolver allowance are prepaid
before their scans/evaluation, on the existing engine step counter. Fixed
scratch coexistence is included in preflight without enlarging RuntimeValue,
debug-value or binding layouts. A borrowed V12 admission method also retains
an independently decoded/equality-checked CPU view on the caller's existing
verification Budget, preserving its storage floor, consumed work, peak and
sticky denial history. That method meters admission, not subsequent generic
simulate calls or the entire compiler.

The focused gate passed five unit tests, fifteen integration tests and seven
doctests. The corpus includes 512 impulse executions, dense/extreme/cancellation
matrices, all-lane results and canaries, two waves and replay, exact/one-short
limits, and domain/memory/convergence refusals. These are inert verified KIR
fixtures, not execution of a newly authenticated Rust source kernel.

The broad simulator/CLI/debugger/runtime regression passed 726 executions,
zero failed and four ignored across 47 groups, including the actual new CLI
positional-error test. Counts overlap the focused suite.

- Focused receipt: 92,905 bytes,
  f9ab6fcff230fc4c62eabad82b9ed29675f382f1458a8ef5f4796fd27f9c9e6e.
- Compiler-bf16-exact-simulator-regression-r2 receipt: 94,373 bytes,
  3cffb1bdbd194f3087c4ee26d2cf5234c4e3032295a1f77e4044561ab9903f06.
- Selected simulator-library/new-integration-test strict Clippy receipt:
  94,000 bytes,
  e5f87cece30f3356c92f23d443ff040695f8ee60d393d14681ce956519867e7a.

The earlier layoutless test expected too-late a refusal; the canonical verifier
correctly rejects it first. Only the test expectation changed. A first broad
run also correctly refused a symlink temporary directory; the successful
rerun used a real directory without relaxing secure I/O. Both failures remain
retained. Broad strict Clippy remains unsuccessful at existing dependency and
fixture lint sites; the selected pass is not a workspace-wide lint claim.
The focused gate's CLI filter ran zero tests; actual CLI coverage comes from
the broad gate above.

From a configured checkout using its pinned toolchain and cached dependencies:

~~~sh
cargo test --offline --locked -p fe2o3-kir-sim --test matrix_bf16_exact_v1
cargo test --offline --locked -p fe2o3-kir-sim --doc
cargo test --offline --locked -p fe2o3-kir-sim-cli --all-targets
~~~

A genuine-source follow-up must execute the unchanged whole graph while its
original source owner and Budget are still live, join the actual logical
arguments, observe all four actual SSA result components, and check real
output/canaries. The current source stores only component zero. That follow-up,
edited-tile promotion, protected finalization and GPU numerical execution are
not established by these inert tests. The separately completed same-owner
normal compiler continuation does not itself establish numerical execution.

## Fresh MI2 startup, not a live GPU stop

The private selection-enabled R5 debugger now completed a fresh startup-only
session through the corrected fixed MI2 producer. Before it, all 66 Rust and
59 Node controls and five fresh actual benign process-family scenarios passed.
The actual startup used exactly two MI commands, fourteen records and a
240,231-byte collector frame in 254,205 bytes of raw stdout. It selected no
inferior, target, kernel or queue. Normal MI exit, pidfd exit, wait/reap/ECHILD,
stream EOF, current ownership, no child cgroups and cgroup emptiness all passed;
no kill or cleanup deadline expiry was needed.

Completed compiler-one-stop-mi2-startup-r2 receipt: 733,326 bytes,
70ec65d1df5bdccff3e7dba02246981b196a66bf39c302ff5e41e5f0dbaf0d03.

The original failed startup is retained. Its early exact debugger warning
about an unavailable index-cache directory triggered stream refusal; a sticky
semantic error also prevented EOF collection during cleanup. The successor
admits only that exact source-bound warning before the first MI command.
Partial/extra/late stderr still refuses. Semantic refusal remains sticky but
does not prevent bounded raw draining and physical cleanup. Genuine I/O,
ownership and cap failures still fail the proof. A later stale helper-hash
test expectation was corrected against the already rebuilt, pinned helper.

The collected loaded-file/Python-module snapshot is not complete import
history or cache-execution provenance. Independent loaded-closure review found
no blocker to retaining this exact startup-only history: 170 present and 23
absent observed rows, 145 artifact pins and the actual process/MI joins were
checked. The review does not prove which cache bytes executed, complete import
history, isolation or network silence. Historical runtime bindings remain unset.
The review record is 11,987 bytes, SHA256
e588425bf0c7d9f4d75322f6c7b5c8eadd04192981b338b30c5f17126619e775. R5 still lacks the proposed
physical snapshot implementation. Its startup qualification cannot transfer
to a future rebuilt debugger, and does not enable target execution, physical
register/memory capture, runtime acceptance or the published disabled packages.

The three-case, focused/broad simulator and startup gates above retained source
census 7,996 files /115,538,785 bytes /
b68877c166eaf6bb9d6ee3072e65d1a78d89e06fd3c05c164f8a095344fdb062.
These subsequent record-only documentation edits are not retroactively included
in that historical census. Root-runner receipt paths have the fixed
logs/phase28-resume-r14-<gate-name>/receipt.json form.
