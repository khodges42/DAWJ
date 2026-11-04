// SPDX-License-Identifier: GPL-3.0-or-later
import {BUNDLED} from './skins/bundled.mjs';
import {validateSkin,skinStyles,exportSkin,MAX_PACKAGE_BYTES} from './skin-format.mjs';
import {SkinStore,DEFAULT_PREFERENCES,preferences} from './skin-store.mjs';
import {inspectSurface,decodeAssets,nextPaint} from './skin-safety.mjs';
export class SkinManager{
  constructor(storage,onStatus=()=>{}){
    this.store=new SkinStore(storage);this.status=onStatus;this.skins=new Map(BUNDLED.map(s=>['bundled:'+s.metadata.id,validateSkin(s)]));this.prefs={...DEFAULT_PREFERENCES};this.current='bundled:dark';this.previewing=false;this.serial=Promise.resolve();this.active=true;this.epoch=0;
    this.style=document.createElement('style');this.style.id='skin-style';this.style.nonce=document.getElementById('user-style')?.nonce??'';document.head.append(this.style);
    const host=document.createElement('dawj-skin-preview');host.id='skin-preview-host';host.setAttribute('data-skin-ui','');document.body.prepend(host);this.host=host;
    const shadow=host.attachShadow({mode:'closed'});shadow.innerHTML='<span></span><button>Keep skin</button><button>Undo preview</button>';
    const previewStyle=document.createElement('style');previewStyle.nonce=this.style.nonce;previewStyle.textContent=':host{all:initial;display:block;font:14px system-ui;color:#fff;background:#182334;padding:8px;border:2px solid #fff;box-sizing:border-box}button{font:14px system-ui;min-width:90px;min-height:34px;margin-left:12px;background:#fff;color:#111;border:1px solid #111;border-radius:3px}span{font:14px system-ui}';shadow.prepend(previewStyle);
    this.previewText=shadow.querySelector('span');const buttons=shadow.querySelectorAll('button');buttons[0].onclick=()=>this.keep();buttons[1].onclick=()=>this.cancel();this.hidePreview();
    window.__DAWJ_RESET_SKIN__=()=>this.reset();
    this.scheduleExpiry=callback=>setTimeout(callback,15000);
  }
  async initialize(mode='normal'){
    if(mode==='reset')this.store.reset();
    const saved=this.store.load();for(const skin of saved.library)this.skins.set('user:'+skin.metadata.id,skin);
    this.prefs=mode==='normal'?saved.prefs:{...DEFAULT_PREFERENCES};
    if(!this.skins.has(this.prefs.selected)){this.store.warnings.push('Selected skin missing; bundled Dark used.');this.prefs.selected='bundled:dark'}
    try{await this.paint(this.prefs.selected,this.prefs)}catch(e){this.status('Saved skin rejected: '+e.message);this.prefs={...DEFAULT_PREFERENCES};await this.paint('bundled:dark',this.prefs)}
    this.committed={...this.prefs};this.current=this.prefs.selected;
    if(this.store.warnings.length)this.status(this.store.warnings.join(' '));
    this.resize=()=>this.auditSoon();window.addEventListener('resize',this.resize);
    this.observer=new ResizeObserver(()=>this.auditSoon());for(const id of ['decks','transport','inspector'])this.observer.observe(document.getElementById(id));
  }
  async paint(id,prefs,epoch=this.epoch){
    const skin=this.skins.get(id);if(!skin)throw Error('Skin package unavailable');await decodeAssets(skin);
    if(epoch!==this.epoch)throw Error('Skin operation superseded by recovery');
    this.style.textContent=skinStyles(skin);
    if(!this.style.sheet?.cssRules.length)throw Error('Skin stylesheet blocked by CSP');
    const root=document.documentElement;root.style.setProperty('--font-scale',String(prefs.fontScale));root.dataset.enhancedContrast=String(prefs.enhancedContrast);root.dataset.reducedMotion=String(prefs.reducedMotion);
    document.body.dataset.theme=skin.mode;document.body.dataset.skin=id;
    document.dispatchEvent(new Event('dawj-skin-applied'));await nextPaint();
    if(epoch!==this.epoch)throw Error('Skin operation superseded by recovery');
    const errors=inspectSurface();if(errors.length)throw Error(errors.slice(0,3).join('; '));
    this.current=id;this.prefs={...prefs,selected:id};
  }
  preview(id,settings=this.prefs){
    const epoch=this.epoch;
    const task=async()=>{
      if(epoch!==this.epoch)return;
      clearTimeout(this.timer);this.hidePreview();
      try{await this.paint(id,preferences({...settings,version:1,selected:id}),epoch)}
      catch(e){if(epoch!==this.epoch)return;await this.restore();this.status('Preview rejected: '+e.message);throw e}
      this.previewing=true;this.host.style.display='block';this.previewText.textContent='Preview — Keep within 15 seconds, otherwise the saved skin returns.';
      this.timer=this.scheduleExpiry(()=>this.cancel());this.status('Live preview; not saved until Keep.');
    };this.serial=this.serial.catch(()=>{}).then(task);return this.serial;
  }
  setWorkspace(workspace){
    if(!['browse','performance','extended'].includes(workspace))throw Error('Unknown workspace');
    const next={...this.committed,workspace};
    this.store.savePreferences(next);this.committed=next;this.prefs={...this.prefs,workspace};
  }
  keep(){
    if(!this.previewing)return;
    try{this.prefs={...this.prefs,workspace:this.committed.workspace};const users=[...this.skins].filter(([id])=>id.startsWith('user:')).map(([,s])=>s);this.store.saveLibrary(users);this.store.savePreferences(this.prefs);this.committed={...this.prefs};this.hidePreview();this.status('Skin and accessibility settings saved.')}
    catch(e){const message='Skin was not saved: '+e.message;void this.cancel().finally(()=>this.status(message))}
  }
  async restore(){try{await this.paint(this.committed?.selected??'bundled:dark',this.committed??DEFAULT_PREFERENCES)}catch{this.style.textContent='';this.prefs={...DEFAULT_PREFERENCES};this.committed={...this.prefs};await this.paint('bundled:dark',this.prefs)}}
  async cancel(){this.epoch++;this.hidePreview();await this.restore();this.status('Preview reverted to the saved skin.')}
  hidePreview(){clearTimeout(this.timer);this.previewing=false;this.host.style.display='none'}
  reset(){
    this.skins=new Map(BUNDLED.map(s=>['bundled:'+s.metadata.id,validateSkin(s)]));
    this.fallback('Default interface restored. User skins and old CSS overrides cleared.');this.store.reset();
  }
  fallback(message){
    this.epoch++;this.hidePreview();clearTimeout(this.auditTimer);this.style.textContent='';
    document.documentElement.style.removeProperty('--font-scale');document.documentElement.dataset.enhancedContrast='false';document.documentElement.dataset.reducedMotion='true';
    this.current='bundled:dark';this.prefs={...DEFAULT_PREFERENCES};this.committed={...this.prefs};
    // Immediate safe presentation, retaining imported packages for repair unless reset was explicit.
    this.style.textContent=skinStyles(this.skins.get('bundled:dark'));document.body.dataset.theme='dark';document.body.dataset.skin='bundled:dark';
    try{this.store.savePreferences(this.prefs)}catch{/* Safe presentation does not depend on writable storage. */}
    document.dispatchEvent(new Event('dawj-skin-applied'));this.status(message);
  }
  async importText(text){const epoch=this.epoch;if(new TextEncoder().encode(text).length>MAX_PACKAGE_BYTES)throw Error('Skin package exceeds 2 MiB');const skin=validateSkin(text);await decodeAssets(skin);if(epoch!==this.epoch)throw Error('Import cancelled by interface recovery');const id='user:'+skin.metadata.id;this.skins.set(id,skin);return id}
  remove(id){if(!id.startsWith('user:'))throw Error('Bundled skins cannot be removed');if(this.current===id||this.committed.selected===id)this.fallback('Removed active skin; default interface restored.');this.skins.delete(id);this.store.saveLibrary([...this.skins].filter(([key])=>key.startsWith('user:')).map(([,skin])=>skin));this.status('User skin removed.')}
  exportCurrent(){return exportSkin(this.skins.get(this.current))}
  auditSoon(){if(!this.active)return;clearTimeout(this.auditTimer);this.auditTimer=setTimeout(()=>{
    if(!this.active)return;const errors=inspectSurface();if(errors.length&&(!this.current.startsWith('bundled:')||this.previewing)){this.fallback('Unsafe skin layout reverted: '+errors[0])}
  },180)}
  destroy(){this.active=false;this.hidePreview();clearTimeout(this.auditTimer);this.observer?.disconnect();window.removeEventListener('resize',this.resize)}
}
export class SkinPanel{
  constructor(manager){
    this.manager=manager;const $=id=>document.getElementById(id);this.$=$;
    $('skin-select').onchange=()=>this.preview();$('skin-preview').onclick=()=>this.preview();
    $('skin-keep').onclick=()=>{manager.keep();this.refresh()};$('skin-cancel').onclick=()=>void manager.cancel().then(()=>this.refresh());$('skin-reset').onclick=()=>{manager.reset();this.refresh()};
    $('skin-import').onchange=async e=>{const epoch=manager.epoch;try{const file=e.target.files[0];if(!file)return;if(file.size>MAX_PACKAGE_BYTES)throw Error('Package limit: 2 MiB');const text=await file.text();if(epoch!==manager.epoch)throw Error('Import cancelled by interface recovery');const id=await manager.importText(text);this.refresh(id);await this.preview()}catch(error){this.message(error.message)}finally{e.target.value=''}};
    $('skin-remove').onclick=()=>{try{manager.remove($('skin-select').value);this.refresh()}catch(e){this.message(e.message)}};
    $('skin-export').onclick=()=>{try{const blob=new Blob([manager.exportCurrent()],{type:'application/json'});const url=URL.createObjectURL(blob),a=document.createElement('a');a.href=url;a.download=manager.skins.get(manager.current).metadata.id+'.dawjskin.json';a.click();setTimeout(()=>URL.revokeObjectURL(url),30000)}catch(e){this.message(e.message)}};
    $('skin-preview-css').onclick=async()=>{try{
      const source=manager.skins.get($('skin-select').value)??manager.skins.get(manager.current),name=$('skin-name').value.trim();const skin=validateSkin({...source,metadata:{...source.metadata,id:'custom-'+source.metadata.id.slice(0,40),name:name||source.metadata.name+' edited'},css:$('skin-css').value});
      const id='user:'+skin.metadata.id;manager.skins.set(id,skin);this.refresh(id);await this.preview();
    }catch(e){this.message(e.message)}};
    this.refresh();
  }
  message(message){this.$('skin-status').textContent=message}
  refresh(selected=this.manager.current){const $=this.$;$('skin-select').replaceChildren();for(const[id,skin]of this.manager.skins){const option=document.createElement('option');option.value=id;option.textContent=skin.metadata.name+(id.startsWith('user:')?' (user)':'');$('skin-select').append(option)}$('skin-select').value=selected;
    const skin=this.manager.skins.get(selected);$('skin-css').value=skin?.css??'';$('skin-name').value=skin?.metadata.name??'';$('skin-font-scale').value=this.manager.prefs.fontScale;$('skin-contrast').checked=this.manager.prefs.enhancedContrast;$('skin-motion').checked=this.manager.prefs.reducedMotion;
  }
  async preview(){try{const $=this.$;await this.manager.preview($('skin-select').value,{version:1,workspace:this.manager.committed.workspace,selected:$('skin-select').value,fontScale:Number($('skin-font-scale').value),enhancedContrast:$('skin-contrast').checked,reducedMotion:$('skin-motion').checked});$('skins').close()}catch(e){this.message(e.message)}}
}
