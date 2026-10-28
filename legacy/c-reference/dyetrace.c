/* dyetrace.c - a dye palette finder for Mabinogi, re-implementing the
 * behaviour of DyeLite 13.
 *
 *   Find field : locate the 256x256 dye palette inside the Mabinogi window
 *   Scan       : capture it, read the five sample offsets from the client's
 *                markers, and search for a position that holds the colour
 *   Auto       : Pick Color -> capture -> read offsets -> search -> click,
 *                or Cancel and repeat
 *
 * Build: cl /O2 /W3 dyetrace.c /link user32.lib gdi32.lib
 * Tests: dyetrace.exe --selftest  (writes selftest.txt)
 */

#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define CAP_W        256
#define CAP_H        256
#define SCAN_W       254        /* palette pixels: 0..253, measured       */
#define SCAN_H       254        /* 254 = black separator, 255 = background */
#define MAX_TARGETS  5
#define MIN_SLOTS    4   /* the original assumes a 4- or 5-slot table */

#define MASK_NONE        0
#define MASK_MATCH       1
#define MASK_MATCH_ERR   10

#define COL_GREEN  0x0000FF00u
#define COL_RED    0x00FF0000u

/* ---- data model (mirrors the original object layout) -------------------- */
typedef struct Target {
    int r, g, b, tol;
    int enabled;
} Target;

static Target  g_targets[MAX_TARGETS];
static int     g_targetCount = 1;

static unsigned int  g_result[SCAN_H * SCAN_W];   /* +0x40 in the original   */
static unsigned char g_mask[SCAN_H * SCAN_W];     /* +0x3F050 in the original */
static int           g_hits;
/* scan window inside the 256x256 grab */
static int   g_boundL = 0, g_boundT = 0, g_boundR = SCAN_W, g_boundB = SCAN_H;

static int   g_originX = -1, g_originY = -1;      /* +0x4EC54 / +0x4EC58    */
static int   g_tolerance = 0;
static int   g_mouseDelay = 100;                  /* "mouseDelay"           */
static int   g_standardDelay = 100;               /* "standardDelay"        */
static int   g_autoMode = 0;
static int   g_bestX = 0, g_bestY = 0;   /* best picker location           */
static int   g_bestSuccess = 0, g_bestDist = 0, g_enabledCount = 0;
static int   g_cell = 21;                /* picker cell size in px         */
/* dye-window buttons, relative to the palette origin (measured on Client.exe:
 * Pick Color at +147,+397, Cancel at +231,+397; the 2009 original used
 * +0x85/+0xD5, +0x194 which is the same pair 14 px to the left)             */
static int   g_btnPickX = 147, g_btnCancelX = 231, g_btnY = 397;
static int   g_autoState = 0, g_autoTicks = 0, g_hoverTry = 0;
static int   g_slotTwo = 0;                       /* click 0x85 or 0xD5     */

/* capture surface (DAT_004b8dc8) */
static HDC      g_capDC;
static HBITMAP  g_capBmp;
static void    *g_capBits;
static unsigned int g_grid[CAP_W * CAP_H];   /* marker-free gradient  */
static int g_gridValid = 0, g_useSaved = 0;
static int g_noClick = 0;                    /* --autotest: never click */
static HDC      g_screenDC;

/* preview surface */
static HDC      g_prevDC;
static HBITMAP  g_prevBmp;
static void    *g_prevBits;
static int      g_havePreview = 0;

static HWND g_main;
static HWND g_editR[MAX_TARGETS], g_editG[MAX_TARGETS], g_editB[MAX_TARGETS];
static HWND g_editTol[MAX_TARGETS], g_chkOn[MAX_TARGETS];
static HWND g_status, g_preview;
static HWND g_btnFind, g_btnScan, g_btnAuto, g_btnStop, g_btnPick, g_btnColor;
static HWND g_editPickX, g_editCancelX, g_editBtnY;
static HWND g_editHex, g_editVar, g_editSlots;
static int  g_colorR = 0, g_colorG = 0, g_colorB = 0;
static int  g_var = 0, g_desired = 5;
static HBRUSH g_bg;

/* ---- small helpers ------------------------------------------------------ */

static int iabs_i(int v) { return v < 0 ? -v : v; }

static unsigned int pixel_at_buf(const unsigned int *buf, int stride,
                                 int x, int y)
{
    return buf[(size_t)y * stride + x];
}

/* per-channel comparison, exactly the original's test (0x4a70ac) */
static int color_matches(unsigned int px, int r, int g, int b, int tol)
{
    int pr = (int)((px >> 16) & 0xFF);   /* red   */
    int pg = (int)((px >> 8) & 0xFF);    /* green */
    int pb = (int)(px & 0xFF);           /* blue  */
    if (iabs_i(r - pr) > tol) return 0;
    if (iabs_i(g - pg) > tol) return 0;
    if (iabs_i(b - pb) > tol) return 0;
    return 1;
}

/* ---- palette bounds ---------------------------------------------------- */
static int line_is_palette(const unsigned int *buf, int stride,
                          int horizontal, int pos, int lo, int hi)
{
    int i, n = 0, tot = 0, mn = 255, mx = 0;

    for (i = lo; i < hi; i++) {
        unsigned int px = horizontal ? pixel_at_buf(buf, stride, i, pos)
                                     : pixel_at_buf(buf, stride, pos, i);
        int r = (int)((px >> 16) & 0xFF);
        int g = (int)((px >> 8) & 0xFF);
        int b = (int)(px & 0xFF);
        int cmx = r > g ? (r > b ? r : b) : (g > b ? g : b);
        int cmn = r < g ? (r < b ? r : b) : (g < b ? g : b);
        if (cmx > 60 && (cmx - cmn) > 30) n++;
        if (cmn < mn) mn = cmn;
        if (cmx > mx) mx = cmx;
        tot++;
    }
    if (tot == 0) return 0;
    if (n * 2 < tot) return 0;        /* less than half the line is colourful */
    if (mx - mn < 16) return 0;       /* flat strip -> frame, not palette      */
    return 1;
}

#define BOUND_INSET_MAX 16

static void reset_bounds(void)
{
    g_boundL = 0; g_boundT = 0;
    g_boundR = SCAN_W; g_boundB = SCAN_H;
}

/* ---- the matcher (0x4a70ac) -------------------------------------------- */

static int scan_buffer(const unsigned int *src, int stride,
                       int r, int g, int b, int tol,
                       int detectErrors, int clearFirst)
{
    int x, y, marked = 0;

    if (clearFirst) {
        for (y = 0; y < SCAN_H; y++) {
            for (x = 0; x < SCAN_W; x++) {
                g_mask[y * SCAN_W + x] = MASK_NONE;
                g_result[y * SCAN_W + x] = 0x00FFFFFFu;
            }
        }
        g_hits = 0;
    }

    for (y = g_boundT; y < g_boundB; y++) {
        for (x = g_boundL; x < g_boundR; x++) {
            unsigned int px = pixel_at_buf(src, stride, x, y);
            unsigned char *m = &g_mask[(y - 1) * SCAN_W + (x - 1)];

            if (!color_matches(px, r, g, b, tol))
                continue;

            if (detectErrors && *m < MASK_MATCH_ERR) {
                *m = MASK_MATCH_ERR;
                g_result[(y - 1) * SCAN_W + (x - 1)] = COL_RED;
                marked++;
            } else if (!detectErrors && *m == MASK_NONE) {
                *m = MASK_MATCH;
                g_result[(y - 1) * SCAN_W + (x - 1)] = COL_GREEN;
                marked++;
            }
        }
    }
    g_hits += marked;
    return marked;
}

/* ---- colour-field detection ------------------------------------------- */
static int colorful(unsigned int px)
{
    int r = (int)((px >> 16) & 0xFF);
    int g = (int)((px >> 8) & 0xFF);
    int b = (int)(px & 0xFF);
    int mx = r > g ? (r > b ? r : b) : (g > b ? g : b);
    int mn = r < g ? (r < b ? r : b) : (g < b ? g : b);
    return (mx > 60) && ((mx - mn) > 30);
}

/* A palette is a 256x256 square whose four borders are edges and whose
 * interior varies, which is independent of its colours. */
static int lum_of(unsigned int px)
{
    int r = (int)((px >> 16) & 0xFF);
    int g = (int)((px >> 8) & 0xFF);
    int b = (int)(px & 0xFF);
    return (r * 77 + g * 151 + b * 28) >> 8;
}

#define BORDER_SLACK 2

static int line_grad_h(const unsigned int *buf, int stride, int x, int y, int len)
{
    int i, s = 0;
    for (i = 0; i < len; i++)
        s += iabs_i(lum_of(pixel_at_buf(buf, stride, x + i, y)) -
                    lum_of(pixel_at_buf(buf, stride, x + i, y - 1)));
    return s / (len ? len : 1);
}

static int line_grad_v(const unsigned int *buf, int stride, int x, int y, int len)
{
    int i, s = 0;
    for (i = 0; i < len; i++)
        s += iabs_i(lum_of(pixel_at_buf(buf, stride, x, y + i)) -
                    lum_of(pixel_at_buf(buf, stride, x - 1, y + i)));
    return s / (len ? len : 1);
}

/* Score a candidate: each border is searched within +-2 px and the score
 * is edge-dominant, so a busy interior cannot beat a real square. */
static int square_score(const unsigned int *buf, int stride, int x, int y,
                        int *outEdge, int *outRange)
{
    int i, mn = 255, mx = 0, top = 0, bot = 0, lft = 0, rgt = 0, e;

    for (i = -BORDER_SLACK; i <= BORDER_SLACK; i++) {
        e = line_grad_h(buf, stride, x, y + i, CAP_W);
        if (e > top) top = e;
        e = line_grad_h(buf, stride, x, y + CAP_H - 1 + i, CAP_W);
        if (e > bot) bot = e;
        e = line_grad_v(buf, stride, x + i, y, CAP_H);
        if (e > lft) lft = e;
        e = line_grad_v(buf, stride, x + CAP_W - 1 + i, y, CAP_H);
        if (e > rgt) rgt = e;
    }
    for (i = 0; i < CAP_W; i += 8) {
        int j;
        for (j = 0; j < CAP_H; j += 8) {
            int l = lum_of(pixel_at_buf(buf, stride, x + i, y + j));
            if (l < mn) mn = l;
            if (l > mx) mx = l;
        }
    }
    if (outEdge) *outEdge = top + bot + lft + rgt;
    if (outRange) *outRange = mx - mn;
    return (top + bot + lft + rgt) * 2 + (mx - mn);
}

