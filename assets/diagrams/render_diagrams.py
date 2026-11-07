from pathlib import Path
import xml.etree.ElementTree as ET
from html import escape
from PIL import Image, ImageDraw, ImageFont
import json, math
OUT=Path(__file__).resolve().parent
BG='#f5f3eb';INK='#243b35';MUTED='#61736c';ACCENT='#28755b';SOFT='#e1eae1';LINE='#aabbb0';WHITE='#ffffff'
FONT='/System/Library/Fonts/Supplemental/Arial.ttf'
manifest={}
class Diagram:
 def __init__(self,id,title,desc,lesson_ids,stage):
  self.id=id;self.title=title;self.desc=desc;self.ids=lesson_ids;self.stage=stage
  self.root=ET.Element('svg',{'xmlns':'http://www.w3.org/2000/svg','width':'960','height':'400','viewBox':'0 0 960 400','role':'img','aria-labelledby':f'{id}-title {id}-desc'})
  ET.SubElement(self.root,'title',{'id':id+'-title'}).text=title
  ET.SubElement(self.root,'desc',{'id':id+'-desc'}).text=desc
  self.rect(0,0,960,400,BG)
  self.text(32,38,title,25)
 def el(self,tag,**a):return ET.SubElement(self.root,tag,{k.replace('_','-'):str(v) for k,v in a.items()})
 def rect(self,x,y,w,h,fill=SOFT,stroke='none',sw=1,rx=0):return self.el('rect',x=x,y=y,width=w,height=h,fill=fill,stroke=stroke,stroke_width=sw,rx=rx)
 def line(self,x1,y1,x2,y2,colour=LINE,width=2,dash=None):
  e=self.el('line',x1=x1,y1=y1,x2=x2,y2=y2,stroke=colour,stroke_width=width)
  if dash:e.set('stroke-dasharray',dash)
  return e
 def text(self,x,y,s,size=20,fill=INK,anchor='start'):
  e=self.el('text',x=x,y=y,fill=fill,font_family='Arial, sans-serif',font_size=size,text_anchor=anchor);e.text=s;return e
 def box(self,x,y,w,h,label,sub=None,fill=SOFT):
  self.rect(x,y,w,h,fill,LINE,1,5)
  self.text(x+w/2,y+h/2-3 if sub else y+h/2+7,label,21,anchor='middle')
  if sub:self.text(x+w/2,y+h/2+26,sub,16,MUTED,'middle')
 def arrow(self,x1,y1,x2,y2,colour=ACCENT):
  self.line(x1,y1,x2,y2,colour,3)
  a=math.atan2(y2-y1,x2-x1);p=[(x2,y2),(x2-11*math.cos(a-.45),y2-11*math.sin(a-.45)),(x2-11*math.cos(a+.45),y2-11*math.sin(a+.45))]
  self.el('polygon',points=' '.join(f'{x},{y}' for x,y in p),fill=colour)
 def note(self,s):self.text(32,370,s,17,MUTED)
 def save(self):
  file=self.id+'.svg';ET.indent(self.root);ET.ElementTree(self.root).write(OUT/file,encoding='unicode',xml_declaration=True)
  render(OUT/file,OUT/(self.id+'.png'))
  manifest[self.id]=dict(file=self.id+'.png',svg=file,png=self.id+'.png',title=self.title,caption=self.desc,lesson_ids=self.ids,stages=[self.stage],verified=False)
