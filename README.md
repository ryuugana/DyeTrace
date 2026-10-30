# DyeTrace

**An open-source dye palette finder for Mabinogi** — pick the colour you want, and it finds the spot in the dye gradient that produces it, then clicks it for you.

Windows & Linux · Rust · single executable · no runtime dependencies

![DyeTrace scanning a dye palette](docs/captures/ui.png)

---

## Why this exists

DyeLite is used by a lot of players, but its source code was never published anywhere. DyeTrace is an open-source take on it: the same core workflow — choose a colour, find where it exists in the dye gradient, apply it — rebuilt from scratch and documented, with the minimum capabilities of DyeLite.

It is not a byte-for-byte port. It reproduces the behaviour by reading the screen the same way DyeLite does, and the parts that matter (how the dye window samples the palette, how a match is scored) were measured against the live client and cross-checked against DyeLite's own output.

## What it does

- **Finds the dye palette automatically** inside the Mabinogi window.
- **Reads the five sample offsets from the client itself** every attempt, instead of assuming a fixed spacing. The offsets are not universal — they depend on the dye/part set.
- **Matches by count, not by slot order.** You give one colour, a variance, and how many of the five samples must hold it (1–5).
- **Shows a live hit map** - a white pane with red where the colour exists, refreshed ten times a second while the dye window is open. Boxes and numbers mark the sampled positions once a scan has measured them.
- **Auto mode** runs the whole cycle: `Pick Color` → capture → read offsets → search → click, or `Cancel` and try again. Presses are gated on the button's label, so a dye is never applied by accident.
- **Stops on demand** with a key you choose: press `Stop Key`, press the key, and it is watched wherever the keyboard focus is.
- **Self-tests** (`--selftest`) so you can check a build without opening the game.

## Repository layout

- `crates/core` - platform-free matching: palette detection, legend offsets, the search, and the twelve behaviour checks as tests
- `crates/platform` - the `Screen` / `Mouse` / `GameWindow` traits, plus BMP replay fakes so the pipeline runs without a client
- `crates/platform-win` - the Win32 backend (`BitBlt`, verified pointer moves, window focus)
- `crates/platform-linux` - the X11 backend (`GetImage`, XTEST pointer input, `QueryKeymap`), spoken directly over the X protocol with no C headers
- `crates/platform-native` - picks the backend for the target, so the CLI and the window contain no platform branches
- `crates/app` - the CLI: `--selftest`, `--replay`, `--probe`, `--snapshot`, `--auto`
- `crates/engine` - the dye cycle itself, shared by the CLI and the window
- `crates/ui` - the windowed interface (egui): colour picker, presets, hit map

## Requirements

- Windows 10 or later, **or** Linux with an X11 display.
- On Wayland, the game must run through **XWayland** (GNOME and KDE start it automatically). DyeTrace talks the X11 protocol for capture and input, so a game running natively on Wayland is not reachable.
- Mabinogi running in a **window** (not exclusive fullscreen), with the dye window open and the palette visible.
- The client must be the foreground window: DyeTrace reads the palette from the screen.

The Linux backend speaks X11 directly through a pure-Rust client, so no libX11 development package is needed to build or run it.

## Build

```sh
cargo build --release
#   target/release/dyetrace     - CLI
#   target/release/dyetrace-ui  - window

cargo test
#   behaviour checks + offset/search/backend tests
```

On Windows the executables end in `.exe`. The release binary is self-contained on both platforms.

## Linux notes

- The window runs under Wayland or X11 like any egui application, but the game is read and clicked through X11: under a Wayland session that means XWayland, which is where the game runs too.
- Whole-window capture reads the client's own drawable, so the palette can be read while another window covers it on a composited desktop.
- Rootless XWayland does not let a client read the root window. `--snapshot` then assembles the rectangle from the visible top-level windows, which is enough for layout and palette diagnosis. `--probe`, `--auto` and the window preview read the client window directly and are unaffected.
- The stop key is polled with `QueryKeymap`, so it works while the game has the keyboard focus, just as `GetAsyncKeyState` does on Windows.
- The Python helpers in `tools/` are Windows-only diagnostics (they use `ctypes.WinDLL`); the Rust CLI covers the same ground.

## Usage

`dyetrace-ui` (`dyetrace-ui.exe` on Windows) is the window: the controls and the colour picker on the
left, the search preview on the right, and one line of feedback along the
bottom.

The preview looks after itself: as soon as the dye window is on screen it shows
the hit map for the chosen colour, and it keeps it current. It pauses while a
colour is being picked (the button reads Start Dyeing) and resumes on Cancel,
because a legend drawn over the grid would otherwise end up in the picture. The
sample markers belong to the roll they were measured on: a rolled palette means
they disappear until a run measures them again.

