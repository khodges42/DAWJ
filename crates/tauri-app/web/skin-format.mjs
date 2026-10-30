// SPDX-License-Identifier: GPL-3.0-or-later
// Portable data only. No DOM, native APIs, executable imports or CSS extension points.
export const FORMAT_VERSION=1,SURFACE_VERSION=1,MAX_PACKAGE_BYTES=2*1024*1024;
export const COLOR_KEYS=['background','surface','raised','primary','secondary','tertiary','text','muted','deckA','deckB','onPrimary','border','focus','meterActive','meterWarn','meterClip','grid'];
export const TOKEN_DEFAULTS={background:'#171A20',surface:'#242A34',raised:'#313B49',primary:'#73C5DF',secondary:'#E2B67E',tertiary:'#B7A7ED',text:'#EDF2F9',muted:'#B8C2D1',deckA:'#73C5DF',deckB:'#E2B67E',onPrimary:'#141414',border:'#68788B',focus:'#EDF2F9',meterActive:'#64D998',meterWarn:'#F4C35D',meterClip:'#FF7F7F',grid:'#718096',fontFamily:'system-ui, sans-serif',fontSize:14,spacing:8,controlHeight:32,controlRadius:4};
export const VARIABLE_MAP=Object.fromEntries(Object.keys(TOKEN_DEFAULTS).map(k=>[k,'--'+(COLOR_KEYS.includes(k)?'color-':'')+k.replace(/[A-Z]/g,c=>'-'+c.toLowerCase())]));
const fonts=new Set(['system-ui','sans-serif','serif','monospace','ui-sans-serif','ui-monospace','Segoe UI','Arial','Verdana','Tahoma','Trebuchet MS','Georgia','Times New Roman','Courier New','Inter','Roboto','Noto Sans','DejaVu Sans']);
const numeric={fontSize:[12,18],spacing:[4,12],controlHeight:[32,44],controlRadius:[0,12]};
const properties=new Set(['color','background-color','background-image','background-repeat','background-size','background-position','border-color','border-width','border-style','border-radius','border-left-color','border-right-color','border-top-color','border-bottom-color','box-shadow','outline-color','font-size','font-weight','line-height','letter-spacing','text-align','padding','padding-top','padding-right','padding-bottom','padding-left','margin','margin-top','margin-right','margin-bottom','margin-left','gap','row-gap','column-gap','display','grid-template-columns','grid-template-rows','align-items','justify-content','width','height','min-width','max-width','min-height','max-height']);
const paint=new Set(['color','background-color','background-image','background-repeat','background-size','background-position','border-color','border-left-color','border-right-color','border-top-color','border-bottom-color','outline-color','box-shadow']);
const ids=new Set(['decks','deck-0','deck-1','transport','clock','modes','mode-status','phrase-panel','timeline','phrase-position','inspector','selected-name','inspection','rule-status','cue-status','notices','notice','midi-badge','error','settings','help','mapping-list','profile-status','midi-status','led-status','led-list','led-values']);
const classes=new Set(['deck','slots','identity','state','actions','context','bar','meters','truncate','mapping-row','led-row']);
const tags=new Set(['body','header','footer','main','section','article','button','input','select','option','label','h1','h2','small','p','div','span','output','meter','canvas','dialog','fieldset','legend','details','summary']);
function fail(message){throw Error('Skin: '+message)}
function object(value,keys,what){if(!value||typeof value!=='object'||Array.isArray(value))fail(what+' must be an object');for(const key of Object.keys(value))if(!keys.includes(key))fail('unsupported '+what+' field '+key)}
function text(value,max,what,required=true){if(typeof value!=='string'||value.length>max||(required&&!value.trim())||/[\x00-\x08\x0B\x0C\x0E-\x1F]/.test(value))fail('invalid '+what);return value}
export function contrast(a,b){
  const light=h=>{const rgb=h.slice(1).match(/../g).map(n=>parseInt(n,16)/255).map(n=>n<=.04045?n/12.92:((n+.055)/1.055)**2.4);return rgb[0]*.2126+rgb[1]*.7152+rgb[2]*.0722};const x=light(a),y=light(b);return (Math.max(x,y)+.05)/(Math.min(x,y)+.05);
}
function crc32(bytes){let c=0xffffffff;for(const b of bytes){c^=b;for(let n=0;n<8;n++)c=(c>>>1)^((c&1)?0xedb88320:0)}return (c^0xffffffff)>>>0}
export function pngBytes(data){
  if(typeof data!=='string'||data.length>350000||!data.length||data.length%4||!/^[A-Za-z0-9+/]+={0,2}$/.test(data))fail('invalid or oversized PNG base64');
  const bytes=Uint8Array.from(atob(data),c=>c.charCodeAt(0));if(bytes.length>256*1024||bytes.length<57)fail('PNG must be 57 bytes–256 KiB');
  const sig=[137,80,78,71,13,10,26,10];if(sig.some((b,i)=>bytes[i]!==b))fail('asset is not PNG');
  const v=new DataView(bytes.buffer);let offset=8,first=true,idat=false,ended=false,count=0;
  while(offset+12<=bytes.length){
    if(++count>128)fail('too many PNG chunks');const len=v.getUint32(offset),end=offset+12+len;if(end>bytes.length)fail('truncated PNG');
    const type=String.fromCharCode(...bytes.slice(offset+4,offset+8));
    if(crc32(bytes.slice(offset+4,end-4))!==v.getUint32(end-4))fail('PNG checksum mismatch');
    if(first){if(type!=='IHDR'||len!==13)fail('PNG header missing');const w=v.getUint32(offset+8),h=v.getUint32(offset+12);if(!w||!h||w>1024||h>1024||w*h>1048576)fail('PNG dimensions exceed 1024×1024');first=false}
    if(['acTL','fcTL','fdAT'].includes(type))fail('animated images are not supported');
    if(type==='IDAT')idat=true;
    if(type==='IEND'){if(len||end!==bytes.length)fail('invalid PNG end');ended=true;break}
    offset=end;
  }
  if(!ended||!idat)fail('PNG image data/end missing');return bytes;
}
function selectorAllowed(selector){
  if(!selector||selector.length>220||!/^[a-zA-Z0-9_#.\s>+~,\[\]="':-]+$/.test(selector))fail('unsupported CSS selector');
  let s=selector.replace(/\[(data-active|data-selected|aria-pressed)=(?:"(?:true|false)"|'(?:true|false)'|true|false)\]/g,'');
  if(/[\[\]]/.test(s))fail('unsupported CSS attribute selector');
  s=s.replace(/:(hover|focus-visible|focus|disabled|active)\b/g,'');if(s.includes(':'))fail('unsupported CSS pseudo selector');
  s=s.replace(/([.#])([a-zA-Z0-9_-]+)/g,(_,prefix,name)=>{if(!(prefix==='#'?ids:classes).has(name))fail('undocumented CSS selector '+prefix+name);return ''});
  for(const tag of s.split(/[\s>+~,]+/).filter(Boolean))if(!tags.has(tag))fail('undocumented CSS tag '+tag);
}
function declarations(body,selector,assets){
  const out=[];
  for(const declaration of body.split(';')){
    if(!declaration.trim())continue;const colon=declaration.indexOf(':');if(colon<0)fail('invalid CSS declaration');
    const prop=declaration.slice(0,colon).trim().toLowerCase();let value=declaration.slice(colon+1).trim();
    if(!properties.has(prop)||!value||value.length>300)fail('unsupported CSS property/value '+prop);
    if(prop==='box-shadow')fail('Paint effects outside component bounds are not supported');
    if((selector.split(',').some(s=>s.trim()==='body')||/[:\[]/.test(selector))&&!paint.has(prop))fail('global/state CSS may change paint only');
    if(prop==='background-image'){
      if(value==='none'){out.push(prop+':none');continue}
      const m=/^url\(["']?asset:([A-Za-z0-9_/-]+\.png)["']?\)$/.exec(value);if(!m||!assets.has(m[1]))fail('background-image must reference a packaged PNG with asset:path');
      out.push(prop+':url("data:image/png;base64,'+assets.get(m[1])+'")');continue;
    }
    if(!/^[A-Za-z0-9#%.,()\s_+*/-]+$/.test(value)||/url|expression|image-set|attr\s*\(|env\s*\(/i.test(value))fail('unsafe CSS value');
    if((prop==='color'||prop.endsWith('-color'))&&!/^(#[a-f0-9]{6}|var\(--color-[a-z-]+\))$/i.test(value))fail('CSS colors must be opaque hex or semantic color variables');
    for(const m of value.matchAll(/var\(\s*(--[a-z0-9-]+)/gi))if(!Object.values(VARIABLE_MAP).includes(m[1]))fail('unknown CSS variable '+m[1]);
    if(prop==='display'&&!['block','inline-block','flex','inline-flex','grid'].includes(value))fail('unsupported display value');
    if(prop==='display'&&!selector.split(',').every(s=>/(?:\.deck|\.slots|\.actions|\.bar|\.mapping-row|\.led-row|#decks|#inspector|#phrase-panel|article)$/.test(s.trim())))fail('Display changes are limited to documented layout containers');
    out.push(prop+':'+value);
  }
  return out.join(';');
}
export function compileCss(input,assets=new Map()){
  text(input,32768,'CSS',false);
  if(/\\|[\x00-\x08\x0B\x0C\x0E-\x1F]/.test(input))fail('CSS escapes/control characters are not permitted');
  const source=input.replace(/\/\*[\s\S]*?\*\//g,'');if(source.includes('/*')||source.includes('*/')||source.includes('!'))fail('invalid comments or !important');
  let offset=0,rules=0;
  const block=(nested=false)=>{
    let out='';
    while(offset<source.length){
      while(/\s/.test(source[offset]??'')&&offset<source.length)offset++;
      if(offset>=source.length){if(nested)fail('unclosed media block');return out}
      if(source[offset]==='}'){if(!nested)fail('unexpected CSS close');offset++;return out}
      const open=source.indexOf('{',offset);if(open<0)fail('CSS rule missing block');const head=source.slice(offset,open).trim();offset=open+1;
      if(++rules>96)fail('too many CSS rules');
      if(head.startsWith('@')){
        if(nested||!/^@media\s+\((min|max)-(width|height):\s*(?:[5-9]\d\d|1\d{3}|2[0-5]\d\d)px\)$/.test(head))fail('only bounded width/height @media is permitted');
        out+=head+'{'+block(true)+'}';continue;
      }
      selectorAllowed(head);const end=source.indexOf('}',offset);if(end<0||source.slice(offset,end).includes('{'))fail('invalid nested CSS');
      out+=head+'{'+declarations(source.slice(offset,end),head,assets)+'}';offset=end+1;
    }
    if(nested)fail('unclosed media block');return out;
  };
  return block();
}
export function validateSkin(input){
  if(typeof input==='string'){if(new TextEncoder().encode(input).length>MAX_PACKAGE_BYTES)fail('package exceeds 2 MiB');try{input=JSON.parse(input)}catch{fail('invalid JSON')}}
  object(input,['schemaVersion','surfaceVersion','metadata','mode','variables','css','assets'],'package');
  if(input.schemaVersion!==FORMAT_VERSION||input.surfaceVersion!==SURFACE_VERSION)fail('unsupported package or surface version');
  const meta=input.metadata;object(meta,['id','name','author','version','description','license'],'metadata');
  if(!/^[a-z0-9][a-z0-9-]{0,47}$/.test(text(meta.id,48,'id')))fail('invalid id');
  text(meta.name,80,'name');if(!/^\d+\.\d+\.\d+(?:-[a-z0-9.-]+)?$/.test(text(meta.version,32,'version')))fail('invalid metadata version');
  for(const[k,max]of [['author',120],['description',512],['license',120]])if(meta[k]!==undefined)text(meta[k],max,k,false);
  if(!['dark','light','high-contrast'].includes(input.mode))fail('invalid mode');
  object(input.variables,Object.keys(TOKEN_DEFAULTS),'variables');
  const variables={...TOKEN_DEFAULTS,...input.variables};
  for(const key of COLOR_KEYS)if(typeof variables[key]!=='string'||!/^#[0-9a-f]{6}$/i.test(variables[key]))fail('invalid color '+key);
  for(const[key,[min,max]]of Object.entries(numeric))if(!Number.isFinite(variables[key])||variables[key]<min||variables[key]>max)fail('out-of-range variable '+key);
  text(variables.fontFamily,200,'font family');const families=variables.fontFamily.split(',').map(n=>n.trim());
  if(families.some(n=>!fonts.has(n))||!families.some(n=>['system-ui','sans-serif','serif','monospace'].includes(n)))fail('use documented local font families with a generic fallback');
  for(const bg of ['background','surface','raised'])if(contrast(variables.text,variables[bg])<4.5)fail('text contrast below 4.5 on '+bg);
  if(!Array.isArray(input.assets??[])||(input.assets??[]).length>4)fail('at most four PNG assets');
  const assets=new Map();
  for(const asset of input.assets??[]){object(asset,['path','mime','data'],'asset');
    if(typeof asset.path!=='string'||asset.path.length>100||! /^(?:[A-Za-z0-9_-]+\/)*[A-Za-z0-9_-]+\.png$/.test(asset.path)||asset.mime!=='image/png'||assets.has(asset.path))fail('invalid/duplicate local PNG path');
    pngBytes(asset.data);assets.set(asset.path,asset.data);
  }
  const css=input.css??'';compileCss(css,assets);
  const skin={schemaVersion:FORMAT_VERSION,surfaceVersion:SURFACE_VERSION,metadata:{...meta},mode:input.mode,variables,css,assets:[...assets].map(([path,data])=>({path,mime:'image/png',data}))};
  if(new TextEncoder().encode(JSON.stringify(skin)).length>MAX_PACKAGE_BYTES)fail('package exceeds 2 MiB');return skin;
}
export function exportSkin(skin){return JSON.stringify(validateSkin(skin),null,2)+'\n'}
export function skinStyles(skin){
  skin=validateSkin(skin);const variables=Object.entries(skin.variables).map(([key,value])=>VARIABLE_MAP[key]+':'+(numeric[key]&&key!=='controlRadius'?value+'px':key==='controlRadius'?value+'px':value)).join(';');
  return ':root{'+variables+'}\n'+compileCss(skin.css,new Map(skin.assets.map(a=>[a.path,a.data])));
}
