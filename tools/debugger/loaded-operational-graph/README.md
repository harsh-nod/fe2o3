# Loaded-input graph and resource-policy planning

These are pure, bounded CPU planning APIs for debugger input review. They do not
open the named files, invoke a filesystem provider, approve an input limit,
launch a debugger, or grant compiler/native/GPU authority.

- `compileOperationalGraph(Buffer)` validates a complete UTF-8 JSON graph,
  preserves each named path and all its roles, rejects conflicting pins or
  identities, and returns an immutable structural snapshot.
- `exactReadEnvelope(entries, aliases, passes)` computes logical content-byte,
  exact-chunk/EOF and metadata-provider call envelopes. Its inputs are structural
  data; the result is not an IO measurement or admission token.
- `proposeResourcePolicy(Buffer, options)` adds the fixed historical-profile
  phase accounting and explicitly unresolved supervisor, output, currentness,
  scheduling and memory obligations. It requires the exact historical 1,173-name
  shape; it is not a generic operational supervisor.

The graph schema is `fe2o3-loaded-operational-graph-input-v1`, with exactly
`schema`, `claims`, `aliases`, `imports`, `unresolved`, and `outputs`.
The tests provide small synthetic examples and refusal cases. Every readable
claim names an explicit whole-file pin; absence claims have null pins and remain
separate metadata obligations. Different names are not merged merely because
their hashes or physical targets match. An independently selected alias target
retains its own charge.

Unknown identity and ownership fields stay unknown. No accepted structural input,
including an empty unresolved list, sets `qualified`, `execution_authority`,
`complete_operational_graph`, or `root_cap_change_approved` to true.

Run the 44 fixture-free controls from the repository root:

```sh
node --test tools/debugger/loaded-operational-graph/graph-policy.test.mjs
```

These exact source bytes also passed root-owned historical integration
qualification on mi350: 13 additional controls using the separately supplied
76-role fixture manifest and an immutable historical seed. The host-specific
seed, inactive integration driver and raw execution receipts are not bundled in
this portable directory. The 44 default controls do not claim that historical
integration coverage, and no automatic fixture download or host-path fallback
is supplied.

The original qualified graph/policy receipt SHA-256 is
`3e3813b667276e7b02af4de1ecfc34a5bd4b2c04eb717991cea73e4ec83f2ba1`.
Independent source review and root pin/copy/import checks preceded execution.

## Limits and next integration

The proposed controlled-read subtotal excludes actual module-loader IO, fixture
metadata, resource probes, request reads and any extra repository scan. Provider
calls are not kernel syscall counts; typed/logical memory sizes are not process
RSS bounds. Output caps do not prove that every report fits.

The existing generic qualification runner does not implement the exact chunk,
EOF, aggregate-call, absence and individual source-selection contracts proposed
here. Increasing only its input count would not make it compatible. A separately
reviewed guard, read-only adapter and bounded exclusive evidence writer are still
required before operational reads. Historical identities must not be silently
refreshed when current files differ.

See the adjacent [loaded profile](../loaded-profile/README.md) and
[bounded reader](../loaded-input-reader/README.md). This planning component does
not establish live register/LDS capture or close a broad milestone.
