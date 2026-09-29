# Locked Publication Fixture

`locked-publication-v5.bin` is inert V5 handoff framing for real artifact lease,
token, and Linux OFD-lock tests. Its source/history leaves are synthetic. It is
not proof, protected compiler execution, source equivalence, or GPU evidence.

- Length: 3241 bytes.
- SHA-256: `643fd1b1d6fe886f6a76a44272748fc6a6b6f6473df17a163e20251e1dc3ef1f`.
- Generator: `compiler_module_handoff_v5_fixture_tests.rs::outer()` in
  `fe2o3-artifact-transaction`, source commit `42757001d`.
- Generated with nightly `2026-04-03` by writing that owner's
  `canonical_bytes()`. The temporary export-only test was removed afterward.

The broker tests decode these bytes with the actual V5 decoder, publish them
through the artifact transaction, and acquire the actual lease/token pair.
They never construct a synthetic native compiler observation or bypass its
production constructor. The binary avoids copying the existing fixture builder
and its unrelated compiler-lineage dependencies into this crate.