static int find_square_field(const unsigned int *buf, int stride,
                             int w, int h, int *outX, int *outY)
{
    int x, y, best = -1, bx = 0, by = 0, be = 0, br = 0;

    for (y = 3; y + CAP_H + BORDER_SLACK < h; y += 4) {
        for (x = 3; x + CAP_W + BORDER_SLACK < w; x += 4) {
            int e = 0, r = 0;
            int s = square_score(buf, stride, x, y, &e, &r);
            if (s > best) { best = s; bx = x; by = y; be = e; br = r; }
        }
    }
    if (best < 0)
        return 0;
    /* refine +-4 px: a 4 px grid rarely lands exactly on the border */
    {
        int rx, ry;
        for (ry = by - 4; ry <= by + 4; ry++) {
            for (rx = bx - 4; rx <= bx + 4; rx++) {
                int e = 0, r = 0, s;
                if (rx < 3 || ry < 3 ||
                    rx + CAP_W + BORDER_SLACK >= w || ry + CAP_H + BORDER_SLACK >= h)
                    continue;
                s = square_score(buf, stride, rx, ry, &e, &r);
                if (s > best) { best = s; bx = rx; by = ry; be = e; br = r; }
            }
        }
    }
    if (be < 24 || br < 30)
        return 0;                       /* no square with real borders */

    /* snap to the exact border line: the candidate may be up to BORDER_SLACK
     * pixels off, and a 2 px misalignment would put frame pixels in the scan */
    {
        int d, bestE = -1, bd = 0;
        for (d = -BORDER_SLACK; d <= BORDER_SLACK; d++) {
            int e = line_grad_h(buf, stride, bx, by + d, CAP_W);
            if (e > bestE) { bestE = e; bd = d; }
        }
        by += bd;
        bestE = -1; bd = 0;
        for (d = -BORDER_SLACK; d <= BORDER_SLACK; d++) {
            int e = line_grad_v(buf, stride, bx + d, by, CAP_H);
            if (e > bestE) { bestE = e; bd = d; }
        }
        bx += bd;
    }
    *outX = bx;
    *outY = by;
    return 1;
}

static int find_field_in_buffer(const unsigned int *buf, int stride,
                                int w, int h, int *outX, int *outY)
{
    int bestX = 0, bestY = 0, bestScore = -1;
    int x, y, dx, dy;

    for (y = 0; y + SCAN_H <= h; y += 8) {
        for (x = 0; x + SCAN_W <= w; x += 8) {
            int score = 0;
            for (dy = 0; dy < SCAN_H; dy += 4)
                for (dx = 0; dx < SCAN_W; dx += 4)
                    if (colorful(pixel_at_buf(buf, stride, x + dx, y + dy)))
                        score++;
            if (score > bestScore) {
                bestScore = score;
                bestX = x;
                bestY = y;
            }
        }
    }
    /* need at least 25% of the sampled pixels to be colourful */
    if (bestScore < (SCAN_W / 4) * (SCAN_H / 4) / 4)
        return 0;
    *outX = bestX;
    *outY = bestY;
    return 1;
}

/* ---- screen capture ---------------------------------------------------- */

static int capture_init(void)
{
    BITMAPINFO bi;
    g_screenDC = GetDC(NULL);
    if (!g_screenDC) return 0;

    ZeroMemory(&bi, sizeof(bi));
    bi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bi.bmiHeader.biWidth = CAP_W;
    bi.bmiHeader.biHeight = -CAP_H;      /* top-down */
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    bi.bmiHeader.biCompression = BI_RGB;

    g_capDC = CreateCompatibleDC(g_screenDC);
    g_capBmp = CreateDIBSection(g_capDC, &bi, DIB_RGB_COLORS, &g_capBits, NULL, 0);
    if (!g_capBmp) return 0;
    SelectObject(g_capDC, g_capBmp);

    g_prevDC = CreateCompatibleDC(g_screenDC);
    bi.bmiHeader.biWidth = SCAN_W;
    bi.bmiHeader.biHeight = -SCAN_H;
    g_prevBmp = CreateDIBSection(g_prevDC, &bi, DIB_RGB_COLORS, &g_prevBits, NULL, 0);
    if (!g_prevBmp) return 0;
    SelectObject(g_prevDC, g_prevBmp);
    return 1;
}

static int capture_at(int x, int y)
{
    if (!g_capDC) return 0;
    return BitBlt(g_capDC, 0, 0, CAP_W, CAP_H, g_screenDC, x, y, SRCCOPY) != 0;
}

static void fill_rect32(unsigned int *buf, int stride, int w, int h,
                        int x0, int y0, int x1, int y1, unsigned int c)
{
    int x, y;
    if (x0 < 0) x0 = 0;
    if (y0 < 0) y0 = 0;
    if (x1 > w) x1 = w;
    if (y1 > h) y1 = h;
    for (y = y0; y < y1; y++)
        for (x = x0; x < x1; x++)
            buf[y * stride + x] = c;
}

/* is the palette covered by this window? */
static int origin_is_covered(void)
{
    RECT mr, pr;

    if (!g_main || g_originX < 0 || !GetWindowRect(g_main, &mr)) return 0;
    pr.left = g_originX; pr.top = g_originY;
    pr.right = g_originX + CAP_W;
    pr.bottom = g_originY + CAP_H;
    return !(pr.right <= mr.left || pr.left >= mr.right ||
             pr.bottom <= mr.top || pr.top >= mr.bottom);
}

/* grab the Mabinogi window (or the desktop) into a temporary buffer and look
 * for the colour field.  Mirrors FUN_004a6cf8. */
static int find_color_field(int *outX, int *outY)
{
    HWND wnd = FindWindowA("Mabinogi", NULL);
    RECT rc;
    int wx = 0, wy = 0, ww, wh;
    HDC mem;
    HBITMAP bmp;
    void *bits;
    BITMAPINFO bi;
    int ok;

    if (wnd && GetWindowRect(wnd, &rc) && (rc.right - rc.left) > 50) {
        wx = rc.left; wy = rc.top;
        ww = rc.right - rc.left; wh = rc.bottom - rc.top;
    } else {
        wx = 0; wy = 0;
        ww = GetDeviceCaps(g_screenDC, HORZRES);
        wh = GetDeviceCaps(g_screenDC, VERTRES);
    }
    if (ww < SCAN_W || wh < SCAN_H) return 0;

    ZeroMemory(&bi, sizeof(bi));
    bi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bi.bmiHeader.biWidth = ww;
    bi.bmiHeader.biHeight = -wh;
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    bi.bmiHeader.biCompression = BI_RGB;

    mem = CreateCompatibleDC(g_screenDC);
    bmp = CreateDIBSection(mem, &bi, DIB_RGB_COLORS, &bits, NULL, 0);
    if (!bmp) { DeleteDC(mem); return 0; }
    SelectObject(mem, bmp);

    ok = BitBlt(mem, 0, 0, ww, wh, g_screenDC, wx, wy, SRCCOPY) != 0;
    if (ok && g_main) {
        /* paint this window's rectangle out of the grab */
        RECT mr;
        if (GetWindowRect(g_main, &mr))
            fill_rect32((unsigned int *)bits, ww, ww, wh,
                        mr.left - wx, mr.top - wy, mr.right - wx, mr.bottom - wy,
                        0x00808080u);
    }
    if (ok) {
        ok = find_square_field((const unsigned int *)bits, ww, ww, wh,
                               &g_originX, &g_originY);
        if (!ok)                     /* fallback: largest saturated region */
            ok = find_field_in_buffer((const unsigned int *)bits, ww, ww, wh,
                                      &g_originX, &g_originY);
    }
    if (ok) { g_originX += wx; g_originY += wy; }

    DeleteObject(bmp);
    DeleteDC(mem);
    if (!ok) return 0;
    *outX = g_originX;
    *outY = g_originY;
    return 1;
}

/* ---- UI plumbing ------------------------------------------------------- */

static void set_status(const char *text)
{
    SetWindowTextA(g_status, text);
}

static int read_int(HWND edit, int def)
{
    char buf[32];
    GetWindowTextA(edit, buf, sizeof(buf));
    if (!buf[0]) return def;
    return atoi(buf);
}

static void read_targets(void)
{
    char buf[32];
    char *p;
    unsigned int c = 0;
    int i;

    if (!g_editHex) {            /* headless (--autotest): targets pre-set */
        for (i = 0; i < MAX_TARGETS; i++) {
            g_targets[i].r = g_colorR;
            g_targets[i].g = g_colorG;
            g_targets[i].b = g_colorB;
            g_targets[i].tol = g_var;
            g_targets[i].enabled = 1;
        }
        g_enabledCount = MAX_TARGETS;
        return;
    }

    GetWindowTextA(g_editHex, buf, sizeof(buf));
    p = buf;
    while (*p == '#' || *p == ' ') p++;
    c = (unsigned int)strtoul(p, NULL, 16);

    g_colorR = (int)((c >> 16) & 0xFF);
    g_colorG = (int)((c >> 8) & 0xFF);
    g_colorB = (int)(c & 0xFF);

    g_var = read_int(g_editVar, 0);
    if (g_var < 0) g_var = 0;
    if (g_var > 255) g_var = 255;

    g_desired = read_int(g_editSlots, MAX_TARGETS);
    if (g_desired < 1) g_desired = 1;
    if (g_desired > MAX_TARGETS) g_desired = MAX_TARGETS;

    /* every sample is compared against the same colour; the order of the five
     * markers does not matter, only how many of them hold this colour */
    for (i = 0; i < MAX_TARGETS; i++) {
        g_targets[i].r = g_colorR;
        g_targets[i].g = g_colorG;
        g_targets[i].b = g_colorB;
        g_targets[i].tol = g_var;
        g_targets[i].enabled = 1;
    }
    g_enabledCount = MAX_TARGETS;
}

/* ---- marker / offset / picker engine ----------------------------------- */

static int wrap_mod(int v, int m) { v %= m; if (v < 0) v += m; return v; }

/* sample offsets: g_offX/Y[i] is where slot i is relative to the cursor */
static int g_offX[MAX_TARGETS], g_offY[MAX_TARGETS], g_offCount = 0;
static int g_markerCount = 0, g_anchorX = 0, g_anchorY = 0;
static int g_mx[MAX_TARGETS], g_my[MAX_TARGETS];
static unsigned int g_mfill[MAX_TARGETS];
static int g_cursorPX = 0, g_cursorPY = 0;   /* cursor in palette coords */
static int g_clickOffX = 0, g_clickOffY = 0, g_clickValid = 0;
static int g_offFromCursor = 0;      /* offsets were measured from the cursor */
static int g_patX[MAX_TARGETS], g_patY[MAX_TARGETS], g_patCount = 0; /* last good */
static void ensure_foreground(void);
static int move_cursor_to(int x, int y);
static int changed_pixels(void);
static unsigned int grid_at(const unsigned int *buf, int x, int y);
static int capture_is_palette(void);
static int save_bmp24(const char *path, const unsigned int *px, int w, int h);

/* A marker is a dark-outlined square with a flat interior. */
static int marker_black(unsigned int c)
{
    return ((c >> 16) & 0xFF) < 70 && ((c >> 8) & 0xFF) < 70 && (c & 0xFF) < 70;
}

static int marker_flat(const unsigned int *buf, int st, int x, int y, int len, int hgt)
{
    unsigned int ref = buf[(y + 2) * st + (x + 2)];
    int rr = (int)((ref >> 16) & 0xFF), rg = (int)((ref >> 8) & 0xFF), rb = (int)(ref & 0xFF);
    int jx, jy;

    for (jy = y + 2; jy <= y + hgt - 3; jy++) {
        for (jx = x + 2; jx <= x + len - 3; jx++) {
            unsigned int p = buf[jy * st + jx];
            if (iabs_i((int)((p >> 16) & 0xFF) - rr) > 8) return 0;
            if (iabs_i((int)((p >> 8) & 0xFF) - rg) > 8) return 0;
            if (iabs_i((int)(p & 0xFF) - rb) > 8) return 0;
        }
    }
    return 1;
}

