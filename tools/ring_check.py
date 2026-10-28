"""Compare the ring around a real dye palette with the false positive."""

import struct


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
        y = row if topdown else H - 1 - row
        base = off + row * stride
        line = []
        for x in range(w):
            o = base + x * (bpp // 8)
            line.append((d[o + 2], d[o + 1], d[o]))
        rows.append(line)
    if not topdown:
        rows.reverse()
    return w, H, rows


def ring_stats(name, path):
    w, h, px = read_bmp(path)

    def lum(p):
        return (p[0] * 77 + p[1] * 151 + p[2] * 28) >> 8

    last_row = [lum(px[254][x]) for x in range(w)]
    last_col = [lum(px[y][254]) for y in range(h)]
    bg_row = [lum(px[255][x]) for x in range(w)]
    bg_col = [lum(px[y][255]) for y in range(h)]

    print(f'{name}: {w}x{h}')
    for label, values in (('row 254', last_row), ('col 254', last_col),
                          ('row 255', bg_row), ('col 255', bg_col)):
        print(f'  {label}: min {min(values):3d}  max {max(values):3d}  '
              f'range {max(values) - min(values):3d}')


ring_stats('dye palette (real)', 'probe_out/palette.bmp')
print()
ring_stats('world square (false)', 'probe_out2/palette.bmp')
