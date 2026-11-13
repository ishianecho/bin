pub fn maybe_restart(_no_restart: bool) -> Result<(), String> {
    Ok(())
}

pub fn service_state() -> String {
    "unavailable".into()
}

pub fn service_action(_action: &str) -> Result<(), String> {
    Err("service management is not supported on this platform".into())
}

pub fn sync_tproxy_env(_port: i64, _changed: bool) -> Result<(), String> {
    Ok(())
}