static int detect_markers(int *mx, int *my, int max)
{
    const unsigned int *buf = (const unsigned int *)g_capBits;
    const int st = CAP_W, W = CAP_W, H = CAP_H;
    int x, y, n = 0;

    if (!buf) return 0;
    for (y = 0; y < H - 2 && n < max; y++) {
        for (x = 0; x < W - 2 && n < max; x++) {
            int len = 0, v1 = 0, v2 = 0, i, ok = 0, cx, cy, dup = 0;

            if (!marker_black(buf[y * st + x])) continue;
            while (x + len < W && len <= 10 && marker_black(buf[y * st + x + len])) len++;
            if (len < 7 || len > 10) continue;
            while (y + v1 < H && v1 <= 10 && marker_black(buf[(y + v1) * st + x])) v1++;
            while (y + v2 < H && v2 <= 10 && marker_black(buf[(y + v2) * st + x + len - 1])) v2++;
            if (iabs_i(v1 - v2) > 1 || v1 < 7 || v1 > 10) continue;
            if (!marker_black(buf[(y + v1 - 1) * st + x + len - 1])) continue;
            /* nothing black directly above/below: rejects a hollow inside a
             * larger dark structure (the leader line only touches the left) */
            if (y > 0) {
                int q, bad = 0;
                for (q = x + 2; q <= x + len - 3; q++)
                    if (marker_black(buf[(y - 1) * st + q])) bad = 1;
                if (bad) continue;
            }
            if (y + v1 < H) {
                int q, bad = 0;
                for (q = x + 2; q <= x + len - 3; q++)
                    if (marker_black(buf[(y + v1) * st + q])) bad = 1;
                if (bad) continue;
            }
            for (i = 0; i < len; i++) if (marker_black(buf[(y + v1 - 1) * st + x + i])) ok++;
            if (ok < len - 1) continue;

            cx = x + len / 2;
            cy = y + v1 / 2;
            /* the interior may be any colour, black included */
            if (!marker_flat(buf, st, x, y, len, v1)) continue;

            for (i = 0; i < n; i++)
                if (iabs_i(mx[i] - cx) <= 3 && iabs_i(my[i] - cy) <= 3) dup = 1;
            if (dup) continue;
            mx[n] = cx;
            my[n] = cy;
            n++;
        }
    }
    return n;
}

/* fallback: DyeRite's 21 px cell, used only when no markers are on screen */
static void set_cell_offsets(void)
{
    int i;
    for (i = 0; i < MAX_TARGETS; i++) {
        int lo = -(g_cell / 2) - (g_cell & 1);
        int hi = g_cell / 2;
        switch (i) {
        case 0:  g_offX[i] = 0;  g_offY[i] = 0;  break;
        case 1:  g_offX[i] = lo; g_offY[i] = lo; break;
        case 2:  g_offX[i] = hi; g_offY[i] = lo; break;
        case 3:  g_offX[i] = lo; g_offY[i] = hi; break;
        default: g_offX[i] = hi; g_offY[i] = hi; break;
        }
    }
    g_offCount = MAX_TARGETS;
}

/* Build the offsets from the detected markers, relative to the marker nearest
 * the current cursor (that one is the sampled pixel under the pointer). */
static int detect_crosshairs(int *ox, int *oy, int max)
{
    static unsigned char bright[CAP_W * CAP_H];
    const unsigned int *live = (const unsigned int *)g_capBits;
    const unsigned int *ref = g_grid;
    int cand[64], cx[64], cy[64];
    int x, y, i, j, nc = 0, n = 0;

    if (!live || !g_gridValid) return 0;

    for (y = 0; y < CAP_H; y++) {
        for (x = 0; x < CAP_W; x++) {
            unsigned int p = live[y * CAP_W + x], q = ref[y * CAP_W + x];
            int dr = iabs_i((int)((p >> 16) & 0xFF) - (int)((q >> 16) & 0xFF));
            int dg = iabs_i((int)((p >> 8) & 0xFF) - (int)((q >> 8) & 0xFF));
            int db = iabs_i((int)(p & 0xFF) - (int)(q & 0xFF));
            int mn = (int)(p & 0xFF);
            if ((int)((p >> 8) & 0xFF) < mn) mn = (int)((p >> 8) & 0xFF);
            if ((int)((p >> 16) & 0xFF) < mn) mn = (int)((p >> 16) & 0xFF);
            bright[y * CAP_W + x] =
                (dr > 20 || dg > 20 || db > 20) && mn >= 150;
        }
    }

    for (y = 2; y < CAP_H - 2 && nc < 64; y++) {
        for (x = 2; x < CAP_W - 2 && nc < 64; x++) {
            int c = 0, dx, dy;
            if (!bright[y * CAP_W + x]) continue;
            for (dy = -2; dy <= 2; dy++)
                for (dx = -2; dx <= 2; dx++)
                    c += bright[(y + dy) * CAP_W + x + dx];
            if (c < 9) continue;
            cand[nc] = c; cx[nc] = x; cy[nc] = y; nc++;
        }
    }

    /* strongest peaks first, ignoring anything within 5 px of one already kept */
    for (i = 0; i < nc && n < max; i++) {
        int bi = -1, bs = -1, dup = 0;
        for (j = 0; j < nc; j++) {
            if (cand[j] > bs) { bs = cand[j]; bi = j; }
        }
        if (bi < 0) break;
        for (j = 0; j < n; j++)
            if (iabs_i(ox[j] - cx[bi]) <= 5 && iabs_i(oy[j] - cy[bi]) <= 5) dup = 1;
        if (!dup) { ox[n] = cx[bi]; oy[n] = cy[bi]; n++; }
        cand[bi] = -1;
    }
    return n;
}

/* merge one pointer-relative sample offset, ignoring near-duplicates */
/* distance from a legend box's centre to the crosshair it belongs to */
#define CROSSHAIR_DX (-10)

static void add_offset(int dx, int dy)
{
    int j;

    for (j = 0; j < g_offCount; j++) {
        int ex = iabs_i(wrap_mod(dx - g_offX[j] + 127, 254) - 127);
        int ey = iabs_i(wrap_mod(dy - g_offY[j] + 127, 254) - 127);
        if (ex <= 3 && ey <= 3) return;
    }
    if (g_offCount < MAX_TARGETS) {
        g_offX[g_offCount] = dx;
        g_offY[g_offCount] = dy;
        g_offCount++;
    }
}

static int build_offsets(void)
{
    int mx[MAX_TARGETS], my[MAX_TARGETS];
    POINT cur;
    int n, i, anchor = -1, bestd = 13 * 13;

    n = detect_markers(mx, my, MAX_TARGETS);
    g_markerCount = n;
    for (i = 0; i < n; i++) { g_mx[i] = mx[i]; g_my[i] = my[i]; }
    if (n < 2) return 0;      /* caller decides what to fall back to */

    GetCursorPos(&cur);
    cur.x -= g_originX;
    cur.y -= g_originY;
    g_cursorPX = cur.x;
    g_cursorPY = cur.y;

    /* The crosshair is the sample point; its box is drawn CROSSHAIR_DX to the
 * right and shows the colour found there. */
    for (i = 0; i < n; i++) {
        int dx = mx[i] - cur.x, dy = my[i] - cur.y;
        int d = dx * dx + dy * dy;
        if (d <= bestd) { bestd = d; anchor = i; }
    }

    g_offCount = 0;
    for (i = 0; i < n; i++) {
        /* the crosshair sits CROSSHAIR_DX px to the left of the box centre */
        int sx = mx[i] + CROSSHAIR_DX, sy = my[i];
        if (i == anchor) add_offset(0, 0);   /* the pointer's own crosshair */
        else             add_offset(sx - cur.x, sy - cur.y);
    }
    /* the pixel under the pointer is always one of the five samples, even when
     * its own square is hidden by the cursor (detection then reports four) */
    add_offset(0, 0);

    if (g_offCount < 2) return 0;

    g_anchorX = mx[anchor >= 0 ? anchor : 0];
    g_anchorY = my[anchor >= 0 ? anchor : 0];
    g_clickValid = (cur.x >= 0 && cur.x < SCAN_W && cur.y >= 0 && cur.y < SCAN_H);
    g_clickOffX = 0;
    g_clickOffY = 0;
    g_offFromCursor = 1;        /* offsets already include the pointer slot */
    g_patCount = g_offCount;    /* remember it for frames without markers   */
    for (i = 0; i < g_offCount; i++) { g_patX[i] = g_offX[i]; g_patY[i] = g_offY[i]; }
    return 1;
}

/* sample at a wrapped position; DyeLite wraps mod 256 and rejects 254/255 */
static int palette_sample(int x, int y, unsigned int *out)
{
    const unsigned int *cap = (const unsigned int *)g_capBits;
    x = wrap_mod(x, CAP_W);
    y = wrap_mod(y, CAP_H);
    if (x >= SCAN_W || y >= SCAN_H) return 0;
    *out = cap[y * CAP_W + x];
    return 1;
}

/* record the colour each legend box displays */
static void record_marker_fills(void)
{
    const unsigned int *cap = (const unsigned int *)g_capBits;
    int i;

    if (!cap) return;
    for (i = 0; i < g_markerCount && i < MAX_TARGETS; i++)
        g_mfill[i] = cap[g_my[i] * CAP_W + g_mx[i]];
}

static int discover_offsets(void)
{
    static const int cx[2] = { 24, 230 };
    static const int cy[2] = { 24, 230 };
    int mx[MAX_TARGETS], my[MAX_TARGETS];
    int ox[10], oy[10];
    int total = 0, n, i, j, k;

    ensure_foreground();
    for (k = 0; k < 2; k++) {
        if (!move_cursor_to(g_originX + cx[k], g_originY + cy[k])) continue;
        Sleep(350);
        if (!capture_at(g_originX, g_originY)) continue;
        n = detect_markers(mx, my, MAX_TARGETS);
        for (i = 0; i < n && total < 10; i++) {
            ox[total] = mx[i] - cx[k];
            oy[total] = my[i] - cy[k];
            total++;
        }
    }

    g_offCount = 0;
    for (i = 0; i < total; i++) {
        int dup = 0;
        for (j = 0; j < g_offCount; j++) {
            int dx = iabs_i(wrap_mod(ox[i] - g_offX[j] + 127, 254) - 127);
            int dy = iabs_i(wrap_mod(oy[i] - g_offY[j] + 127, 254) - 127);
            if (dx <= 3 && dy <= 3) dup = 1;
        }
        if (!dup && g_offCount < MAX_TARGETS) {
            g_offX[g_offCount] = ox[i];
            g_offY[g_offCount] = oy[i];
            g_offCount++;
        }
    }
    g_offFromCursor = 1;
    return g_offCount;
}

