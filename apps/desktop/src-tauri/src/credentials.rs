//! API keys live in Windows Credential Manager, never in files, the database or logs.

use windows::core::{HSTRING, PWSTR};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_FLAGS,
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
};

fn target(provider: &str) -> String {
    format!("{}/api-key/{provider}", app_core::brand::APP_NAME)
}

pub fn save(provider: &str, secret: &str) -> Result<(), String> {
    let target = HSTRING::from(target(provider));
    let user = HSTRING::from(provider);
    let mut blob = secret.as_bytes().to_vec();
    let cred = CREDENTIALW {
        Flags: CRED_FLAGS(0),
        Type: CRED_TYPE_GENERIC,
        TargetName: PWSTR(target.as_ptr() as *mut _),
        Comment: PWSTR::null(),
        LastWritten: FILETIME::default(),
        CredentialBlobSize: blob.len() as u32,
        CredentialBlob: blob.as_mut_ptr(),
        // Stays on this computer for this Windows user; not roamed.
        Persist: CRED_PERSIST_LOCAL_MACHINE,
        AttributeCount: 0,
        Attributes: std::ptr::null_mut(),
        TargetAlias: PWSTR::null(),
        UserName: PWSTR(user.as_ptr() as *mut _),
    };
    let result = unsafe { CredWriteW(&cred, 0) };
    blob.fill(0);
    result.map_err(|e| format!("Could not save the key in Windows Credential Manager: {e}"))
}

pub fn load(provider: &str) -> Option<String> {
    let target = HSTRING::from(target(provider));
    let mut ptr: *mut CREDENTIALW = std::ptr::null_mut();
    unsafe {
        CredReadW(&target, CRED_TYPE_GENERIC, None, &mut ptr).ok()?;
        let cred = &*ptr;
        let bytes =
            std::slice::from_raw_parts(cred.CredentialBlob, cred.CredentialBlobSize as usize)
                .to_vec();
        CredFree(ptr as *const _);
        String::from_utf8(bytes).ok()
    }
}

pub fn delete(provider: &str) {
    let target = HSTRING::from(target(provider));
    unsafe {
        let _ = CredDeleteW(&target, CRED_TYPE_GENERIC, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_load_delete_round_trip() {
        if std::env::var_os("GITHUB_ACTIONS").is_some() {
            return; // no user credential store on the build machines
        }
        let p = format!("selftest-{}", std::process::id());
        assert_eq!(load(&p), None);
        save(&p, "not-a-real-key-ü").unwrap();
        assert_eq!(load(&p).as_deref(), Some("not-a-real-key-ü"));
        save(&p, "replaced").unwrap();
        assert_eq!(load(&p).as_deref(), Some("replaced"));
        delete(&p);
        assert_eq!(load(&p), None);
    }
}
