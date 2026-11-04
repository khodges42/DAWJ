// SPDX-License-Identifier: GPL-3.0-or-later
import {validateSkin,exportSkin} from './skin-format.mjs';
export const LIBRARY_KEY='dawj.skins.v1',PREFERENCES_KEY='dawj.skin-preferences.v1';
export const DEFAULT_PREFERENCES={version:1,workspace:'performance',visualization:'structure',selected:'bundled:dark',fontScale:1,enhancedContrast:false,reducedMotion:true};
export function browserStorage(){try{return globalThis.localStorage}catch{return {getItem(){throw Error('Storage unavailable')},setItem(){throw Error('Storage unavailable')},removeItem(){throw Error('Storage unavailable')}}}}
export function preferences(value){
  if(!value||value.version!==1||typeof value.selected!=='string'||value.selected.length>80||!Number.isFinite(value.fontScale)||value.fontScale<.9||value.fontScale>1.25||typeof value.enhancedContrast!=='boolean'||typeof value.reducedMotion!=='boolean')return {...DEFAULT_PREFERENCES};
  return {version:1,workspace:['browse','performance','extended'].includes(value.workspace)?value.workspace:'performance',visualization:['structure','waveform','hybrid'].includes(value.visualization)?value.visualization:'structure',selected:value.selected,fontScale:value.fontScale,enhancedContrast:value.enhancedContrast,reducedMotion:value.reducedMotion};
}
export class SkinStore{
  constructor(storage){this.storage=storage;this.warnings=[]}
  read(key){try{return this.storage.getItem(key)}catch{this.warnings.push('Skin storage unavailable; changes are temporary.');return null}}
  load(){
    let library=[],prefs={...DEFAULT_PREFERENCES};
    try{const raw=this.read(LIBRARY_KEY);if(raw){if(new TextEncoder().encode(raw).length>4*1024*1024)throw Error('Library exceeds 4 MiB');const data=JSON.parse(raw);if(data.version!==1||!Array.isArray(data.skins)||data.skins.length>12)throw Error('Unsupported skin library');library=data.skins.map(validateSkin)}}catch(e){this.warnings.push('Saved skin library rejected: '+e.message)}
    try{const raw=this.read(PREFERENCES_KEY);if(raw){if(raw.length>1024)throw Error('Preferences too large');prefs=preferences(JSON.parse(raw))}}catch{this.warnings.push('Invalid skin preferences; defaults used.')}
    return {library,prefs};
  }
  saveLibrary(skins){const data=JSON.stringify({version:1,skins:skins.map(s=>JSON.parse(exportSkin(s)))});if(skins.length>12||new TextEncoder().encode(data).length>4*1024*1024)throw Error('Skin library limit: twelve skins / 4 MiB');this.storage.setItem(LIBRARY_KEY,data)}
  savePreferences(prefs){this.storage.setItem(PREFERENCES_KEY,JSON.stringify(preferences(prefs)))}
  reset(){for(const key of [LIBRARY_KEY,PREFERENCES_KEY,'customCSS','theme','accent']){try{this.storage.removeItem(key)}catch{this.warnings.push('Could not clear skin storage; use safe mode on next launch.')}}}
}