static void picker_eval(int x, int y, int *success, int *dist)
{
    int i, s = 0, d = 0;

    for (i = 0; i < MAX_TARGETS; i++) {
        int dr, dg, db, dd;
        unsigned int px;
        if (!g_targets[i].enabled) continue;
        if (i >= g_offCount) continue;
        if (!palette_sample(x + g_offX[i], y + g_offY[i], &px)) continue;
        dr = (int)((px >> 16) & 0xFF) - g_targets[i].r;
        dg = (int)((px >> 8) & 0xFF) - g_targets[i].g;
        db = (int)(px & 0xFF) - g_targets[i].b;
        dd = dr * dr + dg * dg + db * db;
        d += dd;
        if (iabs_i(dr) <= g_targets[i].tol && iabs_i(dg) <= g_targets[i].tol &&
            iabs_i(db) <= g_targets[i].tol)
            s++;
    }
    *success = s;
    *dist = d;
}

static int picker_search(int *outX, int *outY, int *outSucc, int *outDist)
{
    int x, y, bx = -1, by = -1, bs = -1, bd = 1 << 30;

    for (y = 0; y < SCAN_H; y++) {
        for (x = 0; x < SCAN_W; x++) {
            int s, d;
            picker_eval(x, y, &s, &d);
            if (s > bs || (s == bs && d < bd)) { bs = s; bd = d; bx = x; by = y; }
        }
    }
    if (bx < 0) return 0;
    *outX = bx; *outY = by; *outSucc = bs; *outDist = bd;
    return 1;
}

static void draw_box(int cx, int cy, int half, unsigned int color)
{
    int i;
    for (i = -half; i <= half; i++) {
        g_result[wrap_mod(cy - half, SCAN_H) * SCAN_W + wrap_mod(cx + i, SCAN_W)] = color;
        g_result[wrap_mod(cy + half, SCAN_H) * SCAN_W + wrap_mod(cx + i, SCAN_W)] = color;
        g_result[wrap_mod(cy + i, SCAN_H) * SCAN_W + wrap_mod(cx - half, SCAN_W)] = color;
        g_result[wrap_mod(cy + i, SCAN_H) * SCAN_W + wrap_mod(cx + half, SCAN_W)] = color;
    }
}

/* percentage of the capture that looks like a gradient */
static int capture_is_palette(void)
{
    const unsigned int *buf = (const unsigned int *)g_capBits;
    int x, y, n = 0;

    if (!buf) return 0;
    for (y = 0; y < SCAN_H; y++)
        for (x = 0; x < SCAN_W; x++)
            if (colorful(buf[y * CAP_W + x])) n++;
    return n * 100 / (SCAN_W * SCAN_H);
}

/* log the client's own box fills next to the position about to be clicked */
static void log_confirm(int x, int y, int n, int hits, const unsigned int *fills)
{
    FILE *f = fopen("scan.log", "a");
    int i;

    if (!f) return;
    fprintf(f, "confirm pos=(%d,%d) markers=%d hits=%d/%d need=%d :: ",
            x, y, n, hits, g_offCount, g_desired);
    for (i = 0; i < n && i < MAX_TARGETS; i++)
        fprintf(f, "#%06X ", fills[i] & 0xFFFFFF);
    fprintf(f, "\n");
    fclose(f);
}

/* append one line per scan */
static void log_scan(void)
{
    FILE *f = fopen("scan.log", "a");
    int i;

    if (!f) return;
    fprintf(f, "markers=%d cursor=(%d,%d) anchor=(%d,%d) gap=(%d,%d) valid=%d "
               "best=(%d,%d) click=(%d,%d) succ=%d/%d dist=%d sat=%d%%\n",
            g_markerCount, g_cursorPX, g_cursorPY, g_anchorX, g_anchorY,
            g_clickOffX, g_clickOffY, g_clickValid,
            g_bestX, g_bestY, g_bestX + g_clickOffX, g_bestY + g_clickOffY,
            g_bestSuccess, g_desired, g_bestDist, capture_is_palette());
    for (i = 0; i < g_markerCount && i < MAX_TARGETS; i++)
        fprintf(f, "  marker%d at=(%d,%d) fill=#%06X\n", i, g_mx[i], g_my[i],
                g_mfill[i] & 0xFFFFFF);
    save_bmp24("palette_scan.bmp", (const unsigned int *)g_capBits, CAP_W, CAP_H);
    for (i = 0; i < g_offCount; i++) {
        unsigned int c = 0;
        int sx = g_bestX + g_offX[i], sy = g_bestY + g_offY[i];
        int ok = palette_sample(sx, sy, &c);
        fprintf(f, "  slot%d off=(%d,%d) at=(%d,%d) ok=%d colour=#%06X\n",
                i, g_offX[i], g_offY[i], wrap_mod(sx, CAP_W), wrap_mod(sy, CAP_H),
                ok, c & 0xFFFFFF);
    }
    fclose(f);
}

#define PREV_X 430
#define PREV_Y 10
static void invalidate_preview(void)
{
    RECT r;
    r.left = PREV_X; r.top = PREV_Y;
    r.right = PREV_X + SCAN_W; r.bottom = PREV_Y + SCAN_H;
    if (g_main) InvalidateRect(g_main, &r, FALSE);
}

static void copy_result_to_preview(void)
{
    if (g_prevBits) {
        memcpy(g_prevBits, g_result, sizeof(g_result));
        g_havePreview = 1;
    }
    invalidate_preview();
}

static void show_capture(void);

/* ---- the two operations ------------------------------------------------- */

static void do_find_palette(void)
{
    if (!g_capDC && !capture_init()) { set_status("capture init failed"); return; }
    if (find_color_field(&g_originX, &g_originY)) {
        char msg[128];
        int sat = 0;
        reset_bounds();
        if (capture_at(g_originX, g_originY)) {
            show_capture();
            sat = capture_is_palette();
        }
        sprintf(msg, "colour field found at (%d,%d)  gradient %d%%",
                g_originX, g_originY, sat);
        set_status(msg);
    } else {
        g_originX = g_originY = -1;
        set_status("palette not found - open the dye window, "
                   "or press Pick over its top-left corner");
    }
}

/* show the raw grab in the preview pane */
static void show_capture(void)
{
    int y;
    if (!g_capBits) return;
    for (y = 0; y < SCAN_H; y++)
        memcpy(&g_result[y * SCAN_W],
               (unsigned char *)g_capBits + (size_t)y * CAP_W * 4,
               SCAN_W * 4);
    if (g_prevBits) {
        memcpy(g_prevBits, g_result, sizeof(g_result));
        g_havePreview = 1;
    }
    invalidate_preview();
}

/* take the current mouse position as the palette's top-left */
static void do_pick_origin(void)
{
    POINT pt;
    char msg[96];

    GetCursorPos(&pt);
    g_originX = pt.x;
    g_originY = pt.y;
    if (!g_capDC && !capture_init()) { set_status("capture init failed"); return; }
    if (!capture_at(g_originX, g_originY)) { set_status("capture failed"); return; }
    reset_bounds();
    show_capture();
    sprintf(msg, "origin picked at (%d,%d) - press Scan", g_originX, g_originY);
    set_status(msg);
}

/* does the last capture look like the palette? */
static int capture_range(void)
{
    const unsigned int *b = (const unsigned int *)g_capBits;
    int x, y, mn = 255, mx = 0;

    if (!b) return 0;
    for (y = 0; y < CAP_H; y += 4) {
        for (x = 0; x < CAP_W; x += 4) {
            int l = lum_of(b[y * CAP_W + x]);
            if (l < mn) mn = l;
            if (l > mx) mx = l;
        }
    }
    return mx - mn;
}

/* eyedropper: use the colour under the cursor */
static void do_pick_color(void)
{
    POINT pt;
    HDC dc;
    COLORREF c;
    char buf[16];

    GetCursorPos(&pt);
    dc = GetDC(NULL);
    c = GetPixel(dc, pt.x, pt.y);
    ReleaseDC(NULL, dc);
    if (c == CLR_INVALID) { set_status("colour pick failed"); return; }

    sprintf(buf, "%02X%02X%02X", GetRValue(c), GetGValue(c), GetBValue(c));
    SetWindowTextA(g_editHex, buf);
    sprintf(buf, "picked #%s at (%d,%d) - press Scan", buf, pt.x, pt.y);
    set_status(buf);
}

