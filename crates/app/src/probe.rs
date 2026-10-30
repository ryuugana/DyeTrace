//! `--probe`: locate the palette on the live client and dump what the detector
//! saw, so the Rust backend can be compared with the C tool's probe output.

use dyetrace_core::image::Image;
use dyetrace_core::palette;
use dyetrace_platform::bmp;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::Duration;

pub struct Options {
    pub out_dir: PathBuf,
    pub wait_ms: u32,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut out_dir = PathBuf::from(".");
    let mut wait_ms = 4000u32;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                out_dir = PathBuf::from(args.get(i + 1).ok_or("--out needs a directory")?);
                i += 2;
            }
            "--wait" => {
                wait_ms = args
                    .get(i + 1)
                    .ok_or("--wait needs milliseconds")?
                    .parse()
                    .map_err(|_| "--wait needs milliseconds")?;
                i += 2;
            }
            other => return Err(format!("unknown option {other}")),
        }
    }

    Ok(Options { out_dir, wait_ms })
}

fn write_report(dir: &Path, text: &str) {
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join("probe.txt"), text);
}

pub fn run(args: &[String]) -> i32 {
    use dyetrace_platform::traits::{GameWindow, Screen};
    use dyetrace_platform_native::{NativeScreen, NativeWindow};

    let opts = match parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("error: {err}");
            return 2;
        }
    };

    if let Err(err) = std::fs::create_dir_all(&opts.out_dir) {
        eprintln!("cannot create {}: {err}", opts.out_dir.display());
        return 2;
    }

    let window = NativeWindow::mabinogi();
    let screen = NativeScreen::new();

    let id = match window.find() {
        Some(id) => id,
        None => {
            let msg = "client window not found (class \"Mabinogi\")\n";
            print!("{msg}");
            write_report(&opts.out_dir, msg);
            return 1;
        }
    };

    let focused = window.focus(id);
    let rect = match window.rect(id) {
        Some(r) => r,
        None => {
            eprintln!("cannot read the client rectangle");
            return 1;
        }
    };

    let mut report = format!(
        "window      {}x{} at ({},{})\nfocused     {focused}\n",
        rect.w, rect.h, rect.x, rect.y
    );

    // Wait for the dye palette to appear, re-checking the capture.
    let deadline = opts.wait_ms / 250 + 1;
    let mut located = None;
    for _ in 0..deadline {
        // Read the client's own drawable first, so the probe sees it even when
        // another window is on top; a plain screen grab is the fallback.
        let frame = Screen::capture_window(&screen, id)
            .or_else(|| screen.capture(rect.x, rect.y, rect.w as u32, rect.h as u32));
        let Some(frame) = frame else {
            sleep(Duration::from_millis(250));
            continue;
        };
        if let Some(origin) = palette::find_square_field(&frame)
            .or_else(|| palette::find_field(&frame))
            .filter(|origin| palette::has_frame(&frame, *origin))
        {
            let _ = bmp::write_bmp(&opts.out_dir.join("window.bmp"), &frame);
            located = Some((frame, origin));
            break;
        }
        sleep(Duration::from_millis(250));
    }

    let (frame, (ox, oy)) = match located {
        Some(v) => v,
        None => {
            report.push_str("palette     not found\n");
            print!("{report}");
            write_report(&opts.out_dir, &report);
            return 1;
        }
    };

    // Cut the palette out of the frame that was just located, so the picture
    // written is exactly what the detector saw.
    let grab: Option<Image> = frame.sub_image(ox, oy, 256, 256);
    let grab = match grab {
        Some(g) => g,
        None => {
            report.push_str("palette     found but the grab failed\n");
            print!("{report}");
            write_report(&opts.out_dir, &report);
            return 1;
        }
    };

    let gradient = palette::gradient_percent(&grab);
    let _ = bmp::write_bmp(&opts.out_dir.join("palette.bmp"), &grab);

    report.push_str(&format!(
        "palette     ({ox},{oy}) in the window\nscreen      ({},{})\ngradient    {gradient}%\n\
         wrote       window.bmp, palette.bmp\n",
        rect.x + ox,
        rect.y + oy
    ));

    print!("{report}");
    write_report(&opts.out_dir, &report);
    0
}
