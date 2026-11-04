// SPDX-License-Identifier: GPL-3.0-or-later
// Presentation checks only. These never submit performance commands.
function rgb(value){
  if(value==='transparent')return [0,0,0,0];const nums=value.match(/[\d.]+/g)?.map(Number)??[];
  if(value.startsWith('color(srgb'))return [...nums.slice(0,3).map(n=>n*255),nums[3]??1];
  return [...nums.slice(0,3),nums[3]??1];
}
function luminance(color){const c=color.slice(0,3).map(n=>{n/=255;return n<=.04045?n/12.92:((n+.055)/1.055)**2.4});return c[0]*.2126+c[1]*.7152+c[2]*.0722}
function textContrast(el){
  const fg=rgb(getComputedStyle(el).color);let p=el,bg=[0,0,0,0];
  if(fg[3]<.9)return 1;
  while(p&&bg[3]<.99){bg=rgb(getComputedStyle(p).backgroundColor);p=p.parentElement}
  if(bg[3]<.99)return 1;const a=luminance(fg),b=luminance(bg);return (Math.max(a,b)+.05)/(Math.min(a,b)+.05);
}
export function inspectSurface(){
  const errors=[],width=innerWidth,height=innerHeight;
  const essential=[...document.querySelectorAll('body > header button,body > header select,#transport button,#transport input,#transport select,.deck button,.deck input,.deck .state,#inspector button,#inspector input,#inspector select,#phrase-panel button,#phrase-panel input,#phrase-panel select,#mixer input,#mixer button,#mixer meter,#workspace-bar button,#visualization-control select,footer input,footer meter,#clock,#mode-status,#selected-name,#rule-status,#inspection,#phrase-position,footer output')];
  const root=document.documentElement;
  if(root.scrollWidth>width+4)errors.push('Horizontal overflow hides performance controls');
  if(root.scrollHeight>Math.max(1800,height*3))errors.push('Performance surface is excessively tall');
  const a=document.getElementById('deck-0')?.getBoundingClientRect(),b=document.getElementById('deck-1')?.getBoundingClientRect();
  if(!a||!b||Math.abs(a.top-b.top)>5||a.right>b.left+2)errors.push('Both decks must stay side by side');
  for(const el of essential){if(el.closest('[hidden]')||el.closest('details:not([open])')&&!el.closest('summary'))continue;
    if(!el.matches('button,input,select,meter')&&!el.textContent.trim())continue;
    const style=getComputedStyle(el),r=el.getBoundingClientRect(),label=el.id||el.className||el.tagName+':'+el.textContent.slice(0,40);
    if(style.display==='none'||style.visibility!=='visible'||Number(style.opacity)<.2||r.width<1||r.height<1){errors.push('Hidden control '+label);continue}
    const meter=el.tagName==='METER',control=el.matches('button,input,select');
    if(control&&(r.width<27.5||r.height<(el.type==='range'?23.5:31.5)))errors.push('Control below minimum size '+label);
    if(!meter&&parseFloat(style.fontSize)<10.9)errors.push('Text too small '+label);
    for(let p=el.parentElement;p&&p!==document.body;p=p.parentElement){const ps=getComputedStyle(p),pr=p.getBoundingClientRect();if(['hidden','clip'].includes(ps.overflowY)&&(r.top<pr.top-2||r.bottom>pr.bottom+2))errors.push('Clipped control '+label);if(ps.display==='none'||ps.visibility!=='visible'||Number(ps.opacity)<.2)errors.push('Hidden ancestor '+label)}
    if(r.left<-.5||r.right>width+.5)errors.push('Control outside horizontal viewport '+label);
    if(r.top>=0&&r.bottom<=height&&!el.disabled){const hit=document.elementFromPoint(r.left+r.width/2,r.top+r.height/2);if(hit&&hit!==el&&!el.contains(hit)&&!hit.closest('dialog[open]'))errors.push('Covered control '+label+' by '+(hit.id||hit.className||hit.tagName))}
    if(!meter&&!el.disabled&&style.color&&!el.matches('input[type=range]')&&textContrast(el)<3)errors.push('Unreadable control contrast '+label);
  }
  return [...new Set(errors)];
}
export async function decodeAssets(skin){
  for(const asset of skin.assets){const img=new Image();img.src='data:image/png;base64,'+asset.data;try{await img.decode()}catch{throw Error('Cannot decode skin asset '+asset.path)}if(img.naturalWidth>1024||img.naturalHeight>1024)throw Error('Skin asset dimensions exceed budget')}
}
export const nextPaint=()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
