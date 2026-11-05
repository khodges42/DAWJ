// SPDX-License-Identifier: GPL-3.0-or-later
// Fixed native-menu script. Never concatenate skin content into this code.
(()=>{
  let audio=null;try{const value=JSON.parse(localStorage.getItem('dawj.skin-preferences.v1'));if(typeof value?.audio?.device==='string'&&value.audio.device.length<=300)audio={device:value.audio.device,cue:value.audio.cue===true}}catch{}
  for(const key of ['dawj.skins.v1','dawj.skin-preferences.v1','customCSS','theme','accent']){
    try{localStorage.removeItem(key)}catch{/* Recovery also works when storage is unavailable. */}
  }
  if(audio)try{localStorage.setItem('dawj.skin-preferences.v1',JSON.stringify({version:1,workspace:'performance',visualization:'structure',audio,selected:'bundled:dark',fontScale:1,enhancedContrast:false,reducedMotion:true}))}catch{}
  if(typeof window.__DAWJ_RESET_SKIN__==='function')window.__DAWJ_RESET_SKIN__();
  else{
    document.getElementById('skin-style')?.remove();
    document.getElementById('user-style')?.remove();
    location.reload();
  }
})();
