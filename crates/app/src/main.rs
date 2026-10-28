//! DyeTrace command line.
//!
//! `--selftest` runs the twelve behaviour checks.
//! `--replay` drives the same pipeline against saved frames, so palette
//! detection, box detection and the search can be checked without a client.
//! `--probe` runs the Windows backend against the live client.

mod auto;
mod cycle;
mod probe;
mod snapshot;

use dyetrace_core::image::Rgb;
use dyetrace_core::{markers, offsets, palette, search, selftest, MAX_TARGETS};
use dyetrace_platform::fake::FakeScreen;
use dyetrace_platform::traits::Screen;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let code = match args.first().map(String::as_str) {
        Some("--selftest") => run_selftest(),
        Some("--replay") => run_replay(&args[1..]),
        Some("--probe") => probe::run(&args[1..]),
        Some("--snapshot") => snapshot::run(&args[1..]),
        Some("--auto") => auto::run(&args[1..]),
        _ => {
            usage();
            2
        }
    };

    std::process::exit(code);
}

fn usage() {
    println!("DyeTrace - dye palette finder for Mabinogi\n");
    println!("  dyetrace --selftest               run the behaviour checks");
    println!("  dyetrace --replay <clean.bmp> [<hover.bmp>] [options]");
    println!("  dyetrace --probe [--out DIR] [--wait MS]");
    println!("                                     locate the palette on the live client");
    println!("  dyetrace --snapshot <out.bmp>      capture the primary screen");
    println!("  dyetrace --auto [options]          run the dye cycle on the live client\n");
    println!("Auto options:");
    println!("  --colour RRGGBB   colour to look for     --variance N   tolerance");
    println!("  --slots N         samples required (1-5) --cycles N     attempts");
    println!("  --dry-run         search but do not click the gradient\n");
    println!("Replay options:");
    println!("  --pointer X,Y      pointer position in the frame (default: centre)");
    println!("  --colour RRGGBB    search for this colour");
    println!("  --variance N       per-channel tolerance (default 0)");
}

fn run_selftest() -> i32 {
    let checks = selftest::run();
    let mut report = String::new();
    let mut failures = 0;

    for check in &checks {
        let line = format!(
            "{} {}\n",
            if check.passed { "PASS" } else { "FAIL" },
            check.name
        );
        print!("{line}");
        report.push_str(&line);
        if !check.passed {
            failures += 1;
        }
    }

    let summary = format!(
        "\n{} ({} failure(s))\n",
        if failures == 0 { "ALL OK" } else { "FAILED" },
        failures
    );
    print!("{summary}");
    report.push_str(&summary);

    if let Err(err) = std::fs::write("selftest.txt", report) {
        eprintln!("could not write selftest.txt: {err}");
    }

    if failures == 0 {
        0
    } else {
        1
    }
}

struct ReplayOptions {
    clean: PathBuf,
    hover: Option<PathBuf>,
    pointer: Option<(i32, i32)>,
    colour: Option<Rgb>,
    variance: u8,
}

fn parse_replay(args: &[String]) -> Result<ReplayOptions, String> {
    let mut paths = Vec::new();
    let mut pointer = None;
    let mut colour = None;
    let mut variance = 0u8;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--pointer" => {
                let value = args.get(i + 1).ok_or("--pointer needs X,Y")?;
                let (x, y) = value.split_once(',').ok_or("--pointer needs X,Y")?;
                pointer = Some((
                    x.trim().parse().map_err(|_| "--pointer X is not a number")?,
                    y.trim().parse().map_err(|_| "--pointer Y is not a number")?,
                ));
                i += 2;
            }
            "--colour" => {
                let value = args.get(i + 1).ok_or("--colour needs RRGGBB")?;
                let n = u32::from_str_radix(value.trim_start_matches('#'), 16)
                    .map_err(|_| "--colour needs RRGGBB")?;
                colour = Some(Rgb::from_u32(n));
                i += 2;
            }
            "--variance" => {
                variance = args
                    .get(i + 1)
                    .ok_or("--variance needs a number")?
                    .parse()
                    .map_err(|_| "--variance needs a number")?;
                i += 2;
            }
            other => {
                paths.push(PathBuf::from(other));
                i += 1;
            }
        }
    }

    if paths.is_empty() {
        return Err("expected at least one frame".to_string());
    }

    let mut paths = paths.into_iter();
    Ok(ReplayOptions {
        clean: paths.next().unwrap(),
        hover: paths.next(),
        pointer,
        colour,
        variance,
    })
}

