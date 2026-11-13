"""Rebuild PartyLine's selected Plum asset kit from editable vector geometry."""
from pathlib import Path
import colorsys
import hashlib
import html
import io
import json
import re
import xml.etree.ElementTree as ET

import resvg_py
import uharfbuzz as hb
from fontTools.ttLib import TTFont
from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from PIL import Image, ImageCms

ROOT = Path(__file__).resolve().parents[1]
FONT = ROOT/'fonts/SourceSans3-Semibold.otf'
NS = '{http://www.w3.org/2000/svg}'
SRGB = ImageCms.ImageCmsProfile(ImageCms.createProfile('sRGB')).tobytes()
ASSETS = []

ICON = {'edge':'#2B203A','body':'#392B49','receiver':'#D7C7C0','me':'#A393B9','them':'#F3D789'}
THEMES = {
    'light': {'background':'#FFFFFF','foreground':'#271E32','primary':'#684889','primary-foreground':'#FFFFFF','speaker-me':'#4F386E','speaker-them':'#8B6418','muted':'#F2EEF6','muted-foreground':'#62556F','border':'#D8D0E1'},
    'dark': {'background':'#1B1721','foreground':'#F4EDF7','primary':'#BEA6D9','primary-foreground':'#271E32','speaker-me':'#A393B9','speaker-them':'#F3D789','muted':'#30273B','muted-foreground':'#B8AABD','border':'#51445E'},
}

PHONE_D = 'M323 296 H354 Q378 296 378 320 V354 Q378 378 354 378 H330 C317 378 309 389 305 402 C285 455 285 494 305 547 C309 560 317 570 330 570 H354 Q378 570 378 594 V628 Q378 652 354 652 H323 C275 652 242 620 225 577 C194 502 194 446 225 371 C242 328 275 296 323 296 Z'

def document(width,height,art,label,viewbox=None):
    vb=viewbox or f'0 0 {width} {height}'
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="{vb}" role="img" aria-label="{html.escape(label,quote=True)}">\n{art}\n</svg>\n'

def export(stem,width,height,art,label,viewbox=None,svg_stem=None,kind='transparent'):
    svg=document(width,height,art,label,viewbox)
    root=ET.fromstring(svg)
    assert not root.findall('.//'+NS+'text')
    assert not root.findall('.//'+NS+'image')
    svg_path=ROOT/f'{svg_stem or stem}.svg'
    svg_path.write_text(svg)
    raster=resvg_py.svg_to_bytes(svg_string=svg,width=width,height=height)
    im=Image.open(io.BytesIO(raster)).convert('RGBA')
    assert im.size==(width,height)
    if kind=='transparent': assert im.getpixel((0,0))[3]==0
    if kind=='template':
        assert im.getchannel('R').getextrema()==(0,0)
        assert im.getchannel('G').getextrema()==(0,0)
        assert im.getchannel('B').getextrema()==(0,0)
    im.save(ROOT/f'{stem}.png',icc_profile=SRGB)
    ASSETS.append({'png':f'{stem}.png','svg':svg_path.name,'width':width,'height':height,'kind':kind})
    return art

def phone(colors=ICON,monochrome=False,prefix='phone'):
    shape=f'<path d="{PHONE_D}" fill="{colors["receiver"]}"/>'
    if not monochrome:
        shape+=f'''<defs>
<clipPath id="{prefix}-upper"><rect x="160" y="250" width="280" height="140"/></clipPath>
<clipPath id="{prefix}-lower"><rect x="160" y="558" width="280" height="150"/></clipPath>
</defs>
<path d="{PHONE_D}" fill="{colors['me']}" clip-path="url(#{prefix}-upper)"/>
<path d="{PHONE_D}" fill="{colors['them']}" clip-path="url(#{prefix}-lower)"/>'''
    return f'<g transform="translate(-5 0)"><g transform="matrix(1 0 0 1.2122905028 17 -50.7150837989)"><g transform="rotate(-12 292 474)">{shape}</g></g></g>'

def blocks(colors=ICON):
    return f'''<rect x="485" y="296" width="328" height="82" rx="26" fill="{colors['me']}"/>
<rect x="485" y="412" width="224" height="44" rx="22" fill="{colors['me']}"/>
<rect x="485" y="570" width="328" height="82" rx="26" fill="{colors['them']}"/>
<rect x="485" y="686" width="224" height="44" rx="22" fill="{colors['them']}"/>'''

def mark_colors(mode):
    if mode=='dark': return ICON
    return {'receiver':'#392B49','me':THEMES['light']['speaker-me'],'them':THEMES['light']['speaker-them']}

