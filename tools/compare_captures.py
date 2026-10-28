"""Compare the Rust capture with a .NET screen grab of the same desktop."""

import struct


def read_bmp(path):
    d = open(path, 'rb').read()
    off = struct.unpack_from('<I', d, 10)[0]
    w, h = struct.unpack_from('<ii', d, 18)
    bpp = struct.unpack_from('<H', d, 28)[0]
    stride = ((w * bpp // 8) + 3) & ~3
    topdown = h < 0
    H = abs(h)
    bytes_pp = bpp // 8
    px = {}
    for row in range(H):
        y = row if topdown else H - 1 - row
        base = off + row * stride
        for x in range(w):
            o = base + x * bytes_pp
            px[(x, y)] = (d[o + 2], d[o + 1], d[o])
    return w, H, px


w1, h1, a = read_bmp('rust_shot.bmp')
w2, h2, b = read_bmp('ps_shot.bmp')
print(f'rust {w1}x{h1}   .net {w2}x{h2}')

w, h = min(w1, w2), min(h1, h2)
same = 0
close = 0
total = 0
worst = None
for y in range(0, h, 2):
    for x in range(0, w, 2):
        total += 1
        pa, pb = a[(x, y)], b[(x, y)]
        d = max(abs(pa[i] - pb[i]) for i in range(3))
        if d == 0:
            same += 1
        elif d <= 8:
            close += 1
        elif worst is None:
            worst = (x, y, pa, pb)

print(f'exact {same * 100 / total:.1f}%   within 8 {close * 100 / total:.1f}%   '
      f'sampled {total} px')
if worst:
    print(f'first difference at {worst[0]},{worst[1]}: rust {worst[2]} vs .net {worst[3]}')
