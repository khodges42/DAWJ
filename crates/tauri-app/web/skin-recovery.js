// SPDX-License-Identifier: GPL-3.0-or-later
// Fixed native-menu script. Never concatenate skin content into this code.
(()=>{
  for(const key of ['dawj.skins.v1','dawj.skin-preferences.v1','customCSS','theme','accent']){
    try{localStorage.removeItem(key)}catch{/* Recovery also works when storage is unavailable. */}
  }
  if(typeof window.__DAWJ_RESET_SKIN__==='function')window.__DAWJ_RESET_SKIN__();
  else{
    document.getElementById('skin-style')?.remove();
    document.getElementById('user-style')?.remove();
    location.reload();
  }
})();
