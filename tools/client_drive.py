"""Drive the client by hand: raise it, move the pointer, optionally click one of
its buttons, and report where the pointer really is.

Used to capture the frames that the replay harness analyses when the box
detector or the offset table needs checking against the live client.

    python tools/client_drive.py hover          # hover the palette centre
    python tools/client_drive.py click 147      # press Pick Color (palette offset)
"""
import ctypes
import sys
import time
from ctypes import wintypes

user32 = ctypes.WinDLL('user32', use_last_error=True)
user32.FindWindowA.restype = wintypes.HWND
user32.FindWindowA.argtypes = [wintypes.LPCSTR, wintypes.LPCSTR]
user32.SetForegroundWindow.argtypes = [wintypes.HWND]
user32.SwitchToThisWindow.argtypes = [wintypes.HWND, wintypes.BOOL]
user32.GetForegroundWindow.restype = wintypes.HWND
user32.GetWindowRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
user32.SetCursorPos.argtypes = [ctypes.c_int, ctypes.c_int]
user32.GetCursorPos.argtypes = [ctypes.POINTER(wintypes.POINT)]
user32.mouse_event.argtypes = [wintypes.DWORD, wintypes.DWORD, wintypes.DWORD,
                               wintypes.DWORD, ctypes.c_size_t]

ORIGIN = (153, 245)
CENTER = (128, 128)


def focus():
    hwnd = user32.FindWindowA(b'Mabinogi', None)
    if not hwnd:
        raise SystemExit('no client window')
    if user32.GetForegroundWindow() != hwnd:
        user32.SetForegroundWindow(hwnd)
        time.sleep(0.1)
        if user32.GetForegroundWindow() != hwnd:
            user32.SwitchToThisWindow(hwnd, True)
    time.sleep(0.35)
    return hwnd


def rect(hwnd):
    r = wintypes.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    return r.left, r.top, r.right, r.bottom


def where():
    p = wintypes.POINT()
    user32.GetCursorPos(ctypes.byref(p))
    return p.x, p.y


def move(x, y):
    user32.SetCursorPos(x, y)
    time.sleep(0.2)
    # a real motion event makes the client redraw its legend
    user32.mouse_event(0x0001, 3, 2, 0, 0)
    time.sleep(0.2)
    user32.mouse_event(0x0001, ctypes.c_uint32(-3).value, ctypes.c_uint32(-2).value, 0, 0)
    time.sleep(0.25)


def click():
    user32.mouse_event(0x0002, 0, 0, 0, 0)
    time.sleep(0.06)
    user32.mouse_event(0x0004, 0, 0, 0, 0)
    time.sleep(0.45)


def main():
    what = sys.argv[1] if len(sys.argv) > 1 else 'hover'
    hwnd = focus()
    left, top, right, bottom = rect(hwnd)
    print(f'window {right - left}x{bottom - top} at ({left},{top})')
    for value in sys.argv[2:]:
        if what == 'hover':
            dx, dy = (int(v) for v in value.split(','))
            move(left + ORIGIN[0] + dx, top + ORIGIN[1] + dy)
        elif what == 'click':
            move(left + ORIGIN[0] + int(value), top + ORIGIN[1] + 397)
            click()
        print(f'  pointer {where()}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
