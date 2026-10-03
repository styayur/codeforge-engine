fn close_handle(handle: *mut std::ffi::c_void) {
    // SAFETY: the caller transfers ownership of a valid handle to this wrapper.
    unsafe { close(handle) }
}

unsafe fn close(_handle: *mut std::ffi::c_void) {}
