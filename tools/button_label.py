"""Measure the two button labels so a click can be gated on the reading.

    python tools/button_label.py <frame.bmp> [<frame.bmp> ...]
"""
import struct
import sys

ORIGIN = (153, 245)
BUTTON = (147, 397)       # the Pick Color / Start Dyeing button
LABEL = (292, 628, 60, 28)  # x, y, w, h in window pixels (measured once)
THRESHOLDS = (60, 90, 110, 130, 150, 180)


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
            line.append((d[o + 2], d[o + 1], d[o]))
        rows.append(line)
    if not topdown:
        rows.reverse()
    return w, H, rows


def stats(path):
    w, h, rows = read_bmp(path)
    x0, y0, bw, bh = LABEL
    lums = []
    for y in range(y0, y0 + bh):
        for x in range(x0, x0 + bw):
            r, g, b = rows[y][x]
            lums.append((r * 77 + g * 151 + b * 28) >> 8)
    counts = [(t, sum(1 for l in lums if l > t)) for t in THRESHOLDS]
    ink = [l for l in lums if l > 90]
    mean = sum(ink) / len(ink) if ink else 0
    print(f'{path}: counts {counts}  ink {len(ink)}  mean {mean:.1f}')


if __name__ == '__main__':
    for frame in sys.argv[1:]:
        stats(frame)
