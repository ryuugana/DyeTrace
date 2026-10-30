//! `--auto`: run the cycle against the live client.

use dyetrace_engine::cycle::{self, Options};
use dyetrace_core::image::Rgb;

pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut opts = Options::default();
    let mut i = 0;

    while i < args.len() {
        let value = |name: &str| -> Result<&String, String> {
            args.get(i + 1).ok_or_else(|| format!("{name} needs a value"))
        };
        match args[i].as_str() {
            "--colour" => {
                let text = value("--colour")?;
                let n = u32::from_str_radix(text.trim_start_matches('#'), 16)
                    .map_err(|_| "--colour needs RRGGBB")?;
                opts.colour = Rgb::from_u32(n);
                i += 2;
            }
            "--variance" => {
                opts.variance = value("--variance")?
                    .parse()
                    .map_err(|_| "--variance needs 0-255")?;
                i += 2;
            }
            "--slots" => {
                opts.slots = value("--slots")?
                    .parse()
                    .map_err(|_| "--slots needs 1-5")?;
                i += 2;
            }
            "--slot" => {
                // Repeatable, and commas are allowed: --slot 1 --slot 3,5
                let text = value("--slot")?;
                for piece in text.split(',') {
                    let n: usize = piece
                        .trim()
                        .parse()
                        .map_err(|_| "--slot needs 1-5")?;
                    if !(1..=dyetrace_core::MAX_TARGETS).contains(&n) {
                        return Err("--slot needs 1-5".to_string());
                    }
                    if !opts.slots_only.contains(&(n - 1)) {
                        opts.slots_only.push(n - 1);
                    }
                }
                opts.slots_only.sort();
                i += 2;
            }
            "--cycles" => {
                opts.max_cycles = value("--cycles")?
                    .parse()
                    .map_err(|_| "--cycles needs a number")?;
                i += 2;
            }
            "--pick-x" => {
                opts.pick_x = value("--pick-x")?.parse().map_err(|_| "bad --pick-x")?;
                i += 2;
            }
            "--cancel-x" => {
                opts.cancel_x = value("--cancel-x")?.parse().map_err(|_| "bad --cancel-x")?;
                i += 2;
            }
            "--button-y" => {
                opts.button_y = value("--button-y")?.parse().map_err(|_| "bad --button-y")?;
                i += 2;
            }
            "--dry-run" => {
                opts.dry_run = true;
                i += 1;
            }
            other => return Err(format!("unknown option {other}")),
        }
    }

    if opts.slots == 0 || opts.slots > dyetrace_core::MAX_TARGETS {
        return Err("--slots must be 1-5".to_string());
    }

    Ok(opts)
}

#[cfg(not(windows))]
pub fn run(_args: &[String]) -> i32 {
    eprintln!("--auto needs the Windows backend");
    2
}

#[cfg(windows)]
pub fn run(args: &[String]) -> i32 {
    use dyetrace_platform_win::{WinMouse, WinScreen, WinWindow};

    let opts = match parse(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("error: {err}");
            return 2;
        }
    };

    if opts.dry_run {
        println!("dry run: the gradient will not be clicked\n");
    }

    let outcome = cycle::run(
        &WinWindow::mabinogi(),
        &WinScreen::new(),
        &WinMouse::new(),
        &opts,
        |line| println!("{line}"),
    );

    println!();
    match outcome {
        cycle::Outcome::Matched {
            cycles,
            x,
            y,
            count,
            samples,
            clicked,
        } => {
            let what = if opts.slots_only.is_empty() {
                format!("{count}/{samples} samples hold")
            } else {
                cycle::slot_phrase(&opts.slots_only)
            };
            println!(
                "{} ({x},{y}) after {cycles} cycle(s): {what} #{}",
                if clicked { "clicked" } else { "would click" },
                format!("{:02X}{:02X}{:02X}", opts.colour.r, opts.colour.g, opts.colour.b)
            );
            0
        }
        cycle::Outcome::Exhausted { cycles } => {
            println!("no match after {cycles} cycle(s)");
            1
        }
        cycle::Outcome::NoClient => {
            println!("client or gradient not found");
            1
        }
        cycle::Outcome::PointerLost { cycles } => {
            println!("pointer could not be placed (cycle {cycles}); nothing was clicked");
            1
        }
    }
}
