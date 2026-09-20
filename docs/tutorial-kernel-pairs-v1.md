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
  source-driver cases. Each ID has SIMT and tile variant records with a blocker
  owner, issue and reason; the current checked-in records remain pending.
  Matching names, source paths or historical displayed
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
contracts: serial-router, held-fragments, interleaved-stores, and the three
materialized components, plus pipelined-attention and scalar-attention. Their
existing source/feature identities remain distinct, including the repeated
megakernel symbol. This leaves 56 pending display bindings and the historical
GEMM lesson's source gap unresolved. The two attention variants are required
positive compile obligations with separate pending-design simulation requests;
neither inherits another variant's retained KIR or execution evidence. These
registrations bring known kernel identities to 60, not completed pairs or a
proven final curriculum denominator.
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

### Source-Bound Variants

A variant may change from `status: "pending", source: null` to
`status: "source-bound"` with this source object (digest values are placeholders):

```json
{
  "implementationKernelId": "registered-implementation-id",
  "selection": {
    "kind": "fixture",
    "fixtureId": "registered-fixture-id",
    "kernelSymbol": "selected_kernel"
  },
  "selectionSha256": "<64 lowercase hexadecimal digits>",
  "sourcePath": "examples/package/src/kernel.rs",
  "sourceSha256": "<64 lowercase hexadecimal digits>",
  "functionUtf8Offset": 123
}
```

The variant retains `kind` and its blocker owner, issue and reason. `selection`
may instead use an existing `source-driver-case` reference with its exact
`lessonId`, `tabOrdinal` and `caseOrdinal`. It must belong to the declared
implementation ID. That ID can differ from the obligation's `kernelId`, allowing
different source/feature implementations without merging their identities.
The report exposes each registered identity's `selectionSha256`, covering its
package/lock/source-closure digests, Cargo target, features and kernel symbol.
The physical file digest and function-name UTF-8 byte offset must also match the
actual selected source. Both reference kinds use the restricted physical
Cargo/cfg/module validation described above; unsupported syntax stays pending.

This is a declared source association, not verification of SIMT/tile mode,
semantic equivalence or execution. Even two associations to the same source
provide none of those guarantees. `sourceBoundVariantCount` and
`sourceBoundPairCount` count authenticated associations; `variantBindingStatus`
is `pending`, `partial` or `source-bound`. These counts do not establish the final
curriculum denominator or qualify pairs. Complete source binding removes only
`per-kernel-variant-sources` from the report's missing bindings. Numerical
contracts and independent oracles, verified artifacts, target-matched execution
and other production evidence remain required; `per-variant-target-evidence`
remains, stages are `not-evaluated`, and `qualified` stays false with zero
qualified pairs. The separate runtime census and source/display join rules for
`requiredPairCount` remain unchanged.

## Remaining Integration

Resolve the remaining exact source/display joins and coordinate the website
consumer before treating the inventory as exhaustive. Each pair must
bind both real sources, one functional/numerical contract and independent oracle,
then retain compile, verification/artifact, CPU simulation, generated-host and
target-matched KFD evidence. Until those bindings and gates exist, this report
must not be promoted to completed-pair or all-tutorial qualification.
