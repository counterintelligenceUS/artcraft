// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
mod webkit_compositor;

fn main() {
  #[cfg(target_os = "linux")]
  webkit_compositor::configure();

  artcraft_app_lib::run();
}
