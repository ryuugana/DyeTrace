//! Platform layer: the three things the core cannot know.
//!
//! * [`Screen`] — where the pixels come from
//! * [`Mouse`] — how to move and click
//! * [`GameWindow`] — how to find, measure and focus the client
//!
//! Keeping these three small is what makes a second backend cheap.

pub mod bmp;
pub mod fake;
pub mod traits;

pub use dyetrace_core::image::Image;
pub use traits::{GameWindow, Mouse, Rect, Screen, WindowId};
