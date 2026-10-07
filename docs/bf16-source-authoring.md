# Inspect and publish BF16 helper source

The Linux `fe2o3-rustc-extract` binary can inspect a supported Rust BF16 matrix
expression and, with an explicit request, publish a separate Rust helper
candidate. Identity preserves the four returned components; Swap01 swaps the
first two. The compiler rechecks the live source, MIR and verified kernel IR
before writing. Exported selection facts cannot replace that compiler state.

This interface is available at
[89e06d9619ef89302e1399906b293a86f6f4d6ad](https://github.com/harsh-nod/fe2o3/commit/89e06d9619ef89302e1399906b293a86f6f4d6ad).
It publishes source only. It does not compile the candidate, allocate physical
VGPRs, emit an artifact, execute a simulator, or launch a GPU.

## Prepare a current invocation

This guide starts with an already provisioned Linux compiler and a current,
complete rustc argument array for the selected package. It is not a one-command
clean-checkout setup or a qualified replacement for the full Cargo build
pipeline. The qualification directly invoked the actual extractor as a rustc
wrapper; it did not qualify the complete workflow through Cargo's `RUSTC_WRAPPER`.

Use the existing compiler setup with the matching nightly, `rustc-dev`,
`rust-src`, current device/core metadata and runtime libraries. The qualified
profile used `nightly-2026-04-03`, `gfx942`, `-xnack`, Wave64, a single
`[64, 1, 1]` workgroup, and the exact
[direct BF16 fixture](../crates/rustc-codegen-fe2o3/tests/fixtures/tiled-region-inspection-v1/src/lib.rs).
No alternate fixture feature was enabled.

Before the commands below, bind these **local shell values** to your current
setup; none is a new compiler environment variable:

- `BF16_EXTRACTOR`: the absolute path to your matching `fe2o3-rustc-extract`.
- `BF16_WORK`: your task directory containing the selected package at `original/`.
  Its source is `original/src/lib.rs`, with the unchanged 3,950-byte fixture.
- `BF16_RUSTC_ARGS`: a Bash array containing the complete current invocation,
  starting with the actual rustc executable, not the extractor.
- `BF16_TUTORIAL`: the absolute path to a checkout of `fe2o3-kernels` containing
  the request helper used below.

Preserve the actual package source, manifest, lock, dependency metadata, sysroot,
and argument order while inspecting and publishing. The array includes the
current `--extern fe2o3_device=...` and `--extern noprelude:core=...` paths,
dependency search paths and a fresh analysis output directory. Do not copy
historical absolute paths, `.rmeta` names or metadata salts from a report.

The selected argument profile includes:

~~~text
--crate-name fe2o3_tiled_region_inspection_v1_fixture
--edition=2024 --crate-type=lib --target=amdgcn-amd-amdhsa
--emit=metadata -Copt-level=3 -Cpanic=abort -Cembed-bitcode=no
-Cdebug-assertions=off -Coverflow-checks=on -Ctarget-cpu=gfx942
-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32
-Zalways-encode-mir -Zunstable-options
~~~

The complete array also needs its actual source, sysroot, output, dependencies
and explicit `-Cmetadata` argument. This abbreviated profile is not a runnable
invocation. The wrapper captures the current package manifest and rederives
portable metadata and crate binding; do not supply a fabricated
`FE2O3_CRATE_BINDING_ID_V1` or Cargo observation.

The extractor may be dynamically linked. Its backend library must match the
executable, and `LD_LIBRARY_PATH` must resolve that backend and the matching
rustc runtime. The qualified build used its release directory followed by the
toolchain's `lib` directory. Matching a filename alone is insufficient.

## Inspect without publishing

Start without other `FE2O3_EXTRACT_*` output modes, origin, composition promotion
or normal-composition opt-ins. These modes are mutually exclusive with this
source action. Define this small shell wrapper over the **actual public binary**:

~~~bash
bf16_source_action() {
  local -a bf16_mode=(
    "FE2O3_EXTRACT_BF16_TILE_SOURCE_DIRECTORY_V1=$1"
  )
  if [ "$#" -eq 2 ]; then
    bf16_mode+=("FE2O3_EXTRACT_BF16_TILE_PROMOTION_REQUEST_V1=$2")
  fi
  env -u FE2O3_EXTRACT_BF16_TILE_PROMOTION_REQUEST_V1 \
    CARGO_PRIMARY_PACKAGE=1 \
    CARGO_MANIFEST_DIR="$BF16_WORK/original" \
    CARGO_PKG_NAME=fe2o3-tiled-region-inspection-v1-fixture \
    CARGO_PKG_VERSION=0.0.0 \
    CARGO_CRATE_NAME=fe2o3_tiled_region_inspection_v1_fixture \
    FE2O3_EXTRACT_CRATE_V1=fe2o3_tiled_region_inspection_v1_fixture \
    "${bf16_mode[@]}" \
    "$BF16_EXTRACTOR" "${BF16_RUSTC_ARGS[@]}"
}

cd "$BF16_WORK"
bf16_source_action "$BF16_WORK/inspect"
~~~

The output directory must not already exist. Require process exit zero, then
read `inspect/observation.json`. Success has `mode: "inspect"`,
`status: "inspected"` and `source_postflight_ok: true`. It creates no candidate.
The report's `selection` contains:

| Field | Meaning |
| --- | --- |
| `semantic_sha256` | The current source-semantic identity |
| `canonical_sha256` | Compiler canonical identity, **not** SHA-256 of serialized IR |
| `mir_sha256` | The selected source MIR digest |
| `original_sha256` | SHA-256 of the original source file |

The values select a current compiler-owned object on the next invocation. They
are not permission to load an edited IR object or resume an old compiler session.

## Request a separate candidate

The tutorial's request helper copies the four selectors into the existing
closed request schema. It checks the lesson's source and report fields; it
cannot authenticate a report or prove that its producing process succeeded.
Always use the fresh successful report, not a checked-in example.

~~~bash
mkdir -p identity/src swap01/src

node "$BF16_TUTORIAL/examples/bf16-generated-source/make-request.mjs" \
  "$BF16_WORK/inspect/observation.json" identity \
  "$BF16_WORK/identity.request.json"

bf16_source_action "$BF16_WORK/publish-identity" \
  "$BF16_WORK/identity.request.json"

node "$BF16_TUTORIAL/examples/bf16-generated-source/make-request.mjs" \
  "$BF16_WORK/inspect/observation.json" swap01 \
  "$BF16_WORK/swap01.request.json"

bf16_source_action "$BF16_WORK/publish-swap01" \
  "$BF16_WORK/swap01.request.json"
~~~

Stop on a nonzero exit from any command. Request files, output directories and
candidate files use new names; neither the helper nor the publisher overwrites
an existing file. Keep the original package unchanged between actions.

The request schema is `fe2o3-bf16-tile-source-promotion-request-v1`.
In addition to the four digests, it has exactly these selection fields:

~~~json
{
  "original_path": "original/src/lib.rs",
  "candidate_path": "identity/src/lib.rs",
  "helper_name": "__fe2o3_bf16_tile_identity",
  "return_order": "identity"
}
~~~

This is an explanatory fragment, not a complete request. Swap01 changes the
candidate path, helper suffix and `return_order` to `swap01`. The request is
limited to 8,192 bytes. Paths are bounded relative Rust filenames; they must be
distinct. Other return orders, arbitrary helper names and arbitrary source
shapes are outside this lesson's supported profile.

A successful publication report has `status: "candidate_created"`,
`source_postflight_ok: true`, `publication.created_new: true` and
`publication.original_overwritten: false`. The reported candidate hash and byte
count identify the new file. Device and inode values are decimal strings,
not floating-point addresses. Report serialization is bounded to 16 KiB;
this is not a bound on all compiler allocations.

## Preserve failures and recompile fresh

Check both the process exit and the retained report. A report cannot override
a later fatal compiler error. Preserve the source, request, candidate, output
directory and diagnostics when anything fails; do not automatically delete or
retry a candidate whose publication status is uncertain.

| Outcome | Required interpretation |
| --- | --- |
| `not_attempted` | This publisher did not attempt candidate creation; diagnostic outputs may still exist |
| `may_have_created_candidate` | Inspect retained paths and any published facts, including after an error |
| No final report | A panic, abort, kill or earlier error can prevent reporting; absence is not proof of rollback |

The qualified public refusal cases include malformed request JSON, a stale MIR
selector, an existing candidate path and a promotion request without the source
output mode. An existing candidate remained byte-for-byte and inode-identical.
Its failed report conservatively retained `may_have_created_candidate`.

`candidate_compiled` and `fresh_compilation_observed` remain false in the
publication result. Use a new package/source invocation to admit any candidate
you want to inspect or modify further. The public source action is not a public
CPU replay command. At the linked implementation revision, the normal route still refuses
`BF16 nominal source-ranked projection`; a CPU test does not discharge the
missing caller capability, tensor-layout, full-wave and exact-result obligations.

## What has been qualified

The actual public-binary campaign completed 13 supervised processes:
one inspection, two source publications and two separate fresh-candidate
frontend checks, plus preparation. The fresh checks used 36 positive CPU
requests and 32 expected CPU refusals, retaining two complete 105,440-byte
sidecars. A separate nine-process campaign checked one inspection and the four
public CLI refusals above. Neither campaign emitted a normal artifact or ran
a GPU.

The source was tested at census
`65cb667a72b1ffedbc850715bd3d1923ea939ff77500c8c0f005eeb62c7ee94a`
(9,708 files, 136,378,600 bytes), before publication as the implementation commit
linked above. The audited root receipts are
`eccc73fb6cfd1efac9b605ab7b8d2daa6c5d9a853116b5cb45e6581eb2d83797`
for the positive cohort and
`58e16583ba4730aa67ec153b1e24aae5ea624790cec736093bcef4c8b9faf69a`
for the refusal cohort.

The [tutorial](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/bf16-generated-source-promotion-v1.md)
contains the three source files and inert request/content tests.
The [earlier developer qualification](generated-bf16-source-and-bindings-qualification-20261001.md)
records a different four-session CPU campaign and the separate imported-bindings
storage checkpoint. None of these results establishes physical register control,
hardware execution, whole-action memory bounds or broad milestone completion.

## Internal consuming engineering continuation — October 7

A non-test, crate-private consuming continuation now retains the same live source,
ranked-projection owner, descriptor and accounting state through the fixed
engineering Worker loan. `observe_engineering_and_drop_v1` returns only unit
success or a typed failure; it exports no owner, artifact or reusable capability.
It replays source/descriptor joins before the engine, preserves prior resource
denials, and drops the owner on success, refusal and unwind.

The separately supervised Identity and Swap01 frontend sessions completed four
Worker invocations in total. Two additional paired campaigns rejected an invalid
return profile and invalid owner-entry conditions before any Worker invocation.
All three campaigns passed independent raw receipt/cleanup joins and point-in-time
checks that their recorded processes and exact scopes were absent. These checks
are not a complete Worker PID roster, hostile-process isolation, or numerical/GPU
qualification.

The actual campaigns used source census
`39da40e5eca87c12e7d104b03b429f84769139a2304bf4ebc066c39c6830db8f`
and test ELF SHA256
`405d943ff3217a539d1d522e76ab5df823337900347c3319bdbdd51bb7bf278b`.
The source changes are commit `2cd6180a892274bba787283261ef5710dfefdb34`;
these historical actuals are not relabeled as executions of later merged trees.
The three-case independent review is SHA256
`aed86860d4b62073cf0bc5b00bb78f08a7c008c4a15e9f6e9aa8f9a4fc508837`.

This is an internal engineering checkpoint, not a new user command. The public
source/Cargo workflow above is unchanged. Ordinary ranked attachment and canonical
call consumption, complete formal admission, protected artifact publication and
target-qualified GPU execution remain separate requirements. No M4 or U4
completion follows from this continuation.
