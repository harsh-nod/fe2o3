# Promote an ordered assembly region into a Rust helper

This is an explicit, create-new source editing workflow for the bounded ordered-u32
composition path. It does not decompile arbitrary machine code or preserve a
compiler owner across source edits.

The first publisher accepts a direct, flat `amdgpu_ordered_program!` occurrence in
the selected kernel root, with three direct u32 arguments. It inserts a body-local
`#[inline(never)] fn (u32, u32, u32) -> u32` helper and replaces that occurrence
with its call. Authenticated HIR/source spans choose the insertion and replacement
sites. Wrapper macros, local or constant operand captures, nested helper promotion,
and generated-name collisions are refused. The original file is never overwritten.

## Workflow

1. Run the ordered-composition diagnostic driver on the actual source and retain
   its `observation.json`. Select a root-region definition from its `definitions`
   roster and retain the source file's SHA-256.
2. Write a bounded promotion request. Its identities and ordinal are selectors;
   they cannot reconstruct the source seed or authorize a write by themselves.
3. Explicitly invoke `run_ordered_composition_source_promotion_driver_v1(args,
   diagnostic_output, request_path)`. This starts a new frontend callback. The
   callback must independently obtain the same selected semantic/canonical owner,
   current source file, authenticated expansion and HIR insertion site.
4. Inspect the newly created candidate. Build a genuine package/metadata binding
   for that candidate and run a **fresh frontend**. Publication alone does not
   establish that the candidate compiles, passes checked lowering, or preserves
   the old program's behavior.

The extractor also exposes
`FE2O3_EXTRACT_ORDERED_COMPOSITION_PROMOTION_REQUEST_V1`, only with the explicit
`FE2O3_EXTRACT_DIAGNOSTIC_ORDERED_COMPOSITION_DIRECTORY_V1` mode. Setting the
diagnostic selector alone does not publish source. The library-driver qualifier
described below does not exercise this binary selector; its end-to-end
qualification is separate.

## Closed request

The request is at most 8192 bytes, read from a regular non-symlink file. Unknown
fields and duplicate fields are refused. The following is a **template**, not
runnable input: replace all three digest placeholders using the current actual
source observation. Paths are bounded relative Rust source paths interpreted from
the live frontend's working directory.

```json
{
  "schema": "fe2o3-ordered-composition-source-promotion-request-v1",
  "semantic_sha256": "<64 lowercase nonzero hex digits from the actual observation>",
  "canonical_sha256": "<64 lowercase nonzero hex digits from the actual observation>",
  "definition_ordinal": 0,
  "original_path": "src/kernel.rs",
  "original_sha256": "<64 lowercase nonzero hex digits for the current source file>",
  "candidate_path": "src/kernel_candidate.rs",
  "helper_name": "__fe2o3_region_0123456789abcdef"
}
```

The candidate path must differ from the original and must not exist. Its parent
directory must already exist. The definition ordinal is less than eight and must
name the selected live owner's actual root region. The helper name must be exactly
`__fe2o3_region_` followed by sixteen lowercase hexadecimal digits. That grammar
does not establish uniqueness: the later actual HIR namespace collision check is
still required.

Omitting `edit` (or passing null) copies the selected typed program and register
roles. An optional edit can supply 1–16 typed instructions and five register
bindings in input0, input1, input2, scratch, output order:

```json
{
  "registers": [10, 11, 12, 8, 9],
  "instructions": [
    {"kind": "binary", "opcode": "xor", "destination": "scratch",
     "left": "input0", "right": "input1"},
    {"kind": "binary", "opcode": "and", "destination": "output",
     "left": "scratch", "right": "input2"}
  ]
}
```

Insert that object as the request's `edit` field. Binary opcodes are add, subtract,
and, or and xor; moves use `kind: "move"` and `source`. Only scratch/output are
destinations. Input registers are read-only; aliases, unsupported register numbers
and read-before-definition programs are rejected by the existing typed program
and register validators. There is no raw Rust, assembly string or arbitrary
instruction escape in this request.

A typed edit can intentionally change behavior. `program_edited` records a
difference in the typed instruction/register representation, **not** an
equivalence theorem. Even an unchanged representation requires fresh compilation.

## Outputs and refusal effects

The diagnostic directory always describes the **original live owner**:
`canonical-v17.bin`, `canonical.ll`, and, on successful completion,
`observation.json`. Its optional `source_promotion` record contains the candidate
hash, size and inode/device facts, together with
`fresh_compilation_required: true` and `fresh_compilation_observed: false`.
Those are historical publication facts, not custody or launch authority.

An action can fail after original-owner diagnostics were written. Publication or
a later accounting/report failure can also leave a candidate. Preserve the
output and inspect the effect diagnostic before retrying:

- `NotAttempted` means the publisher did not attempt its create-new operation.
- `MayHaveCreatedCandidate` conservatively includes an attempted no-replace
  publication, even when an existing candidate caused refusal. It does not claim
  that a new file was created.

Do not infer absence from a failed compiler callback or missing final report.
There is no automatic overwrite, rollback, recompile, native compilation or GPU
launch.

## Qualification scope

The actual-source public library-driver R1 gate completed with 17 isolated
children: 14 public driver invocations and three fresh promoted-source callbacks,
96 bounded CPU cases, three publications and ten exact refusals. Five refusals
occur before frontend startup and are not counted as compiler sessions. It
retains separate source/package metadata for original and generated files,
validates a shared dependency closure, checks offset-view canaries and
initialization, and preserves partial outputs and existing candidates.

The extractor-binary R2 outer gate and independent retained-artifact review also
completed separately from the library result. Fresh callback canonical hashes
were observed live, not independently retained as new canonical files. The exact
receipts, source-normal qualification and pending native boundaries are recorded
in [the dated qualification note](ordered-composition-qualification-20260924.md).

This workflow is a bounded source-to-helper-to-fresh-source slice. It does not
close general authoring, arbitrary round trips, normal checked/protected
compilation, hardware execution, or the broader milestone exits.
