// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
  // WebKitGTK's DMA-BUF renderer and GL compositor draw a blank window when
  // the display cannot create a modern GL context. That includes older Intel
  // GPUs, some NVIDIA drivers, and X11 forwarding (ssh -X) onto a server that
  // only offers indirect GLX. The Linux dev scripts already export these two
  // variables. Set ARTCRAFT_WEBKIT_GPU=1 to keep the accelerated compositor.
  #[cfg(target_os = "linux")]
  if std::env::var_os("ARTCRAFT_WEBKIT_GPU").is_none() {
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
      std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    if std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
      std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }
  }

  artcraft_app_lib::run();
}
