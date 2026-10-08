# Ordinary saved-recipe diagnostic output

This additive example interface is a source proposal, not a qualified tutorial
or a completed #282 U4 milestone. Its authored controls have not been executed.
It does not change the public API, canonical recipe codec, compilation pipeline,
source ownership, proof gates or device-launch behavior.

## Command shape

The existing Linux example remains the entry:
`crates/rustc-codegen-fe2o3/examples/source_local_order_recipe_v1.rs`.
Existing `create` and `replay` commands retain their argument counts, human
success message, ordinary driver, failure exit and output-file behavior.
The only additional exact command names are:

```text
create-json SOURCE_REL SHA ORDER RELATION STRENGTH BINDING RECIPE_OUT LLVM_OUT -- RUSTC_ARGV...
replay-json SOURCE_REL SHA RECIPE_IN RECIPE_SHA LLVM_OUT -- RUSTC_ARGV...
```

These have the same positional arguments as `create` and `replay`, respectively.
There is no general `--json` flag, pass selector, callback or cancel/resume mode.
Source paths, lowercase SHA predicates, exact/advisory relations, binding modes,
bounded retained recipe files, complete rustc arguments and absolute new output
paths retain the existing checks. The usage error retains its existing command grammar and adds a short
notice naming the two diagnostic aliases.

The caller must supply a separately bound compiler, sysroot, complete rustc argv,
working directory, source/dependency/tool currentness, process/output limits and
environment. Do not substitute an invented compiler command or a private test
probe. Rustc arguments retain ordinary compiler effects.

## Framing and failure meaning

A completed diagnostic record is one LF-terminated line beginning with
`FE2O3_SOURCE_LOCAL_ORDER_REPORT_V1 `, followed by a JSON object. A leading LF
separates it from prior compiler stdout. The JSON including its final LF is
limited to 65,536 bytes; the constant prefix and separating LF are additional.
The whole object is serialized within that bound before stdout is touched.

The report is not a clean-stdout promise. Preserve raw stdout, stderr and the
actual child exit status. A consumer must reject absent, duplicate, truncated,
malformed or conflicting records rather than selecting a convenient substring.
A stdout error can leave a partial prefix or line and returns the existing
nonzero example error. A complete-looking line with a failed flush or conflicting
exit status is not an accepted command. Other stdout is compiler output, not
another report or authority source.

Only the one ordinary public driver's returned Attempt is projected:

- On compiler acceptance, the existing recipe (Create only) and LLVM
  `create_new` writes must first complete, including their existing sync calls.
  The report then has `result.status = "accepted"` and
  `publication.status = "completed"`; the command must exit zero.
- On a returned compiler refusal, after the existing retained-recipe recheck,
  the report has `result.status = "refused"` and
  `publication.status = "not_attempted"`. Its phase, diagnostic and
  compiler-fatal flag come directly from the typed failure. The command still
  follows its original nonzero exit and stderr diagnostic path.
- Argument parsing, request construction/recipe decoding, recipe-file custody,
  output preflight, later file publication and stdout serialization/write errors
  are example errors, not invented compiler phases. They can produce no complete
  record. They must not be normalized into a typed compiler refusal.

Outputs remain non-transactional. A successfully written recipe may remain if
the later LLVM write fails; both may remain if stdout fails. Nothing rolls back,
overwrites old files, proves later file currentness or repairs a partial command.
No accepted record is emitted before the unchanged output writes finish.

## Observation fields, not authority

The `fe2o3-source-local-order-report-v1` label identifies only this diagnostic
projection. There is no decoder for importing it into compiler/proof owners.

`request.expected_current_source_sha256` is the caller's conflict predicate,
not a claim that a preflight refusal observed those current bytes. The accepted
`evidence.source_sha256` is the existing API's source observation. The report
also retains both original callback counts.

Accepted evidence uses public getters for actual source/semantic identities,
the five instance axes, original/input/output digest-length observations, order,
relation, exact/advisory strength, source-binding mode, actual constraint result,
region/result order, prefix bytes, transition digest/length/counts, fresh formal
counts, LLVM/descriptor/recipe hashes, and canonical work/storage observations.
Digests remain exact arrays of byte integers; they are not new canonical owners.
The descriptor producer and composition are existing API values, not inferred
tool or hardware identities.

`llvm_returned` and optional `recipe_returned` describe returned bytes, not an
independent reread of published files. Replay does not regenerate a recipe.
`publication.output_file_currentness_authenticated` is always false.
The publisher's `transactional` flag is always false.

Refusal keeps success evidence and returned-byte descriptions null. Its phase is
one of the API's existing request, frontend, eligibility, recipe_binding,
continuation, constraint, observation or source_currentness variants. No
cancellation result or timing stage is synthesized.

Report authority is `observation_only`. Execution authentication, proof,
artifact and launch authority are false. The report does not authenticate itself
or make a saved recipe into a receipt. Timing and complete-owner memory are null,
not zero; existing canonical work/storage fields are not a whole-owner memory
budget or a performance qualification.

## Focused controls and future qualification

Twelve authored example tests cover exact legacy/additive command parsing,
exhaustive typed phase and schedule/constraint labels, the real public driver's
empty-argv preflight refusal (zero callbacks), the request/observation boundary,
exact report field roster, non-transactional refusal publication, bounded
serialization including LF, exact prefix framing, write/serializer failure and
retention of the single ordinary driver/create-new output path.

They do not manufacture an accepted API object, simulate a compiler callback or
claim genuine success coverage. Root must separately build and run this example
test target with the current source, toolchain and limits, then qualify ordinary
success/refusal paths with real source and new files. Existing public API tests,
canonical codec checks and saved-recipe integration controls remain applicable.

## fe2o3-kernels tutorial integration plan (not implemented here)

The existing lesson is `docs/saved-local-order-recipes-v1.md`. Coordinate its
owner (#275) before changing it or shared site navigation/inventory. A dedicated
leaf can be added at `examples/saved-local-order-recipes/lab.mjs` with adjacent
`lab.test.mjs`; it should consume this ordinary diagnostic output, not the
private timing adapter or a second recipe codec.

The lesson should guide an author through a genuine Create, exact-revision
Replay, checked rebind after an actual Rust initializer edit, an exact-revision
refusal after that edit, and an advisory relation that is actually not honored.
For every process, retain raw streams/exit, exact source/recipe/LLVM files and
the independently bound compiler invocation. Join source and recipe predicates
to their retained bytes, compare all five instance axes and N/I/L observations,
and reread/hash successful output files before describing them as current.
Show actual typed refusal diagnostics without relabeling missing reports.

The leaf's bounded parser must require one complete LF-framed report, its exact
schema/field shapes and byte-array lengths, safe integer bounds and closed
labels, consistent accepted/refused nullability and exit status, and all false
authority flags. Reject duplicate/unknown/missing fields, unsupported labels,
wrong shapes, record splicing/truncation, authority escalation and replaced
source/recipe/LLVM bytes. JSON recognition is display validation, never
reconstruction of canonical receipt or execution authority.

Keep the command examples parameterized until root records the actual example
ELF, complete argv/env/cwd, current source and output hashes under a new execution
window. Add machine-readable links to those genuine runs only after their
commands pass. Do not reuse the earlier U4 benchmark as evidence for this new
ordinary-example interface, and do not mark U4, native execution, full owner
memory, cancellation, tutorials or site publication complete from this patch.
