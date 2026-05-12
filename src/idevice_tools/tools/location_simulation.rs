// Jackson Coxson
// location_simulation - spoof the device's GPS.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[component]
pub fn Page() -> impl IntoView {
    let state = use_idevice_state();
    let lat = RwSignal::<String>::new(String::new());
    let lon = RwSignal::<String>::new(String::new());
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_set = move |_| {
        let lat_s = lat.get_untracked();
        let lon_s = lon.get_untracked();
        let Ok(lat_f) = lat_s.parse::<f64>() else {
            error.set(Some(format!("Latitude not a number: {lat_s:?}")));
            return;
        };
        let Ok(lon_f) = lon_s.parse::<f64>() else {
            error.set(Some(format!("Longitude not a number: {lon_s:?}")));
            return;
        };
        error.set(None);
        status.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_set(state, lat_f, lon_f).await {
                    Ok(()) => status.set(Some(format!("Set location to {lat_f}, {lon_f}."))),
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        error.set(Some(e));
                    }
                }
                busy.set(false);
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (state, lat_f, lon_f);
            busy.set(false);
        }
    };

    let on_clear = move |_| {
        error.set(None);
        status.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_clear(state).await {
                    Ok(()) => status.set(Some("Cleared.".to_string())),
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        error.set(Some(e));
                    }
                }
                busy.set(false);
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = state;
            busy.set(false);
        }
    };

    view! {
        <Title text="location_simulation - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"location_simulation"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Override the device's reported GPS coordinates."
                </p>
            </div>
            <fieldset class="space-y-2 rounded border border-stone-200 p-3 dark:border-stone-700">
                <legend class="px-1 text-xs font-bold uppercase tracking-wide text-stone-500 dark:text-stone-400">
                    "Coordinates"
                </legend>
                <div class="grid grid-cols-1 gap-2 sm:grid-cols-2">
                    <label class="flex flex-col gap-1 text-sm dark:text-stone-200">
                        <span class="text-xs text-stone-500 dark:text-stone-400">"Latitude"</span>
                        <input
                            type="text"
                            inputmode="decimal"
                            class="rounded border border-stone-300 bg-white px-2 py-1 text-sm dark:border-stone-600 dark:bg-stone-800"
                            placeholder="37.3349"
                            prop:value=move || lat.get()
                            on:input=move |ev| lat.set(leptos::prelude::event_target_value(&ev))
                        />
                    </label>
                    <label class="flex flex-col gap-1 text-sm dark:text-stone-200">
                        <span class="text-xs text-stone-500 dark:text-stone-400">"Longitude"</span>
                        <input
                            type="text"
                            inputmode="decimal"
                            class="rounded border border-stone-300 bg-white px-2 py-1 text-sm dark:border-stone-600 dark:bg-stone-800"
                            placeholder="-122.0090"
                            prop:value=move || lon.get()
                            on:input=move |ev| lon.set(leptos::prelude::event_target_value(&ev))
                        />
                    </label>
                </div>
                <div class="flex flex-wrap gap-2">
                    <button
                        class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                        on:click=on_set
                        disabled=move || busy.get()
                    >
                        {move || if busy.get() { "Setting..." } else { "Set" }}
                    </button>
                    <button
                        class="rounded border border-stone-400 px-3 py-1.5 text-sm hover:bg-stone-100 disabled:opacity-50 dark:border-stone-500 dark:text-stone-100 dark:hover:bg-stone-700"
                        on:click=on_clear
                        disabled=move || busy.get()
                    >
                        "Clear"
                    </button>
                </div>
                <Show when=move || error.with(|e| e.is_some())>
                    <div class="rounded bg-red-100 p-2 text-sm text-red-700 dark:bg-red-900 dark:text-red-200">
                        {move || error.get().unwrap_or_default()}
                    </div>
                </Show>
                <Show when=move || status.with(|s| s.is_some())>
                    <div class="rounded bg-green-100 p-2 text-sm text-green-800 dark:bg-green-900 dark:text-green-200">
                        {move || status.get().unwrap_or_default()}
                    </div>
                </Show>
            </fieldset>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
async fn run_set(state: IdeviceState, lat: f64, lon: f64) -> Result<(), String> {
    use idevice::{simulate_location::LocationSimulationService, IdeviceService};
    match crate::idevice_tools::transport::open_rsd(&state).await {
        Ok((mut adapter, mut handshake)) => {
            use idevice::dvt::location_simulation::LocationSimulationClient;
            use idevice::dvt::remote_server::RemoteServerClient;
            use idevice::RsdService;

            let mut rs: RemoteServerClient<Box<dyn idevice::ReadWrite>> =
                RemoteServerClient::connect_rsd(&mut adapter, &mut handshake)
                    .await
                    .map_err(|e| format!("RemoteServerClient::connect_rsd: {e:?}"))?;
            rs.read_message(0)
                .await
                .map_err(|e| format!("read_message: {e:?}"))?;
            let mut client = LocationSimulationClient::new(&mut rs)
                .await
                .map_err(|e| format!("LocationSimulationClient::new: {e:?}"))?;
            client
                .set(lat, lon)
                .await
                .map_err(|e| format!("set: {e:?}"))
        }
        Err(rsd_err) => {
            state.push_log(format!(
                "CoreDeviceProxy unavailable, falling back: {rsd_err}"
            ));
            let provider = crate::idevice_tools::transport::build_provider(&state)?;
            let mut svc = LocationSimulationService::connect(&provider)
                .await
                .map_err(|e| format!("LocationSimulationService::connect: {e:?}"))?;
            svc.set(&format!("{lat}"), &format!("{lon}"))
                .await
                .map_err(|e| format!("set: {e:?}"))
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn run_clear(state: IdeviceState) -> Result<(), String> {
    use idevice::{simulate_location::LocationSimulationService, IdeviceService};
    match crate::idevice_tools::transport::open_rsd(&state).await {
        Ok((mut adapter, mut handshake)) => {
            use idevice::dvt::location_simulation::LocationSimulationClient;
            use idevice::dvt::remote_server::RemoteServerClient;
            use idevice::RsdService;

            let mut rs: RemoteServerClient<Box<dyn idevice::ReadWrite>> =
                RemoteServerClient::connect_rsd(&mut adapter, &mut handshake)
                    .await
                    .map_err(|e| format!("RemoteServerClient::connect_rsd: {e:?}"))?;
            rs.read_message(0)
                .await
                .map_err(|e| format!("read_message: {e:?}"))?;
            let mut client = LocationSimulationClient::new(&mut rs)
                .await
                .map_err(|e| format!("LocationSimulationClient::new: {e:?}"))?;
            client.clear().await.map_err(|e| format!("clear: {e:?}"))
        }
        Err(rsd_err) => {
            state.push_log(format!(
                "CoreDeviceProxy unavailable, falling back: {rsd_err}"
            ));
            let provider = crate::idevice_tools::transport::build_provider(&state)?;
            let mut svc = LocationSimulationService::connect(&provider)
                .await
                .map_err(|e| format!("LocationSimulationService::connect: {e:?}"))?;
            svc.clear().await.map_err(|e| format!("clear: {e:?}"))
        }
    }
}
