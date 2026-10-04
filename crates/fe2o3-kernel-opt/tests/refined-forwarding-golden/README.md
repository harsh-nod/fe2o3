# Canonical-Wire Chain Goldens

Each `.hex` file contains literal canonical V12 bytes, using the existing
lowercase-hex transport convention. The normal bounded decoder and verifier
must admit those bytes. The integration target then runs the actual owning
induction refinement followed by actual owning cross-block private forwarding
on its returned output. Both pairs are independently replayed against the live
owners and complete lineage.

`.golden` files prescribe complete `CHECK-BEFORE`, `CHECK-R`, `CHECK-AFTER`, and
`CHECK-REMARK` observations. Expected output is never used as transformation
input. Thirteen cases cover genuine simultaneous rewrites, diamonds, duplicate
edge occurrences, multiple functions, an ungrounded syntactic phi cycle,
unsupported induction stride, existing unchecked arithmetic, clobbers, trap
cuts, volatile access, unequal alignment, differing Stores, and no-op output.
These are scalar/private-memory cases, not coverage of every operation family.

The test target additionally checks idempotence, repeatability in two fresh
processes, typed malformed-wire refusals, fresh-admitted hostile outputs,
foreign-input and complete-lineage refusal, exact measured W/P success,
last-charge W-1 refusal, and first private-scope reservation refusal at the
inherited storage floor. It does not claim a private header byte extent or a
final-peak P-1 phase oracle.

Run the `refined_forwarding_wire_goldens` integration target in
`fe2o3-kernel-opt`. Its ignored child is invoked twice by its normal parent;
manual ignored-test execution is not needed for coverage.

This is canonical-wire golden coverage, not Pliron textual lit, ordinary Rust
source, native output, runtime, policy/default activation, or publication
authority. The existing Pliron textual runner does not expose these canonical
owning transformations; that integration gap remains open.
