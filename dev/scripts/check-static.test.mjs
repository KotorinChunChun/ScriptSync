import {test} from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {checkStatic} from './check-static.mjs';
const repo=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
function fixture(t){
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'scriptsync-static-'));t.after(()=>fs.rmSync(root,{recursive:true,force:true}));
 const files=['package.json','package-lock.json','dev/msix-version.json','src-tauri/tauri.conf.json','src-tauri/Cargo.toml','src-tauri/Cargo.lock','index.html','src/appInfo.ts','dev/release-page/index.html',...['scriptsync-project-validation','scriptsync-project-release'].map(s=>`.agents/skills/${s}/SKILL.md`)];
 for(const file of files){fs.mkdirSync(path.dirname(path.join(root,file)),{recursive:true});fs.copyFileSync(path.join(repo,file),path.join(root,file));}
 for(const file of files.filter(f=>f.endsWith('SKILL.md'))){for(const m of fs.readFileSync(path.join(root,file),'utf8').matchAll(/\[[^\]]*\]\(([^)]+)\)/g)){if(/^(https?:|#)/.test(m[1]))continue;const p=path.resolve(root,path.dirname(file),m[1].split('#')[0]);fs.mkdirSync(path.dirname(p),{recursive:true});if(!fs.existsSync(p))fs.writeFileSync(p,'参照fixture');}}
 return root;
}
function edit(root,file,fn){const p=path.join(root,file);fs.writeFileSync(p,fn(fs.readFileSync(p,'utf8')));}
test('現行の版と参照を読み取り専用で検査',t=>assert.equal(checkStatic(fixture(t),{releasePage:true}).ok,true));
test('タイトルだけ古い場合も検知',t=>{const root=fixture(t);edit(root,'index.html',s=>s.replace(/\d+\.\d+\.\d+\.\d+/,'99.99.99.99'));assert(checkStatic(root).errors.some(e=>e.includes('HTMLタイトル')));});
test('未公開開発中は配布ページの旧版を許容、公開前は拒否',t=>{const root=fixture(t);edit(root,'dev/release-page/index.html',s=>s.replaceAll(/\d+\.\d+\.\d+\.\d+/g,'99.99.99.99'));assert.equal(checkStatic(root).ok,true);assert.equal(checkStatic(root,{releasePage:true}).ok,false);});
test('不正な再配布番号を拒否',t=>{const root=fixture(t);edit(root,'dev/msix-version.json',s=>{const j=JSON.parse(s);j.revision=65536;return JSON.stringify(j);});assert(checkStatic(root).errors.some(e=>e.includes('再配布番号')));});
test('スキル参照切れを検知',t=>{const root=fixture(t);edit(root,'.agents/skills/scriptsync-project-validation/SKILL.md',s=>s+'\n[missing](missing.md)');assert(checkStatic(root).errors.some(e=>e.includes('参照先なし')));});