def render(src,dst):
 root=ET.parse(src).getroot();im=Image.new('RGB',(1920,800),BG);draw=ImageDraw.Draw(im)
 def coord(v):return float(v)*2
 for e in root:
  tag=e.tag.split('}')[-1];a=e.attrib;fill=a.get('fill',INK);stroke=a.get('stroke','none');sw=max(1,round(float(a.get('stroke-width','1'))*2))
  if tag=='rect':
   x,y,w,h=[coord(a[k]) for k in ('x','y','width','height')];draw.rounded_rectangle((x,y,x+w,y+h),radius=coord(a.get('rx','0')),fill=None if fill=='none' else fill,outline=None if stroke=='none' else stroke,width=sw)
  elif tag=='line':
   xy=[coord(a[k]) for k in ('x1','y1','x2','y2')]
   if a.get('stroke-dasharray'):
    x1,y1,x2,y2=xy;length=math.hypot(x2-x1,y2-y1)
    for t in range(0,int(length),16):
     u=min(length,t+8);draw.line((x1+(x2-x1)*t/length,y1+(y2-y1)*t/length,x1+(x2-x1)*u/length,y1+(y2-y1)*u/length),fill=stroke,width=sw)
   else:draw.line(xy,fill=stroke,width=sw)
  elif tag=='polygon':draw.polygon([tuple(coord(v) for v in p.split(',')) for p in a['points'].split()],fill=fill)
  elif tag=='text':
   font=ImageFont.truetype(FONT,round(coord(a['font-size'])));anchor={'start':'ls','middle':'ms','end':'rs'}[a.get('text-anchor','start')]
   draw.text((coord(a['x']),coord(a['y'])),e.text or '',font=font,fill=fill,anchor=anchor)
 im.save(dst)
# 1
D=Diagram('toolchain','Three tools, three jobs','rustup chooses a Rust toolchain. Cargo coordinates a build; rustc compiles source into the executable Cargo launches.',[1,2],'Start')
D.box(40,92,220,76,'rustup','installs / selects toolchain');D.arrow(260,130,330,130)
D.box(330,92,255,76,'Rust toolchain','contains rustc and Cargo');D.box(40,236,220,76,'cargo run','coordinates the work');D.arrow(260,274,340,274)
D.box(340,236,220,76,'rustc','compiles Rust source');D.arrow(560,274,640,274);D.box(640,236,260,76,'your executable','runs your instructions')
D.note('The terminal sends commands; it is not the compiler.');D.save()
# 2
D=Diagram('edit-build-run','Editing is not running','A saved source edit changes the text file. Building creates an executable, and launching that executable runs the changed instructions.',[2,6],'Start')
for x,label,sub in [(35,'Edit','src/main.rs'),(350,'Build','cargo build'),(665,'Run','launch executable')]:D.box(x,135,260,100,label,sub)
D.arrow(295,185,350,185);D.arrow(610,185,665,185);D.text(480,296,'cargo run builds when needed, then launches',22,ACCENT,'middle')
D.note('An already running executable does not change when you save the source file.');D.save()
# 3
D=Diagram('variable-state','One binding, a new value','A mutable row starts at 5. The statement row += 1 replaces the stored value with 6; it does not create two grains.',[3,4,5],'Start')
D.box(70,124,250,120,'row = 5','before the update');D.box(385,151,190,67,'row += 1');D.box(640,124,250,120,'row = 6','after the update');D.arrow(320,185,385,185);D.arrow(575,185,640,185)
D.text(480,307,'The binding still names one position.',22,ACCENT,'middle');D.note('let mut row = 5 allows reassignment. A plain let binding rejects this reassignment.');D.save()
# 4
D=Diagram('cell-coordinates','Coordinates and sizes use different units','A cell at column 3 and row 2 maps to drawing position x=12, y=8 when each cell is 4 drawing pixels wide in the low-DPI prototype.',[5,10,11],'Start')
for r in range(4):
 for c in range(6):D.rect(74+c*45,100+r*45,45,45,ACCENT if (c,r)==(3,2) else WHITE,LINE)
