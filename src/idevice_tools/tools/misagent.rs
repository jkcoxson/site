// Jackson Coxson
// misagent - list, install, remove provisioning profiles.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[derive(Clone)]
struct ProfileEntry {
    index: usize,
    bytes: usize,
    download_url: String,
}

#[component]
pub fn Page() -> impl IntoView {
    view! {
        <Title text="misagent - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"misagent"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Manage provisioning profiles installed on the device."
                </p>
            </div>
            <ListSection />
            <InstallSection />
            <RemoveSection />
        </div>
    }
}

#[component]
fn ListSection() -> impl IntoView {
    let state = use_idevice_state();
    let entries = RwSignal::<Option<Vec<ProfileEntry>>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        error.set(None);
        entries.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_list(state).await {
                    Ok(list) => entries.set(Some(list)),
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
        <Section title="Installed profiles">
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get()
            >
                {move || if busy.get() { "Loading..." } else { "List profiles" }}
            </button>
            <ErrorBlock error />
            <Show when=move || entries.with(|e| e.is_some())>
                <div class="rounded border border-stone-200 dark:border-stone-700">
                    {move || {
                        let list = entries.get().unwrap_or_default();
                        if list.is_empty() {
                            view! {
                                <p class="p-2 text-sm italic text-stone-500 dark:text-stone-400">
                                    "(no profiles)"
                                </p>
                            }
                                .into_any()
                        } else {
                            view! {
                                <ul>
                                    {list
                                        .into_iter()
                                        .map(|e| {
                                            let name = format!("profile-{}.mobileprovision", e.index);
                                            view! {
                                                <li class="flex items-center justify-between border-b border-stone-100 px-2 py-1 text-sm last:border-b-0 dark:border-stone-800 dark:text-stone-100">
                                                    <span class="font-mono">
                                                        {format!("#{} ({} bytes)", e.index, e.bytes)}
                                                    </span>
                                                    <a
                                                        class="text-blue-600 hover:underline dark:text-blue-300"
                                                        href=e.download_url.clone()
                                                        download=name
                                                    >
                                                        "Download"
                                                    </a>
                                                </li>
                                            }
                                        })
                                        .collect_view()}
                                </ul>
                            }
                                .into_any()
                        }
                    }}
                </div>
            </Show>
        </Section>
    }
}