static void do_scan(void)
{
    char msg[128];
    int i;

    if (g_originX < 0 && !find_color_field(&g_originX, &g_originY)) {
        set_status("colour field not found");
        return;
    }
    if (g_useSaved && g_gridValid) {
        /* the gradient saved for this attempt, without the legend */
        memcpy(g_capBits, g_grid, sizeof(g_grid));
    } else {
        if (!capture_at(g_originX, g_originY)) {
            set_status("capture failed");
            return;
        }
        /* re-detect if the cached origin no longer shows a palette */
        if (capture_is_palette() < 25) {      /* stale origin / not the palette */
            g_originX = -1;
            if (!find_color_field(&g_originX, &g_originY) ||
                !capture_at(g_originX, g_originY)) {
                set_status("palette lost - reopen the dye window or press Pick");
                return;
            }
            if (capture_is_palette() < 20) {
                set_status("that does not look like the palette - press Find or Pick");
                return;
            }
        }
        /* keep this grab as the search image */
        memcpy(g_grid, g_capBits, sizeof(g_grid));
        g_gridValid = 1;
    }
    read_targets();

    if (origin_is_covered()) {
        set_status("move this window off the palette - it is covering the colours");
        return;
    }

    reset_bounds();

    g_enabledCount = 0;
    for (i = 0; i < MAX_TARGETS; i++)
        if (g_targets[i].enabled) g_enabledCount++;

    /* preview starts as the palette itself, then the five sample boxes */
    {
        const unsigned int *cap = (const unsigned int *)g_capBits;
        int y, x;
        for (y = 0; y < SCAN_H; y++) {
            for (x = 0; x < SCAN_W; x++) {
                unsigned int p = cap[y * CAP_W + x];
                int dr = iabs_i((int)((p >> 16) & 0xFF) - g_colorR);
                int dg = iabs_i((int)((p >> 8) & 0xFF) - g_colorG);
                int db = iabs_i((int)(p & 0xFF) - g_colorB);
                g_result[y * SCAN_W + x] =
                    (dr <= g_var && dg <= g_var && db <= g_var)
                        ? 0x00FF0000u      /* match: red, like the original */
                        : 0x00FFFFFFu;     /* background: white             */
            }
        }
    }

    if (!g_enabledCount) {
        set_status("tick at least one colour slot");
        copy_result_to_preview();
        return;
    }

    {
        int usedMarkers;

        /* measure the offsets on this frame, not on an earlier one */
        /* reuse the table measured in state 1 */
        usedMarkers = (g_useSaved && g_offCount >= MIN_SLOTS) ? 1 : build_offsets();
        if (g_offCount < MIN_SLOTS) {
            /* the offsets only exist while the pointer is on the gradient */
            ensure_foreground();
            if (move_cursor_to(g_originX + SCAN_W / 2, g_originY + SCAN_H / 2)) {
                mouse_event(MOUSEEVENTF_MOVE, 3, 2, 0, 0);
                Sleep(120);
                mouse_event(MOUSEEVENTF_MOVE, -3, -2, 0, 0);
                Sleep(300);
                if (capture_at(g_originX, g_originY))
                    usedMarkers = build_offsets();
            }
        }
        if (g_offCount < MIN_SLOTS) {
            /* nudge so the client re-lays its squares */
            int tries;
            for (tries = 0; tries < 3 && g_offCount < MIN_SLOTS; tries++) {
                mouse_event(MOUSEEVENTF_MOVE, (tries & 1) ? 4 : -4, 3, 0, 0);
                Sleep(170);
                if (!capture_at(g_originX, g_originY)) break;
                usedMarkers = build_offsets();
            }
        }
        if (g_offCount < MIN_SLOTS && g_patCount >= MIN_SLOTS) {
            /* pointer off the gradient: reuse the pattern from an earlier frame */
            int mx2[MAX_TARGETS], my2[MAX_TARGETS];
            int n2 = detect_markers(mx2, my2, MAX_TARGETS), k;
            g_markerCount = n2;
            for (k = 0; k < n2; k++) { g_mx[k] = mx2[k]; g_my[k] = my2[k]; }
            g_offCount = g_patCount;
            for (k = 0; k < g_patCount; k++) {
                g_offX[k] = g_patX[k];
                g_offY[k] = g_patY[k];
            }
            g_offFromCursor = 1;
            usedMarkers = 1;
        }
        if (g_offCount < 2) {
            set_cell_offsets();
            g_offFromCursor = 0;
            usedMarkers = 0;
        }
        if (g_gridValid) {
            /* the legend colours, then the saved (legend-free) gradient */
            record_marker_fills();
            memcpy(g_capBits, g_grid, sizeof(g_grid));
        }
        if (!usedMarkers)
            set_status("no markers found - hover the palette so the squares show");
        if (picker_search(&g_bestX, &g_bestY, &g_bestSuccess, &g_bestDist)) {
            for (i = 0; i < g_offCount && i < MAX_TARGETS; i++) {
                unsigned int c = 0;
                int ok = 0;
                if (palette_sample(g_bestX + g_offX[i], g_bestY + g_offY[i], &c)) {
                    ok = iabs_i((int)((c >> 16) & 0xFF) - g_colorR) <= g_var &&
                         iabs_i((int)((c >> 8) & 0xFF) - g_colorG) <= g_var &&
                         iabs_i((int)(c & 0xFF) - g_colorB) <= g_var;
                }
                draw_box(g_bestX + g_offX[i], g_bestY + g_offY[i], 5, 0x00000000u);
                draw_box(g_bestX + g_offX[i], g_bestY + g_offY[i], 4,
                         ok ? 0x0000FF00u : 0x0000FFFFu);
            }
            g_hits = g_bestSuccess;
            sprintf(msg, "%d of %d samples hold #%06X (need %d) - cursor to (%d,%d) dist=%d",
                    g_bestSuccess, g_offCount,
                    (g_colorR << 16) | (g_colorG << 8) | g_colorB,
                    g_desired, g_bestX, g_bestY, g_bestDist);
            set_status(msg);
        } else {
            g_hits = 0;
            set_status("no palette data");
        }
        log_scan();
    }
    copy_result_to_preview();
}

/* one iteration per timer tick */
static HWND game_window(void)
{
    return FindWindowA("Mabinogi", NULL);
}

/* raise the client before sending it clicks */
static void ensure_foreground(void)
{
    HWND w = game_window();
    DWORD t = 0;

    if (!w) return;
    if (GetForegroundWindow() == w) return;
    SystemParametersInfoA(0x2000, 0, &t, 0);          /* get lock timeout */
    SystemParametersInfoA(0x2001, 0, (PVOID)0, 0);    /* set to 0         */
    SetForegroundWindow(w);
    Sleep(250);
    SystemParametersInfoA(0x2001, 0, (PVOID)(UINT_PTR)t, 0);
}

/* move the cursor and check it arrived (the client needs the injected
 * move, SetCursorPos alone is not enough) */
static int move_cursor_to(int x, int y)
{
    POINT p;
    int tries;
    int sw = GetSystemMetrics(SM_CXSCREEN) - 1;
    int sh = GetSystemMetrics(SM_CYSCREEN) - 1;

    for (tries = 0; tries < 3; tries++) {
        SetCursorPos(x, y);
        if (sw > 0 && sh > 0)
            mouse_event(MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                        (DWORD)((long long)x * 65535 / sw),
                        (DWORD)((long long)y * 65535 / sh), 0, 0);
        Sleep(40);
        GetCursorPos(&p);
        if (iabs_i(p.x - x) <= 1 && iabs_i(p.y - y) <= 1)
            return 1;
    }
    return 0;
}

/* click at a screen position with mouseDelay between press and release */
static void click_screen(int x, int y)
{
    if (!move_cursor_to(x, y)) return;
    Sleep(g_mouseDelay);
    mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0);
    Sleep(g_mouseDelay);
    mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
}

static void click_pick_color(void) { click_screen(g_originX + g_btnPickX, g_originY + g_btnY); }
static void click_cancel(void)     { click_screen(g_originX + g_btnCancelX, g_originY + g_btnY); }

/* Auto cycle:
 *   0. click Pick Color
 *   1. capture the gradient (no legend on it), move onto the grid, read the
 *      five sample offsets, search
 *   2. match -> move to the position and click, then stop
 *   3. no match -> click Cancel and start over
 * The spacebar or Stop aborts at any point. */
/* raise the client and wait until the dye palette is actually on screen */
static int wait_for_gradient(void)
{
    int tries;

    for (tries = 0; tries < 16; tries++) {
        if (g_originX >= 0 && capture_at(g_originX, g_originY) &&
            capture_is_palette() >= 25)
            return 1;
        g_originX = -1;
        if (find_color_field(&g_originX, &g_originY) &&
            capture_at(g_originX, g_originY) && capture_is_palette() >= 25)
            return 1;
        Sleep(250);
    }
    return 0;
}

static void auto_step(void)
{
    if ((GetAsyncKeyState(VK_SPACE) & 0x8000) != 0) {
        g_autoMode = 0;
        g_autoState = 0;
        KillTimer(g_main, 1);
        set_status("auto mode stopped (spacebar)");
        return;
    }

    switch (g_autoState) {
    case 0:                                  /* start: click Pick Color */
        ensure_foreground();
        if (!wait_for_gradient()) {
            set_status("no gradient - open the dye window and leave it visible");
            g_autoMode = 0;
            KillTimer(g_main, 1);
            return;
        }
        read_targets();
        g_btnPickX = read_int(g_editPickX, 147);
        g_btnCancelX = read_int(g_editCancelX, 231);
        g_btnY = read_int(g_editBtnY, 397);
        ensure_foreground();
        g_useSaved = 0;
        g_gridValid = 0;
        /* roll first; the gradient is grabbed once, in state 1 */
        click_pick_color();
        g_autoState = 1;
        g_autoTicks = 0;
        set_status("auto: clicked Pick Color");
        return;

    case 1:                                  /* discover, then scan */
        if (g_autoTicks == 0) {
            ensure_foreground();
            /* the grab for this attempt: the pointer is on the button, no legend */
            Sleep(260);
            if (capture_at(g_originX, g_originY)) {
                memcpy(g_grid, g_capBits, sizeof(g_grid));
                g_gridValid = 1;
            }
            if (!g_gridValid) {
                set_status("auto: no gradient to search - cancelling");
                g_autoState = 3;
                return;
            }
            /* now move onto the grid and read the five sample offsets */
            set_status("auto: reading the sample offsets");
            move_cursor_to(g_originX + SCAN_W / 2, g_originY + SCAN_H / 2);
            mouse_event(MOUSEEVENTF_MOVE, 3, 2, 0, 0);
            Sleep(120);
            mouse_event(MOUSEEVENTF_MOVE, -3, -2, 0, 0);
            Sleep(320);
            if (capture_at(g_originX, g_originY)) {
                build_offsets();
                /* each box shows the colour at box + CROSSHAIR_DX */
                {
                    FILE *g = fopen("autotest.txt", "a");
                    const unsigned int *cap = (const unsigned int *)g_capBits;
                    int k;
                    if (g) {
                        fprintf(g, "changed px at hover: %d\n", changed_pixels());
                        for (k = 0; k < g_markerCount; k++) {
                            unsigned int fill = cap[g_my[k] * CAP_W + g_mx[k]];
                            unsigned int at0 = grid_at(g_grid, g_mx[k], g_my[k]);
                            unsigned int at10 = grid_at(g_grid, g_mx[k] + CROSSHAIR_DX,
                                                        g_my[k]);
                            fprintf(g, "  box(%3d,%3d) fill=#%06X  grid@box=#%06X  "
                                       "grid@box-10=#%06X %s\n",
                                    g_mx[k], g_my[k], fill & 0xFFFFFF, at0, at10,
                                    (fill & 0xFFFFFF) == at10 ? "MATCH" : "");
                        }
                        fclose(g);
                    }
                }
            }
            g_useSaved = 1;
            g_autoTicks = 1;
        }
        g_autoTicks++;
        if (g_autoTicks < 3) return;
        do_scan();

        if (g_offCount < MIN_SLOTS) {
            char m[96];
            sprintf(m, "auto: only %d sample squares visible - cancelling",
                    g_offCount);
            set_status(m);
            g_autoState = 3;
            return;
        }
        if (g_hits >= g_desired)
            g_autoState = 2;
        else
            g_autoState = 3;
        return;

    case 2:                                  /* move to the match and click */
        if (g_noClick) {
            char m[128];
            sprintf(m, "autotest: would click (%d,%d) %d/%d samples",
                    g_bestX, g_bestY, g_bestSuccess, g_desired);
            set_status(m);
            {
                FILE *f = fopen("autotest.txt", "a");
                if (f) { fprintf(f, "%s\n", m); fclose(f); }
            }
            g_autoMode = 0;          /* stop here; autotest() diagnoses */
            return;
        }
        ensure_foreground();
        if (!move_cursor_to(g_originX + g_bestX + (g_offFromCursor ? 0 : g_clickOffX),
                            g_originY + g_bestY + (g_offFromCursor ? 0 : g_clickOffY))) {
            set_status("auto: cursor did not reach the match - cycle aborted");
            g_autoMode = 0;
            g_autoState = 0;
            KillTimer(g_main, 1);
            return;
        }
        /* read the client's own boxes at this position */
        Sleep(260);
        {
            int mx[MAX_TARGETS], my[MAX_TARGETS], n, i, ok = 0;
            unsigned int fill[MAX_TARGETS];

            if (capture_at(g_originX, g_originY)) {
                n = detect_markers(mx, my, MAX_TARGETS);
                for (i = 0; i < n; i++)
                    fill[i] = ((const unsigned int *)g_capBits)[my[i] * CAP_W + mx[i]];
                for (i = 0; i < n; i++)
                    if (iabs_i((int)((fill[i] >> 16) & 0xFF) - g_colorR) <= g_var &&
                        iabs_i((int)((fill[i] >> 8) & 0xFF) - g_colorG) <= g_var &&
                        iabs_i((int)(fill[i] & 0xFF) - g_colorB) <= g_var) ok++;
                log_confirm(g_bestX, g_bestY, n, ok, fill);
                /* matched -> click; the box reading above is informational only */
            }
        }
        Sleep(g_mouseDelay);
        mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0);
        Sleep(g_mouseDelay);
        mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
        {
            char m[96];
            sprintf(m, "auto: clicked (%d,%d)  %d slots match (need %d)",
                    g_bestX + (g_offFromCursor ? 0 : g_clickOffX),
                    g_bestY + (g_offFromCursor ? 0 : g_clickOffY),
                    g_bestSuccess, g_desired);
            set_status(m);
        }
        g_autoMode = 0;
        g_autoState = 0;
        KillTimer(g_main, 1);
        return;

    default:                                 /* no match: cancel and retry */
        click_cancel();
        Sleep(300);
        g_autoState = 0;
        set_status("auto: no match, cancelled - retrying");
        return;
    }
}

