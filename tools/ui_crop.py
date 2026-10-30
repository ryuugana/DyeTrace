"""Find a top-level window by title and report or capture its rectangle.

    python tools/ui_crop.py DyeTrace shot.bmp docs/captures/ui.png
"""
import ctypes
import sys
from ctypes import wintypes

user32 = ctypes.WinDLL('user32', use_last_error=True)
user32.FindWindowA.restype = wintypes.HWND
user32.FindWindowA.argtypes = [wintypes.LPCSTR, wintypes.LPCSTR]
user32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
user32.SetForegroundWindow.argtypes = [wintypes.HWND]


def rect_of(title):
    hwnd = user32.FindWindowA(None, title.encode())
    if not hwnd:
        raise SystemExit(f'no window titled {title!r}')
    r = wintypes.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    user32.SetForegroundWindow(hwnd)
    return r.left, r.top, r.right, r.bottom


def main():
    title = sys.argv[1] if len(sys.argv) > 1 else 'DyeTrace'
    left, top, right, bottom = rect_of(title)
    print(f'{title}: ({left},{top}) {right - left}x{bottom - top}')
    if len(sys.argv) > 3:
        from PIL import Image
        source, target = sys.argv[2], sys.argv[3]
        Image.open(source).crop((left, top, right, bottom)).save(target)
        print(f'wrote {target}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
