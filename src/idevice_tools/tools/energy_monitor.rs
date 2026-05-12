// Jackson Coxson
// energy_monitor - sample per-PID energy consumption via Instruments.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
struct Sample {
    pid: u32,
    timestamp: i64,
    total: f64,
    cpu: f64,
    gpu: f64,
    networking: f64,
    display: f64,
}

#[component]
pub fn Page() -> impl IntoView {
    let state = use_idevice_state();
    let pid_input = RwSignal::<String>::new(String::new());
    let samples = RwSignal::<Vec<Sample>>::new(Vec::new());
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
            let pids: Vec<u32> = pid_input
                .get_untracked()
                .split(',')
                .filter_map(|s| s.trim().parse::<u32>().ok())
                .collect();
            if pids.is_empty() {
                error.set(Some("Enter at least one PID.".to_string()));
                return;
            }
            error.set(None);
            running.set(true);
            #[cfg(target_arch = "wasm32")]
            {
                *stop_flag.borrow_mut() = false;
                let stop_flag = stop_flag.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(e) = run_stream(state, pids, samples, stop_flag).await {
                        state.push_log(format!("ERROR: {e}"));
                        error.set(Some(e));
                    }
                    running.set(false);
                });
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = (state, pids, samples);
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

    let on_clear = move |_| samples.set(Vec::new());

    view! {
        <Title text="energy_monitor - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"energy_monitor"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Sample energy attributes for one or more PIDs at 1s intervals."
                </p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
                <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                    "PIDs (comma-separated):"
                    <input
                        type="text"
                        class="w-48 rounded border border-stone-300 bg-white px-2 py-1 text-sm dark:border-stone-600 dark:bg-stone-800"
                        placeholder="123,456"
                        prop:value=move || pid_input.get()
                        on:input=move |ev| pid_input.set(leptos::prelude::event_target_value(&ev))
                        disabled=move || running.get()
                    />
                </label>
                <button
                    class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                    on:click=on_start
                    disabled=move || running.get()
                >
                    {move || if running.get() { "Sampling..." } else { "Start" }}
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
                    {move || format!("{} samples", samples.with(Vec::len))}
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
                            <th class="px-2 py-1 text-left">"PID"</th>
                            <th class="px-2 py-1 text-left">"t (s)"</th>
                            <th class="px-2 py-1 text-right">"Total"</th>
                            <th class="px-2 py-1 text-right">"CPU"</th>
                            <th class="px-2 py-1 text-right">"GPU"</th>
                            <th class="px-2 py-1 text-right">"Net"</th>
                            <th class="px-2 py-1 text-right">"Disp"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || {
                            samples
                                .with(|rs| {
                                    rs.iter()
                                        .map(|s| {
                                            view! {
                                                <tr class="border-b border-stone-100 dark:border-stone-800 dark:text-stone-100 font-mono">
                                                    <td class="px-2 py-1">{s.pid}</td>
                                                    <td class="px-2 py-1">{s.timestamp}</td>
                                                    <td class="px-2 py-1 text-right">
                                                        {format!("{:.3}", s.total)}
                                                    </td>
                                                    <td class="px-2 py-1 text-right">
                                                        {format!("{:.3}", s.cpu)}
                                                    </td>
                                                    <td class="px-2 py-1 text-right">
                                                        {format!("{:.3}", s.gpu)}
                                                    </td>
                                                    <td class="px-2 py-1 text-right">
                                                        {format!("{:.3}", s.networking)}
                                                    </td>
                                                    <td class="px-2 py-1 text-right">
                                                        {format!("{:.3}", s.display)}
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
    pids: Vec<u32>,
    rows: RwSignal<Vec<Sample>>,
    stop_flag: Rc<RefCell<bool>>,
) -> Result<(), String> {
    use idevice::dvt::{
        energy_monitor::{EnergyMonitorClient, EnergySample},
        remote_server::RemoteServerClient,
    };

    let (mut adapter, mut handshake) = crate::idevice_tools::transport::open_rsd(&state).await?;
    use idevice::RsdService;
    let mut rs: RemoteServerClient<Box<dyn idevice::ReadWrite>> =
        RemoteServerClient::connect_rsd(&mut adapter, &mut handshake)
            .await
            .map_err(|e| format!("RemoteServerClient::connect_rsd: {e:?}"))?;
    let mut client = EnergyMonitorClient::new(&mut rs)
        .await
        .map_err(|e| format!("EnergyMonitorClient::new: {e:?}"))?;

    let _ = client.stop_sampling(&pids).await;
    client
        .start_sampling(&pids)
        .await
        .map_err(|e| format!("start_sampling: {e:?}"))?;

    while !*stop_flag.borrow() {
        crate::idevice_tools::transport::sleep_ms(1000).await;
        let raw = client
            .sample_attributes(&pids)
            .await
            .map_err(|e| format!("sample_attributes: {e:?}"))?;
        match EnergySample::from_bytes(&raw) {
            Ok(samples) => {
                if !samples.is_empty() {
                    rows.update(|v| {
                        for s in samples {
                            v.push(Sample {
                                pid: s.pid,
                                timestamp: s.timestamp,
                                total: s.total_energy,
                                cpu: s.cpu_energy,
                                gpu: s.gpu_energy,
                                networking: s.networking_energy,
                                display: s.display_energy,
                            });
                        }
                        if v.len() > MAX_ROWS {
                            let drop = v.len() - MAX_ROWS;
                            v.drain(0..drop);
                        }
                    });
                }
            }
            Err(e) => state.push_log(format!("parse: {e:?}")),
        }
    }
    let _ = client.stop_sampling(&pids).await;
    Ok(())
}
