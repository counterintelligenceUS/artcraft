//! Linux WebKitGTK compositor selection.
//!
//! The GPU compositor is the default because the frontend uses three.js WebGL.
//! Software rendering (`WEBKIT_DISABLE_DMABUF_RENDERER` and
//! `WEBKIT_DISABLE_COMPOSITING_MODE`) is opt-in, or automatic when the display
//! looks like X11 forwarding or a direct GLX context cannot be created.

use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};

const GLX_RGBA: c_int = 4;
const GLX_DOUBLEBUFFER: c_int = 5;
const GLX_DEPTH_SIZE: c_int = 12;

static GL_PROBE_FAILED: AtomicBool = AtomicBool::new(false);

pub fn configure() {
  if env_enabled("ARTCRAFT_WEBKIT_GPU") {
    log::info!("WebKit GPU compositor forced by ARTCRAFT_WEBKIT_GPU");
    return;
  }

  let forwarded = std::env::var("DISPLAY").ok().is_some_and(|display| display_is_x11_forwarded(&display));
  let software_flag = env_enabled("ARTCRAFT_WEBKIT_SOFTWARE");
  let direct_gl = if software_flag || forwarded { false } else { glx_direct_context_available() };
  if !software_flag && !forwarded && direct_gl {
    log::info!("WebKit GPU compositor enabled");
    return;
  }

  set_if_unset("WEBKIT_DISABLE_DMABUF_RENDERER");
  set_if_unset("WEBKIT_DISABLE_COMPOSITING_MODE");
  log::info!(
    "WebKit software rendering enabled (software_flag={software_flag}, x11_forwarded={forwarded}, direct_gl={direct_gl})"
  );
}

fn env_enabled(name: &str) -> bool {
  let Ok(value) = std::env::var(name) else {
    return false;
  };
  let value = value.trim();
  if value.is_empty() || value == "0" {
    return false;
  }
  !value.eq_ignore_ascii_case("false")
    && !value.eq_ignore_ascii_case("no")
    && !value.eq_ignore_ascii_case("off")
}

fn display_is_x11_forwarded(display: &str) -> bool {
  // ssh -X sets DISPLAY to a host plus a display number of 10 or higher,
  // for example localhost:10.0. Local servers are :0 or :1.
  let Some((_, rest)) = display.rsplit_once(':') else {
    return false;
  };
  let number = rest.split(['.', '/']).next().unwrap_or("");
  matches!(number.parse::<u32>(), Ok(screen) if screen >= 10)
}

fn set_if_unset(name: &str) {
  if std::env::var_os(name).is_none() {
    std::env::set_var(name, "1");
  }
}

fn glx_direct_context_available() -> bool {
  unsafe { probe_glx_direct_context() }
}

unsafe extern "C" fn on_x_error(_display: *mut c_void, _event: *mut c_void) -> c_int {
  GL_PROBE_FAILED.store(true, Ordering::Relaxed);
  0
}