for c in range(6):D.text(96+c*45,89,str(c),16,MUTED,'middle')
for r in range(4):D.text(60,128+r*45,str(r),16,MUTED,'end')
D.text(74,318,'Grid: column 3, row 2',20);D.arrow(375,193,470,193)
D.box(495,135,400,125,'x = 3 × 4 = 12','y = 2 × 4 = 8');D.note('Position changes where the block is drawn; its width and height stay 4 pixels.');D.save()
# 5
D=Diagram('ownership-borrows','Move ownership, or lend access','A by-value move transfers a non-Copy value. Shared references lend read access; a mutable reference lends exclusive mutable access while used.',[12,13],'Start')
D.box(36,90,230,70,'owner: cell');D.arrow(266,125,350,125);D.box(350,90,245,70,'move: new owner');D.text(630,133,'old binding unusable',18,MUTED)
D.box(36,200,230,70,'owner keeps Cell');D.arrow(266,235,350,235);D.box(350,200,245,70,'&Cell','shared read access');D.text(630,242,'may lend several reads',18,MUTED)
D.box(36,300,230,50,'owner keeps Cell');D.arrow(266,325,350,325);D.box(350,300,245,50,'&mut Cell');D.text(630,333,'exclusive access while used',18,MUTED)
D.save()
#6
D=Diagram('frame-tick-timeline','Drawing frames are not simulation ticks','At a steady 60 FPS, frames arrive roughly every 16.67 milliseconds. At 20 TPS, simulation updates are due every 50 milliseconds; scheduling uses elapsed time, not frame count.',[14,54,55],'M3')
start=130;step=72
D.text(30,140,'Frames',20);D.text(30,242,'Ticks',20)
D.line(start,132,start+step*10,132);D.line(start,234,start+step*10,234)
for i in range(11):
 x=start+i*step;D.rect(x-3,120,6,24,LINE)
 if i%3==0:D.rect(x-5,218,10,32,ACCENT)
for i,label in [(0,'0 ms'),(3,'50 ms'),(6,'100 ms'),(9,'150 ms')]:D.text(start+i*step,294,label,17,MUTED,'middle')
D.note('This regular timeline is an example. Real frames vary; the accumulator retains fractional elapsed time.');D.save()
#7
D=Diagram('row-major-index','A two-dimensional grid in one vector','For a four-column grid, row-major indexing assigns index = row × 4 + column. The highlighted cell at column 2, row 1 therefore has index 6.',[17,18,19],'M0')
for r in range(3):
 for c in range(4):
  x=60+c*65;y=100+r*65;D.rect(x,y,65,65,ACCENT if (c,r)==(2,1) else WHITE,LINE);D.text(x+32.5,y+39,str(r*4+c),23,WHITE if (c,r)==(2,1) else INK,'middle')
D.arrow(360,195,470,195);D.box(495,130,400,125,'index = 1 × 4 + 2','index = 6')
D.note('Check row and column bounds before calculating the index or converting signed coordinates.');D.save()
#8
D=Diagram('negative-chunks','Negative positions still belong to a chunk','With chunk width 64, world x=-1 belongs to chunk -1 at local x=63. Euclidean division gives local coordinates from 0 through 63.',[27,67],'M4')
D.box(55,132,400,110,'chunk −1','world x: −64 through −1');D.box(455,132,400,110,'chunk 0','world x: 0 through 63');D.line(455,105,455,280,ACCENT,3)
D.rect(435,132,20,110,ACCENT);D.text(445,101,'x = −1',18,ACCENT,'middle');D.text(255,293,'−1 = (−1 × 64) + 63',23,ACCENT,'middle');D.text(655,293,'local x remains nonnegative',20,MUTED,'middle')
D.note('Use div_euclid(64) for the chunk and rem_euclid(64) for the local position.');D.save()
#9
D=Diagram('active-region','Storage chunks do not define the simulation boundary','The active rectangle is the visible view plus a 16-cell margin on each side. Overlapping chunks can extend farther; update only their intersection with the active rectangle.',[28,29,70],'M0')
for r in range(3):
 for c in range(5):D.rect(55+c*100,85+r*82,100,82,WHITE,LINE)
