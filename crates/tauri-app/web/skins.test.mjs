// SPDX-License-Identifier: GPL-3.0-or-later
import {test} from 'node:test';import assert from 'node:assert/strict';import fs from 'node:fs';import vm from 'node:vm';
import {BUNDLED} from './skins/bundled.mjs';import {validateSkin,exportSkin,compileCss,skinStyles,pngBytes,MAX_PACKAGE_BYTES} from './skin-format.mjs';
import {SkinStore,LIBRARY_KEY,PREFERENCES_KEY,DEFAULT_PREFERENCES} from './skin-store.mjs';
const copy=()=>structuredClone(BUNDLED[0]);const sample=JSON.parse(fs.readFileSync(new URL('./skins/soft-grid.dawjskin.json',import.meta.url),'utf8'));
class MemoryStorage{constructor(){this.values=new Map()}getItem(k){return this.values.get(k)??null}setItem(k,v){this.values.set(k,String(v))}removeItem(k){this.values.delete(k)}}
test('all nine bundled skins match their portable downloadable files',()=>{assert.equal(BUNDLED.length,9);for(const skin of BUNDLED){assert.equal(validateSkin(skin).schemaVersion,1);assert.deepEqual(JSON.parse(fs.readFileSync(new URL('./skins/'+skin.metadata.id+'.dawjskin.json',import.meta.url),'utf8')),skin)}});
test('requested palettes retain exact colors and modes',()=>{
 const expected=[['dark','#17150D','#FBE439','#FF883A'],['dark','#111113','#F15E38','#F3D04D'],['light','#C6C9C5','#D96237','#66757D'],['dark','#151C23','#F1843E','#54B7AE'],['dark','#211C1B','#6AC8B5','#D98F81'],['light','#B9B6AD','#315E8F','#B44A4F']];
 BUNDLED.slice(3).forEach((s,i)=>assert.deepEqual([s.mode,s.variables.background,s.variables.deckA,s.variables.deckB],expected[i]));
});
test('export/import preserves CSS, variables, metadata and embedded PNG',()=>{assert.deepEqual(validateSkin(exportSkin(sample)),validateSkin(sample));assert.ok(skinStyles(sample).includes('data:image/png;base64,'));assert.ok(pngBytes(sample.assets[0].data).length>57)});
test('unsupported versions, executable fields and malformed metadata reject',()=>{
 for(const patch of [{schemaVersion:2},{surfaceVersion:2},{script:'alert(1)'},{mode:'x'},{metadata:{...copy().metadata,id:'../escape'}},{metadata:{...copy().metadata,version:'latest'}}])assert.throws(()=>validateSkin({...copy(),...patch}));
 assert.throws(()=>validateSkin('{broken'));assert.throws(()=>validateSkin(' '.repeat(MAX_PACKAGE_BYTES+1)));
});
test('font, range and text contrast validation protects base settings',()=>{for(const variables of [{fontSize:0},{controlHeight:1},{spacing:100},{fontFamily:'url(http://example.test)'},{fontFamily:'Wingdings'},{text:'#171A20'},{unrestricted:'bad'}])assert.throws(()=>validateSkin({...copy(),variables:{...copy().variables,...variables}}))});
test('network, imports, positioning, executable CSS and escapes reject',()=>{
 for(const css of ['@import "https://example.test/a.css";','.deck{background-image:url(https://example.test/a.png)}','.deck{background-image:u\\72l(https://example.test)}','@font-face{font-family:X;src:url(file:///x)}','.deck{position:fixed}','body{display:none}','.deck{opacity:0}','.deck{color:transparent}','.deck{background-image:image-set(url(x))}','.deck{box-shadow:0 0 0 99999px black}','.deck{color:#ffffff!important}'])assert.throws(()=>compileCss(css),css);
});
test('state CSS cannot move live controls and undocumented selectors reject',()=>{
 for(const css of ['button:hover{padding:200px}','dialog{display:block}','#skin-preview-host{height:0px}','.deck:has(button){color:#ffffff}'])assert.throws(()=>compileCss(css));
 assert.doesNotThrow(()=>compileCss('.deck button:hover{color:var(--color-primary)} @media (min-width: 1600px){.slots{gap:10px}}'));
});
test('missing, traversal, remote, duplicate and non-PNG assets reject',()=>{
 const s=structuredClone(sample);s.assets=[];assert.throws(()=>validateSkin(s),/packaged PNG/);
 for(const path of ['../a.png','/a.png','file:///a.png','a%2fb.png','x.svg','x\\a.png']){const x=structuredClone(sample);x.assets[0].path=path;assert.throws(()=>validateSkin(x))}
 const x=structuredClone(sample);x.assets.push(x.assets[0]);assert.throws(()=>validateSkin(x));x.assets[0].mime='image/svg+xml';assert.throws(()=>validateSkin(x));
});
test('bad image bytes and checksums reject before browser decoding',()=>{assert.throws(()=>pngBytes(btoa('<svg onload="alert(1)"/>')));const data=atob(sample.assets[0].data);assert.throws(()=>pngBytes(btoa(data.slice(0,-1)+'X')),/checksum/)});
test('selection and packages survive a fresh storage instance',()=>{const storage=new MemoryStorage(),store=new SkinStore(storage);store.saveLibrary([sample]);store.savePreferences({...DEFAULT_PREFERENCES,selected:'user:soft-grid',fontScale:1.2,enhancedContrast:true});const restored=new SkinStore(storage).load();assert.equal(restored.library[0].metadata.id,'soft-grid');assert.equal(restored.prefs.selected,'user:soft-grid');assert.equal(restored.prefs.fontScale,1.2)});
test('corrupt persistence falls back and reset clears legacy CSS',()=>{const storage=new MemoryStorage();storage.setItem(LIBRARY_KEY,'bad JSON');storage.setItem(PREFERENCES_KEY,'bad JSON');storage.setItem('customCSS','body{display:none}');const store=new SkinStore(storage),data=store.load();assert.deepEqual(data.prefs,DEFAULT_PREFERENCES);assert.equal(data.library.length,0);assert.ok(store.warnings.length);store.reset();assert.equal(storage.getItem('customCSS'),null);assert.equal(storage.getItem(PREFERENCES_KEY),null)});
test('quota failures never invent successful persistence',()=>{const storage=new MemoryStorage();storage.setItem=()=>{throw Error('Quota exceeded')};assert.throws(()=>new SkinStore(storage).saveLibrary([sample]),/Quota/)});
test('the exact native recovery script clears bad state with and without a live UI manager',()=>{
 const script=fs.readFileSync(new URL('./skin-recovery.js',import.meta.url),'utf8');
 for(const hasManager of [true,false]){const storage=new MemoryStorage();for(const k of [LIBRARY_KEY,PREFERENCES_KEY,'customCSS'])storage.setItem(k,'bad state');let resets=0,reloads=0,removed=0;
  vm.runInNewContext(script,{localStorage:storage,window:hasManager?{__DAWJ_RESET_SKIN__:()=>resets++}:{},document:{getElementById:()=>({remove:()=>removed++})},location:{reload:()=>reloads++}});
  for(const k of [LIBRARY_KEY,PREFERENCES_KEY,'customCSS'])assert.equal(storage.getItem(k),null);assert.equal(resets,hasManager?1:0);assert.equal(reloads,hasManager?0:1);assert.equal(removed,hasManager?0:2);
 }
});
test('native reset, safe launch and restrictive CSP are independent of skin controls',()=>{const source=fs.readFileSync(new URL('../src/main.rs',import.meta.url),'utf8'),script=fs.readFileSync(new URL('./skin-recovery.js',import.meta.url),'utf8');assert.ok(source.includes('Ctrl+Alt+Shift+R'));assert.ok(source.includes('--safe-mode'));assert.ok(source.includes('--reset-skin'));for(const key of [LIBRARY_KEY,PREFERENCES_KEY,'customCSS'])assert.ok(script.includes(key));const config=JSON.parse(fs.readFileSync(new URL('../tauri.conf.json',import.meta.url),'utf8'));assert.ok(config.app.security.csp.includes("script-src 'self'"));assert.ok(config.app.security.csp.includes("object-src 'none'"));assert.ok(!config.app.security.csp.includes('unsafe-eval'));assert.ok(!config.app.security.csp.includes('unsafe-inline'))});