#[component]
fn InstallSection() -> impl IntoView {
    let state = use_idevice_state();
    let file_name = RwSignal::<Option<String>>::new(None);
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);
    let input_ref = NodeRef::<leptos::html::Input>::new();

    let on_file_change = move |_ev: leptos::ev::Event| {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let Some(input) = input_ref.get_untracked() else {
                return;
            };
            let el: &web_sys::HtmlInputElement = input.unchecked_ref();
            file_name.set(
                el.files()
                    .and_then(|fl| fl.item(0))
                    .map(|f| f.name()),
            );
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = input_ref;
    };

    let on_run = move |_| {
        error.set(None);
        status.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let file = input_ref.get_untracked().and_then(|input| {
                let el: &web_sys::HtmlInputElement = input.unchecked_ref();
                el.files().and_then(|fl| fl.item(0))
            });
            let Some(file) = file else {
                error.set(Some("Pick a .mobileprovision file first.".to_string()));
                busy.set(false);
                return;
            };
            wasm_bindgen_futures::spawn_local(async move {
                match run_install(state, file).await {
                    Ok(()) => status.set(Some("Profile installed.".to_string())),
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
            let _ = (state, input_ref);
            busy.set(false);
        }
    };

    view! {
        <Section title="Install profile">
            <label class="flex flex-col gap-1 text-sm dark:text-stone-200">
                ".mobileprovision file:"
                <input
                    type="file"
                    accept=".mobileprovision,application/x-apple-aspen-config,application/octet-stream"
                    node_ref=input_ref
                    on:change=on_file_change
                    disabled=move || busy.get()
                    class="text-sm"
                /> <Show when=move || file_name.with(|n| n.is_some())>
                    <span class="font-mono text-xs text-stone-500 dark:text-stone-400">
                        {move || file_name.get().unwrap_or_default()}
                    </span>
                </Show>
            </label>
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get() || file_name.with(|n| n.is_none())
            >
                {move || if busy.get() { "Installing..." } else { "Install" }}
            </button>
            <ErrorBlock error />
            <StatusBlock status />
        </Section>
    }
}

#[component]
fn RemoveSection() -> impl IntoView {
    let state = use_idevice_state();
    let id = RwSignal::<String>::new(String::new());
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        let i = id.get_untracked();
        if i.is_empty() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let confirmed = web_sys::window()
                .and_then(|w| w.confirm_with_message(&format!("Remove profile {i}?")).ok())
                .unwrap_or(false);
            if !confirmed {
                return;
            }
        }
        error.set(None);
        status.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_remove(state, i.clone()).await {
                    Ok(()) => status.set(Some(format!("Removed {i}."))),
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
            let _ = (state, i);
            busy.set(false);
        }
    };

    view! {
        <Section title="Remove profile">
            <p class="text-xs text-stone-500 dark:text-stone-400">
                "Profile ID is the UUID listed inside the .mobileprovision metadata."
            </p>
            <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                "Profile ID:"
                <input
                    type="text"
                    class="flex-1 rounded border border-stone-300 bg-white px-2 py-1 font-mono text-sm dark:border-stone-600 dark:bg-stone-800"
                    prop:value=move || id.get()
                    on:input=move |ev| id.set(leptos::prelude::event_target_value(&ev))
                />
            </label>
            <button
                class="rounded bg-red-600 px-3 py-1.5 text-sm text-white hover:bg-red-700 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get() || id.with(|i| i.is_empty())
            >
                {move || if busy.get() { "Removing..." } else { "Remove" }}
            </button>
            <ErrorBlock error />
            <StatusBlock status />
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

#[component]
fn StatusBlock(status: RwSignal<Option<String>>) -> impl IntoView {
    view! {
        <Show when=move || status.with(|s| s.is_some())>
            <div class="rounded bg-green-100 p-2 text-sm text-green-800 dark:bg-green-900 dark:text-green-200">
                {move || status.get().unwrap_or_default()}
            </div>
        </Show>
    }
}

// --- wasm-only backends ---------------------------------------------------

#[cfg(target_arch = "wasm32")]
async fn open_mis(state: &IdeviceState) -> Result<idevice::services::misagent::MisagentClient, String> {
    use idevice::{IdeviceService, misagent::MisagentClient};
    let provider = crate::idevice_tools::transport::build_provider(state)?;
    MisagentClient::connect(&provider)
        .await
        .map_err(|e| format!("MisagentClient::connect: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
async fn run_list(state: IdeviceState) -> Result<Vec<ProfileEntry>, String> {
    let mut c = open_mis(&state).await?;
    let profiles = c
        .copy_all()
        .await
        .map_err(|e| format!("copy_all: {e:?}"))?;
    let mut out = Vec::with_capacity(profiles.len());
    for (index, bytes) in profiles.into_iter().enumerate() {
        let url = bytes_to_blob_url(&bytes, "application/octet-stream")?;
        out.push(ProfileEntry {
            index,
            bytes: bytes.len(),
            download_url: url,
        });
    }
    Ok(out)
}

#[cfg(target_arch = "wasm32")]
async fn run_install(state: IdeviceState, file: web_sys::File) -> Result<(), String> {
    let bytes = read_file_bytes(&file).await?;
    let mut c = open_mis(&state).await?;
    c.install(bytes)
        .await
        .map_err(|e| format!("install: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
async fn run_remove(state: IdeviceState, id: String) -> Result<(), String> {
    let mut c = open_mis(&state).await?;
    c.remove(&id).await.map_err(|e| format!("remove: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
async fn read_file_bytes(file: &web_sys::File) -> Result<Vec<u8>, String> {
    let promise = file.array_buffer();
    let value = wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| format!("File.arrayBuffer: {e:?}"))?;
    let array = js_sys::Uint8Array::new(&value);
    Ok(array.to_vec())
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
