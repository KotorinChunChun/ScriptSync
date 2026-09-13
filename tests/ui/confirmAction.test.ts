import {expect,it,vi} from 'vitest';
import {confirmAction} from '../../src/confirmAction';
it('不可逆操作は確認を待ち、取消と実行を区別する',async()=>{
 HTMLDialogElement.prototype.showModal=vi.fn();
 const pending=confirmAction('保存すると復元できません。');let completed=false;void pending.then(()=>completed=true);
 await Promise.resolve();expect(completed).toBe(false);expect(document.body.textContent).toContain('復元できません');
 document.querySelector<HTMLButtonElement>('[data-cancel]')!.click();expect(await pending).toBe(false);
 const accepted=confirmAction('保存しますか？');document.querySelector<HTMLButtonElement>('[data-confirm]')!.click();expect(await accepted).toBe(true);
 expect(document.querySelector('dialog')).toBeNull();
});

it('実行へ初期フォーカスを置きEnterで確定、Escapeで取消する',async()=>{
 HTMLDialogElement.prototype.showModal=vi.fn();const accepted=confirmAction('保存しますか？');expect(document.activeElement).toBe(document.querySelector('[data-confirm]'));document.querySelector('dialog')!.dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));expect(await accepted).toBe(true);const cancelled=confirmAction('保存しますか？');document.querySelector('dialog')!.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));expect(await cancelled).toBe(false);
});