| Control | Meaning |
|---|---|
| `Selected colour` / `Save as…` | Named colours, stored in `presets.txt` next to the executable. The box names the colour you have: a saved name when it matches one, `Custom` otherwise. |
| `R` `G` `B`, `Hex` | The dye colour. All three stay in sync with the picker. |
| Colour square + hue strip | Pick the colour visually; the square follows the chosen colour. |
| `Slot` | Optional. Press 1–5 to require those samples to hold the colour; press a number again to clear it. Several can be picked. The numbers are the ones the dye window prints above its five previews: 1 is under the cursor, 2–3 above it, 4–5 below. |
| `Dye spots` | How many of the five samples must hold the colour (1–5). Which samples fill that up is where `Slot` comes in. |
| `Retries` | How many new rolls a run may try, after the first, before giving up. `0` keeps rolling until a match or the stop key. |
| `Variance ±` | Per-channel tolerance as a number, or drag the slider below it. `0` = exact match. |
| `Start` | Run the cycle until it clicks a match or the stop key is pressed. |
| `About` | What the tool does, the short version of the steps, and the repository link. |
| `Stop Key` | Press it, then press the key you want: the key shown beside the button is the one that stops a running cycle. Space by default. It is polled globally, so it works while the game has the keyboard focus. |

The preview is white with red where the colour exists, blue squares for the
client's legend, numbered points for the five samples and a green crosshair on
the best position.

### Slots and spots together

`Dye spots` says how many of the five samples must hold the colour; the slot
buttons say where those hits may come from.

| Chosen slots | Rule |
|---|---|
| none | any `Dye spots` of the five will do |
| fewer than spots | every chosen slot must hold it, and the rest of the spots can come from any other sample |
| as many as spots | every chosen slot must hold it |
| more than spots | only the chosen slots are asked, and any `Dye spots` of them will do |

For example, slot 2 with 3 spots needs slot 2 plus two of the others; slots
1 3 5 with 3 spots needs all three; slots 1 3 5 with 2 spots needs any two of the
three. A chosen slot the legend did not show is ignored, and a position is only
clicked when the whole rule is satisfied - a near miss is logged with
`- short of it` and the attempt rolls on.

### The dye window's buttons

The bottom row of the dye window has two buttons whose meaning changes as you
use them:

| Button state | What happens |
|---|---|
| **Pick Color** | White label. Puts the window into picking mode and rolls a new gradient. This is the only state in which the tool may press it. |
| **Start Dyeing** | Grey label, shown as soon as a colour has been picked. Pressing it applies the dye and closes the window. |
| **Cancel** | Discards the chosen colour and **brings the Pick Color label back**, leaving the window open - but only while the label reads Start Dyeing. Pressed while the window is idle it closes the dye window instead. |

A click is therefore never sent blind. DyeTrace reads the label out of the same
grab the gradient comes from - the white label has about forty pixels brighter
than luminance 130 and the grey one none - and acts on what it says:

- **Pick Color** - press it once, then capture the new gradient;
- **Start Dyeing** - press nothing, the pick is already armed;
- **unreadable** - press nothing and stop, rather than guess.

Auto raises the client before it looks for anything, and every Cancel is
confirmed by reading the label back.

The window is driven with:

1. Pick the colour - the swatch, the R/G/B boxes, the hex field, or a saved name.
2. Choose **Dye spots**, and optionally the slots that must hold it.
3. Press **Start**. The window marks the samples it reads and then rolls until
   it finds a match, a stop, or the retries run out.
4. **Stop Key** records a different key to stop a run with.

## How it works

