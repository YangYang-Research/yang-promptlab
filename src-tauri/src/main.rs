#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "macos")]
mod macos_app_bundle;

fn main() {
    #[cfg(target_os = "macos")]
    macos_app_bundle::reexec_from_app_bundle();

    promptlab_desktop_lib::run();
}
