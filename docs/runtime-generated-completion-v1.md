# Private Generated Completion

Development C4 implementation above private I2 and initialized native readback.
R125 Native CPU/test, R118B C1/C2/C3 Admission and R116/V3 Resources remain the
accepted checkpoints. This is not C4, A1/A2, issue #182 or HIP/HSA parity acceptance.

## Ownership And Order

The original prepared carrier remains in the existing owner-only async registry.
After exact physical completion, the production Context path checks its original
plan, source roster, hold, submission and retained native device. The host lends
the original charged destinations alongside the original immutable source;
the runtime path does not extract the decoder, replace allocations or invent a
result owner. Allocation preservation is an implementation and unsafe-contract
invariant, not a capability restriction on the internal mutable vector view.

The same source currentness scope encloses full native readback, complete
read-only validation, native DATA retirement and backend submission release.
Only after the closing source check succeeds can that closure mint a private,
non-Clone `NativeSettlementV1`. Required-callback validation rejects a carrier
that reports success without invoking the closure. The receipt binds the exact
Context submission, backend submission, stream and original hold.

Context bookkeeping consumes and matches that receipt before retiring shells
and allocation credits, releasing the original hold, publishing the physical
completion status and removing the original submission indexes and attempt.
An `Unknown` attempt phase alone is not settlement evidence. Errors and unwinds
retain the original carrier, terminalize the Context and deny retry.

Only then does the async driver consume the original host decoder. The decoder
checks all read-only bytes, decodes the complete output roster, settles disposed
storage credits, and commits the original result gate as its final successful
action. Errors or unwinds leave that gate uncommitted. The driver resolves its
original reply cell once; a decoder failure after conclusive native/Context
settlement is a host-result failure, not an excuse to retry native work.

Stop preserves destructive no-result disposal and never invokes the decoder.
Ordinary progress and drain may now deliver completed generated output without
requiring consumer polling. Public typed completion remains the later C5 API.

## Trust Boundary

The existing safe generated carrier remains descriptive and cannot promise
decoder correspondence. A separate unsafe completion-carrier trait documents
exact source/destination/decoder/gate ownership, pre-lending validation,
exactly-once callback success, error/unwind propagation, and no post-callback
substitution or result mutation. Its production implementation lives on the
private generated host preparation type.

This boundary transports compiler/host correspondence; it does not grant Worker
authority, validate machine effects, or prove GPU or Rust memory safety.
The private settlement receipt records successful runtime control flow, not a
formal refinement proof or independently authenticated execution receipt.

## Validation Scope

Focused tests cover original destination lending and full shape/gate/credit
preflight, same-source currentness and read-only validation, partial-copy owner
retention, async settlement/decode ordering, original reply delivery, decoder and
waker panics, stopped/unpolled observers, and draining completed operations.
Compile-fail examples reject destination extraction, escaped lending borrows and
a safe completion-trait implementation.

Direct Context tests distinguish three boundaries: rejecting omitted callbacks;
rejecting an unavailable retained native device without entering lending while
preserving exact records, credits, hold and host buffers; and exercising the
actual post-native bookkeeping tail with explicit test-only assumed receipts.
The latter checks two-invocation isolation, wrong-receipt and phase rejection,
exact refunds and a single physical-completion notification. It does not execute
a GPU or establish native completion. Its inert adversarial carrier creates no
source/view, Worker authority or native device.

The [sealed CPU qualification](evidence/dev-c4-completion-2026-09-17/README.md)
passes 908 runtime and 262 host tests on GNU and the separately scoped musl
build with optional legacy HIP linkage disabled; seventeen runtime and four
host opt-in tests remain ignored. Strict Clippy, formatting, no-default runtime,
unsafe-source policy and all 53 doctests pass. Canonical integration above
`9e9f3b187` applies the frozen integration patch and matches all thirty-eight
source/inventory hashes and the local qualification anchor `0e9c3cf89`.
The sealed archive retains the original qualification paths and failed initial
musl build; integration does not relabel them as new executions.

Protected Worker/carrier/Context/native composition, native injected-failure
coverage, C5 typed futures,
C6 generated graph/drain integration, production version journals and cross-run
reuse, aggregate memory, formal correspondence and matched performance remain
open. Existing lower-level native readback probes do not establish these joins.
