// Jackson Coxson
// idevice_id - list Apple devices visible to WebUSB.
//
// The browser is silly and has a fake serial number
// for the device, so we'll report both. Once the device
// is actually open and we can ask lockdown for the real
// one we can fill it in.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::{use_idevice_state, DeviceMeta};

#[derive(Clone, Debug, PartialEq, Eq)]
struct ListedDevice {
    webusb_serial: String,
    udid: Option<String>,
    vid: u16,
    pid: u16,
    connected: bool,
}

#[component]
pub fn Page() -> impl IntoView {
    let state = use_idevice_state();
    let devices = RwSignal::<Vec<ListedDevice>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);

    let on_refresh = move |_| {
        error.set(None);
        devices.set(Vec::new());
        #[cfg(target_arch = "wasm32")]
        {
            let connected = state.device.get_untracked();
            wasm_bindgen_futures::spawn_local(async move {
                match list_devices(connected).await {
                    Ok(list) => devices.set(list),
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        error.set(Some(e));
                    }
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = state;
        }
    };

    view! {
        <Title text="idevice_id - idevice tools" />
        <div class="space-y-3">
            <h1 class="text-xl font-bold dark:text-stone-100">"idevice_id"</h1>
            <p class="text-sm text-stone-700 dark:text-stone-300">
                "Lists Apple devices Chrome has remembered for this origin. The iOS UDID is only knowable for the device currently claimed by this tab."
            </p>
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600"
                on:click=on_refresh
            >
                "Refresh"
            </button>
            <CurrentDevice />
            <Show when=move || error.with(|e| e.is_some())>
                <div class="rounded bg-red-100 p-2 text-sm text-red-700 dark:bg-red-900 dark:text-red-200">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>
            <Show
                when=move || devices.with(|d| !d.is_empty())
                fallback=|| {
                    view! {
                        <p class="text-sm italic text-stone-500 dark:text-stone-400">
                            "Click Refresh to enumerate."
                        </p>
                    }
                }
            >
                <table class="w-full border-collapse text-sm">
                    <thead>
                        <tr class="border-b border-stone-300 text-left dark:border-stone-600">
                            <th class="py-1 pr-3 font-semibold dark:text-stone-200">"UDID"</th>
                            <th class="py-1 pr-3 font-semibold dark:text-stone-200">
                                "WebUSB Serial"
                            </th>
                            <th class="py-1 pr-3 font-semibold dark:text-stone-200">"VID:PID"</th>
                            <th class="py-1 pr-3 font-semibold dark:text-stone-200">"Status"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || {
                            devices
                                .get()
                                .into_iter()
                                .map(|d| {
                                    let udid_cell = d
                                        .udid
                                        .clone()
                                        .unwrap_or_else(|| "—".to_string());
                                    let status_cell = if d.connected {
                                        "Connected".to_string()
                                    } else {
                                        "Permitted".to_string()
                                    };
                                    view! {
                                        <tr class="border-b border-stone-200 dark:border-stone-700">
                                            <td class="py-1 pr-3 font-mono dark:text-stone-100">
                                                {udid_cell}
                                            </td>
                                            <td class="py-1 pr-3 font-mono dark:text-stone-100">
                                                {d.webusb_serial.clone()}
                                            </td>
                                            <td class="py-1 pr-3 font-mono dark:text-stone-100">
                                                {format!("{:04x}:{:04x}", d.vid, d.pid)}
                                            </td>
                                            <td class="py-1 pr-3 dark:text-stone-100">{status_cell}</td>
                                        </tr>
                                    }
                                })
                                .collect_view()
                        }}
                    </tbody>
                </table>
            </Show>
        </div>
    }
}

#[component]
fn CurrentDevice() -> impl IntoView {
    let state = use_idevice_state();
    view! {
        <Show when=move || {
            state.device.with(|d| d.is_some())
        }>
            {move || {
                let d: DeviceMeta = state.device.get().unwrap();
                view! {
                    <div class="rounded border border-blue-300 bg-blue-50 p-2 text-sm dark:border-blue-700 dark:bg-blue-900/30 dark:text-stone-100">
                        <div class="font-semibold">"Currently connected"</div>
                        <div class="font-mono text-xs">"UDID: " {d.serial}</div>
                        <div class="font-mono text-xs">
                            "WebUSB serial: " {d.webusb_serial} " ("
                            {format!("{:04x}:{:04x}", d.vid, d.pid)} ")"
                        </div>
                    </div>
                }
            }}
        </Show>
    }
}

#[cfg(target_arch = "wasm32")]
async fn list_devices(connected: Option<DeviceMeta>) -> Result<Vec<ListedDevice>, String> {
    use netmuxd::usb::apple;
    let infos: Vec<_> = nusb::list_devices()
        .await
        .map_err(|e| format!("list_devices: {e}"))?
        .filter(apple::is_apple_mux)
        .collect();
    Ok(infos
        .into_iter()
        .map(|info| {
            let webusb_serial = info
                .serial_number()
                .map(|s| {
                    s.trim_matches(|c: char| c == '\0' || c.is_whitespace())
                        .to_string()
                })
                .unwrap_or_default();
            let (udid, is_connected) = match connected.as_ref() {
                Some(d) if d.webusb_serial == webusb_serial => (Some(d.serial.clone()), true),
                _ => (None, false),
            };
            ListedDevice {
                webusb_serial,
                udid,
                vid: info.vendor_id(),
                pid: info.product_id(),
                connected: is_connected,
            }
        })
        .collect())
}
