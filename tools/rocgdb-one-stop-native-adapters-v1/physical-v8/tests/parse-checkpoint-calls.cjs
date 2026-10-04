// SPDX-License-Identifier: GPL-3.0-or-later
function parseCheckpointCalls(text) {
 const calls=[]; const re=/\brequire\s*\(/g; let m;
 while((m=re.exec(text))) {
   let p=text.indexOf('(',m.index),i=p+1,depth=1,quote=null,escape=false,line=false,block=false;
   const commas=[];
   for(;i<text.length&&depth;i++){
    const a=text[i],b=text[i+1];
    if(line){if(a==='\n')line=false;continue;}
    if(block){if(a==='*'&&b==='/'){block=false;i++;}continue;}
    if(quote){if(escape)escape=false;else if(a==='\\')escape=true;else if(a===quote)quote=null;continue;}
    if(a==='/'&&b==='/'){line=true;i++;continue;}if(a==='/'&&b==='*'){block=true;i++;continue;}
    if(a==='"'||a==="'"){quote=a;continue;}
    if(a==='(')depth++;else if(a===')')depth--;else if(a===','&&depth===1)commas.push(i);
   }
   const end=i, inner=text.slice(p+1,end-1);
   if(commas.length!==1||text.slice(commas[0]+1,end-1).trim()!=='failure::checkpoint_changed')continue;
   calls.push({start:m.index,end,condition:text.slice(p+1,commas[0]),before:text.slice(m.index,end),line:text.slice(0,m.index).split('\n').length});
   re.lastIndex=end;
 }
 return calls;
}
module.exports={parseCheckpointCalls};
