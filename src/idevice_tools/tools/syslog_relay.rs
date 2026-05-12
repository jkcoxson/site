// Jackson Coxson
// syslog_relay - streaming raw syslog lines.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};

#[component]
pub fn Page() -> impl IntoView {
    let state = use_idevice_state();
    let lines = RwSignal::<Vec<String>>::new(Vec::new());
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
                    if let Err(e) = run_stream(state, lines, stop_flag).await {
                        state.push_log(format!("ERROR: {e}"));
                        error.set(Some(e));
                    }
                    running.set(false);
                });
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = (state, lines);
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

    let on_clear = move |_| lines.set(Vec::new());

    view! {
        <Title text="syslog_relay - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"syslog_relay"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Stream raw syslog lines from the device."
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
                    {move || format!("{} lines", lines.with(Vec::len))}
                </span>
            </div>
            <Show when=move || error.with(|e| e.is_some())>
                <div class="rounded bg-red-100 p-2 text-sm text-red-700 dark:bg-red-900 dark:text-red-200">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>
            <pre class="max-h-[60vh] overflow-auto rounded border border-stone-200 bg-stone-50 p-3 text-xs leading-snug dark:border-stone-700 dark:bg-stone-900 dark:text-stone-200">
                {move || lines.with(|v| v.join("\n"))}
            </pre>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
const MAX_LINES: usize = 5000;

#[cfg(target_arch = "wasm32")]
async fn run_stream(
    state: IdeviceState,
    lines: RwSignal<Vec<String>>,
    stop_flag: Rc<RefCell<bool>>,
) -> Result<(), String> {
    use idevice::{IdeviceService, syslog_relay::SyslogRelayClient};
    let provider = crate::idevice_tools::transport::build_provider(&state)?;
    let mut client = SyslogRelayClient::connect(&provider)
        .await
        .map_err(|e| format!("SyslogRelayClient::connect: {e:?}"))?;
    while !*stop_flag.borrow() {
        match client.next().await {
            Ok(line) => {
                lines.update(|v| {
                    v.push(line);
                    if v.len() > MAX_LINES {
                        let drop = v.len() - MAX_LINES;
                        v.drain(0..drop);
                    }
                });
            }
            Err(e) => return Err(format!("syslog next: {e:?}")),
        }
    }
    Ok(())
}