unsafe fn probe_glx_direct_context() -> bool {
  let x11 = libc::dlopen(c"libX11.so.6".as_ptr(), libc::RTLD_LAZY);
  let mut glx = libc::dlopen(c"libGLX.so.0".as_ptr(), libc::RTLD_LAZY);
  if glx.is_null() {
    glx = libc::dlopen(c"libGL.so.1".as_ptr(), libc::RTLD_LAZY);
  }
  if x11.is_null() || glx.is_null() {
    // Can't tell. Leave the GPU compositor on rather than disable WebGL.
    return true;
  }

  type OpenDisplay = unsafe extern "C" fn(*const c_char) -> *mut c_void;
  type CloseDisplay = unsafe extern "C" fn(*mut c_void) -> c_int;
  type DefaultScreen = unsafe extern "C" fn(*mut c_void) -> c_int;
  type InitThreads = unsafe extern "C" fn() -> c_int;
  type ErrorHandler = unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int;
  type SetErrorHandler = unsafe extern "C" fn(Option<ErrorHandler>) -> Option<ErrorHandler>;
  type Sync = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
  type ChooseVisual = unsafe extern "C" fn(*mut c_void, c_int, *mut c_int) -> *mut c_void;
  type CreateContext = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, c_int) -> *mut c_void;
  type DestroyContext = unsafe extern "C" fn(*mut c_void, *mut c_void);
  type IsDirect = unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_int;

  let Some(init_threads) = sym::<InitThreads>(x11, c"XInitThreads") else {
    return true;
  };
  let Some(open_display) = sym::<OpenDisplay>(x11, c"XOpenDisplay") else {
    return true;
  };
  let Some(close_display) = sym::<CloseDisplay>(x11, c"XCloseDisplay") else {
    return true;
  };
  let Some(default_screen) = sym::<DefaultScreen>(x11, c"XDefaultScreen") else {
    return true;
  };
  let Some(set_error_handler) = sym::<SetErrorHandler>(x11, c"XSetErrorHandler") else {
    return true;
  };
  let Some(sync) = sym::<Sync>(x11, c"XSync") else {
    return true;
  };
  let Some(choose_visual) = sym::<ChooseVisual>(glx, c"glXChooseVisual") else {
    return true;
  };
  let Some(create_context) = sym::<CreateContext>(glx, c"glXCreateContext") else {
    return true;
  };
  let Some(destroy_context) = sym::<DestroyContext>(glx, c"glXDestroyContext") else {
    return true;
  };
  let Some(is_direct) = sym::<IsDirect>(glx, c"glXIsDirect") else {
    return true;
  };

  // GTK calls XInitThreads from gtk_init and it must not be preceded by other
  // Xlib calls. Call it here first, then close this display before GTK starts.
  init_threads();
  GL_PROBE_FAILED.store(false, Ordering::Relaxed);
  let previous_handler = set_error_handler(Some(on_x_error));
  let display = open_display(std::ptr::null());
  if display.is_null() {
    set_error_handler(previous_handler);
    return true;
  }

  let screen = default_screen(display);
  let mut attribs = [GLX_RGBA, GLX_DOUBLEBUFFER, GLX_DEPTH_SIZE, 24, 0];
  let visual = choose_visual(display, screen, attribs.as_mut_ptr());
  let mut direct = false;
  if !visual.is_null() {
    let context = create_context(display, visual, std::ptr::null_mut(), 1);
    sync(display, 0);
    if !context.is_null() && !GL_PROBE_FAILED.load(Ordering::Relaxed) {
      direct = is_direct(display, context) != 0;
      destroy_context(display, context);
      sync(display, 0);
    }
  }
  close_display(display);
  set_error_handler(previous_handler);
  direct && !GL_PROBE_FAILED.load(Ordering::Relaxed)
}

unsafe fn sym<T>(lib: *mut c_void, name: &CStr) -> Option<T> {
  let ptr = libc::dlsym(lib, name.as_ptr());
  if ptr.is_null() {
    None
  } else {
    Some(std::mem::transmute_copy::<*mut c_void, T>(&ptr))
  }
}

#[cfg(test)]
mod tests {
  use super::{configure, display_is_x11_forwarded, env_enabled};
  use std::sync::Mutex;

  static ENV_LOCK: Mutex<()> = Mutex::new(());

  struct EnvRestore {
    name: &'static str,
    previous: Option<std::ffi::OsString>,
  }

  impl EnvRestore {
    fn capture(name: &'static str) -> Self {
      Self { name, previous: std::env::var_os(name) }
    }
  }

  impl Drop for EnvRestore {
    fn drop(&mut self) {
      match &self.previous {
        Some(value) => std::env::set_var(self.name, value),
        None => std::env::remove_var(self.name),
      }
    }
  }

  #[test]
  fn display_forwarding_matches_ssh_x_numbers() {
    assert!(!display_is_x11_forwarded(":0"));
    assert!(!display_is_x11_forwarded(":1"));
    assert!(!display_is_x11_forwarded(":0.0"));
    assert!(!display_is_x11_forwarded("localhost:1"));
    assert!(!display_is_x11_forwarded("localhost:1.0"));
    assert!(display_is_x11_forwarded("localhost:10.0"));
    assert!(display_is_x11_forwarded("localhost:10"));
    assert!(display_is_x11_forwarded("127.0.0.1:11.0"));
    assert!(display_is_x11_forwarded("unix:10.0"));
    assert!(!display_is_x11_forwarded(""));
    assert!(!display_is_x11_forwarded("not-a-display"));
  }

