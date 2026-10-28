//! `--snapshot-window`: capture the client window with `PrintWindow`.
//!
//! Used to check whether the client can be read without raising it, which is
//! what keeps the interface usable while a scan runs.

use std::path::Path;

#[cfg(not(windows))]
pub fn run(_args: &[String]) -> i32 {
    eprintln!("--snapshot-window needs the Windows backend");
    2
}

#[cfg(windows)]
pub fn run(args: &[String]) -> i32 {
    use dyetrace_platform::bmp;
    use dyetrace_platform::traits::GameWindow;
    use dyetrace_platform_win::{WinScreen, WinWindow};

    let path = match args.first() {
        Some(p) => Path::new(p),
        None => {
            eprintln!("usage: dyetrace --snapshot-window <out.bmp>");
            return 2;
        }
    };

    let window = WinWindow::mabinogi();
    let Some(id) = window.find() else {
        eprintln!("no client window found");
        return 1;
    };

    let screen = WinScreen::new();
    let Some(image) = screen.capture_window(id.0 as isize) else {
        eprintln!("PrintWindow failed");
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
