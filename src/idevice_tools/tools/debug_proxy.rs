// Jackson Coxson
// debug_proxy - interactive debugserver shell over RemoteXPC.
//
// Holds a single CoreDeviceProxy + Adapter + RSD handshake + DebugProxyClient
// alive for the page lifetime once connected. Commands are sent through a
// channel so the (non-Send) connection lives in a single spawned task.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[cfg(target_arch = "wasm32")]
use std::{cell::RefCell, rc::Rc};
#[cfg(target_arch = "wasm32")]
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

#[cfg(target_arch = "wasm32")]
type Sender = Rc<RefCell<Option<UnboundedSender<String>>>>;

#[component]
pub fn Page() -> impl IntoView {
    let state = use_idevice_state();
    let command = RwSignal::<String>::new(String::new());
    let lines = RwSignal::<Vec<String>>::new(Vec::new());
    let error = RwSignal::<Option<String>>::new(None);
    let connected = RwSignal::<bool>::new(false);
    let connecting = RwSignal::<bool>::new(false);
    #[cfg(target_arch = "wasm32")]
    let sender: Sender = Rc::new(RefCell::new(None));

    let on_connect = {
        #[cfg(target_arch = "wasm32")]
        let sender = sender.clone();
        move |_| {
            if connected.get_untracked() || connecting.get_untracked() {
                return;
            }
            error.set(None);
            connecting.set(true);
            #[cfg(target_arch = "wasm32")]
            {
                let sender = sender.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match run_session(state, lines, connected, sender).await {
                        Ok(()) => {}
                        Err(e) => {
                            state.push_log(format!("ERROR: {e}"));
                            error.set(Some(e));
                        }
                    }
                    connected.set(false);
                    connecting.set(false);
                });
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = (state, lines);
                connecting.set(false);
            }
        }
    };

    let on_send = {
        #[cfg(target_arch = "wasm32")]
        let sender = sender.clone();
        move |_| {
            let cmd = command.get_untracked();
            if cmd.is_empty() {
                return;
            }
            #[cfg(target_arch = "wasm32")]
            {
                let s = sender.borrow();
                if let Some(tx) = s.as_ref() {
                    lines.update(|v| v.push(format!("> {cmd}")));
                    if let Err(e) = tx.send(cmd) {
                        error.set(Some(format!("send: {e}")));
                    } else {
                        command.set(String::new());
                    }
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            let _ = cmd;
        }
    };

    let on_clear = move |_| lines.set(Vec::new());

    view! {
        <Title text="debug_proxy - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"debug_proxy"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Interactive debugserver shell over RemoteXPC. Click connect, then send GDB-style commands."
                </p>
            </div>
            <div class="flex flex-wrap gap-2">
                <button
                    class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                    on:click=on_connect
                    disabled=move || connected.get() || connecting.get()
                >
                    {move || {
                        if connected.get() {
                            "Connected"
                        } else if connecting.get() {
                            "Connecting..."
                        } else {
                            "Connect"
                        }
                    }}
                </button>
                <button
                    class="rounded border border-stone-400 px-3 py-1.5 text-sm hover:bg-stone-100 dark:border-stone-500 dark:text-stone-100 dark:hover:bg-stone-700"
                    on:click=on_clear
                >
                    "Clear log"
                </button>
            </div>
            <Show when=move || error.with(|e| e.is_some())>
                <div class="rounded bg-red-100 p-2 text-sm text-red-700 dark:bg-red-900 dark:text-red-200">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>
            <pre class="max-h-[50vh] overflow-auto rounded border border-stone-200 bg-stone-50 p-3 text-xs leading-snug dark:border-stone-700 dark:bg-stone-900 dark:text-stone-200">
                {move || lines.with(|v| v.join("\n"))}
            </pre>
            <form
                class="flex items-center gap-2"
                on:submit=move |ev| {
                    ev.prevent_default();
                    on_send(ev);
                }
            >
                <input
                    type="text"
                    class="flex-1 rounded border border-stone-300 bg-white px-2 py-1 font-mono text-sm dark:border-stone-600 dark:bg-stone-800"
                    placeholder="qSupported"
                    prop:value=move || command.get()
                    on:input=move |ev| command.set(leptos::prelude::event_target_value(&ev))
                    disabled=move || !connected.get()
                />
                <button
                    class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                    type="submit"
                    disabled=move || !connected.get() || command.with(|c| c.is_empty())
                >
                    "Send"
                </button>
            </form>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
async fn run_session(
    state: IdeviceState,
    lines: RwSignal<Vec<String>>,
    connected: RwSignal<bool>,
    sender_slot: Sender,
) -> Result<(), String> {
    use idevice::{RsdService, debug_proxy::DebugProxyClient};

    let (mut adapter, mut handshake) = crate::idevice_tools::transport::open_rsd(&state).await?;
    let mut dp = DebugProxyClient::connect_rsd(&mut adapter, &mut handshake)
        .await
        .map_err(|e| format!("DebugProxyClient::connect_rsd: {e:?}"))?;

    let (tx, mut rx) = unbounded_channel::<String>();
    *sender_slot.borrow_mut() = Some(tx);
    connected.set(true);

    lines.update(|v| {
        v.push(format!(
            "Connected. {} RSD services advertised.",
            handshake.services.len()
        ))
    });

    while let Some(cmd) = rx.recv().await {
        match dp.send_command(cmd.as_str().into()).await {
            Ok(Some(res)) => lines.update(|v| v.push(res)),
            Ok(None) => lines.update(|v| v.push("(no response)".to_string())),
            Err(e) => {
                lines.update(|v| v.push(format!("ERROR: {e:?}")));
                break;
            }
        }
    }

    *sender_slot.borrow_mut() = None;
    Ok(())
}
