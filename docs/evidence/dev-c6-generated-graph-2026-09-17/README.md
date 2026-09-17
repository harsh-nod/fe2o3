# Generated Graph Development

CPU-qualified development only. This packet does not advance R125,
R118B C1-C3 or R116/V3, or close A1/A2, #182, protected native composition,
formal correspondence or HIP/HSA parity.

See the [C6 contract](../../runtime-generated-graph-v1.md). The generated graph
uses the existing graph executor and operation registry.
Dependencies wait for the original completion receipt after settlement and
decoding. This orders frozen invocations; it does not rebind successor DATA.

## Frozen Source

The 23 source files are frozen above canonical C5 commit
`691a4bdff8b8802d6e72f4a76a8e79b4cf1163c7`. The complete patch has SHA-256
`b94e24ac22122160ee216ccee484e049107a0c39bf519cfbae738665f129e596`;
the source manifest has SHA-256
`bddb7ceeec2722f1521dbed4e3b245f710d4b0beefc7e88210385a768eb3ed66`.
The auditor checks exact changed-source membership, source hashes, compiled
executable hashes, complete matching test rosters and serial command chronology.
The source identities apply to canonical integration; archived paths identify
the actual qualification worktree, not fresh executions in another worktree.

The scoped musl build sets `FE2O3_HIP_SYS_DISABLE=1` while retaining Cargo's
all-features configuration. This disables optional legacy native HIP linkage,
not direct KFD. It does not qualify an unrestricted musl/HIP build.

## Qualification

Frozen-source GNU and scoped musl each pass 934 runtime and 271 host tests, with
seventeen and four ignored respectively. The complete test rosters match across
targets, and source and executable hashes match before and after execution.
All 61 doctests, strict all-targets Clippy, formatting, no-default-feature checks
and unsafe-source policy pass. Parser calibration rejects eighteen malformed
transcripts. The final audit checks closed records, exact commands and ordering.

Nineteen new focused tests and a strengthened direct typed-drain regression
exercise shared production control flow with scripted backends and constructed
Context state. This is CPU composition evidence, not protected native graph
execution. The full phase-controlled owned Stop matrix, production journals,
aggregate residency bounds, formal correspondence and matched performance remain
open. Accepted milestones are unchanged.

## Development History

Exploratory tests exposed an absent terminal-entry guard in registry progress:
after graph Stop, a manually invoked progress step could enter the adopted
driver again. The owner loop already stopped before that call. Registry progress
and the generated driver now independently reject terminal/stopped advancement;
the graph Stop regression asserts no additional hook entry. That initial test
also asserted while holding a fixture mutex, causing a destructor abort on
assertion failure. Its assertion now observes a copied trace outside the lock.
Exploratory outcomes are not frozen-source qualification evidence.

The first expanded mixed-operation test also hit the mock backend's historical
flush roster after ordinary retirement removed its dependency entry. The test
now uses the existing graph fixtures' explicit mock-completion steps, preserving
the actual Context issue/poll/release path. Its failed development record remains
in `raw/exploratory-graph-expanded.*` and is excluded from qualification.

No MI300X workload or performance comparison is run by this packet.