def mark(mode='dark',prefix='mark'):
    c=mark_colors(mode)
    return phone(c,prefix=prefix)+blocks(c)

def fit_mark(x,y,width,mode='dark',prefix='mark'):
    scale=width/602
    return f'<g transform="matrix({scale:.10f} 0 0 {scale:.10f} {x-211*scale:.10f} {y-296*scale:.10f})">{mark(mode,prefix)}</g>'

def fit_phone(x,y,width,height,colors=ICON,monochrome=False,prefix='phone'):
    sx=width/210; sy=height/434
    return f'<g transform="matrix({sx:.10f} 0 0 {sy:.10f} {x-211*sx:.10f} {y-296*sy:.10f})">{phone(colors,monochrome,prefix)}</g>'

font_data=FONT.read_bytes()
font=TTFont(FONT)
glyphs=font.getGlyphSet()
glyph_order=font.getGlyphOrder()
upem=font['head'].unitsPerEm
hb_font=hb.Font(hb.Face(font_data)); hb_font.scale=(upem,upem)

def lettering(text,size,color,tracking=0):
    buf=hb.Buffer();buf.add_str(text);buf.guess_segment_properties()
    hb.shape(hb_font,buf,{'kern':True})
    scale=size/upem; cursor=0;paths=[];bounds=[]
    for info,pos in zip(buf.glyph_infos,buf.glyph_positions):
        transform=(scale,0,0,-scale,cursor+pos.x_offset*scale,-pos.y_offset*scale)
        pen=SVGPathPen(glyphs)
        glyphs[glyph_order[info.codepoint]].draw(TransformPen(pen,transform))
        d=pen.getCommands()
        if d: paths.append(f'<path d="{d}" fill="{color}"/>')
        bp=BoundsPen(glyphs)
        glyphs[glyph_order[info.codepoint]].draw(TransformPen(bp,transform))
        if bp.bounds: bounds.append(bp.bounds)
        cursor+=pos.x_advance*scale+tracking
    bbox=(min(b[0] for b in bounds),min(b[1] for b in bounds),max(b[2] for b in bounds),max(b[3] for b in bounds))
    return '\n'.join(paths),bbox

def text_at(text,size,x,baseline,color='#271E32',tracking=0):
    art,_=lettering(text,size,color,tracking)
    return f'<g transform="translate({x} {baseline})">{art}</g>'

def centered_text(text,size,cx,cy,color):
    art,b=lettering(text,size,color)
    return f'<g transform="translate({cx-(b[0]+b[2])/2:.4f} {cy-(b[1]+b[3])/2:.4f})">{art}</g>'

def wordmark(mode='light'):
    color='#392B49' if mode=='light' else '#F4EDF7'
    letters,b=lettering('PartyLine',138,color,tracking=-1.0)
    text_width=b[2]-b[0]; mark_width=184; gap=34
    left=(845-mark_width-gap-text_width)/2
    mark_height=mark_width*434/602
    art=fit_mark(left,(295-mark_height)/2,mark_width,mode,'word-'+mode)
    tx=left+mark_width+gap-b[0]
    ty=147.5-(b[1]+b[3])/2
    return art+f'<g transform="translate({tx:.5f} {ty:.5f})">{letters}</g>'

def luminance(color):
    values=[int(color[i:i+2],16)/255 for i in (1,3,5)]
    values=[v/12.92 if v<=.04045 else ((v+.055)/1.055)**2.4 for v in values]
    return sum(v*w for v,w in zip(values,(.2126,.7152,.0722)))

def contrast(a,b):
    lo,hi=sorted([luminance(a),luminance(b)])
    return (hi+.05)/(lo+.05)

def hsl(color):
    rgb=[int(color[i:i+2],16)/255 for i in (1,3,5)]
    h,l,s=colorsys.rgb_to_hls(*rgb)
    fmt=lambda n:f'{n:.2f}'.rstrip('0').rstrip('.')
    return f'{fmt(h*360)} {fmt(s*100)}% {fmt(l*100)}%'

# Large icon: concentric 24 px edge and optically centered artwork.
app_art=f'<rect x="100" y="100" width="824" height="824" rx="185" fill="{ICON["edge"]}"/><rect x="124" y="124" width="776" height="776" rx="161" fill="{ICON["body"]}"/>'+mark()
export('app-icon-1024',1024,1024,app_art,'PartyLine app icon',svg_stem='app-icon')
export('app-icon-128',128,128,app_art,'PartyLine app icon at 128 px',viewbox='0 0 1024 1024')

