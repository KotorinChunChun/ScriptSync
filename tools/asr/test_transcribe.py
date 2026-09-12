"""機密音声を使わないCLI/MCP契約の回帰試験。"""
import argparse
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import transcribe


class AdapterTest(unittest.TestCase):
    def args(self, root):
        audio = Path(root) / '音声.wav'
        audio.write_bytes(b'fixture-audio')
        return argparse.Namespace(audio=str(audio), receipt=str(Path(root) / 'receipt.json'), model='small', codex='codex.exe')

    def test_mcp_has_one_fixed_tool_and_rejects_arbitrary_audio(self):
        with tempfile.TemporaryDirectory() as root:
            args = self.args(root)
            requests = [dict(jsonrpc='2.0', id=1, method='tools/list'),
                        dict(jsonrpc='2.0', id=2, method='tools/call', params={'name':'transcribe_current_take', 'arguments':{'audio':'private.wav'}})]
            out = io.StringIO()
            with patch('sys.stdin', io.StringIO('\n'.join(map(json.dumps, requests)))), patch('sys.stdout', out), patch.object(transcribe, 'recognize') as recognize:
                transcribe.serve_mcp(args)
            responses = list(map(json.loads, out.getvalue().splitlines()))
            self.assertEqual(len(responses[0]['result']['tools']), 1)
            self.assertIn('error', responses[1])
            recognize.assert_not_called()
            self.assertFalse(Path(args.receipt).exists())

    def test_mcp_creates_receipt_only_from_actual_engine_result(self):
        with tempfile.TemporaryDirectory() as root:
            args = self.args(root)
            request = dict(jsonrpc='2.0', id=2, method='tools/call', params={'name':'transcribe_current_take', 'arguments':{}})
            with patch('sys.stdin', io.StringIO(json.dumps(request))), patch('sys.stdout', io.StringIO()), patch.object(transcribe, 'recognize', return_value={'engine':'faster-whisper','raw':'テスト'}) as recognize:
                transcribe.serve_mcp(args)
            result = json.loads(Path(args.receipt).read_text())
            self.assertEqual(result['source_sha256'], hashlib.sha256(b'fixture-audio').hexdigest())
            recognize.assert_called_once()

    def test_cli_success_without_tool_receipt_is_failure(self):
        with tempfile.TemporaryDirectory() as root:
            args = self.args(root)
            with patch.object(transcribe.subprocess, 'run', return_value=argparse.Namespace(returncode=0,stdout=b'guessed text',stderr=b'')):
                with self.assertRaisesRegex(RuntimeError, '実行しませんでした'):
                    transcribe.via_codex(args)

    def test_cli_receipt_required_and_model_identity_retained(self):
        with tempfile.TemporaryDirectory() as root:
            args = self.args(root)
            def run(command, **kwargs):
                config = next(value for value in command if value.startswith('mcp_servers.scriptsync.args='))
                mcp_args = json.loads(config.split('=',1)[1])
                receipt = Path(mcp_args[mcp_args.index('--receipt')+1])
                receipt.write_text(json.dumps({'engine':'faster-whisper','raw':'実測','warning':'要確認', 'source_sha256':hashlib.sha256(b'fixture-audio').hexdigest()}))
                self.assertIn('--sandbox', command)
                self.assertIn('read-only', command)
                self.assertNotIn('--dangerously-bypass-approvals-and-sandbox', command)
                return argparse.Namespace(returncode=0,stderr=b'')
            with patch.object(transcribe.subprocess, 'run', side_effect=run):
                result = transcribe.via_codex(args)
            self.assertEqual(result['raw'],'実測')
            self.assertEqual(result['engine'],'faster-whisper')
            self.assertEqual(result['provider'],'codex-cli')


if __name__ == '__main__':
    unittest.main()
