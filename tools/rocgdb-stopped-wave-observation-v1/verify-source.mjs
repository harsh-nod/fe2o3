// SPDX-License-Identifier: GPL-3.0-or-later
import {readSourceStage,COMMIT} from './tests/source-files.mjs';
if(process.argv.length!==4)throw Error('usage: node verify-source.mjs <lifecycle|stopped-wave> <absolute-checkout>');
const v=readSourceStage(process.argv[3],process.argv[2]);
console.log(JSON.stringify({schema:'fe2o3-rocgdb-stopped-wave-selected-source-check-v1',
 expected_upstream_commit:COMMIT,stage:v.stage,files:Object.keys(v.files).length,bytes:v.bytes,
 complete_checkout_verified:false,built_debugger_verified:false,native_authority:false}));
