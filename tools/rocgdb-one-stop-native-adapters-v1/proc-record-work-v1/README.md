# Disabled proc-record logical-work candidate

This source-only candidate follows physical-v11. It changes the work debit in one native owner helper from requested record capacity to the numerically admitted returned extent. It is explicitly a changed logical accounting convention, not measured CPU work. The full1024-byte requested-I/O debit/read, parser and identity guards,131072 work cap, and sticky first-denial note remain. No activation/capture/publication gate changes.

The R73 native attempt failed at work131029+128>131072 in the phase10 diagnostic flush. This candidate is not a proven native fix; old W4 products and consumed authority remain unchanged. See DIAGNOSIS.md for the exact failure, cleanup limits and terminal-schedule limitation.

This folder is not a fully staged physical-v12 package and does not replace physical-v11 selected-source pins. The only proposed production replacement is postimages/gdb/amd-dbgapi-one-stop-native-owner-v1.inc, authenticated against its preimage by EDITS.json. A later complete selected-source package/private build must explicitly adopt it; no native build or lease is supplied here.

CPU-only commands (choose fresh output paths):

```sh
node --test tests/source.test.mjs
g++ -std=c++17 -Wall -Wextra -Werror -pedantic tests/proc-work.cc -o FRESH/proc-work
FRESH/proc-work
g++ -std=c++17 -Wall -Wextra -Werror -pedantic -fsanitize=undefined -fno-sanitize-recover=all tests/proc-work.cc -o FRESH/proc-work-ubsan
FRESH/proc-work-ubsan
```

The C++ fixture uses exact old/new parser fragments, the unchanged real owner ledger, mocked IO, and tracked buffer accesses. It performs no procfs/native/GPU operation. Source controls prove exact inverses and unchanged surrounding code. Results are root-owned and must be recorded separately; author execution is false.
