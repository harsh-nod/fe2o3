# Publication And Integration Notes

The original public packet is copied without changing any file listed in
`summary.json`. Root independently checked all 1,240 archive members against
their retained originals, including the entire 1,218-file accepted campaign,
both matching independent readbacks, the three closed bundle command records,
the original CPU artifact pair, and both signed source commits. A fresh Git
bundle verification also passes. No archived Python code was executed by this
publication audit and no solver, Rust test or GPU workload was rerun.

The root audit and receipt are adjacent files named
`fe2o3-a2-publication-root-audit-20261001.py` and `.json`; the receipt SHA-256 is
`09f384ba61b348330f655598a18f683a34600b8c8b46b27e8cb2861d09703c66`.
They and this note are integration additions, outside the original packet's
file inventory. The audit retains its original local paths and is not a
self-contained portable runner.

Two earlier packaging failures remain preserved locally and are not proof
failures or accepted publication runs. The first preparation used the wrong
keyword for an inherited read-only Git helper and wrote neither a preparation
manifest nor an archive. Its exact original source was reconstructed, with
that provenance recorded, and checked against its previously recorded hash.
The next preparation passed, but the first bundle command rejected a raw SHA
argument with `Refusing to create empty bundle`; its owned process group closed
and no bundle/archive was produced. The second publisher uses an authenticated
detached `HEAD`, tested with an actual temporary two-commit Git bundle before
the successful fresh publication. All original records remain unchanged.

Signed integration `6c0718c1d` and CI/evidence commit `3e016bd77` are published
on `codex/r65-runtime-drain-versions` in both repositories. The separate
[integration packet](../dev-producer-composition-integration-2026-10-01/README.md)
records the four inventory-only guard changes and 21 passing CI commands.
Concrete journal/live/credit refinement and all A0-A7 exits remain open.
