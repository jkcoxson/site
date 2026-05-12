// Jackson Coxson
// diagnosticsservice - capture a sysdiagnose tarball over RemoteXPC.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[component]
pub fn Page() -> impl IntoView {
    let state = use_idevice_state();
    let filename = RwSignal::<Option<String>>::new(None);
    let download_url = RwSignal::<Option<String>>::new(None);
    let expected_len = RwSignal::<usize>::new(0);
    let received = RwSignal::<usize>::new(0);
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        error.set(None);
        status.set(None);
        download_url.set(None);
        filename.set(None);
        expected_len.set(0);
        received.set(0);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_capture(state, filename, download_url, expected_len, received).await {
                    Ok(()) => status.set(Some("Sysdiagnose ready - click Download.".to_string())),
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
            let _ = (state, filename, download_url, expected_len, received);
            busy.set(false);
        }
    };

    let progress_pct = move || {
        let total = expected_len.get();
        let got = received.get();
        if total == 0 {
            0.0
        } else {
            (got as f64 / total as f64) * 100.0
        }
    };

    view! {
        <Title text="diagnosticsservice - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"diagnosticsservice"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Capture a full sysdiagnose tarball. This usually takes several minutes."
                </p>
            </div>
            <div class="flex flex-wrap items-center gap-2">
                <button
                    class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                    on:click=on_run
                    disabled=move || busy.get()
                >
                    {move || if busy.get() { "Capturing..." } else { "Capture" }}
                </button>
                <Show when=move || download_url.with(|u| u.is_some())>
                    <a
                        class="text-sm text-blue-600 hover:underline dark:text-blue-300"
                        href=move || download_url.get().unwrap_or_default()
                        download=move || {
                            filename.get().unwrap_or_else(|| "sysdiagnose.tar.gz".into())
                        }
                    >
                        {move || format!("Download {}", filename.get().unwrap_or_default())}
                    </a>
                </Show>
            </div>
            <Show when=move || expected_len.with(|n| *n > 0)>
                <div class="space-y-1">
                    <div class="h-2 w-full overflow-hidden rounded bg-stone-200 dark:bg-stone-700">
                        <div
                            class="h-full bg-blue-500 transition-all"
                            style:width=move || format!("{}%", progress_pct().min(100.0))
                        ></div>
                    </div>
                    <p class="text-xs font-mono text-stone-600 dark:text-stone-400">
                        {move || {
                            format!(
                                "{} / {} bytes ({:.1}%)",
                                received.get(),
                                expected_len.get(),
                                progress_pct(),
                            )
                        }}
                    </p>
                </div>
            </Show>
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
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
async fn run_capture(
    state: IdeviceState,
    filename: RwSignal<Option<String>>,
    download_url: RwSignal<Option<String>>,
    expected_len: RwSignal<usize>,
    received: RwSignal<usize>,
) -> Result<(), String> {
    use ::futures::StreamExt;
    use idevice::{RsdService, core_device::DiagnostisServiceClient};

    let (mut adapter, mut handshake) = crate::idevice_tools::transport::open_rsd(&state).await?;
    let mut dsc = DiagnostisServiceClient::connect_rsd(&mut adapter, &mut handshake)
        .await
        .map_err(|e| format!("DiagnostisServiceClient::connect_rsd: {e:?}"))?;
    let mut res = dsc
        .capture_sysdiagnose(false)
        .await
        .map_err(|e| format!("capture_sysdiagnose: {e:?}"))?;

    filename.set(Some(res.preferred_filename.clone()));
    expected_len.set(res.expected_length);

    let mut buf: Vec<u8> = Vec::with_capacity(res.expected_length);
    while let Some(chunk) = res.stream.next().await {
        let chunk = chunk.map_err(|e| format!("sysdiagnose chunk: {e:?}"))?;
        buf.extend_from_slice(&chunk);
        received.set(buf.len());
    }

    let url = bytes_to_blob_url(&buf, "application/gzip")?;
    download_url.set(Some(url));
    Ok(())
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
