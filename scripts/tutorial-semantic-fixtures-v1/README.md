# Semantic Fixture Metadata

These 47 records describe bounded simulation inputs, CPU-reference expectations,
and production-export metadata. They are not proof or hardware qualification.

The checkpoint 35 live probe rebuilt the transaction exporter and companion
CLI in an isolated cache, then invoked `--phase prepare` for every fixture.
All 47 freshly observed records are blocked at `FE2O3-TUTORIAL-TXN-007`:
the local protected compiler-execution client profile cannot be admitted.
Artifact coordinates remain null. Earlier retained exporter-build diagnostics
have been replaced; these records still do not report successful compilation,
simulation, GPU execution, or qualification.

`compilerInputContractSha256`, `sourceClosureSha256`, and the derived
`productionExport.diagnosticIdentitySha256` were regenerated with the existing
canonical helpers. The diagnostic binding checks record consistency; it does
not establish observation freshness or authenticate a new compiler result.

`generate-tutorial-semantic-fixtures.py` runs a live production probe, including
when invoked without `--write`. Passing retained records to its `records` API
supports offline metadata regeneration, not diagnostic reproduction. A fresh
observation requires a new probe. Successful production preparation additionally
requires the protected runtime and all compiler checks. Preparation does not
complete the separate simulator, negative-fixture, or hardware gates.
