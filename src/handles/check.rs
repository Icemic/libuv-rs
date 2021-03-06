use crate::{FromInner, HandleTrait, Inner, IntoInner};
use std::convert::TryFrom;
use uv::{uv_check_init, uv_check_start, uv_check_stop, uv_check_t};

callbacks! {
    pub CheckCB(handle: CheckHandle);
}

/// Additional data stored on the handle
#[derive(Default)]
pub(crate) struct CheckDataFields<'a> {
    check_cb: CheckCB<'a>,
}

/// Callback for uv_check_start
extern "C" fn uv_check_cb(handle: *mut uv_check_t) {
    let dataptr = crate::Handle::get_data(uv_handle!(handle));
    if !dataptr.is_null() {
        unsafe {
            if let super::CheckData(d) = &mut (*dataptr).addl {
                d.check_cb.call(handle.into_inner());
            }
        }
    }
}

/// Check handles will run the given callback once per loop iteration, right after polling for i/o.
#[derive(Clone, Copy)]
pub struct CheckHandle {
    handle: *mut uv_check_t,
}

impl CheckHandle {
    /// Create and initialize a new check handle
    pub fn new(r#loop: &crate::Loop) -> crate::Result<CheckHandle> {
        let layout = std::alloc::Layout::new::<uv_check_t>();
        let handle = unsafe { std::alloc::alloc(layout) as *mut uv_check_t };
        if handle.is_null() {
            return Err(crate::Error::ENOMEM);
        }

        let ret = unsafe { uv_check_init(r#loop.into_inner(), handle) };
        if ret < 0 {
            unsafe { std::alloc::dealloc(handle as _, layout) };
            return Err(crate::Error::from_inner(ret as uv::uv_errno_t));
        }

        crate::Handle::initialize_data(uv_handle!(handle), super::CheckData(Default::default()));

        Ok(CheckHandle { handle })
    }

    /// Start the handle with the given callback. This function always succeeds, except when cb is
    /// ().
    pub fn start<CB: Into<CheckCB<'static>>>(&mut self, cb: CB) -> crate::Result<()> {
        // uv_cb is either Some(uv_check_cb) or None
        let cb = cb.into();
        let uv_cb = use_c_callback!(uv_check_cb, cb);

        // cb is either Some(closure) or None - it is saved into data
        let dataptr = crate::Handle::get_data(uv_handle!(self.handle));
        if !dataptr.is_null() {
            if let super::CheckData(d) = unsafe { &mut (*dataptr).addl } {
                d.check_cb = cb;
            }
        }

        crate::uvret(unsafe { uv_check_start(self.handle, uv_cb) })
    }

    /// Stop the handle, the callback will no longer be called. This function always succeeds.
    pub fn stop(&mut self) -> crate::Result<()> {
        crate::uvret(unsafe { uv_check_stop(self.handle) })
    }
}

impl FromInner<*mut uv_check_t> for CheckHandle {
    fn from_inner(handle: *mut uv_check_t) -> CheckHandle {
        CheckHandle { handle }
    }
}

impl Inner<*mut uv::uv_handle_t> for CheckHandle {
    fn inner(&self) -> *mut uv::uv_handle_t {
        uv_handle!(self.handle)
    }
}

impl From<CheckHandle> for crate::Handle {
    fn from(check: CheckHandle) -> crate::Handle {
        crate::Handle::from_inner(Inner::<*mut uv::uv_handle_t>::inner(&check))
    }
}

impl crate::ToHandle for CheckHandle {
    fn to_handle(&self) -> crate::Handle {
        crate::Handle::from_inner(Inner::<*mut uv::uv_handle_t>::inner(self))
    }
}

impl TryFrom<crate::Handle> for CheckHandle {
    type Error = crate::ConversionError;

    fn try_from(handle: crate::Handle) -> Result<Self, Self::Error> {
        let t = handle.get_type();
        if t != crate::HandleType::CHECK {
            Err(crate::ConversionError::new(t, crate::HandleType::CHECK))
        } else {
            Ok((handle.inner() as *mut uv_check_t).into_inner())
        }
    }
}

impl HandleTrait for CheckHandle {}

impl crate::Loop {
    /// Create and initialize a new check handle
    pub fn check(&self) -> crate::Result<CheckHandle> {
        CheckHandle::new(self)
    }
}
