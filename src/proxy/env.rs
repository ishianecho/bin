use super::config::Config;
use std::env;

pub fn apply(cfg: &Config) -> Result<(), String> {
    let http_keys = ["HTTP_PROXY", "HTTPS_PROXY", "http_proxy", "https_proxy"];
    let socks_keys = ["ALL_PROXY", "all_proxy"];
    let no_proxy_keys = ["NO_PROXY", "no_proxy"];
    if !cfg.http.is_empty() {
        for key in http_keys {
            env::set_var(key, &cfg.http);
        }
    } else {
        for key in http_keys {
            env::remove_var(key);
        }
    }
    if !cfg.socks.is_empty() {
        for key in socks_keys {
            env::set_var(key, &cfg.socks);
        }
    } else {
        for key in socks_keys {
            env::remove_var(key);
        }
    }
    if !cfg.http.is_empty() || !cfg.socks.is_empty() {
        let no_proxy = cfg.default_no_proxy();
        for key in no_proxy_keys {
            env::set_var(key, &no_proxy);
        }
    } else {
        for key in no_proxy_keys {
            env::remove_var(key);
        }
    }
    set_user_environment(cfg)
}

#[cfg(windows)]
fn set_user_environment(cfg: &Config) -> Result<(), String> {
    use std::ptr;
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS, HANDLE},
        System::Registry::{
            RegCloseKey, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ,
        },
        UI::WindowsAndMessaging::{
            SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
        },
    };

    let mut key: HANDLE = ptr::null_mut();
    let subkey: Vec<u16> = "Environment\0".encode_utf16().collect();
    let result =
        unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_SET_VALUE, &mut key) };
    if result != ERROR_SUCCESS {
        return Err(format!("set user environment: error {result}"));
    }

    let mut entries: Vec<(&str, &str)> = vec![
        ("HTTP_PROXY", &cfg.http),
        ("HTTPS_PROXY", &cfg.http),
        ("ALL_PROXY", &cfg.socks),
    ];
    let no_proxy = cfg.default_no_proxy();
    if cfg.http.is_empty() && cfg.socks.is_empty() {
        entries.push(("NO_PROXY", ""));
    } else {
        entries.push(("NO_PROXY", &no_proxy));
    }
    for (name, value) in entries {
        let name_utf16: Vec<u16> = name.encode_utf16().chain([0]).collect();
        let result = if value.is_empty() {
            unsafe { windows_sys::Win32::System::Registry::RegDeleteValueW(key, name_utf16.as_ptr()) }
        } else {
            let data: Vec<u16> = value.encode_utf16().chain([0]).collect();
            unsafe {
                RegSetValueExW(
                    key,
                    name_utf16.as_ptr(),
                    0,
                    REG_SZ,
                    data.as_ptr() as *const u8,
                    (data.len() * 2) as u32,
                )
            }
        };
        if result == ERROR_FILE_NOT_FOUND && value.is_empty() {
            continue;
        }
        if result != ERROR_SUCCESS {
            unsafe { RegCloseKey(key) };
            return Err(format!("set user environment {name}: error {result}"));
        }
    }
    unsafe { RegCloseKey(key) };

    let environment: Vec<u16> = "Environment\0".encode_utf16().collect();
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            environment.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5000,
            ptr::null_mut(),
        );
    }
    Ok(())
}

#[cfg(not(windows))]
fn set_user_environment(_cfg: &Config) -> Result<(), String> {
    Ok(())
}
