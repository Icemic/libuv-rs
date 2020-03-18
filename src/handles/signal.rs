use crate::{FromInner, IntoInner};
use uv::{uv_signal_init, uv_signal_start, uv_signal_start_oneshot, uv_signal_stop, uv_signal_t};

/// Additional data stored on the handle
#[derive(Default)]
pub(crate) struct SignalDataFields {
    signal_cb: Option<Box<dyn FnMut(SignalHandle, i32)>>,
}

/// Callback for uv_signal_start
extern "C" fn uv_signal_cb(handle: *mut uv_signal_t, signum: std::os::raw::c_int) {
    let dataptr = crate::Handle::get_data(uv_handle!(handle));
    if !dataptr.is_null() {
        unsafe {
            if let super::SignalData(d) = &mut (*dataptr).addl {
                if let Some(f) = d.signal_cb.as_mut() {
                    f(handle.into_inner(), signum as _);
                }
            }
        }
    }
}

/// Signal handles implement Unix style signal handling on a per-event loop bases.
///
/// Windows notes: Reception of some signals is emulated:
///   * SIGINT is normally delivered when the user presses CTRL+C. However, like on Unix, it is not
///     generated when terminal raw mode is enabled.
///   * SIGBREAK is delivered when the user pressed CTRL + BREAK.
///   * SIGHUP is generated when the user closes the console window. On SIGHUP the program is given
///     approximately 10 seconds to perform cleanup. After that Windows will unconditionally
///     terminate it.
///   * SIGWINCH is raised whenever libuv detects that the console has been resized. When a libuv
///     app is running under a console emulator, or when a 32-bit libuv app is running on 64-bit
///     system, SIGWINCH will be emulated. In such cases SIGWINCH signals may not always be
///     delivered in a timely manner. For a writable TtyHandle libuv will only detect size changes
///     when the cursor is moved. When a readable TtyHandle is used, resizing of the console buffer
///     will be detected only if the handle is in raw mode and is being read.
///   * Watchers for other signals can be successfully created, but these signals are never
///     received. These signals are: SIGILL, SIGABRT, SIGFPE, SIGSEGV, SIGTERM and SIGKILL.
///   * Calls to raise() or abort() to programmatically raise a signal are not detected by libuv;
///     these will not trigger a signal watcher.
///
/// Unix notes
///   * SIGKILL and SIGSTOP are impossible to catch.
///   * Handling SIGBUS, SIGFPE, SIGILL or SIGSEGV via libuv results into undefined behavior.
///   * SIGABRT will not be caught by libuv if generated by abort(), e.g. through assert().
///   * On Linux SIGRT0 and SIGRT1 (signals 32 and 33) are used by the NPTL pthreads library to
///     manage threads. Installing watchers for those signals will lead to unpredictable behavior
///     and is strongly discouraged. Future versions of libuv may simply reject them.
pub struct SignalHandle {
    handle: *mut uv_signal_t,
}

impl SignalHandle {
    /// Create and initialize a new signal handle
    pub fn new(r#loop: &crate::Loop) -> crate::Result<SignalHandle> {
        let layout = std::alloc::Layout::new::<uv_signal_t>();
        let handle = unsafe { std::alloc::alloc(layout) as *mut uv_signal_t };
        if handle.is_null() {
            return Err(crate::Error::ENOMEM);
        }

        let ret = unsafe { uv_signal_init(r#loop.into_inner(), handle) };
        if ret < 0 {
            unsafe { std::alloc::dealloc(handle as _, layout) };
            return Err(crate::Error::from_inner(ret as uv::uv_errno_t));
        }

        crate::Handle::initialize_data(uv_handle!(handle), super::SignalData(Default::default()));

        Ok(SignalHandle { handle })
    }

    /// Start the handle with the given callback, watching for the given signal.
    pub fn start(
        &mut self,
        cb: Option<impl FnMut(SignalHandle, i32) + 'static>,
        signum: i32,
    ) -> crate::Result<()> {
        // uv_cb is either Some(uv_signal_cb) or None
        let uv_cb = cb.as_ref().map(|_| uv_signal_cb as _);

        // cb is either Some(closure) or None - it is saved into data
        let cb = cb.map(|f| Box::new(f) as _);
        let dataptr = crate::Handle::get_data(uv_handle!(self.handle));
        if !dataptr.is_null() {
            if let super::SignalData(d) = unsafe { &mut (*dataptr).addl } {
                d.signal_cb = cb;
            }
        }

        crate::uvret(unsafe { uv_signal_start(self.handle, uv_cb, signum as _) })
    }

    /// Same functionality as start() but the signal handler is reset the moment the signal is
    /// received.
    pub fn start_oneshot(
        &mut self,
        cb: Option<impl FnMut(SignalHandle, i32) + 'static>,
        signum: i32,
    ) -> crate::Result<()> {
        // uv_cb is either Some(uv_signal_cb) or None
        let uv_cb = cb.as_ref().map(|_| uv_signal_cb as _);

        // cb is either Some(closure) or None - it is saved into data
        let cb = cb.map(|f| Box::new(f) as _);
        let dataptr = crate::Handle::get_data(uv_handle!(self.handle));
        if !dataptr.is_null() {
            if let super::SignalData(d) = unsafe { &mut (*dataptr).addl } {
                d.signal_cb = cb;
            }
        }

        crate::uvret(unsafe { uv_signal_start_oneshot(self.handle, uv_cb, signum as _) })
    }

    /// Stop the handle, the callback will no longer be called.
    pub fn stop(&mut self) -> crate::Result<()> {
        crate::uvret(unsafe { uv_signal_stop(self.handle) })
    }
}

impl FromInner<*mut uv_signal_t> for SignalHandle {
    fn from_inner(handle: *mut uv_signal_t) -> SignalHandle {
        SignalHandle { handle }
    }
}

impl IntoInner<*mut uv::uv_handle_t> for SignalHandle {
    fn into_inner(self) -> *mut uv::uv_handle_t {
        uv_handle!(self.handle)
    }
}

impl From<SignalHandle> for crate::Handle {
    fn from(signal: SignalHandle) -> crate::Handle {
        crate::Handle::from_inner(IntoInner::<*mut uv::uv_handle_t>::into_inner(signal))
    }
}

impl crate::HandleTrait for SignalHandle {}

impl crate::Loop {
    /// Create and initialize a new signal handle
    pub fn signal(&self) -> crate::Result<SignalHandle> {
        SignalHandle::new(self)
    }
}