# Small variants align the bars to the output pixel grid.
small32=f'<rect x="3" y="3" width="26" height="26" rx="5.85" fill="{ICON["edge"]}"/><rect x="3.75" y="3.75" width="24.5" height="24.5" rx="5.1" fill="{ICON["body"]}"/>'
small32+=f'<g transform="scale(0.03125)">{phone(prefix="small32")}</g>'
for y,w,h,color in ((9.25,10,2.75,ICON['me']),(13,7,1.5,ICON['me']),(18,10,2.75,ICON['them']),(21.5,7,1.5,ICON['them'])):
    small32+=f'<rect x="15.5" y="{y}" width="{w}" height="{h}" rx="{min(.8,h/2)}" fill="{color}"/>'
export('app-icon-32',32,32,small32,'PartyLine icon tuned for 32 px')
small16=f'<rect x="1.5" y="1.5" width="13" height="13" rx="2.93" fill="{ICON["edge"]}"/><rect x="2" y="2" width="12" height="12" rx="2.43" fill="{ICON["body"]}"/>'
small16+=fit_phone(3.25,4.5,3.25,7.5,prefix='small16')
for y,w,h,color in ((4.5,4.75,1.5,ICON['me']),(6.5,3.25,1,ICON['me']),(9,4.75,1.5,ICON['them']),(11,3.25,1,ICON['them'])):
    small16+=f'<rect x="7.75" y="{y}" width="{w}" height="{h}" rx="{min(.5,h/2)}" fill="{color}"/>'
export('app-icon-16',16,16,small16,'PartyLine icon tuned for 16 px')

for mode,suffix in (('light',''),('dark','-dark')):
    side_art=fit_mark(40,(500-420*434/602)/2,420,mode,'sidebar'+suffix)
    export('logo-collapsed'+suffix,500,500,side_art,'PartyLine sidebar mark for '+mode+' backgrounds')
    export('logo'+suffix,845,295,wordmark(mode),'PartyLine wordmark for '+mode+' backgrounds')

# One receiver silhouette; recording adds one solid dot.
tray=fit_phone(4.75,2.5,10.5,17,{'receiver':'#000000'},True)
recording=tray+'<circle cx="18.5" cy="4.5" r="2" fill="#000000"/>'
for stem,art in (('tray-template',tray),('tray-recording-template',recording)):
    export(stem,22,22,art,'PartyLine '+stem,kind='template')
    export(stem+'@2x',44,44,art,'PartyLine '+stem+' at 2x',viewbox='0 0 22 22',kind='template')

# Finder supplies the two icons; these rectangles remain blank around them.
installer='<rect width="660" height="400" fill="#F4F0F7"/>'
installer+='<g transform="translate(201 18) scale(0.3053254438)">'+wordmark('light')+'</g>'
installer+='<path d="M300 215 H360 M349 204 L360 215 L349 226" fill="none" stroke="#857493" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>'
installer+=centered_text('Drag PartyLine to Applications.',20,330,350,'#4F386E')
export('dmg-background',660,400,installer,'PartyLine installer background',kind='background')
export('dmg-background@2x',1320,800,installer,'PartyLine installer background at 2x',viewbox='0 0 660 400',kind='background')

# Theme tokens and contrast evidence.
checks={}
for mode,palette in THEMES.items():
    checks[mode]={role:round(contrast(palette[role],palette['background']),3) for role in ('foreground','speaker-me','speaker-them','muted-foreground')}
    assert min(checks[mode].values())>=4.5
    checks[mode]['primary-button']=round(contrast(palette['primary'],palette['primary-foreground']),3)
    checks[mode]['muted-text']=round(contrast(palette['muted'],palette['muted-foreground']),3)
    checks[mode]['speaker-grayscale-separation']=round(contrast(palette['speaker-me'],palette['speaker-them']),3)
    assert checks[mode]['primary-button']>=4.5 and checks[mode]['muted-text']>=4.5

rows=['# PartyLine palette','','The selected icon uses Plum, lavender for ME, and pale gold for THEM. The light interface uses darker speaker colors for readable text.','','| Token | Light hex | Light HSL | Dark hex | Dark HSL |','|---|---|---|---|---|']
for role in THEMES['light']:
    label={'primary':'Brand / primary','speaker-me':'ME','speaker-them':'THEM'}.get(role,role)
    a=THEMES['light'][role];b=THEMES['dark'][role]
    rows.append(f'| {label} | `{a}` | `{hsl(a)}` | `{b}` | `{hsl(b)}` |')
