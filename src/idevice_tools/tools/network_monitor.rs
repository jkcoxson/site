// Jackson Coxson

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
struct Row {
    tag: &'static str,
    body: String,
}

#[component]
pub fn Page() -> impl IntoView {
    let state = use_idevice_state();
    let rows = RwSignal::<Vec<Row>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);
    let running = RwSignal::<bool>::new(false);
    #[cfg(target_arch = "wasm32")]
    let stop_flag: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));

    let on_start = {
        #[cfg(target_arch = "wasm32")]
        let stop_flag = stop_flag.clone();
        move |_| {
            if running.get_untracked() {
                return;
            }
            error.set(None);
            running.set(true);
            #[cfg(target_arch = "wasm32")]
            {
                *stop_flag.borrow_mut() = false;
                let stop_flag = stop_flag.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(e) = run_stream(state, rows, stop_flag).await {
                        state.push_log(format!("ERROR: {e}"));
                        error.set(Some(e));
                    }
                    running.set(false);
                });
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = (state, rows);
                running.set(false);
            }
        }
    };

    let on_stop = {
        #[cfg(target_arch = "wasm32")]
        let stop_flag = stop_flag.clone();
        move |_| {
            #[cfg(target_arch = "wasm32")]
            {
                *stop_flag.borrow_mut() = true;
            }
        }
    };

    let on_clear = move |_| rows.set(Vec::new());

    view! {
        <Title text="network_monitor - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"network_monitor"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Stream interface / connection events from Instruments."
                </p>
            </div>
            <div class="flex flex-wrap gap-2">
                <button
                    class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                    on:click=on_start
                    disabled=move || running.get()
                >
                    {move || if running.get() { "Streaming..." } else { "Start" }}
                </button>
                <button
                    class="rounded border border-stone-400 px-3 py-1.5 text-sm hover:bg-stone-100 disabled:opacity-50 dark:border-stone-500 dark:text-stone-100 dark:hover:bg-stone-700"
                    on:click=on_stop
                    disabled=move || !running.get()
                >
                    "Stop"
                </button>
                <button
                    class="rounded border border-stone-400 px-3 py-1.5 text-sm hover:bg-stone-100 dark:border-stone-500 dark:text-stone-100 dark:hover:bg-stone-700"
                    on:click=on_clear
                >
                    "Clear"
                </button>
                <span class="self-center text-xs text-stone-500 dark:text-stone-400">
                    {move || format!("{} events", rows.with(Vec::len))}
                </span>
            </div>
            <Show when=move || error.with(|e| e.is_some())>
                <div class="rounded bg-red-100 p-2 text-sm text-red-700 dark:bg-red-900 dark:text-red-200">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>
            <div class="max-h-[60vh] overflow-auto rounded border border-stone-200 dark:border-stone-700">
                <table class="w-full text-xs">
                    <thead class="sticky top-0 bg-stone-100 dark:bg-stone-800 dark:text-stone-200">
                        <tr>
                            <th class="w-32 px-2 py-1 text-left">"Type"</th>
                            <th class="px-2 py-1 text-left">"Details"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || {
                            rows.with(|rs| {
                                rs.iter()
                                    .map(|r| {
                                        view! {
                                            <tr class="border-b border-stone-100 align-top dark:border-stone-800 dark:text-stone-100">
                                                <td class="px-2 py-1 font-mono">{r.tag}</td>
                                                <td class="px-2 py-1 font-mono break-words">
                                                    {r.body.clone()}
                                                </td>
                                            </tr>
                                        }
                                    })
                                    .collect_view()
                            })
                        }}
                    </tbody>
                </table>
            </div>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
const MAX_ROWS: usize = 5000;

#[cfg(target_arch = "wasm32")]
async fn run_stream(
    state: IdeviceState,
    rows: RwSignal<Vec<Row>>,
    stop_flag: Rc<RefCell<bool>>,
) -> Result<(), String> {
    use idevice::dvt::remote_server::RemoteServerClient;

    match crate::idevice_tools::transport::open_rsd(&state).await {
        Ok((mut adapter, mut handshake)) => {
            use idevice::RsdService;
            let mut rs: RemoteServerClient<Box<dyn idevice::ReadWrite>> =
                RemoteServerClient::connect_rsd(&mut adapter, &mut handshake)
                    .await
                    .map_err(|e| format!("RemoteServerClient::connect_rsd: {e:?}"))?;
            stream_inner(&mut rs, &rows, &stop_flag).await
        }
        Err(rsd_err) => {
            state.push_log(format!(
                "CoreDeviceProxy unavailable, falling back: {rsd_err}"
            ));
            use idevice::IdeviceService;
            let provider = crate::idevice_tools::transport::build_provider(&state)?;
            let mut rs: RemoteServerClient<Box<dyn idevice::ReadWrite>> =
                RemoteServerClient::connect(&provider)
                    .await
                    .map_err(|e| format!("RemoteServerClient::connect: {e:?}"))?;
            stream_inner(&mut rs, &rows, &stop_flag).await
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn stream_inner(
    rs: &mut idevice::dvt::remote_server::RemoteServerClient<Box<dyn idevice::ReadWrite>>,
    rows: &RwSignal<Vec<Row>>,
    stop_flag: &Rc<RefCell<bool>>,
) -> Result<(), String> {
    use idevice::dvt::network_monitor::{NetworkEvent, NetworkMonitorClient};
    let mut client = NetworkMonitorClient::new(rs)
        .await
        .map_err(|e| format!("NetworkMonitorClient::new: {e:?}"))?;
    client
        .start_monitoring()
        .await
        .map_err(|e| format!("start_monitoring: {e:?}"))?;

    while !*stop_flag.borrow() {
        match client.next_event().await {
            Ok(event) => {
                let row = match event {
                    NetworkEvent::InterfaceDetection(e) => Row {
                        tag: "INTERFACE",
                        body: format!("idx={} name={}", e.interface_index, e.name),
                    },
                    NetworkEvent::ConnectionDetection(e) => {
                        let l = e
                            .local_address
                            .map(|a| format!("{}:{}", a.addr, a.port))
                            .unwrap_or_else(|| "?".into());
                        let r = e
                            .remote_address
                            .map(|a| format!("{}:{}", a.addr, a.port))
                            .unwrap_or_else(|| "?".into());
                        Row {
                            tag: "CONNECT",
                            body: format!(
                                "pid={} {l} -> {r} if={} sn={}",
                                e.pid, e.interface_index, e.serial_number
                            ),
                        }
                    }
                    NetworkEvent::ConnectionUpdate(e) => Row {
                        tag: "UPDATE",
                        body: format!(
                            "sn={} rx_b={} tx_b={} rx_p={} tx_p={}",
                            e.connection_serial, e.rx_bytes, e.tx_bytes, e.rx_packets, e.tx_packets
                        ),
                    },
                    NetworkEvent::Unknown(t) => Row {
                        tag: "UNKNOWN",
                        body: format!("type={t}"),
                    },
                };
                rows.update(|v| {
                    v.push(row);
                    if v.len() > MAX_ROWS {
                        let drop = v.len() - MAX_ROWS;
                        v.drain(0..drop);
                    }
                });
            }
            Err(e) => return Err(format!("next_event: {e:?}")),
        }
    }
    let _ = client.stop_monitoring().await;
    Ok(())
}
