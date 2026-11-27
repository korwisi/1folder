use crate::{core, undo::UndoSelection};
use serde::Serialize;
use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;

#[repr(C)]
pub struct OnefolderResult {
    pub json: *mut c_char,
    pub error: *mut c_char,
}

fn empty_string() -> *mut c_char {
    CString::new("").unwrap().into_raw()
}

fn result<T: Serialize>(value: T) -> OnefolderResult {
    match serde_json::to_string(&value) {
        Ok(json) => OnefolderResult {
            json: CString::new(json).unwrap().into_raw(),
            error: empty_string(),
        },
        Err(error) => error_result(error.to_string()),
    }
}

fn error_result(message: String) -> OnefolderResult {
    let error = CString::new(message).unwrap_or_else(|_| CString::new("Invalid error").unwrap());
    OnefolderResult {
        json: std::ptr::null_mut(),
        error: error.into_raw(),
    }
}

fn config_from_path(path: *const c_char) -> Result<crate::config::Config, String> {
    if path.is_null() {
        return Err("Config path is null".to_string());
    }
    let path = unsafe { CStr::from_ptr(path) }.to_string_lossy();
    core::load_config(Path::new(path.as_ref())).map_err(|error| format!("{error:#}"))
}

#[no_mangle]
pub extern "C" fn onefolder_status(config_path: *const c_char) -> OnefolderResult {
    match config_from_path(config_path)
        .and_then(|config| core::status(&config).map_err(|error| format!("{error:#}")))
    {
        Ok(value) => result(value),
        Err(error) => error_result(error),
    }
}

#[no_mangle]
pub extern "C" fn onefolder_preview(config_path: *const c_char) -> OnefolderResult {
    match config_from_path(config_path)
        .and_then(|config| core::preview(&config, None).map_err(|error| format!("{error:#}")))
    {
        Ok(value) => result(value),
        Err(error) => error_result(error),
    }
}

#[no_mangle]
pub extern "C" fn onefolder_run_once(
    config_path: *const c_char,
    dry_run: c_int,
) -> OnefolderResult {
    match config_from_path(config_path).and_then(|config| {
        core::run_once(&config, dry_run != 0, None).map_err(|error| format!("{error:#}"))
    }) {
        Ok(value) => result(value),
        Err(error) => error_result(error),
    }
}

#[no_mangle]
pub extern "C" fn onefolder_history(config_path: *const c_char) -> OnefolderResult {
    match config_from_path(config_path)
        .and_then(|config| core::history(&config).map_err(|error| format!("{error:#}")))
    {
        Ok(value) => result(value),
        Err(error) => error_result(error),
    }
}

#[no_mangle]
pub extern "C" fn onefolder_set_target_icon(config_path: *const c_char) -> OnefolderResult {
    match config_from_path(config_path).and_then(|config| {
        core::set_target_folder_icon(&config).map_err(|error| format!("{error:#}"))
    }) {
        Ok(value) => result(value),
        Err(error) => error_result(error),
    }
}

#[no_mangle]
pub extern "C" fn onefolder_create_sidebar_alias(config_path: *const c_char) -> OnefolderResult {
    match config_from_path(config_path).and_then(|config| {
        core::create_finder_sidebar_alias(&config).map_err(|error| format!("{error:#}"))
    }) {
        Ok(value) => result(value),
        Err(error) => error_result(error),
    }
}

#[no_mangle]
pub extern "C" fn onefolder_undo_last(
    config_path: *const c_char,
    dry_run: c_int,
) -> OnefolderResult {
    let selection = UndoSelection {
        all: false,
        count: Some(1),
        within: None,
        src_prefix: None,
    };
    match config_from_path(config_path).and_then(|config| {
        core::undo(&config, dry_run != 0, &selection).map_err(|error| format!("{error:#}"))
    }) {
        Ok(value) => result(value),
        Err(error) => error_result(error),
    }
}

#[no_mangle]
/// Releases a string allocated by the 1folder Rust library.
///
/// # Safety
/// `value` must be null or point to a string allocated with `CString::into_raw`
/// by this library. It must be passed only once.
pub unsafe extern "C" fn onefolder_free_string(value: *mut c_char) {
    if !value.is_null() {
        unsafe {
            drop(CString::from_raw(value));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_result(result: OnefolderResult) -> String {
        assert!(
            result.error.is_null() || unsafe { CStr::from_ptr(result.error).to_bytes().is_empty() }
        );
        let json = unsafe { CStr::from_ptr(result.json).to_string_lossy().into_owned() };
        unsafe {
            onefolder_free_string(result.json);
            onefolder_free_string(result.error);
        }
        json
    }

    #[test]
    fn ffi_status_returns_serialized_status() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("config.toml");
        let watched = tmp.path().join("watched");
        let target = tmp.path().join("target");
        std::fs::create_dir_all(&watched).unwrap();
        std::fs::write(
            &config_path,
            format!(
                "target_dir = \"{}\"\n[watch]\ndirs = [\"{}\"]\n",
                target.display(),
                watched.display()
            ),
        )
        .unwrap();
        let path = CString::new(config_path.to_string_lossy().as_bytes()).unwrap();

        let json = read_result(onefolder_status(path.as_ptr()));

        assert!(json.contains("\"target_dir\""));
        assert!(json.contains("\"watched_dirs\""));
    }
}