fn run_replay(args: &[String]) -> i32 {
    let opts = match parse_replay(args) {
        Ok(o) => o,
        Err(err) => {
            eprintln!("error: {err}");
            return 2;
        }
    };

    let clean = match FakeScreen::from_bmp(&opts.clean) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("cannot read {}: {err}", opts.clean.display());
            return 2;
        }
    };

    let origin = palette::find_square_field(clean.frame())
        .or_else(|| palette::find_field(clean.frame()));

    let (ox, oy) = match origin {
        Some(o) => o,
        None => {
            println!("no palette found in {}", opts.clean.display());
            return 1;
        }
    };

    println!("frame      {}  {}x{}", opts.clean.display(), clean.frame().width(), clean.frame().height());
    println!("palette    ({ox},{oy})");

    let grab = match clean.capture(ox, oy, 256, 256) {
        Some(g) => g,
        None => {
            println!("palette at ({ox},{oy}) does not fit inside the frame");
            return 1;
        }
    };
    println!("gradient   {}%", palette::gradient_percent(&grab));

    let pointer = opts.pointer.unwrap_or((127, 127));

    if let Some(hover_path) = &opts.hover {
        let hover = match FakeScreen::from_bmp(hover_path) {
            Ok(s) => s,
            Err(err) => {
                eprintln!("cannot read {}: {err}", hover_path.display());
                return 2;
            }
        };
        let live = match hover.capture(ox, oy, 256, 256) {
            Some(g) => g,
            None => {
                println!("hover frame does not cover the palette");
                return 1;
            }
        };

        let boxes = markers::detect_boxes(&live, MAX_TARGETS);
        println!("boxes      {} on screen", boxes.len());
        for (i, centre) in boxes.iter().enumerate() {
            let fill = markers::box_fill(&live, *centre);
            let (sx, sy) = markers::sample_point(*centre);
            println!(
                "  box {i}  at ({},{})  shows #{:02X}{:02X}{:02X}  samples ({sx},{sy})",
                centre.0, centre.1, fill.r, fill.g, fill.b
            );
        }

        let table = offsets::from_boxes(&boxes, pointer);
        println!("offsets    {} (pointer at both {})", table.len(), format!("{},{}", pointer.0, pointer.1));
        for (i, (dx, dy)) in table.iter().enumerate() {
            println!("  slot {i}  {dx:+},{dy:+}");
        }

        if table.len() < dyetrace_core::MIN_SLOTS {
            println!("verdict    too few slots to search");
            return 1;
        }

        if let Some(colour) = opts.colour {
            match search::search(&grab, &table, colour, opts.variance) {
                Some(m) => {
                    println!(
                        "search     #{} at ({},{})  {}/{} samples  distance {}",
                        format!("{:02X}{:02X}{:02X}", colour.r, colour.g, colour.b),
                        m.x,
                        m.y,
                        m.count,
                        table.len(),
                        m.distance
                    );
                }
                None => println!("search     no candidate"),
            }
        }
    }

    if let Some(colour) = opts.colour {
        let map = search::hit_map(&grab, colour, opts.variance);
        let red = map.iter().filter(|p| **p == 0x00FF_0000).count();
        println!(
            "hit map    {} pixels of #{}",
            red,
            format!("{:02X}{:02X}{:02X}", colour.r, colour.g, colour.b)
        );
    }

    0
}
