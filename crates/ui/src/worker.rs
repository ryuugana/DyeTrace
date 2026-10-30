//! Background worker: keeps the interface responsive while the client is being
//! driven.  One thread owns the platform objects and runs one job at a time.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use dyetrace_core::image::Rgb;
use dyetrace_platform::traits::GameWindow;
use dyetrace_engine::cycle::{self, Options, Scan};

use crate::preview::{Capture, Preview};

#[derive(Clone, Debug)]
pub enum Job {
    /// Find the palette and report where it is.
    Find,
    /// Prepare a search and report the best candidate; never clicks.
    Scan(Options),
    /// Run the whole cycle.
    Auto(Options),
}

#[derive(Debug)]
pub enum Msg {
    Status(String),
    Scanned(Box<Scan>),
    Finished,
}

pub struct Worker {
    jobs: Sender<Job>,
    messages: Receiver<Msg>,
    stop: Arc<AtomicBool>,
    pub busy: bool,
}

impl Worker {
    pub fn spawn(preview: &Preview) -> Self {
        let (job_tx, job_rx) = channel::<Job>();
        let (msg_tx, msg_rx) = channel::<Msg>();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let capture = preview.capture();

        std::thread::spawn(move || {
            while let Ok(job) = job_rx.recv() {
                run_job(job, &capture, &msg_tx, &thread_stop);
            }
        });

        Self {
            jobs: job_tx,
            messages: msg_rx,
            stop,
            busy: false,
        }
    }

    pub fn submit(&mut self, job: Job) {
        self.stop.store(false, Ordering::SeqCst);
        self.busy = true;
        let _ = self.jobs.send(job);
    }

    pub fn request_stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn poll(&mut self) -> Vec<Msg> {
        let mut out = Vec::new();
        while let Ok(msg) = self.messages.try_recv() {
            if matches!(msg, Msg::Finished) {
                self.busy = false;
            }
            out.push(msg);
        }
        out
    }

    /// Stop flag handed to the engine so long jobs can bail out.
    pub fn stop_flag(&self) -> Option<Arc<AtomicBool>> {
        Some(Arc::clone(&self.stop))
    }
}

#[cfg(not(windows))]
fn run_job(_job: Job, _capture: &Capture, messages: &Sender<Msg>, _stop: &Arc<AtomicBool>) {
    let _ = messages.send(Msg::Finished);
}

#[cfg(windows)]
fn run_job(job: Job, capture: &Capture, messages: &Sender<Msg>, stop: &Arc<AtomicBool>) {
    use dyetrace_platform_win::{WinMouse, WinWindow};

    let window = WinWindow::mabinogi();
    let mouse = WinMouse::new();

    let send = |msg: Msg| {
        let _ = messages.send(msg);
    };

    match job {
        Job::Find => {
            send(Msg::Status("looking for the palette".to_string()));
            let id = window.find();
            let found = id.and_then(|id| {
                window.focus(id);
                cycle::locate(&window, capture, id)
            });
            match found {
                Some((rect, origin)) => send(Msg::Status(format!(
                    "palette at ({},{}) in a {}x{} window",
                    origin.0, origin.1, rect.w, rect.h
                ))),
                None => send(Msg::Status(
                    "no palette - open the dye window and keep it visible".to_string(),
                )),
            }
        }

        Job::Scan(mut opts) => {
            opts.stop = Some(Arc::clone(stop));
            send(Msg::Status("scanning".to_string()));
            match cycle::scan_once(&window, capture, &mouse, &opts) {
                Ok(scan) => {
                    let mut summary = match scan.best {
                        Some(best) => format!(
                            "best ({},{}) - {} #{}",
                            best.x,
                            best.y,
                            cycle::found_phrase(best.count, scan.asked, scan.asked < scan.table.len()),
                            hex(opts.colour)
                        ),
                        None => "no candidate".to_string(),
                    };
                    if scan.armed {
                        summary.push_str(" (armed the picker)");
                    }
                    send(Msg::Status(summary));
                    send(Msg::Scanned(Box::new(scan)));
                }
                Err(err) => send(Msg::Status(fail_text(err))),
            }
        }

        Job::Auto(mut opts) => {
            opts.stop = Some(Arc::clone(stop));
            send(Msg::Status("auto running - Stop to end".to_string()));
            let outcome = cycle::run(&window, capture, &mouse, &opts, |_| {});
            send(Msg::Status(match outcome {
                cycle::Outcome::Matched {
                    x,
                    y,
                    count,
                    samples,
                    chosen_only,
                    clicked,
                    ..
                } => format!(
                    "{} ({x},{y}) - {} #{}",
                    if clicked { "clicked" } else { "would click" },
                    cycle::found_phrase(count, samples, chosen_only),
                    hex(opts.colour)
                ),
                cycle::Outcome::Exhausted { cycles } => {
                    if stop.load(Ordering::Relaxed) {
                        format!("stopped after {cycles} cycle(s)")
                    } else {
                        format!("no match after {cycles} cycle(s)")
                    }
                }
                cycle::Outcome::NoClient => "client or gradient not found".to_string(),
                cycle::Outcome::PointerLost { cycles } => {
                    format!("pointer could not be placed in cycle {cycles}")
                }
            }));
        }
    }

    send(Msg::Finished);
}

#[cfg(windows)]
fn fail_text(err: cycle::ScanFail) -> String {
    match err {
        cycle::ScanFail::NoClient => "no client window found".to_string(),
        cycle::ScanFail::NoGradient => "no palette - is the dye window open?".to_string(),
        cycle::ScanFail::Pointer => "the pointer could not be placed".to_string(),
        cycle::ScanFail::NoLegend => "the client never drew its sample squares - reopen the dye window".to_string(),
        cycle::ScanFail::ButtonUnknown => "cannot read the dye button - leaving it alone".to_string(),
    }
}

pub fn hex(colour: Rgb) -> String {
    format!("{:02X}{:02X}{:02X}", colour.r, colour.g, colour.b)
}
