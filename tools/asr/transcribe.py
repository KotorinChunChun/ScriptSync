"""ローカルASR。原稿・期待読みは入力しない。漢字からのかなは要聴取の候補。"""
import argparse
import importlib.metadata
import json
import sys
import os
import hashlib
import shutil
import subprocess
import tempfile
from pathlib import Path

def recognize(args):
    from faster_whisper import WhisperModel
    model = WhisperModel(args.model, device='cpu', compute_type='int8',
                         download_root=str(Path(__file__).parent / 'models'),
                         local_files_only=not args.prepare)
    if args.prepare:
        return {'prepared': args.model}
    import pykakasi
    kana = pykakasi.kakasi()
    segments, info = model.transcribe(args.audio, language='ja', beam_size=5,
                                     word_timestamps=True, condition_on_previous_text=False,
                                     initial_prompt=None, vad_filter=False)
    words = []
    texts = []
    for segment in segments:
        texts.append(segment.text)
        for word in segment.words or []:
            words.append({'text': word.word, 'kana': ''.join(x['hira'] for x in kana.convert(word.word)),
                          'start': word.start, 'end': word.end, 'confidence': word.probability})
    raw = ''.join(texts)
    result = {'engine': 'faster-whisper', 'version': importlib.metadata.version('faster-whisper'),
              'model': args.model, 'device': 'cpu', 'compute_type': 'int8', 'raw': raw,
              'kana': ''.join(x['hira'] for x in kana.convert(raw)), 'words': words,
              'duration': info.duration, 'needs_review': True,
              'warning': '漢字ASRからのかな候補です。誤読が漢字に復元されて隠れる可能性があります。実音声を聴いて確認してください。'}
    return result


def serve_mcp(args):
    """固定された1音声のみ扱うMCPツール。任意パス・任意コマンドは受け付けない。"""
    cached = None
    for line in sys.stdin:
        request = None
        try:
            request = json.loads(line)
            if 'id' not in request:
                continue
            method = request.get('method')
            if method == 'initialize':
                result = {'protocolVersion': request.get('params', {}).get('protocolVersion', '2024-11-05'),
                          'capabilities': {'tools': {}}, 'serverInfo': {'name': 'scriptsync-asr', 'version': '1.0'}}
            elif method == 'ping':
                result = {}
            elif method == 'tools/list':
                result = {'tools': [{'name': 'transcribe_current_take',
                    'description': '指定済みのテイク1件をローカルfaster-whisperで認識する。音声からの実測のみ。',
                    'annotations': {'readOnlyHint': True, 'destructiveHint': False, 'idempotentHint': True, 'openWorldHint': False},
                    'inputSchema': {'type': 'object', 'properties': {}, 'additionalProperties': False}}]}
            elif method == 'tools/call':
                params = request.get('params', {})
                if params.get('name') != 'transcribe_current_take' or params.get('arguments', {}):
                    raise ValueError('このツールは引数なしのtranscribe_current_takeだけを実行できます')
                if cached is None:
                    cached = recognize(args)
                    cached['source_sha256'] = hashlib.sha256(Path(args.audio).read_bytes()).hexdigest()
                    Path(args.receipt).write_text(json.dumps(cached, ensure_ascii=True), encoding='utf-8')
                result = {'content': [{'type': 'text', 'text': json.dumps(cached, ensure_ascii=True)}]}
            else:
                print(json.dumps({'jsonrpc': '2.0', 'id': request['id'], 'error': {'code': -32601, 'message': 'Unknown method'}}), flush=True)
                continue
            print(json.dumps({'jsonrpc': '2.0', 'id': request['id'], 'result': result}), flush=True)
        except Exception as error:
            print(json.dumps({'jsonrpc': '2.0', 'id': request.get('id') if isinstance(request, dict) else None,
                              'error': {'code': -32603, 'message': str(error)}}), flush=True)


