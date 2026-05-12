// Jackson Coxson
// os_trace_relay - structured os_log stream from the device.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug)]
struct LogRow {
    pid: u32,
    timestamp: String,
    level: &'static str,
    image: String,
    subsystem: String,
    category: String,
    message: String,
}

#[component]
pub fn Page() -> impl IntoView {
    let state = use_idevice_state();
    let pid_filter = RwSignal::<String>::new(String::new());
    let rows = RwSignal::<Vec<LogRow>>::new(Vec::new());
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
                let pid = pid_filter
                    .get_untracked()
                    .trim()
                    .parse::<u32>()
                    .ok();
                wasm_bindgen_futures::spawn_local(async move {
                    if let Err(e) = run_stream(state, rows, pid, stop_flag).await {
                        state.push_log(format!("ERROR: {e}"));
                        error.set(Some(e));
                    }
                    running.set(false);
                });
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = (state, rows, pid_filter);
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
        <Title text="os_trace_relay - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"os_trace_relay"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Structured os_log events. Optionally filter by PID."
                </p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
                <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                    "PID (optional):"
                    <input
                        type="text"
                        inputmode="numeric"
                        class="w-24 rounded border border-stone-300 bg-white px-2 py-1 text-sm dark:border-stone-600 dark:bg-stone-800"
                        prop:value=move || pid_filter.get()
                        on:input=move |ev| pid_filter.set(leptos::prelude::event_target_value(&ev))
                        disabled=move || running.get()
                    />
                </label>
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
                    {move || format!("{} entries", rows.with(Vec::len))}
                </span>
            </div>
            <Show when=move || error.with(|e| e.is_some())>
                <div class="rounded bg-red-100 p-2 text-sm text-red-700 dark:bg-red-900 dark:text-red-200">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>
            <div class="max-h-[60vh] overflow-auto rounded border border-stone-200 dark:border-stone-700">
                <table class="w-full table-fixed text-xs">
                    <thead class="sticky top-0 bg-stone-100 dark:bg-stone-800 dark:text-stone-200">
                        <tr>
                            <th class="w-32 px-2 py-1 text-left">"Time"</th>
                            <th class="w-12 px-2 py-1 text-left">"PID"</th>
                            <th class="w-14 px-2 py-1 text-left">"Lvl"</th>
                            <th class="w-32 px-2 py-1 text-left">"Image"</th>
                            <th class="w-48 px-2 py-1 text-left">"Subsystem/Cat"</th>
                            <th class="px-2 py-1 text-left">"Message"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || {
                            rows.with(|rs| {
                                rs.iter()
                                    .map(|r| {
                                        view! {
                                            <tr class="border-b border-stone-100 align-top dark:border-stone-800 dark:text-stone-100">
                                                <td class="px-2 py-1 font-mono">{r.timestamp.clone()}</td>
                                                <td class="px-2 py-1 font-mono">{r.pid}</td>
                                                <td class="px-2 py-1">{r.level}</td>
                                                <td class="px-2 py-1 truncate font-mono">
                                                    {r.image.clone()}
                                                </td>
                                                <td class="px-2 py-1 truncate font-mono">
                                                    {format!("{}/{}", r.subsystem, r.category)}
                                                </td>
                                                <td class="px-2 py-1 break-words">{r.message.clone()}</td>
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
    rows: RwSignal<Vec<LogRow>>,
    pid: Option<u32>,
    stop_flag: Rc<RefCell<bool>>,
) -> Result<(), String> {
    use idevice::{IdeviceService, os_trace_relay::{LogLevel, OsTraceRelayClient}};
    let provider = crate::idevice_tools::transport::build_provider(&state)?;
    let client = OsTraceRelayClient::connect(&provider)
        .await
        .map_err(|e| format!("OsTraceRelayClient::connect: {e:?}"))?;
    let mut receiver = client
        .start_trace(pid)
        .await
        .map_err(|e| format!("start_trace: {e:?}"))?;

    while !*stop_flag.borrow() {
        match receiver.next().await {
            Ok(entry) => {
                let level = match entry.level {
                    LogLevel::Notice => "Notice",
                    LogLevel::Info => "Info",
                    LogLevel::Debug => "Debug",
                    LogLevel::Error => "Error",
                    LogLevel::Fault => "Fault",
                };
                let (subsystem, category) = match entry.label {
                    Some(l) => (l.subsystem, l.category),
                    None => (String::new(), String::new()),
                };
                let row = LogRow {
                    pid: entry.pid,
                    timestamp: entry.timestamp.format("%H:%M:%S%.3f").to_string(),
                    level,
                    image: entry.image_name,
                    subsystem,
                    category,
                    message: entry.message,
                };
                rows.update(|v| {
                    v.push(row);
                    if v.len() > MAX_ROWS {
                        let drop = v.len() - MAX_ROWS;
                        v.drain(0..drop);
                    }
                });
            }
            Err(e) => return Err(format!("os_trace next: {e:?}")),
        }
    }
    Ok(())
}