/* ---- window procedure -------------------------------------------------- */

static void create_controls(HWND h)
{
    int y = 10;
    HINSTANCE inst = (HINSTANCE)GetWindowLongA(h, GWL_HINSTANCE);

    CreateWindowA("STATIC", "colour #", WS_CHILD | WS_VISIBLE,
                  8, y + 3, 60, 18, h, NULL, inst, NULL);
    g_editHex = CreateWindowA("EDIT", "000000", WS_CHILD | WS_VISIBLE | WS_BORDER,
                              70, y, 80, 22, h, (HMENU)400, inst, NULL);
    CreateWindowA("STATIC", "variance", WS_CHILD | WS_VISIBLE,
                  158, y + 3, 60, 18, h, NULL, inst, NULL);
    g_editVar = CreateWindowA("EDIT", "0", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER,
                              218, y, 40, 22, h, (HMENU)401, inst, NULL);
    CreateWindowA("STATIC", "slots", WS_CHILD | WS_VISIBLE,
                  266, y + 3, 40, 18, h, NULL, inst, NULL);
    g_editSlots = CreateWindowA("EDIT", "5", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER,
                                308, y, 32, 22, h, (HMENU)402, inst, NULL);

    y = 42;
    g_btnFind = CreateWindowA("BUTTON", "Find field", WS_CHILD | WS_VISIBLE,
                              8, y, 80, 26, h, (HMENU)200, inst, NULL);
    g_btnScan = CreateWindowA("BUTTON", "Scan", WS_CHILD | WS_VISIBLE,
                              94, y, 60, 26, h, (HMENU)201, inst, NULL);
    g_btnAuto = CreateWindowA("BUTTON", "Auto", WS_CHILD | WS_VISIBLE,
                              160, y, 60, 26, h, (HMENU)202, inst, NULL);
    g_btnStop = CreateWindowA("BUTTON", "Stop", WS_CHILD | WS_VISIBLE,
                              226, y, 60, 26, h, (HMENU)203, inst, NULL);
    g_btnPick = CreateWindowA("BUTTON", "Pick", WS_CHILD | WS_VISIBLE,
                              292, y, 50, 26, h, (HMENU)204, inst, NULL);
    g_btnColor = CreateWindowA("BUTTON", "Pick col", WS_CHILD | WS_VISIBLE,
                               348, y, 62, 26, h, (HMENU)205, inst, NULL);

    y = 76;
    CreateWindowA("STATIC", "btn x/y:", WS_CHILD | WS_VISIBLE,
                  8, y + 2, 56, 18, h, NULL, inst, NULL);
    g_editPickX = CreateWindowA("EDIT", "147", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER,
                                64, y, 40, 20, h, (HMENU)303, inst, NULL);
    g_editCancelX = CreateWindowA("EDIT", "231", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER,
                                  108, y, 40, 20, h, (HMENU)304, inst, NULL);
    g_editBtnY = CreateWindowA("EDIT", "397", WS_CHILD | WS_VISIBLE | WS_BORDER | ES_NUMBER,
                               152, y, 40, 20, h, (HMENU)305, inst, NULL);

    g_status = CreateWindowA("STATIC", "ready", WS_CHILD | WS_VISIBLE,
                             8, y + 26, 400, 20, h, NULL, inst, NULL);

    g_preview = NULL;        /* the preview is painted by the main window */
}

static LRESULT CALLBACK wndproc(HWND h, UINT msg, WPARAM wp, LPARAM lp)
{
    switch (msg) {
    case WM_CREATE:
        create_controls(h);
        SetTimer(h, 2, 500, NULL);       /* refresh the preview */
        return 0;

    case WM_COMMAND:
        switch (LOWORD(wp)) {
        case 200: do_find_palette(); return 0;
        case 201: g_useSaved = 0; do_scan(); return 0;
        case 202:
            g_autoMode = 1;
            g_autoState = 0;
            g_autoTicks = 0;
            SetTimer(h, 1, 400, NULL);
            set_status("auto mode running - spacebar or Stop to end");
            return 0;
        case 203:
            g_autoMode = 0;
            g_autoState = 0;
            KillTimer(h, 1);
            set_status("stopped");
            return 0;
        case 204:
            do_pick_origin();
            return 0;
        case 205:
            do_pick_color();
            return 0;
        }
        return 0;

    case WM_TIMER:
        if (wp == 1 && g_autoMode) auto_step();
        if (wp == 2) invalidate_preview();
        return 0;

    case WM_PAINT:
        {
            PAINTSTRUCT ps;
            HDC dc = BeginPaint(h, &ps);
            if (g_prevBits && g_havePreview) {
                BITMAPINFO bi;
                ZeroMemory(&bi, sizeof(bi));
                bi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
                bi.bmiHeader.biWidth = SCAN_W;
                bi.bmiHeader.biHeight = -SCAN_H;
                bi.bmiHeader.biPlanes = 1;
                bi.bmiHeader.biBitCount = 32;
                bi.bmiHeader.biCompression = BI_RGB;
                SetDIBitsToDevice(dc, PREV_X, PREV_Y, SCAN_W, SCAN_H, 0, 0, 0,
                                  SCAN_H, g_prevBits, &bi, DIB_RGB_COLORS);
            }
            EndPaint(h, &ps);
        }
        return 0;

    case WM_CTLCOLORSTATIC:
        SetBkColor((HDC)wp, GetSysColor(COLOR_BTNFACE));
        return (LRESULT)g_bg;

    case WM_DESTROY:
        KillTimer(h, 1);
        KillTimer(h, 2);
        PostQuitMessage(0);
        return 0;
    }
    return DefWindowProcA(h, msg, wp, lp);
}

/* ---- self test --------------------------------------------------------- */

static int g_fail;

static void check(int cond, const char *what)
{
    FILE *f = fopen("selftest.txt", "a");
    if (f) { fprintf(f, "%s %s\n", cond ? "PASS" : "FAIL", what); fclose(f); }
    if (!cond) g_fail++;
}

static void self_test(void)
{
    unsigned int *img;
    int i, x, y, n;

    DeleteFileA("selftest.txt");
    if (!g_capDC) capture_init();

    img = (unsigned int *)malloc(sizeof(unsigned int) * CAP_W * CAP_H);
    for (i = 0; i < CAP_W * CAP_H; i++)
        img[i] = 0x00202020u;                     /* dark grey */

    /* colourful field: 200x200 gradient at (30,30) -> detectable */
    for (y = 30; y < 230; y++)
        for (x = 30; x < 230; x++)
            img[y * CAP_W + x] = (unsigned int)((x * 4) << 16) |
                                 (unsigned int)((y * 4) << 8) | 0x40u;

    /* target patch: exactly RGB(120,60,200), 40x40 at (100,100) */
    for (y = 100; y < 140; y++)
        for (x = 100; x < 140; x++)
            img[y * CAP_W + x] = (120u << 16) | (60u << 8) | 200u;

    /* two isolated pixels of the same colour, away from the patch */
    img[60 * CAP_W + 60] = (120u << 16) | (60u << 8) | 200u;
    img[240 * CAP_W + 240] = (120u << 16) | (60u << 8) | 200u;

    /* 1. matcher finds exactly the patch + the two single pixels.
     *    The palette is scanned 1:1, so the patch keeps its coordinates. */
    reset_bounds();
    n = scan_buffer(img, CAP_W, 120, 60, 200, 4, 1, 1);
    check(n == 40 * 40 + 2, "matcher hit count = patch + 2 singles");

    /* 2. mask semantics: a second, secondary pass must not recolour hits */
    reset_bounds();
    scan_buffer(img, CAP_W, 120, 60, 200, 0, 0, 0);
    check(g_result[100 * SCAN_W + 100] == COL_RED, "primary hit stays red after 2nd pass");

    /* 3. tolerance actually matters: tolerance 0 with an off-by-one colour */
    reset_bounds();
    n = scan_buffer(img, CAP_W, 121, 60, 200, 0, 1, 1);
    check(n == 0, "tolerance 0 rejects off-by-one colour");

    /* 4. field detector finds the colourful area */
    {
        int fx = -1, fy = -1;
        check(find_field_in_buffer(img, CAP_W, CAP_W, CAP_H, &fx, &fy) == 1,
              "colour field detected");
        check(fx >= 0 && fx <= 60 && fy >= 0 && fy <= 60,
              "detected field near the colourful area");
    }

    /* 5. a uniform (blank) image yields no field and no hits */
    for (i = 0; i < CAP_W * CAP_H; i++) img[i] = 0x00FFFFFFu;
    {
        int fx = -1, fy = -1;
        check(find_field_in_buffer(img, CAP_W, CAP_W, CAP_H, &fx, &fy) == 0,
              "blank image rejected by field detector");
        reset_bounds();
        check(scan_buffer(img, CAP_W, 120, 60, 200, 4, 1, 1) == 0,
              "blank image has no hits");
    }

    /* 6. black inside the palette must be hittable (black is a real dye
     *    colour, so nothing may discard dark rows). */
    for (i = 0; i < CAP_W * CAP_H; i++) img[i] = 0x00204080u;
    for (y = 100; y < 120; y++)
        for (x = 60; x < 80; x++)
            img[y * CAP_W + x] = 0x00000000u;
    reset_bounds();
    check(scan_buffer(img, CAP_W, 0, 0, 0, 0, 1, 1) == 20 * 20,
          "black patch inside the palette is hittable");

    free(img);

    /* 7. five-point picker: place five distinct colours at the sample points of
     *    a known (near-border, so wrapping) position and find it again */
    if (g_capBits) {
        const int px0 = 4, py0 = 250;
        unsigned int *cap = (unsigned int *)g_capBits;
        int i, dx, dy, wx, wy, fx = -1, fy = -1, succ = 0, dist = 0;

        for (i = 0; i < CAP_W * CAP_H; i++) cap[i] = 0x00123456u;
        g_cell = 21;
        set_cell_offsets();
        for (i = 0; i < MAX_TARGETS; i++) {
            unsigned int c;
            dx = g_offX[i];
            dy = g_offY[i];
            wx = ((px0 + dx) % CAP_W + CAP_W) % CAP_W;
            wy = ((py0 + dy) % CAP_H + CAP_H) % CAP_H;
            c = ((unsigned)(40 + i * 30) << 16) | ((unsigned)(80 + i * 20) << 8) |
                (unsigned)(120 + i * 10);
            cap[wy * CAP_W + wx] = c;
            g_targets[i].r = (int)((c >> 16) & 0xFF);
            g_targets[i].g = (int)((c >> 8) & 0xFF);
            g_targets[i].b = (int)(c & 0xFF);
            g_targets[i].tol = 0;
            g_targets[i].enabled = 1;
        }
        check(picker_search(&fx, &fy, &succ, &dist) == 1 && succ == 5,
              "picker finds all five samples");
        check(fx == px0 && fy == py0, "picker location correct with wrap-around");
    }

    /* 8. the square detector finds a 256x256 palette on a flat background */
    {
        unsigned int *big = (unsigned int *)malloc(sizeof(unsigned int) * 400 * 400);
        int fx = -1, fy = -1, found;

        for (i = 0; i < 400 * 400; i++) big[i] = 0x00303030u;
        for (y = 40; y < 40 + CAP_H; y++)
            for (x = 30; x < 30 + CAP_W; x++)
                big[y * 400 + x] = (unsigned int)((x * 3) << 16) |
                                   (unsigned int)((y * 3) << 8) | 0x80u;

        found = find_square_field(big, 400, 400, 400, &fx, &fy);
        check(found == 1, "square palette detector finds the field");
        check(found && iabs_i(fx - 30) <= 1 && iabs_i(fy - 40) <= 1,
              "square detector snaps to the exact border");
        free(big);
    }

    {
        FILE *f = fopen("selftest.txt", "a");
        if (f) { fprintf(f, "\n%s (%d failure(s))\n", g_fail ? "FAILED" : "ALL OK", g_fail); fclose(f); }
    }
}


