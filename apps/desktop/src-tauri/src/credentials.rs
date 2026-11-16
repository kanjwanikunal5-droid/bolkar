//! API keys live in Windows Credential Manager, never in files, the database or logs.

use windows::core::{HSTRING, PWSTR};
use windows::Win32::Foundation::FILETIME;
use windows::Win32::Security::Credentials::{
    CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_FLAGS,
    CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
};

/// Keys are filed under the app's permanent id (it never changes, unlike the display name).
fn target(provider: &str) -> String {
    format!("{}/api-key/{provider}", app_core::brand::APP_ID)
}

/// Where earlier versions kept the key: under the display name, current or previous.
fn legacy_targets(provider: &str) -> Vec<String> {
    std::iter::once(app_core::brand::APP_NAME)
        .chain(app_core::brand::PREVIOUS_NAMES.iter().copied())
        .map(|name| format!("{name}/api-key/{provider}"))
        .collect()
}

fn read(target: &str) -> Option<String> {
    let target = HSTRING::from(target);
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

pub fn save(provider: &str, secret: &str) -> Result<(), String> {
    write(&target(provider), provider, secret)
}

fn write(target: &str, provider: &str, secret: &str) -> Result<(), String> {
    let target = HSTRING::from(target);
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
    if let Some(key) = read(&target(provider)) {
        return Some(key);
    }
    // Saved by an earlier version under the display name: copy it to the permanent place, so
    // renaming the app never asks the user for the key again. The old entry is kept, so an
    // older version still installed (or reinstalled) keeps working; `delete` removes both.
    for old in legacy_targets(provider) {
        if let Some(key) = read(&old) {
            let _ = save(provider, &key);
            return Some(key);
        }
    }
    None
}

pub fn delete(provider: &str) {
    for t in std::iter::once(target(provider)).chain(legacy_targets(provider)) {
        unsafe {
            let _ = CredDeleteW(&HSTRING::from(t.as_str()), CRED_TYPE_GENERIC, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_saved_under_an_old_name_is_found_and_copied() {
        if std::env::var_os("GITHUB_ACTIONS").is_some() {
            return;
        }
        let p = format!("selftest-move-{}", std::process::id());
        let old = legacy_targets(&p).pop().unwrap();
        write(&old, &p, "secret-value-123").unwrap();
        assert_eq!(read(&target(&p)), None);
        assert_eq!(load(&p).as_deref(), Some("secret-value-123"));
        assert_eq!(
            read(&target(&p)).as_deref(),
            Some("secret-value-123"),
            "copied to the id"
        );
        assert_eq!(
            read(&old).as_deref(),
            Some("secret-value-123"),
            "old entry kept for an older version"
        );
        delete(&p);
        assert_eq!(load(&p), None);
        assert_eq!(read(&old), None, "delete removes the old entry too");
    }

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
