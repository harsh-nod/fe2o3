// SPDX-License-Identifier: GPL-3.0-or-later
// Opt-in, read-only selected-source verification. Does not apply patches or build.
import {readSourceStage,COMMIT} from './tests/source-files.mjs';
if(process.argv.length!==4)throw Error('usage: node verify-source.mjs <upstream|native|mi-safe-points|lifecycle> <absolute-checkout>');
const value=readSourceStage(process.argv[3],process.argv[2]);
console.log(JSON.stringify({
 schema:'fe2o3-rocgdb-selected-source-check-v1',expected_upstream_commit:COMMIT,
 stage:value.stage,files:Object.keys(value.files).length,bytes:value.bytes,
 complete_checkout_verified:false,built_debugger_verified:false,native_authority:false,
}));
