// Jackson Coxson
// process_control - launch an app via Instruments and drop its memory limit.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[component]
pub fn Page() -> impl IntoView {
    let state = use_idevice_state();
    let bundle = RwSignal::<String>::new(String::new());
    let kill_existing = RwSignal::<bool>::new(false);
    let start_suspended = RwSignal::<bool>::new(false);
    let drop_memlimit = RwSignal::<bool>::new(true);
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_launch = move |_| {
        let id = bundle.get_untracked();
        if id.is_empty() {
            return;
        }
        let ks = kill_existing.get_untracked();
        let ss = start_suspended.get_untracked();
        let dml = drop_memlimit.get_untracked();
        error.set(None);
        status.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_launch(state, id, ks, ss, dml).await {
                    Ok(msg) => status.set(Some(msg)),
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
            let _ = (state, id, ks, ss, dml);
            busy.set(false);
        }
    };

    view! {
        <Title text="process_control - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"process_control"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Launch a bundle ID via Instruments. Optionally drops the memory limit so heavy debuggers can attach."
                </p>
            </div>
            <fieldset class="space-y-2 rounded border border-stone-200 p-3 dark:border-stone-700">
                <legend class="px-1 text-xs font-bold uppercase tracking-wide text-stone-500 dark:text-stone-400">
                    "Launch"
                </legend>
                <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                    "Bundle ID:"
                    <input
                        type="text"
                        class="flex-1 rounded border border-stone-300 bg-white px-2 py-1 font-mono text-sm dark:border-stone-600 dark:bg-stone-800"
                        placeholder="com.example.MyApp"
                        prop:value=move || bundle.get()
                        on:input=move |ev| bundle.set(leptos::prelude::event_target_value(&ev))
                    />
                </label>
                <div class="flex flex-wrap gap-4">
                    <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                        <input
                            type="checkbox"
                            prop:checked=move || kill_existing.get()
                            on:change=move |ev| {
                                kill_existing.set(leptos::prelude::event_target_checked(&ev))
                            }
                        />
                        "Kill existing"
                    </label>
                    <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                        <input
                            type="checkbox"
                            prop:checked=move || start_suspended.get()
                            on:change=move |ev| {
                                start_suspended.set(leptos::prelude::event_target_checked(&ev))
                            }
                        />
                        "Start suspended"
                    </label>
                    <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                        <input
                            type="checkbox"
                            prop:checked=move || drop_memlimit.get()
                            on:change=move |ev| {
                                drop_memlimit.set(leptos::prelude::event_target_checked(&ev))
                            }
                        />
                        "Disable memory limit after launch"
                    </label>
                </div>
                <button
                    class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                    on:click=on_launch
                    disabled=move || busy.get() || bundle.with(|b| b.is_empty())
                >
                    {move || if busy.get() { "Launching..." } else { "Launch" }}
                </button>
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
async fn run_launch(
    state: IdeviceState,
    bundle: String,
    kill_existing: bool,
    start_suspended: bool,
    drop_memlimit: bool,
) -> Result<String, String> {
    use idevice::dvt::remote_server::RemoteServerClient;

    match crate::idevice_tools::transport::open_rsd(&state).await {
        Ok((mut adapter, mut handshake)) => {
            use idevice::RsdService;
            let mut rs: RemoteServerClient<Box<dyn idevice::ReadWrite>> =
                RemoteServerClient::connect_rsd(&mut adapter, &mut handshake)
                    .await
                    .map_err(|e| format!("RemoteServerClient::connect_rsd: {e:?}"))?;
            rs.read_message(0)
                .await
                .map_err(|e| format!("read_message: {e:?}"))?;
            launch_inner(&mut rs, bundle, kill_existing, start_suspended, drop_memlimit).await
        }
        Err(rsd_err) => {
            state.push_log(format!("CoreDeviceProxy unavailable, falling back: {rsd_err}"));
            use idevice::IdeviceService;
            let provider = crate::idevice_tools::transport::build_provider(&state)?;
            let mut rs: RemoteServerClient<Box<dyn idevice::ReadWrite>> =
                RemoteServerClient::connect(&provider)
                    .await
                    .map_err(|e| format!("RemoteServerClient::connect: {e:?}"))?;
            launch_inner(&mut rs, bundle, kill_existing, start_suspended, drop_memlimit).await
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn launch_inner(
    rs: &mut idevice::dvt::remote_server::RemoteServerClient<Box<dyn idevice::ReadWrite>>,
    bundle: String,
    kill_existing: bool,
    start_suspended: bool,
    drop_memlimit: bool,
) -> Result<String, String> {
    use idevice::dvt::process_control::ProcessControlClient;
    let mut pc = ProcessControlClient::new(rs)
        .await
        .map_err(|e| format!("ProcessControlClient::new: {e:?}"))?;
    let pid = pc
        .launch_app(bundle.clone(), None, None, start_suspended, kill_existing)
        .await
        .map_err(|e| format!("launch_app: {e:?}"))?;
    let mut msg = format!("Launched {bundle} with PID {pid}.");
    if drop_memlimit {
        match pc.disable_memory_limit(pid).await {
            Ok(()) => msg.push_str(" Memory limit disabled."),
            Err(e) => msg.push_str(&format!(" disable_memory_limit failed: {e:?}")),
        }
    }
    Ok(msg)
}
