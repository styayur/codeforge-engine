fn close_handle(handle: *mut std::ffi::c_void) {
    unsafe { close(handle) }
}

unsafe fn close(_handle: *mut std::ffi::c_void) {}
