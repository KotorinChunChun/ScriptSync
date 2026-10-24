"""ScriptSyncの2つのWeb版だけを既存WebsiteのFTPS経路で配信する。"""
import argparse,hashlib,importlib.util,json,sys
from pathlib import Path
from datetime import datetime,timezone
from urllib.request import Request,urlopen
ROOT=Path(__file__).resolve().parents[2]
SITE=Path('C:/develop/excel-chunchun')
REMOTE='/home/excel-chunchun/www'
PUBLIC='https://excel-chunchun.sakura.ne.jp/'
LITE=('.htaccess','index.html','app.js','style.css','api.php','download.html','download.js','download.css','core.mjs','audio-edit.mjs','recording.mjs','playback.mjs','trim-editor.mjs','exchange.mjs','pcm-worklet.js','THIRD_PARTY.md','vendor/LICENSE.txt','vendor/zip-full.min.js','storage/.htaccess')
def digest(data):return hashlib.sha256(data).hexdigest()
def payloads():
    result={}
    for name in LITE:result['test/scriptsyncweb/'+name]=(SITE/'www/test/scriptsyncweb'/name).read_bytes()
    for file in (ROOT/'dist-web').rglob('*'):
        if file.is_file():result['test/scriptsync/'+file.relative_to(ROOT/'dist-web').as_posix()]=file.read_bytes()
    result['test/menu.json']=(SITE/'www/test/menu.json').read_bytes()
    if not result.get('test/scriptsync/index.html') or not result.get('test/scriptsync/help.html'):raise RuntimeError('Webビルドが不足しています')
    if any('/dev/' in name or '/scripts/' in name or '/.git' in name or name.endswith(('.map','.zip','.db','.pfx','.pem')) for name in result):raise RuntimeError('公開対象外のファイルがあります')
    return result
def publish_batch(base,ensure_remote_dir,files,names,run,progress):
    from ftplib import error_perm
    # 接続の寿命・一時切断を考慮。完了した同一内容は再送しない。
    for attempt in range(3):
        try:
            with base.connect() as ftp:
                listings={};directories=set()
                for name in names:
                    target=REMOTE+'/'+name;parent=str(Path(target).parent).replace('\\','/')
                    if parent not in listings:
                        try:listings[parent]=base.listing(ftp,parent)
                        except error_perm as error:
                            if not str(error).startswith('550'):raise
                            listings[parent]={}
                    exists=Path(name).name in listings[parent]
                    old=base.read_remote(ftp,target) if exists else None
                    if old!=files[name]:
                        if old is not None:
                            backup=run/'before'/name
                            if not backup.exists():backup.parent.mkdir(parents=True,exist_ok=True);backup.write_bytes(old)
                        if not name.startswith(('test/scriptsync/','test/scriptsyncweb/')) and name!='test/menu.json':raise RuntimeError('配信先が許可範囲外です')
                        if parent not in directories:ensure_remote_dir(ftp,parent);directories.add(parent)
                        base.replace_remote(ftp,target,files[name],run.name,'644')
                        if base.read_remote(ftp,target)!=files[name]:raise RuntimeError('FTPS読戻し不一致：'+name)
                    progress[name]={'sha256':digest(files[name]),'verified':True}
                    (run/'progress.json').write_text(json.dumps(progress,indent=2),encoding='utf-8')
            return
        except (EOFError,ConnectionError,TimeoutError,OSError):
            if attempt==2:raise
            print(f'FTPS再接続 {attempt+1}/2（同一内容は保持）',flush=True)
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--stage',action='store_true');parser.add_argument('--execute',action='store_true');args=parser.parse_args()
    files=payloads();manifest={name:{'sha256':digest(data),'size':len(data)} for name,data in files.items()}
    run=ROOT/'dev/history/v0.0.8/logs'/('publication-'+datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ'));run.mkdir(parents=True)
    (run/'manifest.json').write_text(json.dumps(manifest,indent=2),encoding='utf-8')
    print(f'DryRun: {len(files)}ファイル / {sum(map(len,files.values()))} bytes / {PUBLIC}',flush=True)
    if args.stage:
        for name,data in files.items():
            if name.startswith('test/scriptsync/'):
                target=SITE/'www'/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
    if not args.execute:return
    sys.path.insert(0,str(SITE/'scripts'))
    from upload_ftp import ensure_remote_dir
    spec=importlib.util.spec_from_file_location('existing_ftps',SITE/'scripts/publish-othello.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base)
    # 拒否規則・資産・入口・メニューの順。接続を小さい単位で更新する。
    ordered=sorted(files,key=lambda n:(not n.endswith('.htaccess'),n=='test/menu.json',n.endswith('index.html'),n))
    progress={}
    for start in range(0,len(ordered),8):
        publish_batch(base,ensure_remote_dir,files,ordered[start:start+8],run,progress)
        print(f'FTPS確認 {len(progress)}/{len(files)}',flush=True)
    results={}
    for name,data in files.items():
        if name.endswith('.htaccess') or name.endswith('api.php'):continue
        with urlopen(Request(PUBLIC+name+'?verify='+run.name,headers={'Cache-Control':'no-cache'}),timeout=90) as response:
            body=response.read();content_type=response.headers.get_content_type()
            if response.status!=200 or digest(body)!=digest(data):raise RuntimeError('HTTPS読戻し不一致：'+name)
            if name.endswith('.wasm') and content_type!='application/wasm':raise RuntimeError('WASM MIME不一致')
            if name.endswith(('.js','.mjs')) and content_type not in ('text/javascript','application/javascript'):raise RuntimeError('JS MIME不一致')
            results[name]={'status':200,'sha256':digest(body),'mime':content_type}
    (run/'verified.json').write_text(json.dumps(results,indent=2),encoding='utf-8')
    print(f'公開・FTPS全件・HTTPS {len(results)}件のhash/MIME一致: {run}',flush=True)
if __name__=='__main__':main()