D.rect(107,115,360,188,SOFT,ACCENT,2);D.rect(139,147,296,124,WHITE,ACCENT,2)
D.text(287,211,'VISIBLE VIEW',21,ACCENT,'middle');D.text(510,301,'chunk grid',17,MUTED,'middle')
D.line(139,147,139,115,ACCENT,2);D.text(156,139,'16 cells',15,ACCENT)
D.text(602,148,'Visible cells: draw + update',19);D.text(602,203,'Margin cells: update only',19);D.text(602,258,'Outside: frozen',19)
D.note('Schematic: the 16-cell margin is clipped inside each overlapping storage chunk.');D.save()
#10
D=Diagram('heat-and-exposure','Sky exposure and air contact are different questions','A roof blocks direct rain into a room. A sheltered wall can still touch local air and exchange heat. Local air contact must not imply exposure to the sky.',[33,34,38],'M1')
D.rect(90,122,310,24,INK);D.rect(90,146,24,145,INK);D.rect(376,146,24,145,INK);D.rect(90,291,310,20,INK)
for x in [150,230,310]:D.arrow(x,76,x,112)
D.text(246,208,'sheltered air',22,MUTED,'middle');D.arrow(329,233,372,233);D.text(470,140,'Roof: sky-exposed',22);D.text(470,208,'Inside wall: air-exposed',22);D.text(470,257,'Inside wall: not sky-exposed',22)
D.note('Use the surface heightmap for sky exposure and local neighbour tests for air contact.');D.save()
#11
D=Diagram('four-phase-schedule','Four phases select separated chunk sets','Parity labels define four chunk sets. Only one label is dispatched in a phase. This illustrates scheduling; the actual rule footprint must still prove that reads and writes cannot conflict.',[39,40],'M1')
colours=[SOFT,WHITE,WHITE,WHITE]
for r in range(4):
 for c in range(6):
  label=(c%2)+2*(r%2);x=60+c*62;y=90+r*62;D.rect(x,y,62,62,ACCENT if label==0 else WHITE,LINE);D.text(x+31,y+39,str(label),22,WHITE if label==0 else INK,'middle')
D.text(520,150,'Phase 0 runs highlighted chunks.',22);D.text(520,215,'Then phases 1, 2 and 3 run.',22);D.text(520,280,'Non-neighbouring ≠ automatically safe.',20,ACCENT)
D.note('Cross-chunk reach, destination reads and later phases belong in the disjointness proof.');D.save()
#12
D=Diagram('support-anchor','A group needs support outside itself','Adjacent rigid cells form one connected group. Their internal contacts are not external support. A grounded column can anchor the group, while an identical floating row falls.',[47,48],'M2')
D.text(240,100,'SUPPORTED',19,ACCENT,'middle');D.text(710,100,'FLOATING',19,MUTED,'middle')
for x in [100,150,200,250,300]:D.rect(x,145,50,45,SOFT,LINE)
for y in [190,235,280]:D.rect(100,y,50,45,SOFT,LINE)
D.rect(65,325,335,15,INK);D.arrow(125,312,125,333)
for x in [570,620,670,720,770]:D.rect(x,145,50,45,SOFT,LINE)
D.arrow(695,208,695,285);D.text(695,321,'No outside anchor',20,MUTED,'middle')
D.note('Internal neighbours cannot support one another in a circle. Search-cap exceptions are explicit approximations.');D.save()
#13
D=Diagram('random-ticks-calendar','Calendar advance and sampled work stay explicit','An executed tick advances step_tick once. Its integer multiplier advances world_tick by M. Slow-process picks scale with M up to a cap; the fallback is approximate.',[55,56,57],'M3')
D.box(30,130,230,105,'one executed tick','step_tick += 1');D.arrow(260,180,335,180);D.box(335,90,270,80,'calendar','world_tick += M');D.box(335,230,270,80,'sampled slow work','P = min(16 × M, 256)');D.line(300,180,300,270,ACCENT,3);D.arrow(300,270,335,270)
D.text(650,137,'M = 4 → P = 64',21,ACCENT);D.text(650,277,'M > 16 → capped fallback',19,MUTED)
D.note('Extra samples do not make fast-forward identical: climate, physics and spatial chains still need tests.');D.save()
#14
D=Diagram('save-generation','Commit the pointer after the snapshot is complete','Write a coherent new generation of region files first. Only after its files are complete does the committed manifest select that generation. A crash before commitment leaves the old generation selected.',[75,76,77],'M4')
D.box(35,95,240,82,'generation A','current complete save');D.box(35,235,240,82,'generation B','write complete region files')
D.box(400,160,220,86,'manifest','selected generation');D.arrow(400,184,275,136);D.text(296,145,'before',15,MUTED)
D.line(400,221,275,276,LINE,2,'4 4');D.text(316,276,'after commit',15,ACCENT)
D.text(670,151,'1. Finish B’s files.',20);D.text(670,212,'2. Commit manifest → B.',20);D.text(670,273,'3. Retain recoverable A.',20)
D.note('One writer prevents concurrent writes; a committed generation prevents mixed-tick or mixed-file saves.');D.save()
#15
D=Diagram('pixel-units','One cell occupies 2 × 2 art pixels','A simulation cell occupies two by two art pixels. Each art pixel expands to a two by two block of physical pixels, producing a four by four physical-pixel cell.',[10,80],'M5')
D.box(40,140,190,100,'1 simulation cell');D.arrow(230,190,305,190)
for r in range(2):
 for c in range(2):D.rect(320+c*48,142+r*48,48,48,ACCENT if (r+c)%2==0 else SOFT,LINE)
