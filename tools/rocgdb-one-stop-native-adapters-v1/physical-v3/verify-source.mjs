// SPDX-License-Identifier: GPL-3.0-or-later
import {readSourceStage,MAX_TOTAL} from './tests/source-files.mjs';
if(process.argv.length!==4)throw Error('usage: node verify-source.mjs <absolute-source-root> <physical-publication-disabled-v2|physical-host-entry-maintenance-disabled-v3>');
const r=readSourceStage(process.argv[2],process.argv[3]);
console.log(JSON.stringify({schema:'fe2o3-rocgdb-one-stop-physical-v3-source-check-v1',stage:r.stage,selected_files:Object.keys(r.files).length,selected_bytes:r.bytes,combined_source_cap:MAX_TOTAL,activation_available:false,capture_available:false,publication_available:false,complete_checkout_verified:false,built_debugger_verified:false,native_authority:false}));
