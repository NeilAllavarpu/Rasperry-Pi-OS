use core::ffi::{c_size_t, c_uchar};

use crate::errno::Error;

pub type fpos_t = u64;

struct Pipe {
    /// ID of the pipe to write to
    id: u16,
}

enum FileType {
    Pipe(Pipe),
}

pub struct FILE {
    is_error: bool,
    blocking: bool,
    inner: FileType,
}

impl FILE {
    pub fn fputc(&mut self, c: c_uchar) -> crate::Result<()> {
        match self.inner {
            FileType::Pipe(_) => {
                // Talk to pipe arbiter here
                todo!()
            }
        }
    }

    pub fn fgetc(&mut self) -> crate::Result<c_uchar> {
        match self.inner {
            FileType::Pipe(_) => {
                // Talk to pipe arbiter here
                todo!()
            }
        }
    }

    pub fn fread(&mut self, buffer: &mut [c_uchar]) -> (c_size_t, Option<Error>) {
        for (n, byte) in buffer.iter_mut().enumerate() {
            match self.fgetc() {
                Ok(c) => *byte = c,
                Err(err) => return (n, Some(err)),
            }
        }
        (buffer.len(), None)
    }

    pub fn fwrite(&mut self, buffer: &[c_uchar]) -> (c_size_t, Option<Error>) {
        for (n, &byte) in buffer.iter().enumerate() {
            if let Err(err) = self.fputc(byte) {
                return (n, Some(err));
            }
        }
        (buffer.len(), None)
    }
}

/// C compatible interface, as specified by POSIX
pub mod ffi {
    use crate::{errno, EOF};

    use super::FILE;
    use core::{
        ffi::{c_int, c_size_t, c_uchar, c_void},
        ptr::NonNull,
    };

    #[no_mangle]
    pub unsafe extern "C" fn fputc(c: c_int, stream: *mut FILE) -> c_int {
        assert!(
            stream.is_aligned(),
            "Stream should be a valid, aligned pointer"
        );
        let value = unsafe { stream.as_mut() }
            .expect("Stream should not be null")
            .fputc(c as c_uchar);
        match value {
            Ok(()) => c,
            Err(err) => {
                errno::set_errno(err);
                EOF
            }
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn fgetc(stream: *mut FILE) -> c_int {
        assert!(
            stream.is_aligned(),
            "Stream should be a valid, aligned pointer"
        );
        let value = unsafe { stream.as_mut() }
            .expect("Stream should not be null")
            .fgetc();
        match value {
            Ok(c) => c_int::from(c),
            Err(err) => {
                errno::set_errno(err);
                EOF
            }
        }
    }

    #[no_mangle]
    pub unsafe extern "C" fn fwrite(
        ptr: *const c_void,
        size: c_size_t,
        nitems: c_size_t,
        stream: *mut FILE,
    ) -> c_size_t {
        let ptr = NonNull::new(ptr.cast_mut()).expect("Buffer should not be null");
        let bytes = size
            .checked_mul(nitems)
            .expect("Buffer size should not overflow");
        let buffer = NonNull::slice_from_raw_parts(ptr.cast::<c_uchar>(), bytes);
        assert!(
            stream.is_aligned(),
            "Stream should be a valid, aligned pointer"
        );

        let buffer = unsafe { buffer.as_ref() };
        let (count, err) = unsafe { stream.as_mut() }
            .expect("Stream should not be null")
            .fwrite(buffer);

        if let Some(err) = err {
            errno::set_errno(err)
        }
        count
    }

    #[no_mangle]
    pub unsafe extern "C" fn fread(
        ptr: *const c_void,
        size: c_size_t,
        nitems: c_size_t,
        stream: *mut FILE,
    ) -> c_size_t {
        let ptr = NonNull::new(ptr.cast_mut()).expect("Buffer should not be null");
        let bytes = size
            .checked_mul(nitems)
            .expect("Buffer size should not overflow");
        let mut buffer = NonNull::slice_from_raw_parts(ptr.cast::<c_uchar>(), bytes);
        assert!(
            stream.is_aligned(),
            "Stream should be a valid, aligned pointer"
        );

        let buffer = unsafe { buffer.as_mut() };
        let (count, err) = unsafe { stream.as_mut() }
            .expect("Stream should not be null")
            .fread(buffer);

        if let Some(err) = err {
            errno::set_errno(err)
        }
        count
    }
}