def via_codex(args):
    """CLIが実際にツールを実行して残した結果のみ採用。自由文の推測は採用しない。"""
    executable = args.codex or shutil.which('codex.exe') or shutil.which('codex')
    if not executable or (os.name == 'nt' and Path(executable).suffix.lower() != '.exe'):
        raise RuntimeError('Codex CLIの実行ファイルが見つかりません。設定から導入案内を開き、認証後にcodex.exeを指定してください')
    with tempfile.TemporaryDirectory(prefix='scriptsync-codex-') as temp:
        receipt = Path(temp) / 'recognized.json'
        # TOML文字列としてJSONのエスケープを使い、シェルへ連結しない。
        server_args = ['-I', str(Path(__file__).resolve()), '--mcp', '--audio', str(Path(args.audio).resolve()),
                       '--model', args.model, '--receipt', str(receipt)]
        prompt = ('transcribe_current_take ツールを1回実行して、この音声ファイルの文字起こしを取得してください。'
                  'ツールが失敗した場合は失敗とだけ報告してください。原稿から推測したり本文を書き換えたりしないでください。'
                  'シェル実行、ファイルの書換え、外部サービスへの音声送信は不要です。')
        command = [executable, 'exec', '--skip-git-repo-check', '--ephemeral', '--ignore-user-config',
                   '--sandbox', 'read-only', '-C', temp, '-c', 'approval_policy="never"',
                   '-c', 'features.shell_tool=false', '-c', 'web_search="disabled"',
                   '-c', 'mcp_servers.scriptsync.command=' + json.dumps(sys.executable),
                   '-c', 'mcp_servers.scriptsync.args=' + json.dumps(server_args),
                   '-c', 'mcp_servers.scriptsync.tool_timeout_sec=600',
                   '-c', 'mcp_servers.scriptsync.enabled_tools=["transcribe_current_take"]', prompt]
        options = {'creationflags': subprocess.CREATE_NO_WINDOW} if os.name == 'nt' else {}
        completed = subprocess.run(command, stdin=subprocess.DEVNULL, capture_output=True, timeout=650, **options)
        if completed.returncode != 0:
            raise RuntimeError('Codex CLI実行失敗：' + completed.stderr.decode('utf-8', errors='replace')[-1500:])
        if not receipt.is_file():
            detail = completed.stderr.decode('utf-8', errors='replace')[-1600:]
            raise RuntimeError('Codex CLIが認識ツールを実行しませんでした。推測による文字起こしは採用しません。' + detail)
        result = json.loads(receipt.read_text(encoding='utf-8'))
        expected_hash = hashlib.sha256(Path(args.audio).read_bytes()).hexdigest()
        if result.get('source_sha256') != expected_hash or result.get('engine') != 'faster-whisper':
            raise RuntimeError('Codex CLIの認識結果と対象音声が一致しません')
        result['provider'] = 'codex-cli'
        result['warning'] += ' Codex CLI経由で同じローカル認識器を実行しました。独立した認識モデルではありません。'
        return result


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--audio')
    p.add_argument('--prepare', action='store_true')
    p.add_argument('--model', default='small', choices=['tiny', 'base', 'small', 'medium', 'large-v3'])
    p.add_argument('--provider', default='local', choices=['local', 'codex'])
    p.add_argument('--codex', default='')
    p.add_argument('--mcp', action='store_true')
    p.add_argument('--receipt')
    args = p.parse_args()
    if not args.prepare and (not args.audio or not Path(args.audio).is_file()):
        p.error('音声ファイルがありません')
    if args.mcp:
        if not args.receipt:
            p.error('結果の保存先がありません')
        serve_mcp(args)
        return
    result = via_codex(args) if args.provider == 'codex' and not args.prepare else recognize(args)
    print(json.dumps(result, ensure_ascii=True))

if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        print(json.dumps({'error': str(error)}, ensure_ascii=True), file=sys.stderr)
        raise SystemExit(1)
