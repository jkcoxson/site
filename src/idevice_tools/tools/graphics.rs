// Jackson Coxson
// graphics - stream FPS / GPU memory samples from Instruments.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

#[derive(Clone)]
struct Row {
    timestamp: u64,
    fps: f64,
    alloc: u64,
    in_use: u64,
    driver: u64,
    gpu: String,
    recoveries: u64,
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
        <Title text="graphics - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"graphics"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Sample GPU memory and Core Animation FPS."
                </p>
            </div>
            <div class="flex flex-wrap gap-2">
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
                    {move || format!("{} samples", rows.with(Vec::len))}
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
                            <th class="px-2 py-1 text-left">"timestamp(µs)"</th>
                            <th class="px-2 py-1 text-right">"fps"</th>
                            <th class="px-2 py-1 text-right">"alloc"</th>
                            <th class="px-2 py-1 text-right">"in use"</th>
                            <th class="px-2 py-1 text-right">"driver"</th>
                            <th class="px-2 py-1 text-left">"gpu"</th>
                            <th class="px-2 py-1 text-right">"recov"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || {
                            rows.with(|rs| {
                                rs.iter()
                                    .map(|r| {
                                        view! {
                                            <tr class="border-b border-stone-100 font-mono dark:border-stone-800 dark:text-stone-100">
                                                <td class="px-2 py-1">{r.timestamp}</td>
                                                <td class="px-2 py-1 text-right">
                                                    {format!("{:.2}", r.fps)}
                                                </td>
                                                <td class="px-2 py-1 text-right">{r.alloc}</td>
                                                <td class="px-2 py-1 text-right">{r.in_use}</td>
                                                <td class="px-2 py-1 text-right">{r.driver}</td>
                                                <td class="px-2 py-1 truncate">{r.gpu.clone()}</td>
                                                <td class="px-2 py-1 text-right">{r.recoveries}</td>
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
            state.push_log(format!("CoreDeviceProxy unavailable, falling back: {rsd_err}"));
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
    use idevice::dvt::graphics::GraphicsClient;
    let mut client = GraphicsClient::new(rs)
        .await
        .map_err(|e| format!("GraphicsClient::new: {e:?}"))?;
    client
        .start_sampling(0.0)
        .await
        .map_err(|e| format!("start_sampling: {e:?}"))?;

    while !*stop_flag.borrow() {
        match client.sample().await {
            Ok(s) => rows.update(|v| {
                v.push(Row {
                    timestamp: s.timestamp,
                    fps: s.fps,
                    alloc: s.alloc_system_memory,
                    in_use: s.in_use_system_memory,
                    driver: s.in_use_system_memory_driver,
                    gpu: s.gpu_bundle_name,
                    recoveries: s.recovery_count,
                });
                if v.len() > MAX_ROWS {
                    let drop = v.len() - MAX_ROWS;
                    v.drain(0..drop);
                }
            }),
            Err(e) => return Err(format!("sample: {e:?}")),
        }
    }
    let _ = client.stop_sampling().await;
    Ok(())
}
