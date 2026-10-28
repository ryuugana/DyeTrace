//! Hand-written Win32 declarations.
//!
//! Only the handful of calls the three traits need, so the backend adds no
//! dependencies and the release binary stays small.  Handles are `isize`
//! (pointer-sized), which is what the ABI uses on both x86 and x64.

#![allow(non_snake_case, non_camel_case_types, clippy::upper_case_acronyms)]

use core::ffi::c_void;

pub type HWND = isize;
pub type HDC = isize;
pub type HBITMAP = isize;
pub type HGDIOBJ = isize;
pub type BOOL = i32;

pub const SRCCOPY: u32 = 0x00CC_0020;
pub const DIB_RGB_COLORS: u32 = 0;
pub const BI_RGB: u32 = 0;

pub const MOUSEEVENTF_MOVE: u32 = 0x0001;
pub const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
pub const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
pub const MOUSEEVENTF_ABSOLUTE: u32 = 0x8000;

pub const SM_CXSCREEN: i32 = 0;
pub const SM_CYSCREEN: i32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BITMAPINFOHEADER {
    pub size: u32,
    pub width: i32,
    pub height: i32,
    pub planes: u16,
    pub bit_count: u16,
    pub compression: u32,
    pub size_image: u32,
    pub x_pels_per_meter: i32,
    pub y_pels_per_meter: i32,
    pub clr_used: u32,
    pub clr_important: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BITMAPINFO {
    pub header: BITMAPINFOHEADER,
    /// One placeholder entry: BI_RGB at 32 bpp needs no colour table.
    pub colours: [u32; 1],
}

#[link(name = "user32")]
extern "system" {
    pub fn GetDC(hwnd: HWND) -> HDC;
    pub fn ReleaseDC(hwnd: HWND, hdc: HDC) -> i32;

    pub fn CreateCompatibleDC(hdc: HDC) -> HDC;
    pub fn DeleteDC(hdc: HDC) -> BOOL;
    pub fn SelectObject(hdc: HDC, obj: HGDIOBJ) -> HGDIOBJ;
    pub fn BitBlt(
        dst: HDC,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        src: HDC,
        src_x: i32,
        src_y: i32,
        rop: u32,
    ) -> BOOL;

    pub fn GetCursorPos(point: *mut POINT) -> BOOL;
    pub fn SetCursorPos(x: i32, y: i32) -> BOOL;
    pub fn mouse_event(flags: u32, dx: u32, dy: u32, data: u32, extra: usize);

    pub fn FindWindowA(class: *const u8, title: *const u8) -> HWND;
    pub fn IsWindow(hwnd: HWND) -> BOOL;
    pub fn GetWindowRect(hwnd: HWND, rect: *mut RECT) -> BOOL;
    pub fn SetForegroundWindow(hwnd: HWND) -> BOOL;
    pub fn GetForegroundWindow() -> HWND;

    pub fn GetSystemMetrics(index: i32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateDIBSection(
        hdc: HDC,
        info: *const BITMAPINFO,
        usage: u32,
        bits: *mut *mut c_void,
        section: isize,
        offset: u32,
    ) -> HBITMAP;
    pub fn DeleteObject(obj: HGDIOBJ) -> BOOL;
}
