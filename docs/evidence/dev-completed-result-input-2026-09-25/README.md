# Completed Result To Read-Only Input

The consuming `GeneratedRuntimeReadSlice::from_charged_result` API preserves
completed typed storage and its original result credit while the existing
argument path admits and encodes the next immutable input. See the
[ownership and accounting contract](../../runtime-charged-generated-results-v1.md#completed-input-chaining).

## Scope

This is host-data implementation and CPU development evidence. Conversion avoids
one caller-side typed clone/allocation; ordinary preparation still performs
linear encoding into a new byte buffer. No benchmark, hardware execution,
formal adapter refinement, Worker authority, in-flight dependency transfer or
device-resident graph dataflow is claimed. The production result destructor and
credit ledger are unchanged. A1/A2 and accepted runtime lane checkpoints remain
unchanged.

## Validation

Validation is in progress. Final signed-source results and cleanup receipts
must be present before the campaign is reported successful.

Nine new unit tests cover typed pointer/bit preservation across ownership and
thread moves, unchanged credit before binding, independent shared-account peak
admission, byte/member exhaustion, pre/post-encoding errors and panics, payload
and type/index rejection, empty member accounting, legacy encoding, private
read-only/custody/mapping guards and actual CPU preparation/projection. Synthetic
decoding and structural HSACO fixtures grant no native authority. Injected
`Error::Allocation` and bind-closure panics are not allocator failure or scalar
encoder-panic tests. Refund-fault behavior is inherited, not newly fault-injected.

A downstream compiler-generated argument fixture consumes completed data through
the public API and genuine executable parameter. Compile failures retain exact
Rust error classification for result reuse, cloning and raw storage extraction.

## Reproduction

From a committed source tree with the pinned Rust toolchains/dependencies and an
existing disposable Cargo target:

```sh
python3 -I -B docs/evidence/dev-completed-result-input-2026-09-25/run.py --output OUTPUT --target CARGO_TARGET
```

The runner authenticates the inherited process controller before executing the
same bytes, retains phase commands/logs and process-group cleanup observations,
and checks source continuity. Fixtures use a child of the disposable target,
not the repository's shared fixture target. Its source inventory and warm-target
reuse are development boundaries, not a complete tool/dependency authentication
or relocated-packet replay audit. Hardware-only ignores and excluded legacy
HIP/HSA host features are not execution passes.
