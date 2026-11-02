// SPDX-License-Identifier: GPL-3.0-or-later
import {BUNDLED} from './skins/bundled.mjs';import {validateSkin,exportSkin} from './skin-format.mjs';
import {SkinStore,LIBRARY_KEY,PREFERENCES_KEY,DEFAULT_PREFERENCES} from './skin-store.mjs';import {inspectSurface,nextPaint} from './skin-safety.mjs';
export async function skinSmoke(manager,invoke){
 const keys=[LIBRARY_KEY,PREFERENCES_KEY,'customCSS','theme','accent'],saved=new Map(keys.map(k=>[k,localStorage.getItem(k)]));
 const assert=(yes,message)=>{if(!yes)throw Error('Skin smoke: '+message)};const viewports=[],zoomScales=[];
 try{
  manager.reset();
  for(const [width,height]of [[1280,820],[1366,768],[1600,900],[1920,1080],[1100,740]]){
   await invoke('skin_test_window',{width,height});await new Promise(r=>setTimeout(r,70));await nextPaint();scrollTo(0,0);
   for(const skin of BUNDLED){await manager.paint('bundled:'+skin.metadata.id,DEFAULT_PREFERENCES);assert(getComputedStyle(document.documentElement).getPropertyValue('--color-background').trim().toUpperCase()===skin.variables.background.toUpperCase(),'palette token did not apply: '+skin.metadata.name);const rgb=skin.variables.background.slice(1).match(/../g).map(n=>parseInt(n,16));assert(getComputedStyle(document.body).backgroundColor===`rgb(${rgb.join(', ')})`,'painted palette did not apply: '+skin.metadata.name)}
   assert(!inspectSurface().length,'unsafe viewport '+JSON.stringify(inspectSurface()));viewports.push([innerWidth,innerHeight]);
  }
  await invoke('skin_test_window',{width:1920,height:1080});
  for(const zoom of [1.25,1.5,2]){await invoke('skin_test_zoom',{zoom});await new Promise(r=>setTimeout(r,100));await nextPaint();assert(!inspectSurface().length,'zoom layout '+zoom+': '+inspectSurface().join('; '));zoomScales.push({zoom,dpr:devicePixelRatio,viewport:[innerWidth,innerHeight]})}
  await invoke('skin_test_zoom',{zoom:1});
  await invoke('skin_test_window',{width:1280,height:820});await new Promise(r=>setTimeout(r,70));
  await manager.paint('bundled:dark',{...DEFAULT_PREFERENCES,fontScale:1.25,enhancedContrast:true});assert(!inspectSurface().length,'accessibility layout');manager.reset();
  const sample=await(await fetch('skins/soft-grid.dawjskin.json')).text(),id=await manager.importText(sample);await manager.preview(id);manager.keep();
  assert(getComputedStyle(document.getElementById('deck-0')).backgroundImage.includes('data:image/png;base64,'),'sample asset was not painted');
  assert(new SkinStore(localStorage).load().prefs.selected===id,'selection persistence');assert(validateSkin(exportSkin(manager.skins.get(id))).assets.length===1,'asset roundtrip');
  const bad=validateSkin({...BUNDLED[0],metadata:{...BUNDLED[0].metadata,id:'bad-layout',name:'Bad layout'},css:'.deck { margin-left: 1800px; }'});manager.skins.set('user:bad-layout',bad);
  try{await manager.preview('user:bad-layout')}catch{/* A synchronous rejection or the resize audit can win. */}
  assert(manager.current!=='user:bad-layout'&&!manager.previewing&&!inspectSurface().length,'unusable layout remained active: '+manager.current);assert(manager.skins.has(id),'automatic fallback deleted imported packages');
  const racing=manager.preview('bundled:psychic-genesis');manager.reset();await racing;assert(manager.current==='bundled:dark','queued preview won over reset');
  const expiry=manager.scheduleExpiry;manager.scheduleExpiry=callback=>setTimeout(callback,40);await manager.preview('bundled:psychic-genesis');await new Promise(r=>setTimeout(r,120));await nextPaint();assert(manager.current==='bundled:dark'&&!manager.previewing,'preview expiry failed');manager.scheduleExpiry=expiry;
  localStorage.setItem(PREFERENCES_KEY,JSON.stringify({...DEFAULT_PREFERENCES,selected:'user:bad-layout'}));manager.style.textContent='body{display:none!important}';assert(getComputedStyle(document.body).display==='none','broken CSS simulation');
  await invoke('skin_test_reset');await new Promise(r=>setTimeout(r,60));await nextPaint();assert(getComputedStyle(document.body).display!=='none'&&manager.current==='bundled:dark','native reset failed');assert(localStorage.getItem(PREFERENCES_KEY)===null,'preferences not cleared');
  return {skins:true,palettes:BUNDLED.length,skin_viewports:viewports,skin_recovery:true,skin_assets:true,skin_accessibility:true,zoom_scales:zoomScales};
 }finally{manager.reset();for(const[key,value]of saved){if(value===null)localStorage.removeItem(key);else localStorage.setItem(key,value)}}
}
