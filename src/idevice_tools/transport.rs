// Jackson Coxson
// WebUSB + netmuxd transport

#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;

use idevice::{pairing_file::PairingFile, services::lockdown::LockdownClient, Idevice};
use leptos::prelude::*;
use netmuxd::usb::apple::{self, APPLE_VID};
use netmuxd::usb::mux::UsbMuxHandle;
use netmuxd::usb::provider::UsbMuxProvider;
use nusb::DeviceInfo;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{UsbDeviceFilter, UsbDeviceRequestOptions};

use super::state::{load_pairing_for, save_pairing_for, DeviceMeta, IdeviceState};

thread_local! {
    static MUX: RefCell<Option<UsbMuxHandle>> = const { RefCell::new(None) };
}

pub fn get_mux() -> Result<UsbMuxHandle, String> {
    MUX.with(|m| m.borrow().clone())
        .ok_or_else(|| "Click \"Connect device\" first.".to_string())
}

pub fn has_mux() -> bool {
    MUX.with(|m| m.borrow().is_some())
}

const CONNECT_ATTEMPTS: usize = 5;
const CONNECT_RETRY_DELAY_MS: i32 = 400;

/// Show the WebUSB picker, enumerate via nusb, then open and claim the mux
/// interface. The handle is stashed in [`MUX`] and the device meta is
/// written to the shared signal so the UI reacts.
pub async fn connect_iphone(state: IdeviceState) -> Result<(), String> {
    if has_mux() {
        state.push_log("Mux already open. Reload the page to reconnect.");
        return Ok(());
    }

    let nav = web_sys::window()
        .ok_or_else(|| "no window".to_string())?
        .navigator();

    let has_usb = js_sys::Reflect::has(nav.as_ref(), &JsValue::from_str("usb")).unwrap_or(false);
    if !has_usb {
        return Err("WebUSB is not available in this browser.".to_string());
    }
    let usb = nav.usb();

    let filter = UsbDeviceFilter::new();
    filter.set_vendor_id(APPLE_VID);
    let filters = [filter];
    let opts = UsbDeviceRequestOptions::new(&filters);

    state.push_log("Requesting WebUSB device picker...");
    JsFuture::from(usb.request_device(&opts))
        .await
        .map_err(|e| format!("requestDevice: {e:?}"))?;
    state.push_log("Permission granted.");

    let mut last_err: Option<String> = None;
    for attempt in 1..=CONNECT_ATTEMPTS {
        match try_open_mux_once(&state).await {
            Ok((handle, meta)) => {
                MUX.with(|m| *m.borrow_mut() = Some(handle));
                let serial = meta.serial.clone();
                state.device.set(Some(meta));
                load_existing_pairing(&state, &serial);
                state.push_log("Mux task ready.");
                return Ok(());
            }
            Err(e) => {
                state.push_log(format!(
                    "Connect attempt {attempt}/{CONNECT_ATTEMPTS} failed: {e}"
                ));
                last_err = Some(e);
                if attempt < CONNECT_ATTEMPTS {
                    sleep_ms(CONNECT_RETRY_DELAY_MS).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| "connect failed".to_string()))
}

async fn try_open_mux_once(state: &IdeviceState) -> Result<(UsbMuxHandle, DeviceMeta), String> {
    let info = nusb::list_devices()
        .await
        .map_err(|e| format!("list_devices: {e}"))?
        .find(apple::is_apple_mux)
        .ok_or_else(|| "No Apple usbmuxd device permitted (not yet enumerated?)".to_string())?;

    let vid = info.vendor_id();
    let pid = info.product_id();
    state.push_log(format!(
        "Found {vid:04x}:{pid:04x}  {}",
        info.serial_number().unwrap_or("(no serial)"),
    ));

    state.push_log("Opening device and claiming mux interface...");
    let opened = apple::open_mux(&info)
        .await
        .map_err(|e| format!("open_mux: {e}"))?;

    let webusb_serial = normalize_serial(&info);

    state.push_log("Spawning usbmuxd-v2 mux task...");
    let (exit_tx, _exit_rx) = tokio::sync::oneshot::channel();
    let handle = netmuxd::usb::mux::spawn(
        1,
        webusb_serial.clone(),
        opened.reader,
        opened.writer,
        exit_tx,
    );

    state.push_log("Querying lockdown for UniqueDeviceID...");
    match fetch_udid(&handle).await {
        Ok(udid) => {
            state.push_log(format!("UDID: {udid}"));
            Ok((
                handle,
                DeviceMeta {
                    serial: udid,
                    webusb_serial,
                    vid,
                    pid,
                },
            ))
        }
        Err(e) => {
            handle.shutdown().await;
            Err(e)
        }
    }
}

async fn fetch_udid(handle: &UsbMuxHandle) -> Result<String, String> {
    let stream = handle
        .connect(LockdownClient::LOCKDOWND_PORT)
        .await
        .map_err(|e| format!("mux connect (lockdown): {e}"))?;
    let idevice = Idevice::new(Box::new(stream), "jkcoxson-idevice-tools");
    let mut lockdown = LockdownClient::new(idevice);
    let value = lockdown
        .get_value(Some("UniqueDeviceID"), None)
        .await
        .map_err(|e| format!("get_value(UniqueDeviceID): {e:?}"))?;
    match value {
        plist::Value::String(s) => Ok(s),
        other => Err(format!("UniqueDeviceID returned non-string: {other:?}")),
    }
}

fn normalize_serial(info: &DeviceInfo) -> String {
    info.serial_number()
        .map(|s| {
            s.trim_matches(|c: char| c == '\0' || c.is_whitespace())
                .to_string()
        })
        .unwrap_or_default()
}

fn load_existing_pairing(state: &IdeviceState, serial: &str) {
    match load_pairing_for(serial) {
        Ok(Some(xml)) => {
            state.push_log(format!(
                "Loaded existing pairing file for {serial} from browser storage."
            ));
            state.pairing_xml.set(Some(xml));
        }
        Ok(None) => {
            state.push_log(format!(
                "No saved pairing file for {serial} - pair the device to enable TLS-protected features."
            ));
            state.pairing_xml.set(None);
        }
        Err(e) => {
            state.push_log(format!("ERROR reading pairing file: {e}"));
            state.pairing_xml.set(None);
        }
    }
}

pub async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        if let Some(win) = web_sys::window() {
            let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = JsFuture::from(promise).await;
}

pub async fn open_lockdown() -> Result<LockdownClient, String> {
    let handle = get_mux()?;
    let stream = handle
        .connect(LockdownClient::LOCKDOWND_PORT)
        .await
        .map_err(|e| format!("mux connect: {e}"))?;
    let idevice = Idevice::new(Box::new(stream), "jkcoxson-idevice-tools");
    Ok(LockdownClient::new(idevice))
}

pub fn load_pairing_file(state: &IdeviceState) -> Result<PairingFile, String> {
    let xml = state
        .pairing_xml
        .get_untracked()
        .ok_or_else(|| "No pairing file. Click \"Pair device\" first.".to_string())?;
    PairingFile::from_bytes(xml.as_bytes()).map_err(|e| format!("parse pairing file: {e:?}"))
}

pub async fn pair_device(state: IdeviceState) -> Result<(), String> {
    let serial = state
        .device
        .get_untracked()
        .map(|d| d.serial)
        .ok_or_else(|| "Connect a device before pairing.".to_string())?;

    let mut lockdown = open_lockdown().await?;
    let host_id = uuid::Uuid::new_v4().to_string().to_uppercase();
    let system_buid = uuid::Uuid::new_v4().to_string().to_uppercase();
    state.push_log("Pairing - accept the trust prompt on the device...");

    let pairing_file = lockdown
        .pair(host_id, system_buid, None)
        .await
        .map_err(|e| format!("pair: {e:?}"))?;

    let bytes = pairing_file
        .serialize()
        .map_err(|e| format!("serialize pairing file: {e:?}"))?;
    let xml = String::from_utf8(bytes).map_err(|e| format!("utf8: {e:?}"))?;

    save_pairing_for(&serial, &xml)?;
    state.pairing_xml.set(Some(xml));
    state.push_log(format!(
        "Paired. Pairing file saved for {serial} in browser storage."
    ));
    Ok(())
}

pub fn build_provider(state: &IdeviceState) -> Result<UsbMuxProvider, String> {
    let pairing_file = load_pairing_file(state)?;
    let mux = get_mux()?;
    Ok(UsbMuxProvider::new(
        mux,
        pairing_file,
        "jkcoxson-idevice-tools",
    ))
}

pub async fn open_rsd(
    state: &IdeviceState,
) -> Result<
    (
        idevice::tcp::handle::AdapterHandle,
        idevice::rsd::RsdHandshake,
    ),
    String,
> {
    use idevice::{core_device_proxy::CoreDeviceProxy, rsd::RsdHandshake, IdeviceService};

    let provider = build_provider(state)?;
    let proxy = CoreDeviceProxy::connect(&provider)
        .await
        .map_err(|e| format!("CoreDeviceProxy::connect: {e:?}"))?;
    let rsd_port = proxy.tunnel_info().server_rsd_port;
    let adapter = proxy
        .create_software_tunnel()
        .map_err(|e| format!("create_software_tunnel: {e:?}"))?;
    let mut adapter = adapter.to_async_handle();
    let stream = adapter
        .connect(rsd_port)
        .await
        .map_err(|e| format!("adapter.connect(rsd_port={rsd_port}): {e:?}"))?;
    let handshake = RsdHandshake::new(stream)
        .await
        .map_err(|e| format!("RsdHandshake::new: {e:?}"))?;
    Ok((adapter, handshake))
}

pub async fn ios_major_version(state: &IdeviceState) -> Result<u32, String> {
    let mut lockdown = open_lockdown().await?;
    let pairing = load_pairing_file(state)?;
    lockdown
        .start_session(&pairing)
        .await
        .map_err(|e| format!("start_session: {e:?}"))?;
    let value = lockdown
        .get_value(Some("ProductVersion"), None)
        .await
        .map_err(|e| format!("get_value(ProductVersion): {e:?}"))?;
    let s = value
        .as_string()
        .ok_or_else(|| "ProductVersion not a string".to_string())?;
    s.split('.')
        .next()
        .and_then(|x| x.parse::<u32>().ok())
        .ok_or_else(|| format!("unparseable ProductVersion {s:?}"))
}
