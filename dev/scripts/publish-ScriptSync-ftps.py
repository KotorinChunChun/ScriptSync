"""署名済みScriptSync 5ファイルだけを、既存FTPSアダプターで公開する。"""
import argparse
import hashlib
import importlib.util
import json
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
VERSION = json.loads((ROOT / 'dev/msix-version.json').read_text(encoding='utf-8-sig'))
PACKAGE_VERSION = str(VERSION['applicationVersion']) + '.' + str(VERSION['revision'])
REMOTE = '/home/excel-chunchun/www/download/ScriptSync'
NAMES = (f'ScriptSync_{PACKAGE_VERSION}_x64.msix', 'ScriptSync.cer',
         'ScriptSync.appinstaller', 'icon.png', 'index.html')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def publish(args):
    website = args.website.resolve(strict=True)
    artifact = args.artifact.resolve(strict=True)
    site = website / 'www/download/ScriptSync'
    payloads = {name: (artifact / name).read_bytes() for name in NAMES}
    for name, data in payloads.items():
        if (site / name).read_bytes() != data:
            raise RuntimeError('ステージ内容が一致しません: ' + name)
        print(f'{REMOTE}/{name} {len(data)} bytes SHA256={sha(data)}', flush=True)
    if not args.execute:
        print('DryRun成功。ネットワーク接続・更新なし。')
        return
    baseline = json.loads(args.baseline.read_text(encoding='utf-8-sig'))
    if set(baseline) != set(NAMES):
        raise RuntimeError('公開前の5ファイルの記録が必要です')
    sys.path.insert(0, str(website / 'scripts'))
    spec = importlib.util.spec_from_file_location('existing_ftps', website / 'scripts/publish-othello.py')
    adapter = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(adapter)
    run = datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%SZ') + '-scriptsync-' + PACKAGE_VERSION
    backup = args.backup_directory.resolve() / run
    backup.mkdir(parents=True, exist_ok=False)
    originals = {}
    report = {'remote': REMOTE, 'version': PACKAGE_VERSION, 'originals': {}, 'published': []}
    with adapter.connect() as ftp:
        ftp.timeout = 120
        ftp.sock.settimeout(120)
        entries = adapter.listing(ftp, REMOTE)
        # 既存ファイルを全件確認して退避するまで、一切送信しない。
        for name in NAMES:
            original = adapter.read_remote(ftp, REMOTE + '/' + name) if name in entries else None
            if name.endswith('.msix'):
                if original is not None and original != payloads[name]:
                    raise RuntimeError('同じ版の異なるMSIXが公開済みです')
            elif (sha(original) if original is not None else None) != baseline[name]:
                raise RuntimeError('公開側に未取得の変更があります: ' + name)
            originals[name] = original
            report['originals'][name] = sha(original) if original is not None else None
            if original is not None:
                (backup / name).write_bytes(original)
        report_path = backup / 'backup-manifest.json'
        report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
        for name, data in payloads.items():
            if originals[name] != data:
                adapter.replace_remote(ftp, REMOTE + '/' + name, data, run, '644')
            if adapter.read_remote(ftp, REMOTE + '/' + name) != data:
                raise RuntimeError('FTPS読戻し不一致: ' + name)
            report['published'].append({'name': name, 'sha256': sha(data)})
            report_path.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
            print('FTPS公開・照合成功: ' + name, flush=True)
    print('復元用退避: ' + str(backup), flush=True)


if __name__ == '__main__':
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')
    parser = argparse.ArgumentParser()
    parser.add_argument('--artifact', type=Path, required=True)
    parser.add_argument('--baseline', type=Path, required=True)
    parser.add_argument('--website', type=Path, default=Path('C:/develop/excel-chunchun'))
    parser.add_argument('--backup-directory', type=Path, required=True)
    parser.add_argument('--execute', action='store_true')
    publish(parser.parse_args())
