# 从 ⊥ 的轮廓派生 ⊤（上下翻转）和 ⊢（顺时针旋转 90°），加进字体子集。
import sys
from fontTools.ttLib import TTFont
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.reverseContourPen import ReverseContourPen
from fontTools.pens.boundsPen import BoundsPen

path = sys.argv[1]
f = TTFont(path)
cmap = f.getBestCmap()
src = cmap[0x22A5]
gs = f.getGlyphSet()
b = BoundsPen(gs); gs[src].draw(b)
x0, y0, x1, y1 = b.bounds
cx, cy = (x0 + x1) / 2, (y0 + y1) / 2
adv, _ = f['hmtx'][src]

def make(name, cp, transform, reverse):
    pen = TTGlyphPen(gs)
    target = ReverseContourPen(pen) if reverse else pen
    gs[src].draw(TransformPen(target, transform))
    glyph = pen.glyph()
    f['glyf'][name] = glyph
    glyph.recalcBounds(f['glyf'])
    f['hmtx'][name] = (adv, glyph.xMin)
    if 'vmtx' in f:
        f['vmtx'][name] = f['vmtx'][src]
    for t in f['cmap'].tables:
        if t.isUnicode():
            t.cmap[cp] = name
    order = f.getGlyphOrder()
    if name not in order:
        order.append(name)
        f.setGlyphOrder(order)

# 上下翻转：y' = y0 + y1 - y（翻转会改变轮廓方向，所以反转回来）
make('uni22A4', 0x22A4, (1, 0, 0, -1, 0, y0 + y1), True)
# 顺时针旋转 90°：(x, y) → (cx + (y - cy), cy - (x - cx))
make('uni22A2', 0x22A2, (0, -1, 1, 0, cx - cy, cy + cx), False)
f['maxp'].numGlyphs = len(f.getGlyphOrder())
f.save(path)
