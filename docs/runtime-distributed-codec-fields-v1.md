# Distributed Codec Field Verification V1

This boundary covers the production `Reader`/`Writer` header and u64 methods in
the distributed publication codec. Each method forwards to the same executable
macro body used by the Verus proof. Header domain comparison also uses one shared
safe equality loop. There is no new trusted equality shim or changed reader
entry precondition.

The nine-input proof covers sequential header validation, error precedence,
cursor advancement, u64 little-endian encoding, writer framing and preservation
outside the written interval. Writers retain their existing capacity premises;
native differential tests separately cover short-buffer panic prefixes. This is
not a proof of either complete wire codec or a native GPU implementation.

## Source And Diagnostic Gates

`check-distributed-codec-fields.py` binds all 305 native model source files,
the exact shared bodies and proof inputs, four real forwarding methods, schema
constant, strict diagnostic classifier, and the 34-case mutation roster.
`test-distributed-codec-fields.py` checks source drift and constructs those
mutants without invoking a compiler or solver.

The measured unfiltered positive count is 63. The calibration records 34 raw
mutant outcomes, but explicitly records zero accepted negative classifications
and zero qualified kills from those captures. A fresh campaign must verify the
signed candidate, full and relocated positives, every exact actual-body mutant,
closing positive, source/tool continuity, and process closure. Historical replay
does not satisfy those requirements. Per-case counts and complete diagnostic
messages are fixed by the calibration; frontend failures and unknown diagnostics
are not logical kills.

Two named mutants generate an auxiliary `Seq::subrange` recommendation note.
The classifier authenticates the pinned vstd file bytes and exact source
coordinates, requires one two-span note with a real selected-function primary,
and does not relax logical-primary attribution. The compiler-generated equality
while-loop def-site sentinel is separately limited to its known auxiliary shape.

## Native CPU Reuse

The accepted equality-loop candidate had 1,125 passing debug model tests with
19 existing manual ignores and 16 passing focused release tests. Subsequent
changes were excluded proof or verification metadata only. Reuse requires exact
native/compiler-input byte and mode identity, retained original test ELF identity,
and the original command, receipt, raw output, and source/tool records. No rebuild
or portable binary-reproducibility claim is implied by reuse.

The safe equality loop replaces the previous slice-equality implementation,
which lowered to a `bcmp` call in the inspected release artifact. It preserves
empty/arbitrary-domain semantics and short-circuit error ordering. Runtime
performance equivalence has not been measured; actual domains contain 43 or 41
bytes. This boundary does not establish full-wire verification, GPU/native or
HIP/HSA parity, performance parity, or milestone completion.
