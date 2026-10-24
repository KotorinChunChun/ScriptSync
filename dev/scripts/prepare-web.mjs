import fs from 'node:fs/promises';import path from 'node:path';
const out=path.resolve('dist-web'),manifest=JSON.parse(await fs.readFile('licenses/third-party/manifest.json','utf8'));
await fs.copyFile('LICENSE',path.join(out,'LICENSE.txt'));
const paths=new Set(manifest.npm.flatMap(p=>p.texts));
for(const relative of paths){const target=path.join(out,'licenses',relative);await fs.mkdir(path.dirname(target),{recursive:true});await fs.copyFile(path.join('licenses/third-party',relative),target);}
await fs.writeFile(path.join(out,'licenses','manifest.json'),JSON.stringify({npm:manifest.npm},null,2)+'\n');
await fs.writeFile(path.join(out,'THIRD-PARTY-NOTICES.md'),'# ScriptSync ブラウザー版の第三者文書\n\n本体はMITです。ブラウザーへ配布するnpmライブラリの帰属と原文を次に示します。ブラウザーから取得するWhisperモデルは取得元の条件に従います。Node専用sharp/onnxruntime-nodeとONNX WebGL専用GUID/OS検出は本版のWASM構成に配布しません。\n\n'+manifest.npm.map(p=>`- ${p.name} ${p.version} (${p.license}): ${p.texts.map(x=>`[文書](licenses/${x})`).join(' / ')}`).join('\n')+'\n');
await fs.copyFile('dev/web/help.html',path.join(out,'help.html'));
await fs.copyFile('dev/web/htaccess',path.join(out,'.htaccess'));
const index=path.join(out,'index.html');await fs.writeFile(index,(await fs.readFile(index,'utf8')).replace(/<title>.*?<\/title>/,'<title>ScriptSync ブラウザー版 0.0.8</title>'));
console.log(`Web公開物: npm ${manifest.npm.length}件の文書と本体ライセンスを準備`);
