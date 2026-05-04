use winreg::RegKey;

pub(crate) fn read_reg_u64_value(key: &RegKey, name: &str) -> Option<u64> {
    if let Ok(v) = key.get_value::<u64, _>(name) {
        return Some(v);
    }
    if let Ok(v) = key.get_value::<u32, _>(name) {
        return Some(v as u64);
    }
    if let Ok(s) = key.get_value::<String, _>(name) {
        return s.parse::<u64>().ok();
    }
    None
}

pub(crate) fn is_registry_capability_key_in_use_recursive(key: &RegKey) -> bool {
    let start = read_reg_u64_value(key, "LastUsedTimeStart").unwrap_or(0);
    let stop = read_reg_u64_value(key, "LastUsedTimeStop").unwrap_or(0);
    if start > 0 && (stop == 0 || stop < start) {
        return true;
    }

    for name in key.enum_keys().filter_map(|r| r.ok()) {
        if let Ok(subkey) = key.open_subkey(&name) {
            if is_registry_capability_key_in_use_recursive(&subkey) {
                return true;
            }
        }
    }
    false
}

pub(crate) fn is_capability_in_use(capability: &str) -> bool {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};

    let base_path = format!(
        r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\{}",
        capability
    );
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(cap_key) = hkcu.open_subkey_with_flags(&base_path, KEY_READ) {
        if is_registry_capability_key_in_use_recursive(&cap_key) {
            return true;
        }
    }
    false
}

pub(crate) fn get_privacy_usage_state() -> (bool, bool) {
    let microphone = is_capability_in_use("microphone");
    let camera = is_capability_in_use("webcam");
    (microphone, camera)
}
