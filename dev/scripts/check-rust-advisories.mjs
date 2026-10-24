/** 公開registryの名前と版だけをOSVへ照合し、保守終了と脆弱性を分ける。 */
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const informational=new Map([
 ['RUSTSEC-2025-0081','unic-char-property'],['RUSTSEC-2025-0075','unic-char-range'],
 ['RUSTSEC-2025-0080','unic-common'],['RUSTSEC-2025-0100','unic-ucd-ident'],['RUSTSEC-2025-0098','unic-ucd-version'],
]);
async function read(url,options){
 const response=await fetch(url,{...options,signal:AbortSignal.timeout(30000)});
 if(!response.ok)throw new Error('OSV照合に失敗しました');return response.json();
}
try{
 const packages=JSON.parse(fs.readFileSync(path.join(root,'licenses/third-party/manifest.json'),'utf8')).rust;
 if(!Array.isArray(packages) || packages.some(p=>!/^[A-Za-z0-9_-]+$/.test(p.name) || !/^[A-Za-z0-9.+-]+$/.test(p.version) || !p.source.startsWith('https://static.crates.io/')))throw new Error('公開registryの一覧ではありません');
 const result=await read('https://api.osv.dev/v1/querybatch',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({queries:packages.map(p=>({package:{ecosystem:'crates.io',name:p.name},version:p.version}))})});
 if(!Array.isArray(result.results) || result.results.length!==packages.length)throw new Error('OSVの照合件数が一致しません');
 let maintenance=0,failures=0;
 for(let i=0;i<packages.length;i++)for(const finding of result.results[i].vulns??[]){
  const p=packages[i];let accepted=false;
  if(informational.get(finding.id)===p.name && p.version==='0.9.0'){
   const details=await read('https://api.osv.dev/v1/vulns/'+encodeURIComponent(finding.id));
   accepted=details.affected.some(item=>item.package.name===p.name && item.database_specific?.informational==='unmaintained');
  }
  if(accepted){maintenance++;console.log(`保守終了通知: ${p.name} ${p.version} ${finding.id}（Tauriの間接依存、修正版なし）`);}
  else{failures++;console.error(`要対応: ${p.name} ${p.version} ${finding.id}`);}
 }
 console.log(`${failures?'失敗':'成功'}: Rust公開依存 ${packages.length}件 / 未解消の脆弱性・不健全性 ${failures}件 / 保守終了通知 ${maintenance}件`);
 process.exitCode=failures?1:0;
}catch{console.error('Rust依存のオンライン照合を完了できません。公開registry一覧と接続を確認してください。');process.exitCode=2;}
