fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window().and_then(|window| window.local_storage().ok().flatten())
}

pub(super) fn read_storage(key: &str) -> Option<String> {
    local_storage()
        .and_then(|storage| storage.get_item(key).ok().flatten())
        .filter(|value| !value.is_empty())
}

pub(super) fn read_bool_storage(key: &str) -> Option<bool> {
    read_storage(key).map(|value| value == "true")
}

pub(super) fn write_storage(key: &str, value: &str) {
    if let Some(storage) = local_storage() {
        let _ = storage.set_item(key, value);
    }
}

pub(super) fn write_optional_storage(key: &str, value: Option<&str>) {
    if let Some(storage) = local_storage() {
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            let _ = storage.set_item(key, value);
        } else {
            let _ = storage.remove_item(key);
        }
    }
}
