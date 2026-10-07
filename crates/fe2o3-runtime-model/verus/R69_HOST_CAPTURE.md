# R69 Host Capture Range Guard

`src/r69_host_capture.rs` exports the production range predicate used by capture
registration and final range revalidation. It accepts exactly when the length
is positive, the destination length is identical, checked `offset + length`
does not overflow `u64`, and the end is at most the allocation extent.

`r69_host_capture_v1.rs` mirrors that executable checked-add guard and specifies
its exact acceptance condition. Its four expected obligations cover executable
correspondence, accepted range properties, overflow rejection and allocation
containment. The Rust/Verus source correspondence is reviewed; the two source
files are not literally shared code. The authenticated runner must verify these
obligations before they are reported as passing.

Five expected-negative fixtures deliberately remove or weaken one condition:
zero length, shorter destination, longer destination, wrapping addition and
missing allocation containment. Each must fail its named postcondition with
exactly `0 verified, 1 errors`; parse errors and unrelated failures do not pass.

This proof does not establish that extracted extents belong to the registered
source, that an allocation incarnation remains current, that storage is
initialized or coherent, or that a private quiescence witness can be issued.
It does not verify native read-into, reply delivery, allocation-free execution,
capture-credit lifetime, the admission lock or the whole drain executor.
R67's credit primitives remain separate; composing these predicates with actual
Context/native ownership and disposal remains a reviewed and tested adapter
boundary rather than a whole-executor refinement theorem.

Run the authenticated suite from the repository root:

```sh
VERUS=/home/harsh/.codex-tmp/r56-verus-0.2026.08.09/install/verus-x86-linux/verus \
    crates/fe2o3-runtime-model/verus/verify-verus.sh
```