  #[test]
  fn env_flag_rejects_explicit_off_values() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let restore = EnvRestore::capture("ARTCRAFT_WEBKIT_SOFTWARE");
    std::env::remove_var("ARTCRAFT_WEBKIT_SOFTWARE");
    assert!(!env_enabled("ARTCRAFT_WEBKIT_SOFTWARE"));
    for off in ["0", "false", "no", "off", ""] {
      std::env::set_var("ARTCRAFT_WEBKIT_SOFTWARE", off);
      assert!(!env_enabled("ARTCRAFT_WEBKIT_SOFTWARE"), "{off}");
    }
    std::env::set_var("ARTCRAFT_WEBKIT_SOFTWARE", "1");
    assert!(env_enabled("ARTCRAFT_WEBKIT_SOFTWARE"));
    drop(restore);
  }

  #[test]
  fn software_flag_disables_the_compositor() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let software = EnvRestore::capture("ARTCRAFT_WEBKIT_SOFTWARE");
    let gpu = EnvRestore::capture("ARTCRAFT_WEBKIT_GPU");
    let dmabuf = EnvRestore::capture("WEBKIT_DISABLE_DMABUF_RENDERER");
    let compositing = EnvRestore::capture("WEBKIT_DISABLE_COMPOSITING_MODE");
    std::env::remove_var("ARTCRAFT_WEBKIT_GPU");
    std::env::remove_var("WEBKIT_DISABLE_DMABUF_RENDERER");
    std::env::remove_var("WEBKIT_DISABLE_COMPOSITING_MODE");
    std::env::set_var("ARTCRAFT_WEBKIT_SOFTWARE", "1");

    configure();

    assert_eq!(std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").as_deref(), Ok("1"));
    assert_eq!(std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").as_deref(), Ok("1"));
    drop((software, gpu, dmabuf, compositing));
  }

  #[test]
  fn gpu_flag_wins_over_forwarded_display() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let software = EnvRestore::capture("ARTCRAFT_WEBKIT_SOFTWARE");
    let gpu = EnvRestore::capture("ARTCRAFT_WEBKIT_GPU");
    let display = EnvRestore::capture("DISPLAY");
    let dmabuf = EnvRestore::capture("WEBKIT_DISABLE_DMABUF_RENDERER");
    let compositing = EnvRestore::capture("WEBKIT_DISABLE_COMPOSITING_MODE");
    std::env::remove_var("ARTCRAFT_WEBKIT_SOFTWARE");
    std::env::remove_var("WEBKIT_DISABLE_DMABUF_RENDERER");
    std::env::remove_var("WEBKIT_DISABLE_COMPOSITING_MODE");
    std::env::set_var("ARTCRAFT_WEBKIT_GPU", "1");
    std::env::set_var("DISPLAY", "localhost:10.0");

    configure();

    assert!(std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none());
    assert!(std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none());
    drop((software, gpu, display, dmabuf, compositing));
  }

  #[test]
  fn forwarded_display_disables_the_compositor_without_a_flag() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let software = EnvRestore::capture("ARTCRAFT_WEBKIT_SOFTWARE");
    let gpu = EnvRestore::capture("ARTCRAFT_WEBKIT_GPU");
    let display = EnvRestore::capture("DISPLAY");
    let dmabuf = EnvRestore::capture("WEBKIT_DISABLE_DMABUF_RENDERER");
    let compositing = EnvRestore::capture("WEBKIT_DISABLE_COMPOSITING_MODE");
    std::env::remove_var("ARTCRAFT_WEBKIT_SOFTWARE");
    std::env::remove_var("ARTCRAFT_WEBKIT_GPU");
    std::env::remove_var("WEBKIT_DISABLE_DMABUF_RENDERER");
    std::env::remove_var("WEBKIT_DISABLE_COMPOSITING_MODE");
    std::env::set_var("DISPLAY", "localhost:10.0");

    configure();

    assert_eq!(std::env::var("WEBKIT_DISABLE_COMPOSITING_MODE").as_deref(), Ok("1"));
    drop((software, gpu, display, dmabuf, compositing));
  }

  #[test]
  fn default_path_keeps_gpu_compositor_when_gl_is_direct() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let display = std::env::var("DISPLAY").unwrap_or_default();
    if display.is_empty() || display_is_x11_forwarded(&display) {
      return;
    }
    let software = EnvRestore::capture("ARTCRAFT_WEBKIT_SOFTWARE");
    let gpu = EnvRestore::capture("ARTCRAFT_WEBKIT_GPU");
    let dmabuf = EnvRestore::capture("WEBKIT_DISABLE_DMABUF_RENDERER");
    let compositing = EnvRestore::capture("WEBKIT_DISABLE_COMPOSITING_MODE");
    std::env::remove_var("ARTCRAFT_WEBKIT_SOFTWARE");
    std::env::remove_var("ARTCRAFT_WEBKIT_GPU");
    std::env::remove_var("WEBKIT_DISABLE_DMABUF_RENDERER");
    std::env::remove_var("WEBKIT_DISABLE_COMPOSITING_MODE");

    configure();

    assert!(std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none(), "direct GL should keep the GPU compositor");
    assert!(std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none());
    drop((software, gpu, dmabuf, compositing));
  }
}