D.text(368,285,'2 × 2 art pixels',19,INK,'middle');D.arrow(440,190,530,190)
for r in range(4):
 for c in range(4):D.rect(560+c*27,137+r*27,27,27,ACCENT if (r//2+c//2)%2==0 else SOFT,LINE)
D.text(614,285,'4 × 4 physical pixels',19,INK,'middle')
D.text(776,176,'Nearest-neighbour',19,ACCENT,'middle');D.text(776,213,'No soft edges',19,MUTED,'middle')
D.note('DPI-adjusted drawing units are not necessarily physical pixels. Check the framebuffer conversion.');D.save()
#16
D=Diagram('release-artifacts','A desktop release is more than a source archive','Optimized native executables, required assets and licence notices become platform-specific packages. Each package needs an actual launch and save/load smoke test.',[96,98,100],'M7')
D.box(35,115,245,85,'source + lockfile','repeatable build inputs');D.arrow(280,157,360,157);D.box(360,115,245,85,'native release build','optimized executable');D.arrow(605,157,685,157);D.box(685,115,240,85,'game package','binary + assets + licences')
D.text(490,280,'Build and test on Mac, Windows and Linux.',22,ACCENT,'middle');D.note('cargo package creates a source crate archive; it does not create a desktop game installer.');D.save()
#17
D=Diagram('shoreline-mass','A drawn ripple cannot move shoreline material','Rendered waves change colour only. Mechanical erosion needs a separate simulation rule that moves sand and respects the approved water-volume policy.',[91,93],'M6')
D.box(35,112,250,95,'visual ripple','changes pixels only');D.box(35,245,250,70,'no cell transfer');D.arrow(160,207,160,245)
D.box(410,112,250,95,'erosion rule','changes world state');D.arrow(660,160,735,160);D.box(735,112,190,95,'sand moves','test counts');D.text(650,292,'Simulation effect ≠ decoration',21,ACCENT,'middle')
D.note('V1 has rendered ripples. V2 shore erosion needs its own bounded rule and acceptance test.');D.save()
(OUT/'manifest.json').write_text(json.dumps(manifest,ensure_ascii=False,indent=2)+'\n')
# Contact sheets preserve the 960px original width for readable visual review.
ids=list(manifest)
for i in range(0,len(ids),2):
 sheet=Image.new('RGB',(1920,1600),BG)
 for j,id in enumerate(ids[i:i+2]):sheet.paste(Image.open(OUT/(id+'.png')),(0,j*800))
 sheet.save(OUT/f'review-{i//2+1:02}.png')
print('Wrote',len(manifest),'SVG sources, PNG renders, manifest and 8 review sheets.')
