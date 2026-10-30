//! The dye cycle, shared by the command line and the windowed UI.
//!
//! Everything here works through the platform traits in `dyetrace-platform`, so
//! the same code drives the live client or a replay fake.

pub mod cycle;

pub use cycle::{scan_once, Options, Outcome, Scan, ScanFail};
