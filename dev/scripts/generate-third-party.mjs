/** Windows配布構成のライセンスと対応ソースを、lockと取得済み依存から生成する。 */
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const destination=path.join(root,'licenses/third-party');
const args=process.argv.slice(2);
if(args.some(arg=>arg!=='--check'))throw new Error('使い方: generate-third-party.mjs [--check]');
const check=args.includes('--check'),files=new Map();
const origins=JSON.parse(fs.readFileSync(path.join(root,'licenses/upstream/origins.json'),'utf8'));
const usedOrigins={};
const hash=data=>crypto.createHash('sha256').update(data).digest('hex');
const lockHash=name=>hash(Buffer.from(fs.readFileSync(path.join(root,name),'utf8').replaceAll('\r\n','\n')));
const compare=(a,b)=>a<b?-1:a>b?1:0;
const safe=(base,relative)=>{
 const result=path.resolve(base,relative);
 if(!result.startsWith(path.resolve(base)+path.sep))throw new Error('出力先が許可範囲外です');
 return result;
};
function runCargo(){
 const result=spawnSync('cargo',['metadata','--manifest-path','src-tauri/Cargo.toml','--locked','--offline','--format-version','1','--filter-platform','x86_64-pc-windows-msvc'],{cwd:root,encoding:'utf8',maxBuffer:32*1024*1024});
 if(result.error || result.status!==0)throw new Error('cargo metadataに失敗しました。Windows対象のcargo fetchを先に実行してください。');
 return JSON.parse(result.stdout);
}
function texts(directory,id){
 const entries=fs.readdirSync(directory,{withFileTypes:true});
 const names=entries.filter(entry=>/^(?:licen[sc]e|copying|notice|unlicense|copyright|authors)(?:[._-].*)?$/i.test(entry.name) && entry.isFile()).map(entry=>entry.name);
 for(const entry of entries.filter(entry=>entry.isDirectory() && /^licenses?$/i.test(entry.name))){
  for(const name of fs.readdirSync(path.join(directory,entry.name)))if(fs.lstatSync(path.join(directory,entry.name,name)).isFile())names.push(entry.name+'/'+name);
 }
 if(names.length===0){
  const origin=origins.packages[id];
  const matches=origin && (id.startsWith('npm-')
   ? JSON.parse(fs.readFileSync(path.join(directory,'package.json'),'utf8')).version===origin.npmVersion
   : JSON.parse(fs.readFileSync(path.join(directory,'.cargo_vcs_info.json'),'utf8')).git.sha1===origin.revision);
  if(!matches)throw new Error('上流のライセンス本文が見つかりません: '+id);
  usedOrigins[id]=origin;
  return origin.files.map(file=>{
   const data=fs.readFileSync(safe(path.join(root,'licenses/upstream'),file.file));
   if(hash(data)!==file.sha256)throw new Error('上流ライセンスのhash不一致: '+id);
   const relative=`texts/${id}/${path.basename(file.file)}`;files.set(relative,data);return relative;
  });
 }
 const result=[];
 for(const name of names.sort()){
  const relative=`texts/${id}/${name}`;
  files.set(relative,fs.readFileSync(path.join(directory,name)));result.push(relative);
 }
 return result;
}
function selectedLicense(expression){
 if(!expression)throw new Error('ライセンス未定義の依存があります');
 if(expression.includes(' OR ') && expression.includes('MIT'))return 'MIT';
 if(expression.includes(' OR ') && expression.includes('Apache-2.0'))return 'Apache-2.0';
 if(/\b(?:A?GPL|LGPL)-/.test(expression))throw new Error('標準配布構成へGPL/LGPL依存が入りました。組込み方と配布条件を確認してください。');
 return expression;
}
const metadata=runCargo(),packages=new Map(metadata.packages.map(p=>[p.id,p])),nodes=new Map(metadata.resolve.nodes.map(n=>[n.id,n]));
const pending=[metadata.resolve.root],seen=new Set();
while(pending.length){
 const id=pending.pop();if(seen.has(id))continue;seen.add(id);
 for(const dep of nodes.get(id)?.deps??[])if(dep.dep_kinds.some(kind=>kind.kind!=='dev'))pending.push(dep.pkg);
}
const lockText=fs.readFileSync(path.join(root,'src-tauri/Cargo.lock'),'utf8');
const lockPackages=new Map(lockText.split('[[package]]').slice(1).map(block=>{
 const value=name=>block.match(new RegExp(`^${name} = "([^"]+)"`,'m'))?.[1];
 return [`${value('name')}@${value('version')}`,value('checksum')];
}));
const rust=[];
for(const p of [...seen].map(id=>packages.get(id)).filter(p=>p.source).sort((a,b)=>compare(a.name+'@'+a.version,b.name+'@'+b.version))){
 if(!p.source.startsWith('registry+'))throw new Error('公開registry以外の依存は手動確認が必要です: '+p.name);
 const id=`rust-${p.name}-${p.version}`,directory=path.dirname(p.manifest_path),license=selectedLicense(p.license);
 const entry={name:p.name,version:p.version,license:p.license,selectedLicense:license,repository:p.repository,
  source:`https://static.crates.io/crates/${p.name}/${p.name}-${p.version}.crate`,checksum:lockPackages.get(`${p.name}@${p.version}`),texts:texts(directory,id)};
 if(!entry.checksum)throw new Error('Cargo.lockのchecksumがありません: '+p.name);
 if(/MPL-/.test(license)){
  const cache=path.join(path.resolve(directory,'../../..'),'cache',path.basename(path.dirname(directory)),`${p.name}-${p.version}.crate`);
  const source=fs.readFileSync(cache);
  if(hash(source)!==entry.checksum)throw new Error('対応ソースのchecksum不一致: '+p.name);
  entry.bundledSource=`sources/${p.name}-${p.version}.crate`;files.set(entry.bundledSource,source);
 }
 rust.push(entry);
}
const npmLock=JSON.parse(fs.readFileSync(path.join(root,'package-lock.json'),'utf8'));
const npm=[],visited=new Set(),npmPending=Object.keys(npmLock.packages[''].dependencies??{}).map(name=>'node_modules/'+name);
while(npmPending.length){
 const relative=npmPending.pop();if(visited.has(relative))continue;visited.add(relative);
 const info=npmLock.packages[relative];if(!info)throw new Error('npm lockに依存がありません: '+relative);
 const directory=path.join(root,relative),p=JSON.parse(fs.readFileSync(path.join(directory,'package.json'),'utf8'));
 if(p.version!==info.version)throw new Error('導入済みnpm依存とlockが一致しません: '+p.name);
 npm.push({name:p.name,version:p.version,license:p.license,selectedLicense:selectedLicense(p.license),source:info.resolved,integrity:info.integrity,texts:texts(directory,`npm-${p.name.replaceAll('/','+')}-${p.version}`)});
 for(const name of Object.keys(info.dependencies??{})){
  let parent=relative,found;
  while(parent){const candidate=parent+'/node_modules/'+name;if(npmLock.packages[candidate]){found=candidate;break;}const i=parent.lastIndexOf('/node_modules/');parent=i<0?'':parent.slice(0,i);}
  found??='node_modules/'+name;npmPending.push(found);
 }
}
files.set('texts/Kenney-Interface-Sounds-1.0/LICENSE.txt',fs.readFileSync(path.join(root,'public/sounds/Kenney-LICENSE.txt')));
const sdkDirectory=path.join(root,'licenses/upstream/Microsoft-WebView2Loader-1.0.3650.58');
const sdk=JSON.parse(fs.readFileSync(path.join(sdkDirectory,'origin.json'),'utf8'));
const webview=metadata.packages.find(p=>seen.has(p.id) && p.name==='webview2-com-sys');
if(!webview || hash(fs.readFileSync(path.join(path.dirname(webview.manifest_path),'x64/WebView2LoaderStatic.lib')))!==sdk.staticLibrarySha256)throw new Error('WebView2 SDKの版と配布条件を再確認してください');
const sdkTexts=sdk.texts.map(entry=>{
 const data=fs.readFileSync(path.join(sdkDirectory,entry.file));if(hash(data)!==entry.sha256)throw new Error('SDKライセンス本文が一致しません');
 const name=`texts/Microsoft-WebView2Loader-${sdk.version}/${entry.file}`;files.set(name,data);return name;
});
const manifest={schemaVersion:1,target:'x86_64-pc-windows-msvc',includesBuildDependencies:true,upstreamOrigins:usedOrigins,
 cargoLockSha256:lockHash('src-tauri/Cargo.lock'),npmLockSha256:lockHash('package-lock.json'),
 rust,npm:npm.sort((a,b)=>compare(a.name,b.name)),assets:[{name:'Kenney Interface Sounds',version:'1.0',license:'CC0-1.0',text:'texts/Kenney-Interface-Sounds-1.0/LICENSE.txt'},{name:'Microsoft WebView2Loader',version:sdk.version,license:'BSD-3-Clause',source:sdk.source,staticLibrarySha256:sdk.staticLibrarySha256,texts:sdkTexts}],
 files:[...files].sort(([a],[b])=>compare(a,b)).map(([name,data])=>({path:name,bytes:data.length,sha256:hash(data)}))};
