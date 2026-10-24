"""実サーバーへ接続せず再接続・退避・同一内容の省略を検証する。"""
import importlib.util,tempfile,unittest
from pathlib import Path
from types import SimpleNamespace
spec=importlib.util.spec_from_file_location('publish',Path(__file__).with_name('publish-web.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
class PublicationTest(unittest.TestCase):
    def test_disconnect_keeps_original_backup_and_skips_completed_content(self):
        names=['test/scriptsync/a.js','test/scriptsync/b.js'];files=dict(zip(names,[b'new-a',b'new-b']));remote={module.REMOTE+'/'+names[0]:b'old-a'};writes=[];connections=[]
        class Connection:
            def __enter__(self):connections.append(1);return self
            def __exit__(self,*args):return False
        def listing(ftp,parent):return {Path(name).name:{} for name in remote if str(Path(name).parent).replace('\\','/')==parent}
        def replace(ftp,target,data,*args):
            remote[target]=data;writes.append(target)
            if len(writes)==1:raise EOFError('合成切断')
        base=SimpleNamespace(connect=Connection,listing=listing,read_remote=lambda ftp,p:remote[p],replace_remote=replace)
        with tempfile.TemporaryDirectory() as folder:
            run=Path(folder);progress={};module.publish_batch(base,lambda *args:None,files,names,run,progress)
            self.assertEqual((run/'before'/names[0]).read_bytes(),b'old-a');self.assertEqual(len(writes),2);self.assertEqual(len(connections),2);self.assertEqual(len(progress),2);self.assertEqual(remote[module.REMOTE+'/'+names[1]],b'new-b')
    def test_non_target_is_rejected_before_write(self):
        from ftplib import error_perm
        class Connection:
            def __enter__(self):return self
            def __exit__(self,*args):return False
        base=SimpleNamespace(connect=Connection,listing=lambda *args:{})
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaisesRegex(RuntimeError,'許可範囲外'):module.publish_batch(base,lambda *args:None,{'other/app.js':b'no'},['other/app.js'],Path(folder),{})
if __name__=='__main__':unittest.main()
