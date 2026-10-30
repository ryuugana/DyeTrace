"""Bring a window to the front, even from a background process.

    python tools/ui_focus.py DyeTrace

Windows normally refuses SetForegroundWindow to a process the user is not
interacting with.  Sharing the foreground thread's input queue for the call is
the documented way around that.
"""
import ctypes
import sys
import time
from ctypes import wintypes

user32 = ctypes.WinDLL('user32', use_last_error=True)
kernel32 = ctypes.WinDLL('kernel32', use_last_error=True)

user32.FindWindowA.restype = wintypes.HWND
user32.FindWindowA.argtypes = [wintypes.LPCSTR, wintypes.LPCSTR]
user32.GetForegroundWindow.restype = wintypes.HWND
user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
user32.GetWindowThreadProcessId.restype = wintypes.DWORD
user32.AttachThreadInput.argtypes = [wintypes.DWORD, wintypes.DWORD, wintypes.BOOL]
user32.SetForegroundWindow.argtypes = [wintypes.HWND]
user32.BringWindowToTop.argtypes = [wintypes.HWND]
kernel32.GetCurrentThreadId.restype = wintypes.DWORD


def focus(title):
    hwnd = user32.FindWindowA(None, title.encode())
    if not hwnd:
        raise SystemExit(f'no window titled {title!r}')

    foreground = user32.GetForegroundWindow()
    mine = kernel32.GetCurrentThreadId()
    theirs = user32.GetWindowThreadProcessId(foreground, None)

    user32.AttachThreadInput(mine, theirs, True)
    user32.BringWindowToTop(hwnd)
    user32.SetForegroundWindow(hwnd)
    user32.AttachThreadInput(mine, theirs, False)
    time.sleep(0.4)

    return user32.GetForegroundWindow() == hwnd


if __name__ == '__main__':
    title = sys.argv[1] if len(sys.argv) > 1 else 'DyeTrace'
    print('raised' if focus(title) else 'could not raise')
