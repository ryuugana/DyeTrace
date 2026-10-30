"""Measure the ink of the Pick Color / Start Dyeing button in both states."""

import struct

REGION = (100, 385, 70, 40)   # relative to the palette origin (153,245)


def read_bmp(path):
    d = open(path, 'rb').read()
    off = struct.unpack_from('<I', d, 10)[0]
    w, h = struct.unpack_from('<ii', d, 18)
    bpp = struct.unpack_from('<H', d, 28)[0]
    stride = ((w * bpp // 8) + 3) & ~3
    topdown = h < 0
    H = abs(h)
    px = {}
    for row in range(H):
        y = row if topdown else H - 1 - row
        base = off + row * stride
        for x in range(w):
            o = base + x * (bpp // 8)
            px[(x, y)] = (d[o + 2], d[o + 1], d[o])
    return px


def measure(path, ox, oy):
    px = read_bmp(path)
    x0, y0, w, h = REGION
    lums = []
    cells = []
    for y in range(y0, y0 + h):
        for x in range(x0, x0 + w):
            p = px.get((ox + x, oy + y))
            if p is None:
                continue
            lum = (p[0] * 77 + p[1] * 151 + p[2] * 28) >> 8
            lums.append(lum)
            cells.append((x, lum))
    lums.sort()
    background = lums[len(lums) // 2]
    threshold = background + 45
    columns = sorted({x for x, lum in cells if lum > threshold})
    ink = sum(1 for _, lum in cells if lum > threshold)
    if not columns:
        return dict(background=background, ink=0, first=None, last=None, width=0)
    return dict(
        background=background,
        ink=ink,
        first=columns[0],
        last=columns[-1],
        width=columns[-1] - columns[0] + 1,
    )


for name, path in (("start state", "btn_start.bmp"), ("after cancel", "btn_pick.bmp")):
    m = measure(path, 153, 245)
    print(f'{name:14s} bg={m["background"]:3d} ink={m["ink"]:4d} '
          f'cols={m["width"]:3d} (x {m["first"]}..{m["last"]})')
