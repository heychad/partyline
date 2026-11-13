"""Copy the selected assets and regenerate PartyLine's platform icons."""
from pathlib import Path
import shutil, struct, subprocess, tempfile
import resvg_py
from PIL import Image, ImageCms
import io

brand = Path(__file__).resolve().parents[1]
frontend = brand.parent/'frontend'
icons = frontend/'src-tauri/icons'
profile = ImageCms.ImageCmsProfile(ImageCms.createProfile('sRGB')).tobytes()

def png(size, logical_size=None):
    logical_size = logical_size or size
    source = brand/f'app-icon-{logical_size}.svg' if logical_size in (16,32) else brand/'app-icon.svg'
    im = Image.open(io.BytesIO(resvg_py.svg_to_bytes(svg_path=str(source),width=size,height=size))).convert('RGBA')
    result=io.BytesIO(); im.save(result,format='PNG',icc_profile=profile)
    return result.getvalue()

for file in icons.glob('*.png'):
    if file.name.startswith(('tray-', 'dmg-background')):
        continue
    size=Image.open(file).width
    logical = int(file.stem.split('_')[1].split('x')[0]) if file.stem.startswith('icon_') else size
    file.write_bytes(png(size,logical))
(icons/'icon.png').write_bytes(png(1024))

with tempfile.TemporaryDirectory(prefix='partyline-icons-') as temp:
    iconset=Path(temp)/'PartyLine.iconset';iconset.mkdir()
    for size in (16,32,128,256,512):
        for factor in (1,2):
            name=f'icon_{size}x{size}'+('@2x' if factor==2 else '')+'.png'
            (iconset/name).write_bytes(png(size*factor,size))
    subprocess.run(['iconutil','-c','icns',str(iconset),'-o',str(icons/'app_icon.icns')],check=True)
shutil.copyfile(icons/'app_icon.icns',icons/'icon.icns')

def ico(path,sizes):
    payloads=[png(size) for size in sizes]
    offset=6+16*len(sizes);entries=[]
    for size,data in zip(sizes,payloads):
        entries.append(struct.pack('<BBBBHHII',size if size<256 else 0,size if size<256 else 0,0,0,1,32,len(data),offset))
        offset+=len(data)
    path.write_bytes(struct.pack('<HHH',0,1,len(sizes))+b''.join(entries)+b''.join(payloads))
ico(icons/'app_icon.ico',(16,32,48,64,128,256))
shutil.copyfile(icons/'app_icon.ico',icons/'icon.ico')
ico(frontend/'src/app/favicon.ico',(16,32,48))
for name in ('logo','logo-dark','logo-collapsed','logo-collapsed-dark'):
    shutil.copyfile(brand/f'{name}.png',frontend/f'public/{name}.png')
(frontend/'public/icon_128x128.png').write_bytes(png(128))
(frontend/'public/icon_32x32@2x.png').write_bytes(png(64,32))
for name in ('tray-template','tray-template@2x','tray-recording-template','tray-recording-template@2x'):
    shutil.copyfile(brand/f'{name}.png',icons/f'{name}.png')
shutil.copyfile(brand/'dmg-background.png',icons/'dmg-background.png')
shutil.copyfile(brand/'dmg-background@2x.png',icons/'dmg-background@2x.png')
print('Installed PartyLine icons, logos, tray glyphs, and installer artwork.')
