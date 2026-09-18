# Tutorial Kernel Pair Obligations

Every intended-runnable tutorial kernel needs an explicit SIMT source and a
structured tile source, with independent per-variant/per-target qualification.
A mixed example is additional. See [issue 275](https://github.com/harsh-nod/fe2o3/issues/275).

Inspect the current source contracts using the existing production validator:

```sh
python3 scripts/validate-tutorial-kernel-manifest.py --emit-kernel-pairs
```

To include lexical observations from an exported tutorial runtime projection:

```sh
python3 scripts/validate-tutorial-kernel-manifest.py --emit-kernel-pairs \
  --site-inventory /absolute/path/runtime-curriculum.json
```

The optional projection must pass the existing ordered lesson/tab, exact
displayed-byte and source-metadata checks first. It does not have to retain the
old site Git HEAD when its validated display/source projection is unchanged.
Without it, displayed declarations are unknown, not guessed from current files.

## Report Contract

### Live Compiler Source Census

Set `FE2O3_DIAGNOSTIC_SOURCE_CENSUS_PATH_V1` to a new file path and
`FE2O3_DIAGNOSTIC_SOURCE_CENSUS_RUN_ID_V1` to a fresh caller-generated
64-digit lowercase hexadecimal ID when invoking the existing
`fe2o3-rustc-extract` wrapper. Consumers must match that expected `runId`,
arguments and extraction mode, not just find a report at the requested path.
The run ID is diagnostic correlation, not execution authentication.
This optional diagnostic observes
the same authenticated collected closure before the production transaction
consumes it. It does not select a different importer or extraction mode.

The `fe2o3-diagnostic-source-census-v1` JSON records the actual driver arguments,
working directory, extraction mode/version, run ID, selected target, canonical function/definition/instance
identities, compiler-assigned roles, and separate definition and identifier
provenance. Original byte offsets account for BOM/CRLF normalization and are
distinct from rustc's normalized offsets. Expansion and callsite anchors remain
separate. Source SHA-256 values cover original bytes checked against rustc's
recorded source hash; diagnostic paths and stable source-file identities alone
are not content identities. An identifier is available only when its source
token matches the compiled definition, including a possible `r#` prefix.
Generated entry wrappers may therefore have an unavailable identifier even
when their definition anchor is known. Their logical names are not substituted
for missing identifier provenance.

Unavailable files, mismatched bytes, missing identifiers and exhausted bounds
remain explicitly unavailable. The census permits at most 512 functions, 128
source-file observations, 4 MiB per source file, 16 MiB total source reads, 64
macro-expansion levels, 1 MiB of argument bytes and 4 MiB of encoded output.
Existing report paths are never overwritten. Paths aliasing the explicit
extraction output or wrapper crate-binding sidecar disable the report.
Protected output symlinks also disable recording, including dangling links.

`extractionSucceeded` describes only that extraction invocation. A selected
closure may still fail semantic import or a later check; observation preserves
that failure. A rustc fatal error before collection yields unavailable selection
and is re-raised unchanged. A process crash or interruption can leave no
complete report. Diagnostic
errors do not change the extraction result. Empty or incomplete files are not
reports.

This census is not a compiler-execution receipt, proof, executable artifact or
qualification. `diagnosticOnly` is true; `qualified` and
`authenticatesCompilerExecution` are false. The optional consumer below compares
existing fixture contracts; it establishes neither compile-time Cargo/dependency
custody nor generated-source mapping. It does not change any pending display
binding or kernel count.

### Optional Diagnostic Comparison

```sh
python3 scripts/validate-tutorial-kernel-manifest.py --emit-kernel-pairs \
  --source-census /absolute/path/census.json \
  --source-census-request /absolute/path/independent-request.json
```

The two options require each other and `--emit-kernel-pairs`. Existing manifest,
lock, physical package source closure, fixture selection and display contracts
are validated first. Only existing `fixture-source-contract` display bindings
are supported. The consumer reuses that selection and its exact source member,
hash, byte length and scanner-established occurrence; it does not parse another
Rust selection or search for a replacement identifier.

Supply the request independently of the census, retaining expectations before
the invocation. Its exact fields are:

```json
{
  "schema": "fe2o3-tutorial-source-census-request-v1",
  "fixtureId": "<existing fixture ID>",
  "contractSha256": "<existing compilerInput.contractSha256>",
  "runId": "<fresh caller-generated 64 lowercase hexadecimal digits>",
  "arguments": ["<driver argv[0]>", "<every subsequent driver argument in order>"],
  "workingDirectory": "<absolute driver working directory>",
  "extractionMode": {
    "kind": "compiler-handoff",
    "version": 3,
    "expected_target": "gfx950:xnack-"
  },
  "cargoIntent": {
    "packageManifest": "<existing repository-relative Cargo.toml>",
    "packageName": "<Cargo package.name>",
    "cargoTarget": {"kind": "lib", "name": "<library name>", "sourcePath": "<package-relative library source>"},
    "features": ["<sorted direct feature selection>"],
    "defaultFeatures": false
  }
}
```

`arguments` is the exact driver argv, not the Cargo command. Full extraction
mode objects match the producer: `semantic-mir` and `ranked-memory` have only
`kind`; `llvm` also requires nullable `expected_target`; `compiler-handoff`
requires `version` (1 or 3) and nullable `expected_target`; `simulation-bundle`
requires `version` (1 through 6). Field names inside the mode and coordinate
objects preserve the producer's snake case. A non-null expected target and an
available selection target must match the fixture's production target
(`gfx942:xnack-` or `gfx950:xnack-`). Unknown versions require a reviewed update.

Cargo intent stays explicitly caller-supplied. Package/library selection,
direct features and `defaultFeatures` must equal the existing fixture contract.
The existing `cargo_feature_closure` computes the expected feature cfgs, which
must match the driver `--cfg feature="..."`/`--cfg=feature="..."` values exactly.
Response-file arguments and duplicate or malformed feature cfgs reject.
Absence of `feature="default"` never establishes `--no-default-features`: a
package with no default feature can produce the same cfgs for either intent.
Neither these comparisons nor a matching nonce authenticate the invocation or
prove which Cargo/dependency inputs the compiler consumed. Nonce freshness and
the independence of the expected request remain caller responsibilities.

On success the otherwise unchanged pair report gains `sourceCensusComparison`,
schema `fe2o3-tutorial-source-census-comparison-v1`. It retains the invocation,
`callerCargoIntent`, enabled features, fixture/contract identity, extraction
outcome and complete validated selection observations. Each `comparisons` row
contains the exact `expected` display/source/token coordinates, `status`
(`matched` or `unresolved`), nullable `functionIdentity` and nullable `reason`.
The diagnostic section fixes `diagnosticOnly=true`, `qualified=false` and
`authenticatesCompilerExecution=false` independently of input claims.

A match requires one `kernel-entry` with an available identifier whose expansion
anchor identifies the exact original file bytes and complete token, including
`r#` where present. Normalized coordinates are checked separately and never
substituted for original offsets. Callsite and definition anchors remain
separate observations. Logical/export names, helpers and available definition
anchors cannot fill an unavailable generated identifier. Such entries remain
unresolved even when their names look identical. Available entries with wrong
files or ranges, duplicate candidates and inconsistent source metadata reject.
Original/normalized endpoint checks use sorted, deduplicated coordinates and
disjoint source slices, scanning each known file at most once for that check.
Paths outside the expected source members are retained diagnostic labels; the
consumer does not open arbitrary census-named files or validate dependency
source custody. `compiledSourceHash` and compiler identities remain diagnostic
metadata; original SHA-256 and lengths are compared to the physical binding.

`extractionSucceeded=false` is preserved with `extractionStatus="failed"`, even
if an identifier matches. A structurally valid failed or unavailable extraction
can be inspected in complete JSON; command success means validation of this
diagnostic comparison only. It does not mean extraction succeeded.

Both inputs require strict UTF-8 JSON, unique keys, exact fields and types,
finite numbers and at most 4 MiB each. Producer argument/function/file/span
bounds apply, including byte-based string limits and integer offsets (booleans
are not integers). Invalid inputs or an over-budget final report reject before
stdout. Without the options the output is unchanged. With them, qualification,
pending bindings, inventory/counts and `runtimeCensusValidated` are unchanged;
the latter still depends solely on the separate site runtime projection.

Focused consumer tests run without Cargo:

```sh
python3 -I -B scripts/tests/tutorial_kernel_manifest.py TutorialSourceCensusTests
```

### Inventory Projection

The JSON schema is `fe2o3-tutorial-kernel-pair-obligations-v2` when the manifest
contains its optional `kernelInventory` extension. Legacy manifests without
that extension retain `fe2o3-tutorial-kernel-pair-obligations-v1`. This is a derived
diagnostic view of `config/tutorial-kernel-manifest-v1.json`, not another stored
inventory, execution receipt, release gate or source of compiler authority.
The existing V2 curriculum and source-item digest domain remain unchanged. The command validates all source
contracts before producing any stdout; it cannot be combined with `--emit-matrix`.

- `fixtureSelections` preserves each fixture/symbol/target input selection and
  its existing contract digest. Complete feature, Cargo target and source-path
  identity is retained. Scope lesson links are not exact displayed bindings.
- `sourceDriverCases` preserves exact lesson/tab/case identity, source contract,
  driver and expectation. A required rejection stays a rejection, not a working
  implementation or an excuse to downgrade a required positive.
- `displayObservations` covers executable Rust kernel tabs. Optional lexical
  names preserve order and duplicates, including cfg alternatives. They do not
  establish rustc feature reachability, selected kernels or runnable behavior.
- `lessonRequirements` retains SIMT/tile and any mixed lesson obligations;
  `sourceBindingGaps` retains the existing unresolved source paths.
- `sourceContractSha256` hashes the whole input manifest encoded as sorted-key,
  compact, ASCII JSON. This content identity is not a signature or qualification.

Do not add these collections to obtain a kernel count. They can overlap, repeat
symbols under different features, and omit still-unbound kernel identities.
For the current incomplete inventory, `requiredPairCount` is null,
`qualifiedPairCount` is zero, `inventoryComplete` and `qualified` are false, variant bindings are pending,
and execution stages are not evaluated. Existing historical successes do not
acquire new source/variant/target bindings from this report.

The report is bounded to 4096 charged projection/inventory records, including
input tabs, references and scanned items, and 16 MiB of encoded JSON. Each Rust
scan is limited to 4 MiB and 4096 declarations. Exhaustion rejects without partial JSON.
The tests run in the existing `scripts/tests/kernel-compile-matrix.sh` CI gate.

## Kernel Identity Extension

The sibling `kernelInventory` in the existing manifest is an explicit
`fe2o3-tutorial-kernel-identities-v1` contract. It does not replace or duplicate
the fixture inputs, source-driver contracts or curriculum source metadata.

- `kernels` assigns stable IDs to exact positive fixture/symbol selections or
  source-driver cases. Each ID has pending SIMT and tile records with a blocker
  owner, issue and reason. Matching names, source paths or historical displayed
  bytes alone cannot establish identity.
- `negativeCases` retains the exact required-refusal source-driver cases outside
  the positive pair roster.
- `displayItems` classifies physical function occurrences, not attribute hits.
  The coordinate is lesson ID, tab ordinal and function-name UTF-8 byte offset.
  Repeated kernel attributes on one function produce one occurrence; distinct
  declarations with the same name remain distinct. Raw-identifier offsets
  include the `r#` prefix.
- The census covers attributed functions in every Rust tab kind, including
  host and comparison tabs, plus ordinary functions in Rust kernel tabs.
  Bare kernel excerpts remain kernels; helper functions, conceptual examples
  and required negatives have explicit separate classifications. Bare-function
  intent is reviewed inventory data, not inferred from lexical syntax.
- A `source-driver-contract` binding must match an existing exact source case
  and its displayed fragment.
- A `fixture-source-contract` is an expected fixture/source contract, not
  rustc-selected execution or semantic admission. It joins an existing
  feature-specific fixture identity to one attributed function occurrence in an
  exact whole-file kernel display. The validator rechecks the physical package
  closure, Cargo manifest/lock, selected source member, file/display digests and
  UTF-8 name offset. Package membership alone is insufficient.
  Selection uses the existing lexer/scanner with a restricted check of ordinary
  sibling modules and literal `feature`/`target_arch` cfg expressions (`all`,
  `any`, `not`) for AMDGPU. Unknown cfg, item macros, transforming attributes,
  build-script cfg, dependency features, nested/inline/path-selected modules,
  duplicate modules or selected symbols reject a claimed binding. Such sources
  retain `pending` until an exact selection can be established. This bounded
  check does not replace rustc or qualify kernel implementations.
  Supported non-cfg forms are bare `kernel`/`inline`, parenthesized
  `allow`/`deny`/`forbid`/`warn`/`doc`/`kernel`, and `inline(always)`/`inline(never)`.
  Qualified attribute paths and name-value forms such as `#[doc = "..."]` are
  unsupported and reject claimed bindings.

Eight GPT-OSS occurrences in tabs 1-6 now retain these expected fixture/source
contracts: serial-router, held-fragments, interleaved-stores, the three
materialized components, pipelined-attention and scalar-attention. The latter
two bind the existing whole-file sources at function-name UTF-8 offsets 1508
and 3041 under `kernel-gpt-oss-decode-pipelined-attention` and
`kernel-gpt-oss-decode-scalar-attention`, respectively. Their source bodies and
the original 47 fixture obligations are unchanged. These are positive,
unqualified candidates: the retained compiler-rejected display labels do not
make them required-negative cases. CPU-reference coverage remains pending;
their Bundle V7 / KIR V12 simulation requests and independent oracles remain
`pending-design`, without content pins or execution receipts.

The inventory derives 50 fixtures, 60 known identities and 56 pending display
bindings. The GPT-OSS lesson-level source gap is cleared; `gemm-proof-plan`'s
historical `examples/tiled_gemm_v1/src/kernel.rs` gap remains. Distinct feature
selections retain distinct identities despite sharing the megakernel symbol.
`requiredPairCount` remains null, `inventoryComplete` false and
`qualifiedPairCount` zero; the known identities are not a final pair denominator.
Other source/display joins remain `pending`; a fixture's lesson scope is not
an exact displayed-source binding.

The V2 report's `kernelInventory` projects these records and reports known
identity, display, negative-case and pending-join counts separately.
`runtimeCensusValidated` requires the actual runtime projection and complete
occurrence reconciliation. Without it, retained offsets are declarations to
check, not observed source facts. Supplying `--site-inventory` checks this
census even when neither report nor matrix output is requested.

`inventoryComplete` additionally requires all required source/display joins,
including coverage of the positive selections.
Only then may `requiredPairCount` name a reconciled denominator. Neither flag
qualifies implementations: pending variants still require their own sources,
oracles and every production/target gate. The current known source selections
must not be presented as the final number of tutorial pairs.

This is a bounded lexical inventory, not a general Rust parser or compiler
feature-reachability proof. The runtime census excludes comments, literals and
`macro_rules!` template bodies and conservatively retains conditional kernel
attributes. Direct fixture selection additionally evaluates only the restricted
literal cfg and module forms described above against the retained Cargo feature
contract. Rust compilation remains the authority for feature reachability and
executable semantics.

## Remaining Integration

Resolve the remaining exact source/display joins and coordinate the website
consumer before treating the inventory as exhaustive. Each pair must
bind both real sources, one functional/numerical contract and independent oracle,
then retain compile, verification/artifact, CPU simulation, generated-host and
target-matched KFD evidence. Until those bindings and gates exist, this report
must not be promoted to completed-pair or all-tutorial qualification.
