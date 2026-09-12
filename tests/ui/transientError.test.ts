import {expect,it,vi} from 'vitest';
import {showTransientError} from '../../src/transientError';
it('中央エラーは3秒後に消え、連続したエラーは最新だけを表示する',()=>{
 vi.useFakeTimers();showTransientError('Whisperを設定から導入してください');expect(document.querySelector('[role=alert]')?.textContent).toContain('Whisper');
 vi.advanceTimersByTime(2000);showTransientError('Codex CLIの実行に失敗しました');expect(document.querySelectorAll('#transient-error')).toHaveLength(1);
 vi.advanceTimersByTime(2999);expect(document.querySelector('#transient-error')).not.toBeNull();vi.advanceTimersByTime(1);expect(document.querySelector('#transient-error')).toBeNull();vi.useRealTimers();
});
