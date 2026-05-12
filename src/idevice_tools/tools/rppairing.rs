// Jackson Coxson
// rppairing - inspect the RemoteXPC tunnel (services list) and produce an
// RPPairing file via the untrusted tunnel service.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[derive(Clone)]
struct ServiceRow {
    name: String,
    port: u16,
    xpc: bool,
    version: String,
}

#[component]
pub fn Page() -> impl IntoView {
    view! {
        <Title text="rppairing - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"rppairing"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "iOS 17+ remote pairing. Inspect the RemoteXPC tunnel or generate an RPPairing file."
                </p>
            </div>
            <TunnelSection />
            <PairSection />
        </div>
    }
}

#[component]
fn TunnelSection() -> impl IntoView {
    let state = use_idevice_state();
    let services = RwSignal::<Option<Vec<ServiceRow>>>::new(None);
    let uuid = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);
    let error = RwSignal::<Option<String>>::new(None);

    let on_run = move |_| {
        error.set(None);
        services.set(None);
        uuid.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_tunnel(state).await {
                    Ok((u, list)) => {
                        uuid.set(Some(u));
                        services.set(Some(list));
                    }
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
        <Section title="Tunnel inspect">
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get()
            >
                {move || if busy.get() { "Connecting..." } else { "List RSD services" }}
            </button>
            <ErrorBlock error />
            <Show when=move || uuid.with(|u| u.is_some())>
                <p class="font-mono text-xs text-stone-500 dark:text-stone-400">
                    {move || format!("Device UUID: {}", uuid.get().unwrap_or_default())}
                </p>
            </Show>
            <Show when=move || services.with(|s| s.is_some())>
                <div class="max-h-[50vh] overflow-auto rounded border border-stone-200 dark:border-stone-700">
                    <table class="w-full text-xs">
                        <thead class="sticky top-0 bg-stone-100 dark:bg-stone-800 dark:text-stone-200">
                            <tr>
                                <th class="px-2 py-1 text-left">"Name"</th>
                                <th class="px-2 py-1 text-right">"Port"</th>
                                <th class="px-2 py-1 text-left">"XPC"</th>
                                <th class="px-2 py-1 text-left">"Version"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                services
                                    .get()
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|s| {
                                        view! {
                                            <tr class="border-b border-stone-100 font-mono dark:border-stone-800 dark:text-stone-100">
                                                <td class="px-2 py-1 truncate">{s.name}</td>
                                                <td class="px-2 py-1 text-right">{s.port}</td>
                                                <td class="px-2 py-1">
                                                    {if s.xpc { "yes" } else { "no" }}
                                                </td>
                                                <td class="px-2 py-1">{s.version}</td>
                                            </tr>
                                        }
                                    })
                                    .collect_view()
                            }}
                        </tbody>
                    </table>
                </div>
            </Show>
        </Section>
    }
}

#[component]
fn PairSection() -> impl IntoView {
    let state = use_idevice_state();
    let hostname = RwSignal::<String>::new(String::new());
    let download_url = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);
    let error = RwSignal::<Option<String>>::new(None);
    let status = RwSignal::<Option<String>>::new(None);

    let on_run = move |_| {
        let h = hostname.get_untracked();
        if h.is_empty() {
            return;
        }
        error.set(None);
        status.set(None);
        download_url.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_pair(state, h.clone()).await {
                    Ok(url) => {
                        download_url.set(Some(url));
                        status.set(Some("Paired. Click Download to save the .plist.".to_string()));
                    }
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
            let _ = (state, h);
            busy.set(false);
        }
    };

    view! {
        <Section title="Create pairing file">
            <p class="text-xs text-stone-500 dark:text-stone-400">
                "Trust on the device when prompted. If a PIN is required, a browser dialog will ask for it."
            </p>
            <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                "Hostname:"
                <input
                    type="text"
                    class="flex-1 rounded border border-stone-300 bg-white px-2 py-1 text-sm dark:border-stone-600 dark:bg-stone-800"
                    placeholder="my-mac"
                    prop:value=move || hostname.get()
                    on:input=move |ev| hostname.set(leptos::prelude::event_target_value(&ev))
                />
            </label>
            <div class="flex flex-wrap gap-2">
                <button
                    class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                    on:click=on_run
                    disabled=move || busy.get() || hostname.with(|h| h.is_empty())
                >
                    {move || if busy.get() { "Pairing..." } else { "Pair" }}
                </button>
                <Show when=move || download_url.with(|u| u.is_some())>
                    <a
                        class="self-center text-sm text-blue-600 hover:underline dark:text-blue-300"
                        href=move || download_url.get().unwrap_or_default()
                        download="rppairing.plist"
                    >
                        "Download pairing file"
                    </a>
                </Show>
            </div>
            <ErrorBlock error />
            <Show when=move || status.with(|s| s.is_some())>
                <div class="rounded bg-green-100 p-2 text-sm text-green-800 dark:bg-green-900 dark:text-green-200">
                    {move || status.get().unwrap_or_default()}
                </div>
            </Show>
        </Section>
    }
}

