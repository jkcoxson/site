// Jackson Coxson
// ideviceinfo - dump lockdown values from the connected device.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;

#[component]
pub fn Page() -> impl IntoView {
    let state = use_idevice_state();
    let output = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);
    let start_session = RwSignal::<bool>::new(true);

    let on_run = move |_| {
        error.set(None);
        output.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            let want_session = start_session.get_untracked();
            wasm_bindgen_futures::spawn_local(async move {
                match run(state, want_session).await {
                    Ok(xml) => output.set(Some(xml)),
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
            busy.set(false);
            let _ = state;
            let _ = start_session;
        }
    };

    view! {
        <Title text="ideviceinfo - idevice tools" />
        <div class="space-y-3">
            <h1 class="text-xl font-bold dark:text-stone-100">"ideviceinfo"</h1>
            <p class="text-sm text-stone-700 dark:text-stone-300">
                "Calls lockdown.get_value(None, None) to dump every key the device exposes."
            </p>
            <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                <input
                    type="checkbox"
                    prop:checked=move || start_session.get()
                    on:change=move |ev| {
                        start_session.set(leptos::prelude::event_target_checked(&ev))
                    }
                />
                "Start TLS session (uses pairing file for protected keys)"
            </label>
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get()
            >
                {move || if busy.get() { "Running..." } else { "Run" }}
            </button>
            <Show when=move || error.with(|e| e.is_some())>
                <div class="rounded bg-red-100 p-2 text-sm text-red-700 dark:bg-red-900 dark:text-red-200">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>
            <Show when=move || output.with(|o| o.is_some())>
                <pre class="max-h-[60vh] overflow-auto rounded border border-stone-200 bg-stone-50 p-3 text-xs leading-snug dark:border-stone-700 dark:bg-stone-900 dark:text-stone-200">
                    {move || output.get().unwrap_or_default()}
                </pre>
            </Show>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
async fn run(
    state: crate::idevice_tools::state::IdeviceState,
    start_session: bool,
) -> Result<String, String> {
    use crate::idevice_tools::transport::{load_pairing_file, open_lockdown};

    let mut lockdown = open_lockdown().await?;

    if start_session {
        let pairing = load_pairing_file(&state)?;
        state.push_log("Starting TLS session...");
        lockdown
            .start_session(&pairing)
            .await
            .map_err(|e| format!("start_session: {e:?}"))?;
    } else {
        state.push_log("Session disabled - returning only unprotected keys.");
    }

    state.push_log("Calling lockdown.get_value(None, None)...");
    let value = lockdown
        .get_value(None, None)
        .await
        .map_err(|e| format!("get_value: {e:?}"))?;

    let mut buf = Vec::new();
    plist::to_writer_xml(&mut buf, &value).map_err(|e| format!("plist serialize: {e:?}"))?;
    let xml = String::from_utf8(buf).map_err(|e| format!("utf8: {e:?}"))?;
    state.push_log(format!("Got {} bytes of plist.", xml.len()));
    Ok(xml)
}
