//! Builds the hello app's host headlessly (no window, no GPU) and drives a few frames.

use archipel_sdk::app::{App, AppOptions, SecretBackend, ThemeMode};
use archipel_sdk::kit::egui::{self, RawInput};
use hello::HelloPlugin;

#[test]
fn the_hello_app_starts_and_hosts_its_views() {
    let root = std::env::temp_dir().join(format!("hello-app-smoke-{}", std::process::id()));
    let mut app = App::new(
        HelloPlugin::new(),
        AppOptions {
            dirs_root: Some(root.clone()),
            secrets: SecretBackend::File,
            theme: ThemeMode::Light,
            ..AppOptions::default()
        },
    )
    .expect("hello has an [app] view");
    assert_eq!(app.title(), "Hello");

    let ctx = egui::Context::default();
    let frame = |app: &mut App| {
        let _ = ctx.run(RawInput::default(), |ctx| app.update(ctx));
    };
    for _ in 0..3 {
        frame(&mut app);
    }
    assert!(app.failure().is_none());
    assert_eq!(app.host().status(), "Hello plugin activated");

    app.run_command(&ctx, "app.toolWindow.hello", archipel_sdk::api::Value::Null);
    app.run_command(&ctx, "hello.openTab", archipel_sdk::api::Value::Null);
    frame(&mut app); // the plugin's tab request is queued until the next frame
    app.run_command(&ctx, "app.settings", archipel_sdk::api::Value::Null);
    frame(&mut app);
    frame(&mut app);
    assert_eq!(app.open_tool_windows(), ["hello"]);
    assert_eq!(app.tab_titles(), ["Hello", "Settings"]);
    let _ = std::fs::remove_dir_all(root);
}