1. **Locate the palette.** The dye palette is a 256×256 square, so DyeTrace looks for the square whose four borders are edges and whose interior varies. That works regardless of which colours the palette happens to contain. The square also has to be ringed by the dye form's one-pixel black line: the client's own windows are squares too, and without that line an inventory panel would do. A cheap pass shortlists corners and only those get the full border test, so locating costs a fraction of a second.
2. **Capture once per attempt.** The gradient is grabbed right after `Pick Color` and before the cursor touches it, so the image never contains the client's marker graphics. The cursor waits below the palette in between, where the client's camera cannot take it.
3. **Read the sample offsets.** Hovering the grid makes the client draw five markers. Each marker is a **crosshair** (the sample point) joined by a short neck to a **legend box** that shows the colour found there; the box sits 10 px to the right of its crosshair. Measured on every box: `box_fill == gradient[box_centre + (-10, 0)]`, exactly. The samples are then numbered the way the dye window numbers its five previews — 1 under the cursor, 2 and 3 above it, 4 and 5 below — so a chosen slot means the same thing on both sides.
4. **Watch the palette.** A thread of its own grabs the palette every 100 ms and publishes it, so the picture on screen is always current and the engine's own grabs travel the same path: what is drawn is what was searched. The picture is skipped when it would contain a legend, and the thread stops publishing while a colour is being picked.
5. **Tell one roll from another.** Each published palette carries a coarse signature (the average colour of a 32x32 grid). A scan records the signature it measured, and its offsets are only drawn while the palette still matches — a rolled palette means the offsets are unknown.
6. **Search.** For every position in the 254×254 palette, sample the five offsets (wrapping ±256, rejecting the separator/background at 254–255) and count how many are within the variance. Keep the best count, then the lowest colour distance. The chosen slots narrow where the hits may come from, as described above; a position only counts when every guaranteed slot holds the colour as well as the spot count being met.
7. **Act.** Move the cursor to the match and click — or click `Cancel` and start over.

## Command line

The command line (`target/release/dyetrace`, or `dyetrace.exe` on Windows):

```sh
dyetrace --selftest                         # the twelve behaviour checks -> selftest.txt
dyetrace --auto --colour 000000 --slots 3   # run the dye cycle on the live client
dyetrace --auto --colour 000000 --slot 2    # same, but require sample 2 to hold it
dyetrace --auto --colour 000000 --slot 1,3,5 # require samples 1, 3 and 5
dyetrace --auto --dry-run                   # same, but never click the gradient
dyetrace --probe --out DIR                  # locate the palette, write window/palette BMPs
dyetrace --replay <clean.bmp> [<hover.bmp>] # run the pipeline against saved frames
dyetrace --snapshot <out.bmp>               # capture the primary screen
```

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `no gradient - open the dye window and leave it visible` | The palette is not on screen, or the client is behind another window. Open the dye window and keep it visible. |
| `move this window off the palette - it is covering the colours` | DyeTrace is overlapping the gradient; move it aside. |
| Auto keeps cancelling | The button offsets built into the engine are not hitting Pick Color / Cancel on your layout. Check them with a screenshot. |
| The dye window closed itself | A Cancel click landed while the button still read Pick Color. Current builds read the label first and only press Cancel when it reads Start Dyeing; if you see this, run the same job from the command line, which prints the lines beginning `button reads`. |
| Nothing gets clicked | The client ignores a bare cursor move (`SetCursorPos` on Windows, `WarpPointer` on X11) for its hover state; DyeTrace also injects real mouse movement. If the cursor cannot be placed, the status says `auto: cursor did not reach the match - cycle aborted` and the cycle stops rather than clicking blind. |
| Wrong spot gets picked | Loosen or tighten `variance`, or change `slots`. Compare against the red hit map — it shows exactly what the search sees. |
| Linux: `client window not found` | The game is not running under XWayland, or its window has neither a `Mabinogi` class nor title. Start the game through XWayland, or use `--probe` to see what is found. |
| Linux: `--snapshot` has black areas | Rootless XWayland cannot read the root window, so DyeTrace assembles the picture from the visible X11 windows; the Wayland desktop and Wayland-native applications are not in it. `--probe`, `--auto` and the preview read the game window directly. |

## Limitations

- Windows and Linux/X11 only (Linux under Wayland needs XWayland); it reads the screen, so the palette must be visible unless the desktop keeps window contents available when covered.
- The Linux backend drives the X11 pointer, not a native Wayland one, so a game running natively on Wayland is out of reach.
- One colour per run. DyeLite allows a different colour per slot; DyeTrace matches a single colour against N of the five samples.
- No profiles: one set of settings, kept in `config.txt`, and one list of named colours in `presets.txt`.
- Only tested against the current English client; the button offsets live in `Options` if a layout differs.
- The UI is English only.

## Files it writes

| File | Contents |
|---|---|
| `selftest.txt` | Output of `--selftest`. |
| `probe.txt`, `*.bmp` | Output of `--probe` (palette detection diagnostics). |
| `config.txt` | The window's own settings, next to the executable: colour, dye spots, retries, variance, chosen slots and the stop key. Written as soon as any of them changes. |
| `presets.txt` | The named colours, one name and hex per line. |

## Credits

- **DyeLite G13** by Merryangel (2009), English fix by Rii — the original tool whose behaviour this reimplements.
- **DyeRite** for insight into how the dye palette is laid out.
