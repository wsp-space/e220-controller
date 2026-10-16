#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod e220;
mod i18n;
mod serial;
mod ui;
mod update;

use gpui_kit::assets::AllAssets;
use gpui_kit::*;

const APP_ID: &str = "WSP-WAKAYAMA.E220Controller";

fn main() {
    velopack::VelopackApp::build().run();

    let app = gpui_kit::application().with_assets(AllAssets);

    app.run(|cx| {
        gpui_kit::init(cx);
        let app_name = "E220 Controller";
        cx.set_app_identity(APP_ID, app_name);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(1280.), px(820.)), cx)),
            titlebar: Some(TitlebarOptions {
                title: Some(app_name.into()),
                ..Default::default()
            }),
            app_id: Some(APP_ID.into()),
            ..Default::default()
        };

        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| ui::E220App::new(window, cx)))
            .expect("failed to open window");
    });
}
