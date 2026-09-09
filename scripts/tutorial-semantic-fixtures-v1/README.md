# Semantic Fixture Metadata

These 47 records describe bounded simulation inputs, CPU-reference expectations,
and production-export metadata. They are not proof or hardware qualification.

The input-only refresh and Rust component-order correction updated 37 distinct
fixture records (30 initially, then 23 with overlap). Their `productionExport`
diagnostics remain historical: diagnostic text, code, stage, `blocked` status,
and null artifact coordinates were retained, not freshly reproduced against the
refreshed inputs or current compiler. All 47 export records remain blocked.

`compilerInputContractSha256`, `sourceClosureSha256`, and the derived
`productionExport.diagnosticIdentitySha256` were regenerated with the existing
canonical helpers. The diagnostic binding checks record consistency; it does
not establish observation freshness or authenticate a new compiler result.

`generate-tutorial-semantic-fixtures.py` runs a live production probe, including
when invoked without `--write`. Passing retained records to its `records` API
supports offline metadata regeneration, not diagnostic reproduction. A fresh
observation requires a new probe with the required production runtime available.
