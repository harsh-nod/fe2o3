# fe2o3-verifier

`fe2o3-verifier` defines the bounded records at the verifier-driver boundary. It
normalizes proof configuration, requested properties, trusted items, measured
tool identities, canonical request bytes, and process arguments. It also parses
a strict result envelope emitted by a future evidence recorder.

The crate does not invoke a shell. `CommandSpec` keeps the program and each
argument separate, and the bounded executor launches only the planned recorder
with an empty environment, null stdin, and fixed working directory. Tests use a
fixture recorder and do not require Verus or a solver installation.

## Trust boundary

- Tool identities and SHA-256 digests are explicit caller inputs. This crate
  validates their shape and policy match; it does not measure or authenticate
  binaries.
- A `Proved` result is evidence, not authority to load or launch a kernel. The
  artifact finalizer must reconstruct and match target, configuration, model,
  invocation, tool, property, and trusted-item identities.
- The parser accepts the recorder envelope, not unstructured Verus output. A
  reviewed recorder must translate Verus and solver outcomes, inventory trusted
  escapes, and emit the envelope only after both tools terminate. The caller
  must also supply the recorder's process termination; only exit code zero can
  produce a parsed result.
- Correlation IDs prevent accidental result mixups. They are not signatures or
  content hashes. Integration must hash the canonical invocation using the
  artifact format's domain-separated SHA-256 scheme.

## Current limitations

There is no reviewed Verus adapter, binary measurement implementation,
proof-record conversion, signature verification, or GPU runtime integration.
Tool identities remain caller-supplied. A timeout kills and reaps the direct
recorder child, but does not yet establish a process group or forcibly terminate
arbitrary descendants. The v1 result model deliberately mirrors the existing
artifact outcomes and properties, but compatibility remains conceptual until
an explicit conversion is implemented and reviewed.