/* ---- probe mode ---------------------------------------------------------
 * dyetrace.exe --probe
 * Raises the Mabinogi window, grabs it, runs the palette detector and writes
 * window.bmp / window_detected.bmp / palette.bmp / hits.bmp / probe.txt so the
 * detection can be checked against the real UI.
 */
static int save_bmp24(const char *path, const unsigned int *px, int w, int h)
{
    FILE *f;
    BITMAPFILEHEADER fh;
    BITMAPINFOHEADER ih;
    unsigned char *row;
    int rowsize = (w * 3 + 3) & ~3;
    int x, y;

    if (!px) return 0;
    f = fopen(path, "wb");
    if (!f) return 0;
    ZeroMemory(&fh, sizeof(fh));
    ZeroMemory(&ih, sizeof(ih));
    ih.biSize = sizeof(ih);
    ih.biWidth = w;
    ih.biHeight = -h;                /* top-down, matches the buffer */
    ih.biPlanes = 1;
    ih.biBitCount = 24;
    ih.biCompression = BI_RGB;
    ih.biSizeImage = rowsize * h;
    fh.bfType = 0x4D42;
    fh.bfOffBits = (DWORD)(sizeof(fh) + sizeof(ih));
    fh.bfSize = fh.bfOffBits + ih.biSizeImage;
    fwrite(&fh, 1, sizeof(fh), f);
    fwrite(&ih, 1, sizeof(ih), f);

    row = (unsigned char *)malloc(rowsize);
    for (y = h - 1; y >= 0; y--) {
        for (x = 0; x < w; x++) {
            unsigned int p = px[(size_t)y * w + x];
            row[x * 3 + 0] = (unsigned char)(p & 0xFF);
            row[x * 3 + 1] = (unsigned char)((p >> 8) & 0xFF);
            row[x * 3 + 2] = (unsigned char)((p >> 16) & 0xFF);
        }
        for (x = w * 3; x < rowsize; x++) row[x] = 0;
        fwrite(row, 1, rowsize, f);
    }
    free(row);
    fclose(f);
    return 1;
}

static void outline(unsigned int *px, int stride, int x, int y, int w, int h,
                    unsigned int color)
{
    int i;
    for (i = 0; i < w; i++) {
        if (y >= 0 && x + i >= 0) px[(size_t)y * stride + x + i] = color;
        if (y + h - 1 >= 0 && x + i >= 0)
            px[(size_t)(y + h - 1) * stride + x + i] = color;
    }
    for (i = 0; i < h; i++) {
        if (x >= 0 && y + i >= 0) px[(size_t)(y + i) * stride + x] = color;
        if (x + w - 1 >= 0 && y + i >= 0)
            px[(size_t)(y + i) * stride + x + w - 1] = color;
    }
}

/* cluster changed pixels and log their boxes (marker discovery) */
static int diff_amount(unsigned int a, unsigned int b)
{
    int dr = (int)((a >> 16) & 0xFF) - (int)((b >> 16) & 0xFF);
    int dg = (int)((a >> 8) & 0xFF) - (int)((b >> 8) & 0xFF);
    int db = (int)(a & 0xFF) - (int)(b & 0xFF);
    if (dr < 0) dr = -dr;
    if (dg < 0) dg = -dg;
    if (db < 0) db = -db;
    return dr > dg ? (dr > db ? dr : db) : (dg > db ? dg : db);
}

static int cluster_diff(const unsigned int *a, const unsigned int *b, FILE *log,
                        int cursorX, int cursorY)
{
    static unsigned char seen[CAP_W * CAP_H];
    static unsigned char hit[CAP_W * CAP_H];
    int i, x, y, n = 0;

    for (i = 0; i < CAP_W * CAP_H; i++) {
        seen[i] = 0;
        hit[i] = diff_amount(a[i], b[i]) > 24;
        if (hit[i]) n++;
    }
    fprintf(log, "diff pixels: %d\n", n);

    for (y = 0; y < CAP_H; y++) {
        for (x = 0; x < CAP_W; x++) {
            int idx = y * CAP_W + x;
            int stack[4096], sp = 0, cnt = 0;
            int minx, maxx, miny, maxy;
            if (!hit[idx] || seen[idx]) continue;
            minx = maxx = x; miny = maxy = y;
            seen[idx] = 1;
            stack[sp++] = idx;
            while (sp > 0) {
                int cur = stack[--sp];
                int cx = cur % CAP_W, cy = cur / CAP_W;
                int d;
                cnt++;
                if (cx < minx) minx = cx;
                if (cx > maxx) maxx = cx;
                if (cy < miny) miny = cy;
                if (cy > maxy) maxy = cy;
                for (d = 0; d < 4; d++) {
                    int nx = cx + (d == 0) - (d == 1);
                    int ny = cy + (d == 2) - (d == 3);
                    int nidx;
                    if (nx < 0 || ny < 0 || nx >= CAP_W || ny >= CAP_H) continue;
                    nidx = ny * CAP_W + nx;
                    if (hit[nidx] && !seen[nidx]) { seen[nidx] = 1; stack[sp++] = nidx; }
                }
            }
            if (cnt >= 20)
                fprintf(log, "  marker? box=(%d,%d)-(%d,%d) %dx%d px=%d  rel-to-cursor=(%d,%d)\n",
                        minx, miny, maxx, maxy, maxx - minx + 1, maxy - miny + 1, cnt,
                        (minx + maxx) / 2 - cursorX, (miny + maxy) / 2 - cursorY);
        }
    }
    return n;
}

