// Jackson Coxson
// mounter - list / lookup / unmount developer disk images.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ImageKind {
    Developer,
    Personalized,
}

#[allow(dead_code)]
impl ImageKind {
    fn as_str(self) -> &'static str {
        match self {
            ImageKind::Developer => "Developer",
            ImageKind::Personalized => "Personalized",
        }
    }

    fn unmount_path(self) -> &'static str {
        match self {
            ImageKind::Developer => "/Developer",
            ImageKind::Personalized => "/System/Developer",
        }
    }
}

#[component]
pub fn Page() -> impl IntoView {
    view! {
        <Title text="mounter - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"mounter"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Inspect and unmount developer disk images. Mounting isn't supported in the browser yet - Apple's DDI download is blocked by CORS."
                </p>
            </div>
            <ListSection />
            <LookupSection />
            <UnmountSection />
        </div>
    }
}

#[component]
fn ListSection() -> impl IntoView {
    let state = use_idevice_state();
    let output = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        error.set(None);
        output.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_list(state).await {
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
            let _ = state;
            busy.set(false);
        }
    };

    view! {
        <Section title="Mounted images">
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get()
            >
                {move || if busy.get() { "Loading..." } else { "Copy devices" }}
            </button>
            <ErrorBlock error />
            <Show when=move || output.with(|o| o.is_some())>
                <pre class="max-h-[50vh] overflow-auto rounded border border-stone-200 bg-stone-50 p-3 text-xs leading-snug dark:border-stone-700 dark:bg-stone-900 dark:text-stone-200">
                    {move || output.get().unwrap_or_default()}
                </pre>
            </Show>
        </Section>
    }
}

#[component]
fn LookupSection() -> impl IntoView {
    let state = use_idevice_state();
    let kind = RwSignal::<ImageKind>::new(ImageKind::Personalized);
    let result = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        error.set(None);
        result.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            let k = kind.get_untracked();
            wasm_bindgen_futures::spawn_local(async move {
                match run_lookup(state, k).await {
                    Ok(s) => result.set(Some(s)),
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
            let _ = (state, kind);
            busy.set(false);
        }
    };

    let radio = move |k: ImageKind, label: &'static str| {
        view! {
            <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                <input
                    type="radio"
                    name="mounter-kind"
                    prop:checked=move || kind.get() == k
                    on:change=move |_| kind.set(k)
                />
                {label}
            </label>
        }
    };

    view! {
        <Section title="Lookup signature">
            <div class="flex flex-wrap items-center gap-4">
                {radio(ImageKind::Developer, "Developer (iOS < 17)")}
                {radio(ImageKind::Personalized, "Personalized (iOS 17+)")}
                <button
                    class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                    on:click=on_run
                    disabled=move || busy.get()
                >
                    {move || if busy.get() { "Looking up..." } else { "Lookup" }}
                </button>
            </div>
            <ErrorBlock error />
            <Show when=move || result.with(|r| r.is_some())>
                <pre class="max-h-[30vh] overflow-auto rounded border border-stone-200 bg-stone-50 p-3 text-xs leading-snug font-mono dark:border-stone-700 dark:bg-stone-900 dark:text-stone-200">
                    {move || result.get().unwrap_or_default()}
                </pre>
            </Show>
        </Section>
    }
}

#[component]
fn UnmountSection() -> impl IntoView {
    let state = use_idevice_state();
    let kind = RwSignal::<ImageKind>::new(ImageKind::Personalized);
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        let k = kind.get_untracked();
        #[cfg(target_arch = "wasm32")]
        {
            let confirmed = web_sys::window()
                .and_then(|w| {
                    w.confirm_with_message(&format!(
                        "Unmount {} at {}?",
                        k.as_str(),
                        k.unmount_path()
                    ))
                    .ok()
                })
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
                match run_unmount(state, k).await {
                    Ok(()) => status.set(Some(format!("Unmounted {}.", k.unmount_path()))),
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
            let _ = (state, k);
            busy.set(false);
        }
    };

    let radio = move |k: ImageKind, label: &'static str| {
        view! {
            <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                <input
                    type="radio"
                    name="mounter-unmount-kind"
                    prop:checked=move || kind.get() == k
                    on:change=move |_| kind.set(k)
                />
                {label}
            </label>
        }
    };

    view! {
        <Section title="Unmount">
            <div class="flex flex-wrap items-center gap-4">
                {radio(ImageKind::Developer, "Developer (/Developer)")}
                {radio(ImageKind::Personalized, "Personalized (/System/Developer)")}
                <button
                    class="rounded bg-red-600 px-3 py-1.5 text-sm text-white hover:bg-red-700 disabled:opacity-50"
                    on:click=on_run
                    disabled=move || busy.get()
                >
                    {move || if busy.get() { "Unmounting..." } else { "Unmount" }}
                </button>
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
async fn open_mounter(
    state: &IdeviceState,
) -> Result<idevice::services::mobile_image_mounter::ImageMounter, String> {
    use idevice::{mobile_image_mounter::ImageMounter, IdeviceService};
    let provider = crate::idevice_tools::transport::build_provider(state)?;
    ImageMounter::connect(&provider)
        .await
        .map_err(|e| format!("ImageMounter::connect: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
async fn run_list(state: IdeviceState) -> Result<String, String> {
    let mut c = open_mounter(&state).await?;
    let entries = c
        .copy_devices()
        .await
        .map_err(|e| format!("copy_devices: {e:?}"))?;
    let mut buf = Vec::new();
    plist::to_writer_xml(&mut buf, &plist::Value::Array(entries))
        .map_err(|e| format!("plist serialize: {e:?}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
async fn run_lookup(state: IdeviceState, kind: ImageKind) -> Result<String, String> {
    let mut c = open_mounter(&state).await?;
    match c.lookup_image(kind.as_str()).await {
        Ok(sig) => {
            let hex: String = sig
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" ");
            Ok(format!("{} bytes:\n{hex}", sig.len()))
        }
        Err(idevice::IdeviceError::NotFound) => Ok("(not mounted)".to_string()),
        Err(e) => Err(format!("lookup_image: {e:?}")),
    }
}

#[cfg(target_arch = "wasm32")]
async fn run_unmount(state: IdeviceState, kind: ImageKind) -> Result<(), String> {
    let mut c = open_mounter(&state).await?;
    c.unmount_image(kind.unmount_path())
        .await
        .map_err(|e| format!("unmount_image: {e:?}"))
}
