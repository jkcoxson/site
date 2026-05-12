// Jackson Coxson
// Shared state for the in-browser idevice tools UI.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeviceMeta {
    pub serial: String,
    pub webusb_serial: String,
    pub vid: u16,
    pub pid: u16,
}

#[derive(Clone, Copy)]
pub struct IdeviceState {
    pub device: RwSignal<Option<DeviceMeta>>,
    pub pairing_xml: RwSignal<Option<String>>,
    pub log: RwSignal<Vec<String>>,
    pub log_level: RwSignal<LogLevel>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub const ALL: &'static [LogLevel] = &[
        LogLevel::Off,
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
        LogLevel::Trace,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Off => "off",
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }

    pub fn level_from_str(s: &str) -> Option<Self> {
        Some(match s {
            "off" => LogLevel::Off,
            "error" => LogLevel::Error,
            "warn" => LogLevel::Warn,
            "info" => LogLevel::Info,
            "debug" => LogLevel::Debug,
            "trace" => LogLevel::Trace,
            _ => return None,
        })
    }

    pub fn as_tracing(self) -> Option<tracing::Level> {
        Some(match self {
            LogLevel::Off => return None,
            LogLevel::Error => tracing::Level::ERROR,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Trace => tracing::Level::TRACE,
        })
    }
}

impl IdeviceState {
    pub fn push_log(&self, line: impl Into<String>) {
        let line = line.into();
        #[cfg(target_arch = "wasm32")]
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(&line));
        self.log.update(|v| v.push(line));
    }

    pub fn clear_log(&self) {
        self.log.update(|v| v.clear());
    }
}

pub const PAIRING_KEY_PREFIX: &str = "idevice_tools_pairing__";
pub const LOG_LEVEL_STORAGE_KEY: &str = "idevice_tools_log_level";

pub fn provide_idevice_state() {
    #[cfg(target_arch = "wasm32")]
    let initial_level = load_log_level().unwrap_or(LogLevel::Info);
    #[cfg(not(target_arch = "wasm32"))]
    let initial_level = LogLevel::Info;

    let state = IdeviceState {
        device: RwSignal::new(None),
        pairing_xml: RwSignal::new(None),
        log: RwSignal::new(Vec::new()),
        log_level: RwSignal::new(initial_level),
    };
    provide_context(state);
}

pub fn use_idevice_state() -> IdeviceState {
    expect_context::<IdeviceState>()
}

#[cfg(target_arch = "wasm32")]
pub fn local_storage() -> Result<web_sys::Storage, String> {
    web_sys::window()
        .ok_or_else(|| "no window".to_string())?
        .local_storage()
        .map_err(|e| format!("localStorage access denied: {e:?}"))?
        .ok_or_else(|| "localStorage unavailable".to_string())
}

#[cfg(target_arch = "wasm32")]
fn pairing_key(serial: &str) -> String {
    format!("{PAIRING_KEY_PREFIX}{serial}")
}

#[cfg(target_arch = "wasm32")]
pub fn save_pairing_for(serial: &str, xml: &str) -> Result<(), String> {
    if serial.is_empty() {
        return Err("Refusing to save a pairing file without a serial number.".to_string());
    }
    local_storage()?
        .set_item(&pairing_key(serial), xml)
        .map_err(|e| format!("localStorage.setItem: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
pub fn load_pairing_for(serial: &str) -> Result<Option<String>, String> {
    if serial.is_empty() {
        return Ok(None);
    }
    local_storage()?
        .get_item(&pairing_key(serial))
        .map_err(|e| format!("localStorage.getItem: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
pub fn save_log_level(level: LogLevel) {
    if let Ok(storage) = local_storage() {
        let _ = storage.set_item(LOG_LEVEL_STORAGE_KEY, level.as_str());
    }
}

#[cfg(target_arch = "wasm32")]
pub fn load_log_level() -> Option<LogLevel> {
    local_storage()
        .ok()?
        .get_item(LOG_LEVEL_STORAGE_KEY)
        .ok()
        .flatten()
        .and_then(|s| LogLevel::level_from_str(&s))
}