rows+=['','## Text contrast','','Ratios below use the exact hex values above. Light text backgrounds are white; dark text backgrounds are `#1B1721`.','','| Pair | Light | Dark |','|---|---:|---:|']
for role in ('speaker-me','speaker-them','foreground','muted-text','primary-button'):
    rows.append(f"| {role} | {checks['light'][role]:.2f}:1 | {checks['dark'][role]:.2f}:1 |")
rows+=['','ME and THEM also differ in brightness. Keep the `ME:` and `THEM:` labels so speaker identity does not rely on color alone.','','## Icon colors','','| Role | Hex | HSL |','|---|---|---|']
for role,color in ICON.items(): rows.append(f'| {role} | `{color}` | `{hsl(color)}` |')
rows+=['','`palette.css` contains matching shadcn-style HSL variables. `logo.*` and `logo-collapsed.*` use the light-background colors; their `-dark` versions use the selected icon colors.','']
(ROOT/'palette.md').write_text('\n'.join(rows))
css=[]
for mode,p in THEMES.items():
    css.append(':root {' if mode=='light' else '.dark {')
    values={**p,'card':p['background'],'card-foreground':p['foreground'],'popover':p['background'],'popover-foreground':p['foreground'],'secondary':p['muted'],'secondary-foreground':p['foreground'],'accent':p['muted'],'accent-foreground':p['foreground'],'input':p['border'],'ring':p['primary']}
    css.extend(f'  --{role}: {hsl(color)};' for role,color in values.items())
    css+=['}','']
(ROOT/'palette.css').write_text('\n'.join(css))
(ROOT/'palette.json').write_text(json.dumps({'light':THEMES['light'],'dark':THEMES['dark'],'icon':ICON},indent=2)+'\n')

# Simulate the speaker colors in linear RGB with the published Machado matrices.
cvd_source=json.loads((ROOT/'tools/cvd-matrices.json').read_text())
def simulate(color,condition):
    if condition=='normal': return color
    rgb=[int(color[i:i+2],16)/255 for i in (1,3,5)]
    linear=[v/12.92 if v<=.04045 else ((v+.055)/1.055)**2.4 for v in rgb]
    if condition=='grayscale':
        transformed=[sum(v*w for v,w in zip(linear,(.2126,.7152,.0722)))]*3
    else:
        transformed=[sum(v*w for v,w in zip(linear,row)) for row in cvd_source['matrices'][condition]]
    transformed=[min(1,max(0,v)) for v in transformed]
    encoded=[v*12.92 if v<=.0031308 else 1.055*v**(1/2.4)-.055 for v in transformed]
    return '#'+''.join(f'{round(v*255):02X}' for v in encoded)

color_checks='<rect width="1000" height="510" fill="#FAF8FC"/>'+text_at('Speaker colors',26,30,48)
color_checks+=text_at('Light',18,310,85,'#62556F')+text_at('Dark',18,650,85,'#62556F')
simulations={}
conditions=(('normal','Original'),('grayscale','Grayscale'),('protanomaly','Protanopia simulation'),('deuteranomaly','Deuteranopia simulation'))
for index,(condition,label) in enumerate(conditions):
    y=105+index*96
    color_checks+=text_at(label,18,30,y+43,'#62556F')
    simulations[condition]={}
    for mode,x in (('light',280),('dark',620)):
        p=THEMES[mode]
        bg=simulate(p['background'],condition)
        me=simulate(p['speaker-me'],condition);them=simulate(p['speaker-them'],condition)
        color_checks+=f'<rect x="{x}" y="{y}" width="310" height="74" rx="8" fill="{bg}"/>'
        color_checks+=text_at('ME:',26,x+30,y+46,me)+text_at('THEM:',26,x+155,y+46,them)
        simulations[condition][mode]={'background':bg,'me':me,'them':them,'me_contrast':round(contrast(me,bg),3),'them_contrast':round(contrast(them,bg),3),'speaker_luminance_ratio':round(contrast(me,them),3)}
        assert min(contrast(me,bg),contrast(them,bg))>=4.5
        assert contrast(me,them)>=1.5
export('color-checks',1000,510,color_checks,'PartyLine speaker color simulations',kind='background')
(ROOT/'color-checks.json').write_text(json.dumps({'source':cvd_source['source'],'model':cvd_source['model'],'conditions':simulations},indent=2)+'\n')

