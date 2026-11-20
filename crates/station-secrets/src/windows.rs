use std::{ffi::c_void, ptr};

#[repr(C)]
struct DataBlob {
    byte_count: u32,
    data: *mut u8,
}

#[link(name = "crypt32")]
unsafe extern "system" {
    fn CryptProtectData(
        input: *mut DataBlob,
        description: *const u16,
        entropy: *mut DataBlob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        output: *mut DataBlob,
    ) -> i32;
    fn CryptUnprotectData(
        input: *mut DataBlob,
        description: *mut *mut u16,
        entropy: *mut DataBlob,
        reserved: *mut c_void,
        prompt: *mut c_void,
        flags: u32,
        output: *mut DataBlob,
    ) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
    fn GetLastError() -> u32;
}

const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;
const CRYPTPROTECT_LOCAL_MACHINE: u32 = 0x4;

pub fn protect_local_machine(plaintext: &[u8]) -> Result<Vec<u8>, String> {
    let byte_count =
        u32::try_from(plaintext.len()).map_err(|_| "station secret is too large".to_string())?;
    let mut input = DataBlob {
        byte_count,
        data: plaintext.as_ptr().cast_mut(),
    };
    let mut output = DataBlob {
        byte_count: 0,
        data: ptr::null_mut(),
    };
    let status = unsafe {
        CryptProtectData(
            &mut input,
            ptr::null(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN | CRYPTPROTECT_LOCAL_MACHINE,
            &mut output,
        )
    };
    if status == 0 {
        return Err(format!(
            "Windows could not protect the station secret (error {}).",
            unsafe { GetLastError() }
        ));
    }
    if output.data.is_null() || output.byte_count == 0 {
        return Err("Windows returned an empty protected station secret.".into());
    }
    let protected = unsafe {
        std::slice::from_raw_parts(output.data.cast_const(), output.byte_count as usize).to_vec()
    };
    unsafe {
        LocalFree(output.data.cast());
    }
    Ok(protected)
}

pub fn unprotect_local_machine(protected: &[u8]) -> Result<Vec<u8>, String> {
    let byte_count = u32::try_from(protected.len())
        .map_err(|_| "encrypted station secret is too large".to_string())?;
    let mut input = DataBlob {
        byte_count,
        data: protected.as_ptr().cast_mut(),
    };
    let mut output = DataBlob {
        byte_count: 0,
        data: ptr::null_mut(),
    };
    let status = unsafe {
        CryptUnprotectData(
            &mut input,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if status == 0 {
        return Err(format!(
            "Windows could not unlock the station secret (error {}).",
            unsafe { GetLastError() }
        ));
    }
    if output.data.is_null() || output.byte_count == 0 {
        return Err("Windows returned an empty unlocked station secret.".into());
    }
    let plaintext = unsafe {
        std::slice::from_raw_parts(output.data.cast_const(), output.byte_count as usize).to_vec()
    };
    unsafe {
        LocalFree(output.data.cast());
    }
    Ok(plaintext)
}
