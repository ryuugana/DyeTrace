//! `--snapshot`: capture the primary screen to a BMP.
//!
//! Used to check the capture path (and the pixel layout) without the client.

use std::path::Path;

pub fn run(args: &[String]) -> i32 {
    use dyetrace_platform::bmp;
    use dyetrace_platform::traits::Screen;
    use dyetrace_platform_native::NativeScreen;

    let path = match args.first() {
        Some(p) => Path::new(p),
        None => {
            eprintln!("usage: dyetrace --snapshot <out.bmp>");
            return 2;
        }
    };

    let screen = NativeScreen::new();
    let (w, h) = screen.screen_size();
    let Some(image) = screen.capture(0, 0, w as u32, h as u32) else {
        eprintln!("capture failed");
        return 1;
    };

    match bmp::write_bmp(path, &image) {
        Ok(()) => {
            println!("wrote {} ({w}x{h})", path.display());
            0
        }
        Err(err) => {
            eprintln!("cannot write {}: {err}", path.display());
            1
        }
    }
}
