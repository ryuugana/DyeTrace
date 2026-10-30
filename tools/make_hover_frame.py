"""Paste the client's marker graphic onto a clean window grab, to exercise the
box detector and offset table through the replay harness.

Geometry used (measured on the live client): each legend box is an 8x8 dark
ring with a flat fill, drawn 10 px to the right of the crosshair it belongs to,
and joined to it by a 1 px neck.
"""
import struct

SRC = r"C:\Development\DyeLite\reconstructed\clone\window.bmp"
DST = r"C:\Development\DyeTrace\hover.bmp"
ORIGIN = (153, 245)
POINTER = (127, 127)
OFFSETS = [(-47, -48), (29, -45), (0, 0), (-44, 23), (26, 30)]
FILLS = [(0xB5, 0x69, 0x82), (0xE5, 0x56, 0x4F), (0x00, 0x00, 0x00),
         (0x9F, 0xB2, 0xDC), (0x72, 0x85, 0xB7)]


def read_bmp(path):
    d = open(path, 'rb').read()
    off = struct.unpack_from('<I', d, 10)[0]
    w, h = struct.unpack_from('<ii', d, 18)
    bpp = struct.unpack_from('<H', d, 28)[0]
    stride = ((w * bpp // 8) + 3) & ~3
    topdown = h < 0
    H = abs(h)
    rows = []
    for row in range(H):
        base = off + row * stride
        line = []
        for x in range(w):
            o = base + x * (bpp // 8)
            line.append(bytearray((d[o], d[o + 1], d[o + 2])))   # BGR
        rows.append(line)
    if not topdown:
        rows.reverse()
    return w, H, rows


def write_bmp(path, w, h, rows):
    stride = (w * 3 + 3) & ~3
    out = bytearray()
    out += b'BM' + struct.pack('<IHHI', 54 + stride * h, 0, 0, 54)
    out += struct.pack('<IiiHHIIiiII', 40, w, -h, 1, 24, 0, stride * h, 0, 0, 0, 0)
    pad = bytes(stride - w * 3)
    for row in rows:
        flat = bytearray()
        for px in row:
            flat += bytes(px)
        out += flat + pad
    open(path, 'wb').write(out)


w, h, rows = read_bmp(SRC)

def put(x, y, bgr):
    if 0 <= x < w and 0 <= y < h:
        rows[y][x] = bytearray(bgr)

BLACK = (0x00, 0x00, 0x00)

for (dx, dy), fill in zip(OFFSETS, FILLS):
    # crosshair = sample point
    sx = ORIGIN[0] + POINTER[0] + dx
    sy = ORIGIN[1] + POINTER[1] + dy
    # box centre sits 10 px right of the crosshair
    bx = sx + 10
    by = sy
    x0, y0 = bx - 4, by - 4
    for y in range(y0, y0 + 8):
        for x in range(x0, x0 + 8):
            edge = x in (x0, x0 + 7) or y in (y0, y0 + 7)
            put(x, y, BLACK if edge else (fill[2], fill[1], fill[0]))
    # neck from the crosshair to the box, touching only the left side
    for x in range(sx + 1, x0):
        put(x, sy, BLACK)
    # crosshair
    for i in range(-2, 3):
        put(sx + i, sy, BLACK)
        put(sx, sy + i, BLACK)

write_bmp(DST, w, h, rows)
print(f"wrote {DST}: {len(OFFSETS)} boxes, pointer at {POINTER} (palette coords)")
