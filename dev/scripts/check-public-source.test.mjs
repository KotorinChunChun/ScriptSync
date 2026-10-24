import {test} from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {scanContent,auditRepository} from './check-public-source.mjs';

test('配布CSPは任意script・eval・外部接続を許さず開発WebSocketは開発時だけ許可する',()=>{
 const config=JSON.parse(fs.readFileSync(new URL('../../src-tauri/tauri.conf.json',import.meta.url),'utf8'));
 const security=config.app.security;
 for(const policy of [security.csp,security.devCsp]){
  assert.equal(typeof policy,'string');
  const directives=new Map(policy.split(';').map(value=>{const [name,...sources]=value.trim().split(/\s+/);return [name,sources];}));
  assert.deepEqual(directives.get('script-src'),["'self'"]);
  assert.deepEqual(directives.get('object-src'),["'none'"]);
  assert.ok(!directives.get('connect-src').some(value=>value==='*' || value==='https:' || value==='http:'));
  assert.ok(directives.get('media-src').includes('http://scriptsync-audio.localhost'));
 }
 assert.ok(!security.csp.includes('ws:'));
 assert.ok(security.devCsp.includes('ws://localhost:1420'));
});

test('既知トークン・UTF16秘密鍵・固定資格情報を値なしで検出する',()=>{
 const value='gh'+'p_'+'a'.repeat(36);
 const result=scanContent('src/config.ts',Buffer.from(`const token = "${value}";\nconst password = "secret-value";`));
 assert.deepEqual(result.map(f=>f.kind),['github-token','credential-literal']);
 assert.ok(!JSON.stringify(result).includes(value));
 const key=Buffer.concat([Buffer.from([255,254]),Buffer.from('-----BEGIN '+'PRIVATE KEY-----','utf16le')]);
 assert.equal(scanContent('source.txt',key)[0].kind,'private-key');
});

test('RustのURL拒否試験の既知合成URIだけを例外とする',()=>{
 const uri='http://'+'user:secret@localhost';
 assert.deepEqual(scanContent('src-tauri/src/services.rs',Buffer.from('#[cfg(test)] mod tests { validate_url("'+uri+'"); }')),[]);
 assert.equal(scanContent('src-tauri/src/services.rs',Buffer.from('validate_url("'+uri+'");'))[0].kind,'url-password');
 assert.equal(scanContent('src/config.ts',Buffer.from('const url="'+uri+'";'))[0].kind,'url-password');
 assert.equal(scanContent('src-tauri/src/services.rs',Buffer.from('#[cfg(test)] "http://'+'user:another-secret@localhost"'))[0].kind,'url-password');
});

test('秘密ファイルを拒否し、例示設定と合成テストの一般パスワードを許可する',()=>{
 assert.equal(scanContent('private/credential.pfx',Buffer.from([0,1]))[0].kind,'private-file');
 assert.equal(scanContent('project.scriptsync',Buffer.from('{}'))[0].kind,'private-file');
 assert.deepEqual(scanContent('.env.example',Buffer.from('API_KEY="your_api_key_here"')),[]);
 assert.deepEqual(scanContent('tests/ui/obs.test.ts',Buffer.from('password: "fixture-password"')),[]);
});

test('現行から削除された秘密もGit履歴の走査で検出する',()=>{
 const root=fs.mkdtempSync(path.join(os.tmpdir(),'scriptsync-source-audit-'));
 const git=(...args)=>{const r=spawnSync('git',['-c',`safe.directory=${root.replaceAll('\\','/')}`,'-c','user.name=fixture','-c','user.email=fixture@example.invalid',...args],{cwd:root});assert.equal(r.status,0);};
 try{
  git('init','-q');fs.writeFileSync(path.join(root,'README.md'),'fixture');
  fs.writeFileSync(path.join(root,'old.txt'),'gh'+'p_'+'b'.repeat(36));
  git('add','.');git('commit','-qm','合成fixture');
  git('rm','-q','old.txt');git('commit','-qm','削除後fixture');
  assert.equal(auditRepository(root).ok,true);
  const report=auditRepository(root,{history:true});
  assert.equal(report.ok,false);assert.equal(report.findings[0].scope,'history');
  assert.equal(report.findings[0].path,'old.txt');
  fs.writeFileSync(path.join(root,'ordinary.txt'),'');git('add','.');git('commit','-qm','同内容の安全名');
  fs.writeFileSync(path.join(root,'.env'),'');git('add','.env');git('commit','-qm','機密名fixture');
  git('rm','-q','.env');git('commit','-qm','機密名削除');
  assert.ok(auditRepository(root,{history:true}).findings.some(f=>f.path==='.env' && f.kind==='private-file'));
 }finally{
  assert.equal(path.dirname(path.resolve(root)),path.resolve(os.tmpdir()));
  assert.ok(path.basename(root).startsWith('scriptsync-source-audit-'));
  fs.rmSync(root,{recursive:true,force:true});
 }
});
