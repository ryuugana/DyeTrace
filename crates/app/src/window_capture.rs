//! `--snapshot-window`: capture the client window without raising it.
//!
//! Uses a whole-window capture (`PrintWindow` on Windows, `GetImage` on the
//! client's drawable on X11), which is what keeps the interface usable while a
//! scan runs.

use std::path::Path;

pub fn run(args: &[String]) -> i32 {
    use dyetrace_platform::bmp;
    use dyetrace_platform::traits::{GameWindow, Screen};
    use dyetrace_platform_native::{NativeScreen, NativeWindow};

    let path = match args.first() {
        Some(p) => Path::new(p),
        None => {
            eprintln!("usage: dyetrace --snapshot-window <out.bmp>");
            return 2;
        }
    };

    let window = NativeWindow::mabinogi();
    let Some(id) = window.find() else {
        eprintln!("no client window found");
        return 1;
    };

    let screen = NativeScreen::new();
    let Some(image) = Screen::capture_window(&screen, id) else {
        eprintln!("whole-window capture failed");
        return 1;
    };

    match bmp::write_bmp(path, &image) {
        Ok(()) => {
            println!(
                "wrote {} ({}x{})",
                path.display(),
                image.width(),
                image.height()
            );
            0
        }
        Err(err) => {
            eprintln!("cannot write {}: {err}", path.display());
            1
        }
    }
}