# A portable overview. All lettering remains outlined in the SVG source.
def embed(stem,x,y,width,height,prefix,white=False):
    src=(ROOT/(stem+'.svg')).read_text()
    node=ET.fromstring(src)
    art=''.join(ET.tostring(child,encoding='unicode') for child in node)
    for identifier in re.findall(r'id="([^"]+)"',art):
        art=art.replace(f'id="{identifier}"',f'id="{prefix}-{identifier}"').replace(f'url(#{identifier})',f'url(#{prefix}-{identifier})').replace(f'href="#{identifier}"',f'href="#{prefix}-{identifier}"')
    if white: art=art.replace('#000000','#FFFFFF')
    return f'<svg x="{x}" y="{y}" width="{width}" height="{height}" viewBox="{node.get("viewBox")}">{art}</svg>'

preview='<rect width="1400" height="1280" fill="#FAF8FC"/>'
preview+=embed('logo',35,18,490,171,'header')
preview+=text_at('Plum',24,1145,112,'#684889')
preview+=text_at('App icon',22,65,220)
preview+=embed('app-icon',55,235,360,360,'master')
preview+=embed('app-icon-128',65,610,128,128,'128')
preview+=embed('app-icon-32',240,656,32,32,'32')
preview+=embed('app-icon-16',333,664,16,16,'16')
preview+=text_at('128 px',16,103,760,'#62556F')+text_at('32 px',16,235,718,'#62556F')+text_at('16 px',16,322,718,'#62556F')
preview+=text_at('Light',20,500,220,'#62556F')
preview+='<rect x="490" y="239" width="845" height="190" fill="#FFFFFF"/>'
preview+=embed('logo-collapsed',535,313,40,40,'side-light')
preview+=embed('logo',645,246,520,181.54,'word-light')
preview+=text_at('Dark',20,500,470,'#62556F')
preview+='<rect x="490" y="489" width="845" height="190" fill="#1B1721"/>'
preview+=embed('logo-collapsed-dark',535,563,40,40,'side-dark')
preview+=embed('logo-dark',645,496,520,181.54,'word-dark')
preview+=text_at('Menu bar · idle / recording',22,65,830)
preview+=embed('tray-template',105,863,22,22,'tray-light')+embed('tray-recording-template',185,863,22,22,'rec-light')
preview+='<rect x="65" y="917" width="350" height="65" rx="12" fill="#1B1721"/>'
preview+=embed('tray-template',105,938,22,22,'tray-dark',True)+embed('tray-recording-template',185,938,22,22,'rec-dark',True)
preview+=text_at('22 px',16,286,879,'#62556F')+text_at('22 px',16,286,955,'#D7C7C0')
preview+=text_at('ME',19,65,1062,'#4F386E')+text_at('THEM',19,225,1062,'#8B6418')
preview+='<rect x="65" y="1080" width="120" height="42" rx="8" fill="#A393B9"/><rect x="225" y="1080" width="120" height="42" rx="8" fill="#F3D789"/>'
preview+=text_at('Installer',22,500,774)
preview+=embed('dmg-background',500,798,660,400,'installer')
export('preview',1400,1280,preview,'PartyLine Plum asset overview',kind='background')

font_info={'family':font['name'].getDebugName(1),'style':font['name'].getDebugName(2),'version':font['name'].getDebugName(5),'license':'SIL Open Font License 1.1','license_url':'https://github.com/adobe-fonts/source-sans/blob/release/LICENSE.md','sha256':hashlib.sha256(font_data).hexdigest()}
validation={'assets':len(ASSETS),'all_pngs':'RGBA, embedded sRGB profile','all_svg_type':'outlined paths; no text elements','master_alpha_bounds':[100,100,924,924],'small_icons':'16 px and 32 px use separate pixel-tuned sources','light_sidebar':'500 px, rendered for review at 40 px','tray':'black RGB only; antialiasing stored in alpha','installer_clear_zones':[[105,125,255,300],[405,125,555,300]],'contrast':checks,'color_simulations':'color-checks.json','font':font_info}
for item in ASSETS:
    p=ROOT/item['png'];im=Image.open(p)
    assert im.mode=='RGBA' and im.size==(item['width'],item['height']) and im.info.get('icc_profile')
    item['png_sha256']=hashlib.sha256(p.read_bytes()).hexdigest()
    item['svg_sha256']=hashlib.sha256((ROOT/item['svg']).read_bytes()).hexdigest()
assert Image.open(ROOT/'app-icon-1024.png').getchannel('A').getbbox()==(100,100,924,924)
installer_im=Image.open(ROOT/'dmg-background.png')
for box in validation['installer_clear_zones']:
    assert len(installer_im.crop(box).getcolors(100000))==1
(ROOT/'asset-manifest.json').write_text(json.dumps({'assets':ASSETS,'validation':validation},indent=2)+'\n')
print(json.dumps({'assets':len(ASSETS),'font':font_info,'contrast':checks,'output':str(ROOT)},indent=2))
