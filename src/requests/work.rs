use crate::{FromInner, Inner, IntoInner};
use uv::{uv_queue_work, uv_work_t};

callbacks! {
    pub WorkCB(req: WorkReq);
    pub AfterWorkCB(req: WorkReq, status: crate::Result<u32>);
}

/// Additional data stored on the request
pub(crate) struct WorkDataFields<'a> {
    work_cb: WorkCB<'a>,
    after_work_cb: AfterWorkCB<'a>,
}

/// Callback for uv_queue_work
extern "C" fn uv_work_cb(req: *mut uv_work_t) {
    let dataptr = crate::Req::get_data(uv_handle!(req));
    if !dataptr.is_null() {
        unsafe {
            if let super::WorkData(d) = &mut *dataptr {
                d.work_cb.call(req.into_inner());
            }
        }
    }
}

extern "C" fn uv_after_work_cb(req: *mut uv_work_t, status: i32) {
    let dataptr = crate::Req::get_data(uv_handle!(req));
    if !dataptr.is_null() {
        unsafe {
            if let super::WorkData(d) = &mut *dataptr {
                let status = if status < 0 {
                    Err(crate::Error::from_inner(status as uv::uv_errno_t))
                } else {
                    Ok(status as _)
                };
                d.after_work_cb.call(req.into_inner(), status);
            }
        }
    }

    // free memory
    let mut req = WorkReq::from_inner(req);
    req.destroy();
}

/// Work request type.
#[derive(Clone, Copy)]
pub struct WorkReq {
    req: *mut uv_work_t,
}

impl WorkReq {
    /// Create a new work request
    pub fn new<CB: Into<WorkCB<'static>>, ACB: Into<AfterWorkCB<'static>>>(
        work_cb: CB,
        after_work_cb: ACB,
    ) -> crate::Result<WorkReq> {
        let layout = std::alloc::Layout::new::<uv_work_t>();
        let req = unsafe { std::alloc::alloc(layout) as *mut uv_work_t };
        if req.is_null() {
            return Err(crate::Error::ENOMEM);
        }

        let work_cb = work_cb.into();
        let after_work_cb = after_work_cb.into();
        crate::Req::initialize_data(
            uv_handle!(req),
            super::WorkData(WorkDataFields {
                work_cb,
                after_work_cb,
            }),
        );

        Ok(WorkReq { req })
    }

    /// Loop that started this request and where completion will be reported
    pub fn r#loop(&self) -> crate::Loop {
        unsafe { (*self.req).loop_ }.into_inner()
    }

    pub fn destroy(&mut self) {
        crate::Req::free_data(uv_handle!(self.req));

        let layout = std::alloc::Layout::new::<uv_work_t>();
        unsafe { std::alloc::dealloc(self.req as _, layout) };
    }
}

impl FromInner<*mut uv_work_t> for WorkReq {
    fn from_inner(req: *mut uv_work_t) -> WorkReq {
        WorkReq { req }
    }
}

impl Inner<*mut uv_work_t> for WorkReq {
    fn inner(&self) -> *mut uv_work_t {
        self.req
    }
}

impl Inner<*mut uv::uv_req_t> for WorkReq {
    fn inner(&self) -> *mut uv::uv_req_t {
        uv_handle!(self.req)
    }
}

impl From<WorkReq> for crate::Req {
    fn from(work: WorkReq) -> crate::Req {
        crate::Req::from_inner(Inner::<*mut uv::uv_req_t>::inner(&work))
    }
}

impl crate::ToReq for WorkReq {
    fn to_req(&self) -> crate::Req {
        crate::Req::from_inner(Inner::<*mut uv::uv_req_t>::inner(self))
    }
}

impl crate::ReqTrait for WorkReq {}

impl crate::Loop {
    /// Initializes a work request which will run the given work_cb in a thread from the
    /// threadpool. Once work_cb is completed, after_work_cb will be called on the loop thread.
    ///
    /// This request can be cancelled with Req::cancel().
    pub fn queue_work<CB: Into<WorkCB<'static>>, ACB: Into<AfterWorkCB<'static>>>(
        &self,
        work_cb: CB,
        after_work_cb: ACB,
    ) -> crate::Result<WorkReq> {
        let work_cb = work_cb.into();
        let uv_work_cb = use_c_callback!(uv_work_cb, work_cb);
        let mut req = WorkReq::new(work_cb, after_work_cb)?;
        let uv_after_work_cb = Some(uv_after_work_cb as _);
        let result = crate::uvret(unsafe {
            uv_queue_work(self.into_inner(), req.inner(), uv_work_cb, uv_after_work_cb)
        });
        if result.is_err() {
            req.destroy();
        }
        result.map(|_| req)
    }
}
