import {expect,it,vi,afterEach} from 'vitest';
import {openArchiveManager} from '../../src/recordingArchives';
afterEach(()=>{document.body.innerHTML='';});
it('現役と削除済みの読み上げ対象を一括確認し、対応付けを強制保存する',async()=>{
 (HTMLDialogElement.prototype as any).showModal=vi.fn();
 document.body.innerHTML='<div id="app"><button id="entry">入口</button></div>';
 const project:any={id:'p',generation:3,script_sources:[{id:'current',name:'現行.md',confirmed_text:'現行'}],archived_sources:[{id:'old',name:'旧.md',confirmed_text:'旧'}],segments:[{id:'active',source_id:'current',text:'現役本文',source_text:'元本文',recording_draft:'修正中',takes:[]}],archived_segments:[{id:'deleted',source_id:'old',text:'削除済み本文',takes:[]}]};
 const rpc=vi.fn(async()=>null),refresh=vi.fn(async()=>{});openArchiveManager({project,getProject:()=>project,rpc,refresh,play:vi.fn(),notice:vi.fn()});
 expect(document.querySelector('.archive-content')?.textContent).toContain('現役本文');expect(document.querySelector('.archive-content')?.textContent).toContain('削除済み本文');
 const rows=[...document.querySelectorAll<HTMLElement>('[data-archive-id]')];expect(rows).toHaveLength(2);rows[0].querySelector<HTMLSelectElement>('[data-archive-source]')!.value='old';rows[0].querySelector<HTMLTextAreaElement>('[data-archive-draft]')!.value='強制した修正';document.querySelector<HTMLButtonElement>('[data-save]')!.click();await Promise.resolve();document.querySelector<HTMLButtonElement>('[data-confirm]')!.click();await Promise.resolve();await vi.waitFor(()=>expect(rpc).toHaveBeenCalledWith('archive_override',{project_id:'p',base_generation:3,confirmed:true,rows:[{id:'active',source_id:'old',source_text:'元本文',text:'元本文',recording_draft:'強制した修正'},{id:'deleted',source_id:'old',source_text:'削除済み本文',text:'削除済み本文',recording_draft:null}]}));
});
