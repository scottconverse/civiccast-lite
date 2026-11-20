use std::{
    ffi::c_void,
    fs::File,
    os::windows::{
        ffi::OsStrExt,
        io::{FromRawHandle, RawHandle},
    },
    path::Path,
    ptr,
};
type Bool = i32;
type Dword = u32;
type Handle = *mut c_void;

const HANDOFF_SDDL: &str = "D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;OW)";
const INVALID_HANDLE_VALUE: Handle = -1_isize as Handle;
const GENERIC_WRITE: Dword = 0x4000_0000;
const CREATE_NEW: Dword = 1;
const FILE_ATTRIBUTE_NORMAL: Dword = 0x0000_0080;
const SECURITY_DESCRIPTOR_REVISION: Dword = 1;
const ERROR_FILE_EXISTS: Dword = 80;
const ERROR_ALREADY_EXISTS: Dword = 183;

#[repr(C)]
struct SecurityAttributes {
    length: Dword,
    security_descriptor: *mut c_void,
    inherit_handle: Bool,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(
        name: *const u16,
        desired_access: Dword,
        share_mode: Dword,
        security_attributes: *mut SecurityAttributes,
        creation_disposition: Dword,
        flags_and_attributes: Dword,
        template_file: Handle,
    ) -> Handle;
    fn GetLastError() -> Dword;
    fn LocalFree(memory: *mut c_void) -> *mut c_void;
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(
        string_security_descriptor: *const u16,
        string_revision: Dword,
        security_descriptor: *mut *mut c_void,
        security_descriptor_size: *mut Dword,
    ) -> Bool;
}

struct SecurityDescriptor {
    raw: *mut c_void,
}

impl SecurityDescriptor {
    fn handoff() -> Result<Self, String> {
        let sddl = HANDOFF_SDDL
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        let mut raw = ptr::null_mut();
        // SAFETY: the SDDL string is null-terminated and `raw` points to writable storage.
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SECURITY_DESCRIPTOR_REVISION,
                &mut raw,
                ptr::null_mut(),
            )
        };
        if converted == 0 {
            Err(last_windows_error(
                "could not create bootstrap handoff security descriptor",
            ))
        } else {
            Ok(Self { raw })
        }
    }
}

impl Drop for SecurityDescriptor {
    fn drop(&mut self) {
        // SAFETY: the SDDL conversion allocated this descriptor; it is freed exactly once.
        unsafe { LocalFree(self.raw) };
    }
}

pub(super) fn create_secure_handoff_file(path: &Path) -> Result<File, String> {
    let wide_path = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let security = SecurityDescriptor::handoff()?;
    let mut attributes = SecurityAttributes {
        length: std::mem::size_of::<SecurityAttributes>() as Dword,
        security_descriptor: security.raw,
        inherit_handle: 0,
    };
    // SAFETY: path and security descriptor remain live through the call, attributes has the
    // Windows ABI layout, CREATE_NEW is atomic, and a successful handle is uniquely owned here.
    let handle = unsafe {
        CreateFileW(
            wide_path.as_ptr(),
            GENERIC_WRITE,
            0,
            &mut attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        // SAFETY: GetLastError reads the calling thread's error value immediately after failure.
        let code = unsafe { GetLastError() };
        if matches!(code, ERROR_FILE_EXISTS | ERROR_ALREADY_EXISTS) {
            Err("bootstrap handoff file already exists".into())
        } else {
            Err(windows_error(
                "could not create bootstrap handoff file",
                code,
            ))
        }
    } else {
        // SAFETY: CreateFileW returned a unique owned file handle. File closes it exactly once.
        Ok(unsafe { File::from_raw_handle(handle as RawHandle) })
    }
}

fn last_windows_error(operation: &str) -> String {
    // SAFETY: GetLastError reads the calling thread's error value.
    windows_error(operation, unsafe { GetLastError() })
}

fn windows_error(operation: &str, code: Dword) -> String {
    format!(
        "{operation}: {} (Windows error {code})",
        std::io::Error::from_raw_os_error(code as i32)
    )
}
