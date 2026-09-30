# Merged Runtime CPU Regression

Source commit: `2699fe63f1d03c60e1649a2a5a29c6b2481f9d3b`.
This is an owned development command, not an independently authenticated proof
or native qualification campaign. No GPU or remote scratch was used.

```sh
cargo test --offline --locked -p fe2o3-runtime --all-features --lib -- --test-threads=1
```

Result: **1,893 passed, zero failed, 32 ignored, zero filtered**. All 1,925
library cases reached the complete test footer. The ignored cases are the
existing native/manual tests; their names and reasons remain in the raw log.
The three earlier `InspectSocket/EPERM` telemetry failures pass in this
unrestricted environment without changing checks, ignores or expectations.

The full library run took 93.63 seconds. Cargo recompiled the four changed
runtime/model/accounting/KFD crates in 5 minutes 18 seconds using an existing
target cache; this was not a clean-cache build. The profile uses optimization
level 1, no debug information, and disabled incremental compilation. The exact
environment, arguments, selected source inventory and compiler tool hashes are
in `inputs.json`. The ELF SHA-256 at completion was
`f478da4a5777bdd6426d216b8dda621b6ebd4b5caf9a6c58f5fc3862a4c0720e`.

Source/tool/namespace continuity checks pass. The command owner reports its
fresh process group absent after completion; no historical or host-wide process
absence is asserted. The development recorder and unchanged process owner are
included with their raw logs, input/output inventories and terminal receipt in
`raw.tar.xz`. The executable, mutable target cache and full source checkout are
not included. This archive is not a standalone reproducible build package.
Archive readback matches every retained file. Its SHA-256 is
`c921c6547bb96790d61dbf321740659896b30d6cad38634236899dd2caed0440`.

| Receipt | SHA-256 |
| --- | --- |
| `result.json` | `5954079b8cb8f2db13fd8b33cabc32e776f64f2089060bf5411e0c29b8aef9f6` |
| `inputs.json` | `41cef182199a3f83cbeb02792181a40dae32f075cdf63f9a29f4e11f5b2a76bf` |
| `owned/stdout.log` | `6f97befe30bf97065b09a553ffc5d95b64ae3e1e2c0edd6e63cf708431c5533c` |
| `owned/stderr.log` | `0288683a98e653041e39cd4eca4b3b76a1cd75a084074a3b4ad2f603fee4583c` |

Older restricted-environment failures remain rejected historical runs. This new
passing runtime-library regression does not qualify the whole workspace, the
ignored native tests, later unmerged candidates, production protected execution,
HIP/HSA parity, a performance improvement or any A0-A7 milestone exit.