#[component]
fn Section(title: &'static str, children: Children) -> impl IntoView {
    view! {
        <fieldset class="space-y-2 rounded border border-stone-200 p-3 dark:border-stone-700">
            <legend class="px-1 text-xs font-bold uppercase tracking-wide text-stone-500 dark:text-stone-400">
                {title}
            </legend>
            {children()}
        </fieldset>
    }
}

#[component]
fn ErrorBlock(error: RwSignal<Option<String>>) -> impl IntoView {
    view! {
        <Show when=move || error.with(|e| e.is_some())>
            <div class="rounded bg-red-100 p-2 text-sm text-red-700 dark:bg-red-900 dark:text-red-200">
                {move || error.get().unwrap_or_default()}
            </div>
        </Show>
    }
}

// --- wasm-only backends ---------------------------------------------------

#[cfg(target_arch = "wasm32")]
async fn run_tunnel(state: IdeviceState) -> Result<(String, Vec<ServiceRow>), String> {
    let (_adapter, handshake) = crate::idevice_tools::transport::open_rsd(&state).await?;
    let mut rows: Vec<ServiceRow> = handshake
        .services
        .iter()
        .map(|(name, svc)| ServiceRow {
            name: name.clone(),
            port: svc.port,
            xpc: svc.uses_remote_xpc,
            version: svc
                .service_version
                .map(|v| v.to_string())
                .unwrap_or_default(),
        })
        .collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    Ok((handshake.uuid.clone(), rows))
}

#[cfg(target_arch = "wasm32")]
async fn run_pair(state: IdeviceState, hostname: String) -> Result<String, String> {
    use idevice::{
        RemoteXpcClient,
        remote_pairing::{RemotePairingClient, RpPairingFile},
    };

    let (mut adapter, handshake) = crate::idevice_tools::transport::open_rsd(&state).await?;
    let ts = handshake
        .services
        .get("com.apple.internal.dt.coredevice.untrusted.tunnelservice")
        .ok_or_else(|| "Untrusted tunnel service not in RSD listing".to_string())?;
    state.push_log(format!("Connecting to untrusted tunnel service on port {}", ts.port));
    let ts_stream = adapter
        .connect(ts.port)
        .await
        .map_err(|e| format!("adapter.connect(tunnel): {e:?}"))?;
    let mut conn = RemoteXpcClient::new(ts_stream)
        .await
        .map_err(|e| format!("RemoteXpcClient::new: {e:?}"))?;
    conn.do_handshake()
        .await
        .map_err(|e| format!("xpc handshake: {e:?}"))?;
    let _ = conn.recv_root().await;

    let mut rpf = RpPairingFile::generate(&hostname);
    let mut rpc = RemotePairingClient::new(conn, &hostname);

    rpc.connect(&mut rpf, prompt_for_pin)
        .await
        .map_err(|e| format!("rppair: {e:?}"))?;

    let bytes = rpf.to_bytes();
    bytes_to_blob_url(&bytes, "application/octet-stream")
}

#[cfg(target_arch = "wasm32")]
async fn prompt_for_pin() -> String {
    web_sys::window()
        .and_then(|w| w.prompt_with_message("Enter the 6-digit pairing PIN displayed on the device:").ok())
        .flatten()
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
fn bytes_to_blob_url(bytes: &[u8], mime: &str) -> Result<String, String> {
    let arr = js_sys::Uint8Array::from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&arr.buffer());
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&parts, &opts)
        .map_err(|e| format!("Blob: {e:?}"))?;
    web_sys::Url::create_object_url_with_blob(&blob).map_err(|e| format!("createObjectURL: {e:?}"))
}
