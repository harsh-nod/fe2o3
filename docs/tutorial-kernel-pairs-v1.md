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

The JSON schema is `fe2o3-tutorial-kernel-pair-obligations-v1`. This is a derived
diagnostic view of `config/tutorial-kernel-manifest-v1.json`, not another stored
inventory, execution receipt, release gate or source of compiler authority.
The existing V2 curriculum remains unchanged. The command validates all source
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
Consequently `requiredPairCount` is null, `qualifiedPairCount` is zero,
`inventoryComplete` and `qualified` are false, variant bindings are pending,
and execution stages are not evaluated. Existing historical successes do not
acquire new source/variant/target bindings from this report.

The report is bounded to 4096 combined binding/display records, 4096 lexical
declarations and 16 MiB of encoded JSON. Exhaustion rejects without partial JSON.
The tests run in the existing `scripts/tests/kernel-compile-matrix.sh` CI gate.

## Remaining Integration

A persistent, exhaustive kernel-pair registry needs an explicitly versioned
curriculum extension with coordinated compiler/site consumers. Each pair must
bind both real sources, one functional/numerical contract and independent oracle,
then retain compile, verification/artifact, CPU simulation, generated-host and
target-matched KFD evidence. Until those bindings and gates exist, this report
must not be promoted to completed-pair or all-tutorial qualification.
