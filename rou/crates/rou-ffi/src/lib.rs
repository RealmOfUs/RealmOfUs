//! The only unsafe boundary. Core operations remain pure, safe Rust.
use rou_core::{Error, formats, ops};
use std::{
    ffi::{CStr, c_char},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr, slice,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RouBuf {
    pub ptr: *mut u8,
    pub len: usize,
}
impl RouBuf {
    const EMPTY: Self = Self {
        ptr: ptr::null_mut(),
        len: 0,
    };
    fn owned(bytes: Vec<u8>) -> Self {
        let mut bytes = bytes.into_boxed_slice();
        let result = Self {
            ptr: bytes.as_mut_ptr(),
            len: bytes.len(),
        };
        std::mem::forget(bytes);
        result
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rou_abi_version() -> u32 {
    catch_unwind(|| formats::ABI_VERSION).unwrap_or(0)
}

/// Invoke a core operation. Returns 0 on success, 1 on a domain error, or 2 if
/// the output pointer is null. All allocated responses must be freed exactly once.
///
/// # Safety
/// `op` must point to a readable NUL-terminated string; `input` must point to
/// `input_len` readable bytes (or be null when length is zero). `out` must be null
/// or point to writable, aligned RouBuf storage. Input and output must not alias.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rou_call(
    op: *const c_char,
    input: *const u8,
    input_len: usize,
    out: *mut RouBuf,
) -> i32 {
    if out.is_null() {
        return 2;
    }
    // SAFETY: the caller supplies valid output storage, checked for null above.
    unsafe {
        out.write(RouBuf::EMPTY);
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        if op.is_null() || (input.is_null() && input_len != 0) || input_len > isize::MAX as usize {
            return Err(Error::new(
                "invalid_argument",
                "invalid operation or input buffer",
            ));
        }
        // SAFETY: valid NUL-terminated op and input slice are the ABI contract.
        let name = unsafe { CStr::from_ptr(op) }
            .to_str()
            .map_err(|_| Error::new("invalid_argument", "operation name must be UTF-8"))?;
        let bytes = if input_len == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(input, input_len) }
        };
        ops::dispatch(name, bytes)
    }))
    .unwrap_or_else(|_| Err(Error::new("internal_panic", "core operation panicked")));
    let response = catch_unwind(AssertUnwindSafe(|| {
        let (status, bytes) = ops::envelope(result);
        (status, RouBuf::owned(bytes))
    }));
    match response {
        Ok((status, buffer)) => {
            // SAFETY: same valid output storage as above.
            unsafe {
                out.write(buffer);
            }
            status
        }
        Err(_) => 3,
    }
}

/// Release a response allocated by rou_call. A null buffer is a no-op.
///
/// # Safety
/// Non-null buffers must be unmodified values returned by rou_call, not already
/// freed. They must not be accessed by another thread during or after this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rou_free(buf: RouBuf) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        if !buf.ptr.is_null() {
            // SAFETY: rou_call allocated this exact boxed slice; the contract
            // transfers ownership back once, with its original length.
            unsafe {
                drop(Box::from_raw(ptr::slice_from_raw_parts_mut(
                    buf.ptr, buf.len,
                )));
            }
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    fn call(op: &str, bytes: &[u8]) -> (i32, Vec<u8>) {
        let op = CString::new(op).unwrap();
        let mut out = RouBuf::EMPTY;
        // SAFETY: live input storage, valid C string and unique output allocation.
        unsafe {
            let status = rou_call(op.as_ptr(), bytes.as_ptr(), bytes.len(), &mut out);
            let copy = slice::from_raw_parts(out.ptr, out.len).to_vec();
            rou_free(out);
            (status, copy)
        }
    }
    #[test]
    fn abi_uses_the_exact_shared_envelopes() {
        assert_eq!(rou_abi_version(), formats::ABI_VERSION);
        for (op, data) in [
            ("config", &b""[..]),
            ("json.strict", &b"{\"a\":1,\"a\":2}"[..]),
            ("digest.sha256", &b"\0\xff"[..]),
            ("unknown", &b"null"[..]),
        ] {
            assert_eq!(call(op, data), ops::envelope(ops::dispatch(op, data)));
        }
    }
    #[test]
    fn invalid_buffers_return_errors_without_allocating_for_null_out() {
        // SAFETY: nulls are expressly supported invalid arguments.
        unsafe {
            assert_eq!(rou_call(ptr::null(), ptr::null(), 1, ptr::null_mut()), 2);
            let mut out = RouBuf::EMPTY;
            assert_eq!(rou_call(ptr::null(), ptr::null(), 1, &mut out), 1);
            let value: serde_json::Value =
                serde_json::from_slice(slice::from_raw_parts(out.ptr, out.len)).unwrap();
            assert_eq!(value["error"]["code"], "invalid_argument");
            rou_free(out);
            rou_free(RouBuf::EMPTY);
        }
    }
    #[test]
    fn simultaneous_call_and_free_have_no_shared_mutable_state() {
        let workers: Vec<_> = (0..8)
            .map(|_| {
                std::thread::spawn(|| {
                    for _ in 0..200 {
                        assert_eq!(call("json.strict", b"[1,true,null]").0, 0);
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().unwrap();
        }
    }
}
