// SPDX-License-Identifier: GPL-3.0-or-later
import {readApiHeader} from './tests/source-files.mjs';
if(process.argv.length!==3)throw Error('usage: node verify-api-header.mjs <absolute-amd-dbgapi-header>');
const pin=readApiHeader(process.argv[2]);
console.log(JSON.stringify({schema:'fe2o3-rocgdb-stopped-wave-api-header-check-v1',
 pin,external_header_only:true,native_library_verified:false,native_authority:false}));
