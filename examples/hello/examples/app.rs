//! The hello plugin as a standalone desktop app:
//!
//! ```bash
//! cargo run -p hello --example app [-- --light | --dark]
//! ```
//!
//! `ARCHIPEL_RENDERER=glow` switches from wgpu to OpenGL.

use archipel_sdk::app::{AppError, AppOptions, ThemeMode, run};

fn main() -> Result<(), AppError> {
    let theme = ThemeMode::from_args(std::env::args().skip(1)).unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2);
    });
    run(
        hello::HelloPlugin::new(),
        AppOptions {
            theme,
            ..AppOptions::default()
        },
    )
}
