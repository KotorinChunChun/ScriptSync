// 依存不要の最小OOXMLテンプレート。本文をノートに保持し、編集可能なPPTXを生成する。
const xml = s => String(s).replace(/[\x00-\x08\x0b\x0c\x0e-\x1f]/g,'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&apos;'}[c]));
const decl = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>';
const ns = 'xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"';
const rel = items => decl+'<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'+items.map(([id,type,target])=>`<Relationship Id="${id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/${type}" Target="${target}"/>`).join('')+'</Relationships>';
const group = '<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>';
function shape(id,name,lines,x,y,cx,cy,size,color) {
  return `<p:sp><p:nvSpPr><p:cNvPr id="${id}" name="${name}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="${x}" y="${y}"/><a:ext cx="${cx}" cy="${cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:noFill/></p:spPr><p:txBody><a:bodyPr wrap="square"/><a:lstStyle/>${lines.map(line=>`<a:p><a:r><a:rPr lang="ja-JP" sz="${size}"><a:solidFill><a:srgbClr val="${color}"/></a:solidFill><a:latin typeface="Yu Gothic"/><a:ea typeface="Yu Gothic"/></a:rPr><a:t>${xml(line)}</a:t></a:r><a:endParaRPr lang="ja-JP"/></a:p>`).join('')}</p:txBody></p:sp>`;
}
function crc32(data) { let c=0xffffffff;for(const b of data){c^=b;for(let i=0;i<8;i++)c=(c>>>1)^((c&1)?0xedb88320:0);}return (c^0xffffffff)>>>0; }
export function zip(files) {
  const entries=[], central=[];let offset=0;
  for(const [name,source] of Object.entries(files)) {
    const data=Buffer.from(source), path=Buffer.from(name), crc=crc32(data), header=Buffer.alloc(30);
    header.writeUInt32LE(0x04034b50);header.writeUInt16LE(20,4);header.writeUInt16LE(0x800,6);header.writeUInt32LE(crc,14);header.writeUInt32LE(data.length,18);header.writeUInt32LE(data.length,22);header.writeUInt16LE(path.length,26);
    entries.push(header,path,data);const c=Buffer.alloc(46);c.writeUInt32LE(0x02014b50);c.writeUInt16LE(20,4);c.writeUInt16LE(20,6);c.writeUInt16LE(0x800,8);c.writeUInt32LE(crc,16);c.writeUInt32LE(data.length,20);c.writeUInt32LE(data.length,24);c.writeUInt16LE(path.length,28);c.writeUInt32LE(offset,42);central.push(c,path);offset+=header.length+path.length+data.length;
  }
  const directory=Buffer.concat(central),end=Buffer.alloc(22);end.writeUInt32LE(0x06054b50);end.writeUInt16LE(entries.length/3,8);end.writeUInt16LE(entries.length/3,10);end.writeUInt32LE(directory.length,12);end.writeUInt32LE(offset,16);return Buffer.concat([...entries,directory,end]);
}
export function createDeck(plan,map) {
  const files={},types=[];
  const add=(name,body,type)=>{files[name]=decl+body;if(type)types.push(`<Override PartName="/${name}" ContentType="application/vnd.openxmlformats-officedocument.presentationml.${type}+xml"/>`);};
  const slides=plan.cues.map((c,i)=>({title:c.title,lines:c.bullets,notes:map.blocks[i].text}));
  slides.unshift({title:plan.title,lines:['ScriptSync プレゼンテーション'],notes:'この資料はScriptSyncの確定出力から生成しました。音声は埋め込まれていません。'});
  add('ppt/presentation.xml',`<p:presentation ${ns}><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="master"/></p:sldMasterIdLst><p:notesMasterIdLst><p:notesMasterId r:id="notesMaster"/></p:notesMasterIdLst><p:sldIdLst>${slides.map((_,i)=>`<p:sldId id="${256+i}" r:id="s${i}"/>`).join('')}</p:sldIdLst><p:sldSz cx="12192000" cy="6858000"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>`,'presentation.main');
  files['_rels/.rels']=rel([['office','officeDocument','ppt/presentation.xml']]);
  files['ppt/_rels/presentation.xml.rels']=rel([['master','slideMaster','slideMasters/slideMaster1.xml'],['notesMaster','notesMaster','notesMasters/notesMaster1.xml'],...slides.map((_,i)=>[`s${i}`,'slide',`slides/slide${i+1}.xml`])]);
  const colors='<p:clrMap accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" bg1="lt1" bg2="lt2" folHlink="folHlink" hlink="hlink" tx1="dk1" tx2="dk2"/>';
  add('ppt/slideMasters/slideMaster1.xml',`<p:sldMaster ${ns}><p:cSld><p:spTree>${group}</p:spTree></p:cSld>${colors}<p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="layout"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>`,'slideMaster');
  files['ppt/slideMasters/_rels/slideMaster1.xml.rels']=rel([['layout','slideLayout','../slideLayouts/slideLayout1.xml'],['theme','theme','../theme/theme1.xml']]);
  add('ppt/slideLayouts/slideLayout1.xml',`<p:sldLayout ${ns} type="blank" preserve="1"><p:cSld name="Blank"><p:spTree>${group}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>`,'slideLayout');
  files['ppt/slideLayouts/_rels/slideLayout1.xml.rels']=rel([['master','slideMaster','../slideMasters/slideMaster1.xml']]);
  add('ppt/notesMasters/notesMaster1.xml',`<p:notesMaster ${ns}><p:cSld><p:spTree>${group}</p:spTree></p:cSld>${colors}<p:notesStyle/></p:notesMaster>`,'notesMaster');
  files['ppt/notesMasters/_rels/notesMaster1.xml.rels']=rel([['theme','theme','../theme/theme1.xml']]);
  const palette={dk1:'102C35',lt1:'FFFFFF',dk2:'163D49',lt2:'F3F6F7',accent1:'087F8C',accent2:'F4B942',accent3:'6699CC',accent4:'AA77AA',accent5:'66AA88',accent6:'DD8866',hlink:'0066CC',folHlink:'663399'};
  files['ppt/theme/theme1.xml']=decl+`<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="ScriptSync"><a:themeElements><a:clrScheme name="ScriptSync">${Object.entries(palette).map(([k,v])=>`<a:${k}><a:srgbClr val="${v}"/></a:${k}>`).join('')}</a:clrScheme><a:fontScheme name="Japanese"><a:majorFont><a:latin typeface="Yu Gothic"/><a:ea typeface="Yu Gothic"/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Yu Gothic"/><a:ea typeface="Yu Gothic"/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="Simple"><a:fillStyleLst>${'<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>'.repeat(3)}</a:fillStyleLst><a:lnStyleLst>${'<a:ln w="9525"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/></a:ln>'.repeat(3)}</a:lnStyleLst><a:effectStyleLst>${'<a:effectStyle><a:effectLst/></a:effectStyle>'.repeat(3)}</a:effectStyleLst><a:bgFillStyleLst>${'<a:solidFill><a:schemeClr val="phClr"/></a:solidFill>'.repeat(3)}</a:bgFillStyleLst></a:fmtScheme></a:themeElements></a:theme>`;
  types.push('<Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>');
  slides.forEach((s,i)=>{
    add(`ppt/slides/slide${i+1}.xml`,`<p:sld ${ns}><p:cSld><p:bg><p:bgPr><a:solidFill><a:srgbClr val="F3F6F7"/></a:solidFill><a:effectLst/></p:bgPr></p:bg><p:spTree>${group}${shape(2,'Title',[s.title],600000,500000,11000000,1200000,3000,'102C35')}${shape(3,'Body',s.lines,650000,2000000,10900000,3900000,2200,'163D49')}${shape(4,'Page',[String(i+1)],11000000,6200000,500000,300000,1100,'087F8C')}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>`,'slide');
    files[`ppt/slides/_rels/slide${i+1}.xml.rels`]=rel([['layout','slideLayout','../slideLayouts/slideLayout1.xml'],['notes','notesSlide',`../notesSlides/notesSlide${i+1}.xml`]]);
    const note=shape(2,'Notes',s.notes.split(/\r?\n/),600000,1000000,5600000,7200000,1200,'102C35').replace('<p:nvPr/>','<p:nvPr><p:ph type="body" idx="1"/></p:nvPr>');
    add(`ppt/notesSlides/notesSlide${i+1}.xml`,`<p:notes ${ns}><p:cSld><p:spTree>${group}${note}</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:notes>`,'notesSlide');
    files[`ppt/notesSlides/_rels/notesSlide${i+1}.xml.rels`]=rel([['slide','slide',`../slides/slide${i+1}.xml`],['master','notesMaster','../notesMasters/notesMaster1.xml']]);
  });
  files['[Content_Types].xml']=decl+'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>'+types.join('')+'</Types>';
  return zip(files);
}
