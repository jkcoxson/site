// Jackson Coxson
// screenshot - capture a device screenshot and display / download it.
// Prefers DVT over RemoteXPC (iOS 17+); falls back to screenshotr if that
// fails. Both paths require the Developer Disk Image to be mounted.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[component]
pub fn Page() -> impl IntoView {
    let state = use_idevice_state();
    let image_url = RwSignal::<Option<String>>::new(None);
    let image_size = RwSignal::<usize>::new(0);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        error.set(None);
        image_url.set(None);
        image_size.set(0);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_capture(state).await {
                    Ok((url, size)) => {
                        image_url.set(Some(url));
                        image_size.set(size);
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
        <Title text="screenshot - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"screenshot"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Capture the current screen. Requires the Developer Disk Image to be mounted."
                </p>
            </div>
            <div class="flex flex-wrap gap-3">
                <button
                    class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                    on:click=on_run
                    disabled=move || busy.get()
                >
                    {move || if busy.get() { "Capturing..." } else { "Take screenshot" }}
                </button>
                <Show when=move || image_url.with(|u| u.is_some())>
                    <a
                        class="self-center text-sm text-blue-600 hover:underline dark:text-blue-300"
                        href=move || image_url.get().unwrap_or_default()
                        download="screenshot.png"
                    >
                        {move || format!("Download ({} bytes)", image_size.get())}
                    </a>
                </Show>
            </div>
            <Show when=move || error.with(|e| e.is_some())>
                <div class="rounded bg-red-100 p-2 text-sm text-red-700 dark:bg-red-900 dark:text-red-200">
                    {move || error.get().unwrap_or_default()}
                </div>
            </Show>
            <Show when=move || image_url.with(|u| u.is_some())>
                <img
                    class="max-h-[70vh] rounded border border-stone-200 bg-white dark:border-stone-700"
                    src=move || image_url.get().unwrap_or_default()
                    alt="device screenshot"
                />
            </Show>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
async fn run_capture(state: IdeviceState) -> Result<(String, usize), String> {
    let bytes = capture_bytes(&state).await?;
    let size = bytes.len();
    let url = bytes_to_blob_url(&bytes, "image/png")?;
    Ok((url, size))
}

#[cfg(target_arch = "wasm32")]
async fn capture_bytes(state: &IdeviceState) -> Result<Vec<u8>, String> {
    use idevice::{screenshotr::ScreenshotService, IdeviceService};

    // Prefer DVT over RemoteXPC (iOS 17+). If CoreDeviceProxy fails, fall
    // back to the lockdown-hosted screenshotr
    match crate::idevice_tools::transport::open_rsd(state).await {
        Ok((mut adapter, mut handshake)) => {
            use idevice::dvt::remote_server::RemoteServerClient;
            use idevice::dvt::screenshot::ScreenshotClient;
            use idevice::RsdService;

            let mut rs: RemoteServerClient<Box<dyn idevice::ReadWrite>> =
                RemoteServerClient::connect_rsd(&mut adapter, &mut handshake)
                    .await
                    .map_err(|e| format!("RemoteServerClient::connect_rsd: {e:?}"))?;
            rs.read_message(0)
                .await
                .map_err(|e| format!("read_message: {e:?}"))?;
            let mut client = ScreenshotClient::new(&mut rs)
                .await
                .map_err(|e| format!("ScreenshotClient::new: {e:?}"))?;
            client
                .take_screenshot()
                .await
                .map_err(|e| format!("dvt screenshot: {e:?}"))
        }
        Err(rsd_err) => {
            state.push_log(format!(
                "CoreDeviceProxy unavailable, falling back: {rsd_err}"
            ));
            let provider = crate::idevice_tools::transport::build_provider(state)?;
            let mut client = ScreenshotService::connect(&provider)
                .await
                .map_err(|e| format!("ScreenshotService::connect: {e:?}"))?;
            client
                .take_screenshot()
                .await
                .map_err(|e| format!("screenshotr: {e:?}"))
        }
    }
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
