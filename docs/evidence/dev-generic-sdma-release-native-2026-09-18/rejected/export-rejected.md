# Rejected Local Export

The first local `prepare.py` invocation on 2026-09-18 exited 1 after copying the
example but before writing the signature, binding or payload manifest. The
exporter's chmod still named the old `runtime-tests` binary and raised
`FileNotFoundError`. No SSH or native operation occurred. The incomplete directory
was renamed to `fe2o3-single-sdma-native-bundle-20260918-rejected-incomplete-export`.

The corrected exporter changes only that chmod target to `queue-example` and
must generate a fresh complete bundle. The rejected directory is not a valid
payload and must not be uploaded.