static void probe(void)
{
    FILE *log = fopen("probe.txt", "w");
    HWND wnd, fg;
    RECT rc;
    HDC mem;
    HBITMAP bmp;
    void *bits = NULL;
    BITMAPINFO bi;
    int wx, wy, ww, wh, ok, ox, oy, e = 0, r = 0, hx = -1, hy = -1, hok;
    DWORD lockTimeout = 0;

    if (!log) return;
    setvbuf(log, NULL, _IONBF, 0);          /* survive a crash while debugging */

    if (!g_capDC && !capture_init()) {
        fprintf(log, "capture_init failed\n");
        fclose(log);
        return;
    }

    wnd = FindWindowA("Mabinogi", NULL);
    if (!wnd) { fprintf(log, "no window with class/title 'Mabinogi'\n"); fclose(log); return; }

    /* the client must be foreground: the palette is captured from the screen */
    SystemParametersInfoA(0x2000, 0, &lockTimeout, 0);      /* get lock timeout */
    SystemParametersInfoA(0x2001, 0, (PVOID)0, 0);          /* set to 0        */
    SetForegroundWindow(wnd);
    Sleep(700);
    fg = GetForegroundWindow();
    fprintf(log, "window=%p foreground=%p match=%d\n", (void *)wnd, (void *)fg, fg == wnd);

    GetWindowRect(wnd, &rc);
    wx = rc.left; wy = rc.top; ww = rc.right - rc.left; wh = rc.bottom - rc.top;
    fprintf(log, "rect=%d,%d %dx%d\n", wx, wy, ww, wh);

    ZeroMemory(&bi, sizeof(bi));
    bi.bmiHeader.biSize = sizeof(BITMAPINFOHEADER);
    bi.bmiHeader.biWidth = ww;
    bi.bmiHeader.biHeight = -wh;
    bi.bmiHeader.biPlanes = 1;
    bi.bmiHeader.biBitCount = 32;
    bi.bmiHeader.biCompression = BI_RGB;

    mem = CreateCompatibleDC(NULL);
    bmp = CreateDIBSection(mem, &bi, DIB_RGB_COLORS, &bits, NULL, 0);
    SelectObject(mem, bmp);
    BitBlt(mem, 0, 0, ww, wh, GetDC(NULL), wx, wy, SRCCOPY);
    save_bmp24("window.bmp", (const unsigned int *)bits, ww, wh);
    fprintf(log, "saved window.bmp\n");

    ok = find_square_field((const unsigned int *)bits, ww, ww, wh, &ox, &oy);
    if (ok)
        square_score((const unsigned int *)bits, ww, ox, oy, &e, &r);
    fprintf(log, "square: ok=%d origin=(%d,%d) edge=%d range=%d\n", ok, ox, oy, e, r);

    hok = find_field_in_buffer((const unsigned int *)bits, ww, ww, wh, &hx, &hy);
    fprintf(log, "heuristic: ok=%d origin=(%d,%d)\n", hok, hx, hy);

    if (ok) {
        unsigned int *b = (unsigned int *)bits;
        outline(b, ww, ox, oy, CAP_W, CAP_H, 0x00FF00FFu);
        save_bmp24("window_detected.bmp", b, ww, wh);
        g_originX = wx + ox;
        g_originY = wy + oy;
        fprintf(log, "screen origin=%d,%d\n", g_originX, g_originY);
        if (!capture_at(g_originX, g_originY))
            fprintf(log, "capture_at failed\n");
        save_bmp24("palette.bmp", (const unsigned int *)g_capBits, CAP_W, CAP_H);
        fprintf(log, "saved palette.bmp\n");
        {
            unsigned int centre = ((const unsigned int *)g_capBits)[128 * CAP_W + 128];
            int hits;
            reset_bounds();
            hits = scan_buffer((const unsigned int *)g_capBits, CAP_W,
                               (int)((centre >> 16) & 0xFF),
                               (int)((centre >> 8) & 0xFF),
                               (int)(centre & 0xFF), 8, 1, 1);
            fprintf(log, "centre colour=%06X tol=8 hits=%d\n", centre, hits);
            save_bmp24("hits.bmp", g_result, SCAN_W, SCAN_H);

            hits = scan_buffer((const unsigned int *)g_capBits, CAP_W, 0, 0, 0, 4, 1, 1);
            fprintf(log, "black (0,0,0) tol=4 hits=%d\n", hits);
            save_bmp24("hits_black.bmp", g_result, SCAN_W, SCAN_H);
        }
    }

    /* ---- marker discovery: cursor away vs cursor over the palette ---- */
    {
        unsigned int *a = (unsigned int *)malloc(CAP_W * CAP_H * 4);
        unsigned int *b = (unsigned int *)malloc(CAP_W * CAP_H * 4);
        int changed;

        /* the client only draws the sample squares while the spoid is armed */
        g_btnPickX = read_int(g_editPickX, 147);
        g_btnCancelX = read_int(g_editCancelX, 231);
        g_btnY = read_int(g_editBtnY, 397);
        click_pick_color();
        Sleep(700);

        move_cursor_to(3, 3);
        Sleep(400);
        capture_at(g_originX, g_originY);
        memcpy(a, g_capBits, CAP_W * CAP_H * 4);
        save_bmp24("palette_clean.bmp", a, CAP_W, CAP_H);
        Sleep(250);
        capture_at(g_originX, g_originY);
        memcpy(b, g_capBits, CAP_W * CAP_H * 4);
        changed = 0;
        {
            int i;
            for (i = 0; i < CAP_W * CAP_H; i++)
                if (diff_amount(a[i], b[i]) > 24) changed++;
        }
        fprintf(log, "animation check (two idle frames): %d px differ\n", changed);

        move_cursor_to(g_originX + 127, g_originY + 127);
        /* a move to the same position sends no WM_MOUSEMOVE */
        mouse_event(MOUSEEVENTF_MOVE, 4, 0, 0, 0);
        Sleep(150);
        mouse_event(MOUSEEVENTF_MOVE, -4, 0, 0, 0);
        Sleep(450);
        capture_at(g_originX, g_originY);
        memcpy(b, g_capBits, CAP_W * CAP_H * 4);
        save_bmp24("palette_marked.bmp", b, CAP_W, CAP_H);
        fprintf(log, "--- cursor at palette (127,127) ---\n");
        cluster_diff(a, b, log, 127, 127);
        {
            int mx[MAX_TARGETS], my[MAX_TARGETS], n, i;
            n = detect_markers(mx, my, MAX_TARGETS);
            fprintf(log, "detect_markers at (127,127): %d\n", n);
            for (i = 0; i < n; i++) {
                unsigned int c = ((const unsigned int *)g_capBits)[my[i] * CAP_W + mx[i]];
                int dx, dy, chg = 0;
                for (dy = -3; dy <= 3; dy++)
                    for (dx = -3; dx <= 3; dx++) {
                        int xx = mx[i] + dx, yy = my[i] + dy;
                        if (xx < 0 || yy < 0 || xx >= CAP_W || yy >= CAP_H) continue;
                        if (diff_amount(a[yy * CAP_W + xx], b[yy * CAP_W + xx]) > 24) chg++;
                    }
                fprintf(log, "  marker%d centre=(%d,%d) rel-to-cursor=(%d,%d) fill=#%06X chg=%d/49\n",
                        i, mx[i], my[i], mx[i] - 127, my[i] - 127, c & 0xFFFFFF, chg);
            }
        }
        /* second view: near the top edge, where every negative dy wraps */
        move_cursor_to(g_originX + 60, g_originY + 3);
        mouse_event(MOUSEEVENTF_MOVE, 3, 0, 0, 0);
        Sleep(150);
        mouse_event(MOUSEEVENTF_MOVE, -3, 0, 0, 0);
        Sleep(450);
        capture_at(g_originX, g_originY);
        save_bmp24("palette_edge.bmp", (const unsigned int *)g_capBits, CAP_W, CAP_H);
        {
            int mx[MAX_TARGETS], my[MAX_TARGETS], n, i;
            n = detect_markers(mx, my, MAX_TARGETS);
            fprintf(log, "edge hover at (60,3): %d markers\n", n);
            for (i = 0; i < n; i++) {
                unsigned int c = ((const unsigned int *)g_capBits)[my[i] * CAP_W + mx[i]];
                fprintf(log, "  edge%d centre=(%d,%d) rel=(%d,%d) fill=#%06X\n",
                        i, mx[i], my[i], mx[i] - 60, my[i] - 3, c & 0xFFFFFF);
            }
        }
        free(a);
        free(b);
    }

    SystemParametersInfoA(0x2001, 0, (PVOID)(UINT_PTR)lockTimeout, 0);
    DeleteObject(bmp);
    DeleteDC(mem);
    fclose(log);
}

static int changed_pixels(void)
{
    const unsigned int *live = (const unsigned int *)g_capBits;
    int x, y, n = 0;

    if (!live || !g_gridValid) return -1;
    for (y = 0; y < CAP_H; y++)
        for (x = 0; x < CAP_W; x++) {
            unsigned int p = live[y * CAP_W + x], q = g_grid[y * CAP_W + x];
            if (iabs_i((int)((p >> 16) & 0xFF) - (int)((q >> 16) & 0xFF)) > 20 ||
                iabs_i((int)((p >> 8) & 0xFF) - (int)((q >> 8) & 0xFF)) > 20 ||
                iabs_i((int)(p & 0xFF) - (int)(q & 0xFF)) > 20) n++;
        }
    return n;
}

static unsigned int grid_at(const unsigned int *buf, int x, int y)
{
    x = wrap_mod(x, CAP_W);
    y = wrap_mod(y, CAP_H);
    if (x >= SCAN_W || y >= SCAN_H) return 0xFFFFFFFFu;
    return buf[y * CAP_W + x] & 0xFFFFFFu;
}

/* Run the Auto cycle without clicking the gradient (diagnostic). */
static void autotest(void)
{
    FILE *f;
    int ticks;

    f = fopen("autotest.txt", "w");
    if (f) { fprintf(f, "autotest: colour #%06X need %d\n",
                     (g_colorR << 16) | (g_colorG << 8) | g_colorB, g_desired);
            fclose(f); }

    g_noClick = 1;
    g_colorR = 0; g_colorG = 0; g_colorB = 0;
    g_var = 0;
    g_desired = 1;

    if (!capture_init()) return;

    g_autoMode = 1;
    g_autoState = 0;
    g_autoTicks = 0;

    for (ticks = 0; ticks < 14 && g_autoMode; ticks++) {
        auto_step();
        Sleep(150);
    }

    f = fopen("autotest.txt", "a");
    if (f) {
        fprintf(f, "end: state=%d offCount=%d markers=%d best=(%d,%d) "
                   "hits=%d need=%d origin=(%d,%d)\n",
                g_autoState, g_offCount, g_markerCount, g_bestX, g_bestY,
                g_bestSuccess, g_desired, g_originX, g_originY);
        fclose(f);
    }

    if (g_autoState != 2 || g_bestX < 0) return;

    {
        unsigned int pred[MAX_TARGETS], live[MAX_TARGETS], fills[MAX_TARGETS];
        int i, n, mx[MAX_TARGETS], my[MAX_TARGETS];
        const unsigned int *grid = (const unsigned int *)g_capBits;

        for (i = 0; i < g_offCount; i++)
            pred[i] = grid_at(grid, g_bestX + g_offX[i], g_bestY + g_offY[i]);

        /* let the client draw its squares at the chosen position */
        move_cursor_to(g_originX + g_bestX, g_originY + g_bestY);
        mouse_event(MOUSEEVENTF_MOVE, 3, 2, 0, 0);
        Sleep(150);
        mouse_event(MOUSEEVENTF_MOVE, -3, -2, 0, 0);
        Sleep(400);
        capture_at(g_originX, g_originY);
        save_bmp24("palette_cand.bmp", (const unsigned int *)g_capBits, CAP_W, CAP_H);
        {
            FILE *g = fopen("autotest.txt", "a");
            if (g) { fprintf(g, "markers on screen at the candidate: %d px\n",
                             changed_pixels()); fclose(g); }
        }
        n = detect_markers(mx, my, MAX_TARGETS);
        for (i = 0; i < n; i++)
            fills[i] = ((const unsigned int *)g_capBits)[my[i] * CAP_W + mx[i]] & 0xFFFFFF;

        /* and the live palette with the pointer parked away */
        move_cursor_to(3, 3);
        Sleep(450);
        capture_at(g_originX, g_originY);
        for (i = 0; i < g_offCount; i++)
            live[i] = grid_at((const unsigned int *)g_capBits,
                              g_bestX + g_offX[i], g_bestY + g_offY[i]);

        f = fopen("autotest.txt", "a");
        if (f) {
            fprintf(f, "--- at best=(%d,%d) squares=%d\n", g_bestX, g_bestY, n);
            for (i = 0; i < g_offCount; i++)
                fprintf(f, "  slot%d off=(%3d,%3d) saved=#%06X live=#%06X %s\n",
                        i, g_offX[i], g_offY[i], pred[i], live[i],
                        pred[i] == live[i] ? "" : "  <- palette changed");
            for (i = 0; i < n; i++)
                fprintf(f, "  client square%d at=(%d,%d) rel=(%d,%d) fill=#%06X\n",
                        i, mx[i], my[i], mx[i] - g_bestX, my[i] - g_bestY, fills[i]);
            fclose(f);
        }
    }
}

/* ---- entry point ------------------------------------------------------- */

int WINAPI WinMain(HINSTANCE inst, HINSTANCE prev, LPSTR cmdline, int show)
{
    WNDCLASSA wc;
    MSG msg;

    (void)prev;

    if (strstr(GetCommandLineA(), "--autotest")) {
        autotest();
        return 0;
    }

    if (strstr(GetCommandLineA(), "--probe")) {
        probe();
        return 0;
    }

    if (strstr(GetCommandLineA(), "--selftest")) {
        self_test();
        return g_fail;
    }

    g_bg = CreateSolidBrush(GetSysColor(COLOR_BTNFACE));
    if (!capture_init()) {
        MessageBoxA(NULL, "capture init failed", "DyeTrace", MB_ICONERROR);
        return 1;
    }
    g_targets[0].tol = 0;

    ZeroMemory(&wc, sizeof(wc));
    wc.lpfnWndProc = wndproc;
    wc.hInstance = inst;
    wc.hCursor = LoadCursorA(NULL, IDC_ARROW);
    wc.hbrBackground = g_bg;
    wc.lpszClassName = "DyeTrace";
    RegisterClassA(&wc);

    g_main = CreateWindowA("DyeTrace", "DyeTrace - dye palette finder",
                           WS_OVERLAPPEDWINDOW, CW_USEDEFAULT, CW_USEDEFAULT,
                           720, 380, NULL, NULL, inst, NULL);
    if (!g_main) return 1;
    ShowWindow(g_main, show);
    UpdateWindow(g_main);

    while (GetMessageA(&msg, NULL, 0, 0) > 0) {
        TranslateMessage(&msg);
        DispatchMessageA(&msg);
    }
    return 0;
}
