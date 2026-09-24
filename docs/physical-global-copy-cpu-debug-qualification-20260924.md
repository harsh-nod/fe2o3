# Physical global-copy CPU debugger qualification — 2026-09-24

The public `fe2o3-debug sim --diagnostic-kir-v21` route passed actual CLI
qualification on mi350 using the ordinary source-produced V21 exports. This
is deterministic CPU observation, not GPU execution or physical-register
capture. The separate [source endpoint](physical-global-copy-source-v21.md)
and [inert continuation](physical-global-copy-inert-handoff-v21.md) retain
their own admission and evidence boundaries.

## Observed matrix

Four primary sessions cover the original and register-edited source variants
at 64 and 128 invocations. Each uses initialized input and output views with
eight-byte offsets and guard bytes. The independent byte oracle checks copied
words, unchanged input, both output guards and the unwritten tail. Two further
128-invocation sessions use output lengths zero and 33; all launched input
lanes still execute the full-EXEC load.

The actual CLI observations include the same SSA binding before and after its
VM wait: pending is unavailable, then the completed load exposes scalar bits.
Both allocations, current-capture identity, pagination, revision-bound
navigation and reverse-selected prior memory snapshots are checked. Symbolic
address halves remain nonnumeric; no source or hardware state is invented.

Each of the six sessions passes 14 transactional refusal controls. Six separate
children check the shared parser's zero-length-memory refusal: an exact
correlationless terminal error, unchanged prior observation, exit status 1 and
no continuation attempt. A malformed request rejected before correlation is
not treated as a normal correlated query error.

Eight bootstrap cases refuse invalid input bounds, initialization, access,
same-backing aliases, forged source authority or the wrong canonical version.
A cumulative-budget session accepts 401 queries before its expected terminal
refusal, with the last accepted cursor and revision unchanged. Four complete
older V20 sessions replay with byte-identical response streams.

The matrix used 31 child processes: six location inspections, six primary/mask
sessions, six terminal-parser controls, eight bootstrap controls, one budget
session and four V20 compatibility sessions. It passed 24 pure harness controls
before launching them. Direct-child exits and streams were observed; this is
not a whole-descendant-family containment claim.

## Retained evidence

All paths below are relative to the retained mi350 task root
`fe2o3-authoring-280-282-mi350.4VZ42zNr`.

| Evidence | Bytes | SHA-256 |
| --- | ---: | --- |
| `phase28-global-copy-cpu-debug-cli-source-r2/report.json` | 28344 | `c75e81d345805893adeb3b6f1842b882ad4de257504f7a92fb5653a00cb62c59` |
| `logs/phase28-resume-r11-compiler-global-copy-cpu-cli-source-v21-r2/receipt.json` | 71926 | `ab3b199347b601ae81480eb3c66a575104817571a1820bd187a4c4e1a956bf20` |
| Qualified `fe2o3-debug` binary | 63543472 | `3d8c6d4061ae920a52e53efdc54d7b144b3bfceffc9af38aea84b8c13c7c9619` |

The earlier `source-v21-r1` run remains a failed qualification. Its harness
incorrectly expected a correlated nonterminal response for a request rejected
by the shared parser before correlation. The R4 harness correction preserved
the production parser and strict normal-response checks, and used fresh
`source-r2` output. The failed run was not relabeled or overwritten.

The integrated compiler regression also passed 8,667 Rust test executions
across 228 reported suites, with zero failures and 216 ignored executions.
Those counts include repeated selections and are not a unique-test inventory.
Bridge and console pure suites passed 114 and 37 tests respectively. This does
not claim a blanket warning-free workspace or hardware acceptance.

## Remaining boundaries

This route has no authenticated source map, physical register capture, GPU
stepping, persistent replay or resumable execution. Detached canonical bytes
cannot reconstruct the source owner's custody or discharge host ABI/memory
conditions. The CPU debugger does not complete the assembly memory,
hardware-debugging or tutorial milestones by itself.
