# Ordered-repeat ordinary LLVM observation

Observe how the normal typed emitter lowers the retained ordered-repeat sources
to LLVM inline assembly. The driver consumes a passed
[source acceptance capture](ordered-repeat-source-acceptance-v1.md), checks its
unchanged inputs and makes exactly four fresh calls to the existing lowerer.

Canonical and mirror observations each passed four fresh lowerings, revalidated
120 retained CPU simulations and eight frontend refusals, and ran zero new
simulations. All 22 pure control groups passed in each fork. See the
[dated qualification evidence](evidence/authoring-repeat-fault-20260922.md#final-publication-qualification) for
exact receipts, tool provenance and the limits of these observations.

## Accepted boundary

Current positive fixture pins include the final one-LF source cleanup. A retained
capture must still match every exact selected source/tool byte, including the
imported source driver. Historical captures are not admitted by a compatibility
exception; obtain a fresh capture when those bytes change.

The input is an immutable successful
`task-ordered-repeat-source-acceptance-v1` capture from the current source-acceptance driver
(with its 19-minute deadline and complete simulator-result validation), with
receipt byte count and SHA-256 supplied independently by the caller. The driver
consumes the receipt at its original absolute path; it does not normalize paths, rewrite the
receipt, copy executable KIR, synthesize KIR or produce a new source admission.

The source capture must contain all four exports (repeat1, repeat2, repeat15,
and a fresh identical-source repeat15), four inspections, 120 completed CPU
simulations and eight exact frontend refusals: 136 stages total. The script
revalidates the retained raw export diagnostics, inspector replies, request and
complete simulation-result bytes, independent arithmetic/canaries and frontend
refusal replies through the source-acceptance validators. Those are **retained**
simulations/refusals, not new executions. It separately checks every historical
selected file pin, including the original sources and actual raw KIR, before
and after the new work.

Exactly four subprocess calls freshly invoke the existing
`lower_diagnostic_ordered_program_v17` example, once for each original KIR.
Each receives precisely `ORIGINAL_INPUT.kir NEW_OUTPUT/<label>.ll`.
The existing lowerer re-admits that exact V17 byte owner and borrows it in the
unchanged typed emitter. Its output directory/files must not already exist.

The four new observations bind:

- Current source bytes, source inventory/preflight, semantic MIR identity,
  canonical KIR identity and raw KIR bytes to the retained normal-source capture.
  Inventory is retained root/contract census, not an opcode hash.
- Actual lowerer report input raw hash, canonical hash/length and whole emitted
  LLVM hash/length. There is no guessed semantic identity or recomputation from
  preflight/source bytes.
- All 16 descriptor slots, counts 2/3/16, and registers
  `[scratch32,out33,input34,input35,input36]`, including padding.
- Exactly one inline-assembly occurrence and its complete line:
  one `v_mov_b32_e32 $0, $1`, followed by precisely N copies of
  `v_add_u32_e32 $0, $0, $2`, joined by the existing emitter's literal
  `\0A\09` separators. The exact constraints are
  `=&{v33},{v34},{v35},{v36},~{v32}`; inputs are the original direct three
  u32 arguments in order. The emitted result name comes from the normal
  inspector's actual result ValueId, not an assumed ordinal.
- The sole emitted store uses that observed result; the kernel ABI, target
  triple, gfx942 CPU/wave64/xnack attributes and required64x1x1 launch match the
  existing emitter spelling. The high-water comment 37 is a binding extent,
  **not a validated final native descriptor**.
- Different repetition counts yield different whole LLVM hashes. The two
  repeat15 observations must retain equal whole LLVM bytes (direct byte comparison
  as well as SHA/length) and equal complete lowerer reports/text observations.
  No path stripping,
  whitespace normalization or statement deletion is performed.

The text matcher is a small exact observer of the known emitter syntax, not a
general LLVM parser/verifier or independent whole-module semantic proof.
Synthetic unit tests intentionally use inert non-KIR byte strings and synthetic
LLVM text only; they are never executable capture inputs.

## Reviewed existing contracts

Implementation was read against these unchanged owners, not invented schemas:

1. `crates/fe2o3-amdgcn-model/examples/lower_diagnostic_ordered_program_v17.rs`:
   positional CLI; single-root typed profile; exact report at `report`;
   retained-file secure-open checks; create-new anonymous LLVM publication.
2. `crates/fe2o3-amdgcn-model/src/lowering/ordered_program_v17.rs`:
   `emit_ordered_program_v17` writes one asm sideeffect and each typed
   instruction in authored order; no assembly string comes from a caller.
3. `crates/fe2o3-amdgcn-model/src/lowering.rs`: the actual `%v<ValueId>`
   names and module/target emission. Existing retained Phase19 LLVM also
   confirms the emitted kernel ABI and store spelling; it is not a repeat
   qualification.
4. Current `scripts/ordered-repeat-source-smoke.mjs`:
   exact receipt fields, original source paths, stage paths and normal command
   contracts. Its imported limits must equal the selected source receipt, including 19 min.
5. Existing `authoring-navigation-v1-process.mjs`: bounded detached child
   transport, stdout/stderr cap, timeout/resource-guard kill and failure to
   reap can never yield success.

No current native observer is reused: their exact accepted instruction profiles
do not admit MOV followed by N ADDs merely because total count is 3 or 16.
O0/O3, HSACO bytes, descriptor decoding, register lifetimes, GPU results,
protected analysis/proof and production resume remain unavailable here.

## Prerequisites and commands

Use a checkout containing the bounded-repeat device implementation and its exact
provider-closure refresh. Record the current source/build-input census and build
matching tools from that checkout. Frozen older exporter/extractor/DSO graphs are
not a substitute. The separate source acceptance capture must pass with the
current source-acceptance driver and retain all its original files.

Implementation and controls:

- [LLVM observation driver](../scripts/ordered-repeat-llvm-observation.mjs)
- [Pure controls](../scripts/tests/ordered-repeat-llvm-observation.test.mjs)

Keep existing helper scripts adjacent under `scripts/`. The new script imports
the source-acceptance validators and the existing process helper; it does not duplicate or
change the compiler/frontend owners. It requires that the exact imported source-acceptance
source script be among the original capture's retained pins. Copying a capture
to a different directory or pairing it with another checkout's imports refuses.

Build the example with pinned Cargo/rustc and a fresh target directory using the
matching current build environment:

    PINNED_CARGO build --offline --locked --jobs 2 \
      -p fe2o3-amdgcn-model \
      --example lower_diagnostic_ordered_program_v17 \
      --message-format=json-render-diagnostics

The example already exists and is auto-discovered; no Cargo manifest, protected
Worker CMake or decoder changes are needed. Reuse the current source-build
provenance, not an unrelated shared-target dependency graph. Independently pin
the resulting executable byte count/SHA and applicable dynamic libraries/current
build inputs in the outer supervision record.

Run the pure controls separately:

    node --test scripts/tests/ordered-repeat-llvm-observation.test.mjs

Reproduce the observation with canonical absolute paths:

    node scripts/ordered-repeat-llvm-observation.mjs \
      --repo /abs/current-candidate \
      --receipt /abs/successful-r2-source-output/receipt.json \
      --receipt-bytes SOURCE_RECEIPT_BYTES \
      --receipt-sha256 SOURCE_RECEIPT_SHA256 \
      --lowerer /abs/current-target/debug/examples/lower_diagnostic_ordered_program_v17 \
      --lowerer-bytes LOWERER_BYTES \
      --lowerer-sha256 LOWERER_SHA256 \
      --output /abs/existing-parent/new-llvm-observation

Numbers and hashes above are placeholders. Supply independently selected exact
identities; the runner must not compute expected identities from whichever file
happens to be at a path and silently adopt it.
The lowerer executable is checked before/after each call and with the final
selected-file census. The source receipt and all its original pins remain
unchanged; no original is opened writable.

## Resource and process contract

- Exactly 4 calls, ≤60 s each; a 270 s cooperative deadline leaves 30 s beneath
  the required five-minute hard process-group supervisor. Cooperative checks do not
  constitute hard realtime/RSS limits; blocking I/O still needs outer control.
- ≤4096 bytes per stdout/stderr stream, ≤64KiB for each actual LLVM output,
  ≤1MiB per parsed source receipt/new receipt and ≤64MiB total output directory.
  The fresh output directory is flat, at most 32 regular files; symlinks and
  unexpected directories refuse. Existing names are never overwritten.
- At most 512 historical pins, ≤512 MiB per selected file and ≤2 GiB historical
  selected bytes, exactly matching the source-capture input contract. Added source/tool/
  helper/output pins bring the closed cap to 560 files and 3 GiB total selected
  bytes. Large selected files are streamed for hashing, not kept as one buffer.
- Every selected read validates canonical non-symlink paths, regular-file size,
  fstat before/after and pathname identity; pins bind SHA, byte count, device,
  inode, mode, nlink, mtime and ctime. Every historical pin is checked initially
  and again at final completion. Every consumed raw historical leaf must already
  be pinned by the original capture.
- Canonical admission has the existing separate 64 MiB storage and 2^26 work
  limits. The emitter has its existing separate 16 MiB text budget, with 64 KiB
  publication checked afterward. These are not summed into an invented
  allocator/RSS guarantee.
- No shell subprocess, Cargo, exporter, simulator, native assembler, linker,
  Worker or GPU command is spawned by this script. Only the four explicit
  lowerer argv calls execute. `LD_PRELOAD` and inherited `FE2O3_*` variables
  are removed; supply and pin any necessary `LD_LIBRARY_PATH` dependencies.
- Outer source/build-tool closure, unchanged runtime libraries, exclusion of
  concurrent writers, current task storage/RAM limits and hard process-group
  cleanup remain required. Before/after selected-file hashes are not protection
  against a privileged peer changing and restoring data between observations.

Output names are four `<label>.ll`, eight `<label>.stdout/.stderr`, then a
new `receipt.json`. If a later check fails, already complete outputs are
retained; `failure.json` is attempted, without cleanup or success relabeling.
A failure during pre-output input validation may produce stderr only, so retain
the enclosing command receipt too.

## Controls and qualification

22 synthetic groups cover all three counts/repeat equality; missing/extra
report fields and authority drift; separate raw/canonical/LLVM identities;
same-size changed input bytes; missing/reordered/extra/e64 instructions;
register/constraint/clobber/argument changes; duplicate or module asm; wrong
result store; kernel ABI/launch/target drift; malformed text and resource limits;
lossy/stale source capture rows/streams; transport failures; exact external pins
and before/after inode/time changes; CLI paths and bounded call counts.

The [dated evidence](evidence/authoring-repeat-fault-20260922.md#final-publication-qualification) records passed
canonical and mirror observations and their external command receipts. Each
checked four whole LLVM outputs and retained the exact original source capture;
revalidating prior CPU results did not execute another simulator.

For changed inputs, rebuild the matching example, obtain a new passed source
capture and repeat these four lowerings under the hard supervisor. The receipt
remains observation-only: native, hardware, physical lifetime, protected proof,
source/closure authentication, production-resume, runtime-scheduling and
milestone-completion claims remain unavailable. The exact emitter text matcher
is not a general LLVM verifier.