files.set('manifest.json',Buffer.from(JSON.stringify(manifest,null,2)+'\n'));
function list(directory){
 if(!fs.existsSync(directory))return [];
 return fs.readdirSync(directory,{withFileTypes:true}).flatMap(entry=>{
  const full=path.join(directory,entry.name);if(entry.isSymbolicLink())throw new Error('生成先にリンクがあります');
  return entry.isDirectory()?list(full):[path.relative(destination,full).replaceAll('\\','/')];
 });
}
const existing=list(destination),oldManifest=fs.existsSync(path.join(destination,'manifest.json'))?JSON.parse(fs.readFileSync(path.join(destination,'manifest.json'),'utf8')):null;
const owned=new Set(['manifest.json',...(oldManifest?.files??[]).map(f=>f.path)]);
for(const name of existing){
 if(!files.has(name)){
  if(check || !owned.has(name))throw new Error('余分な生成ファイルがあります: '+name);
  fs.unlinkSync(safe(destination,name));
 }
}
for(const [relative,data] of files){
 const target=safe(destination,relative);
 if(check){if(!fs.existsSync(target) || !fs.readFileSync(target).equals(data))throw new Error('ライセンス生成物が現在の依存と一致しません: '+relative);}
 else{fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,data);}
}
console.log(`成功: ライセンス${check?'照合':'生成'} Rust ${rust.length} / npm ${npm.length} / 対応ソース ${rust.filter(p=>p.bundledSource).length} / ${files.size}ファイル`);
