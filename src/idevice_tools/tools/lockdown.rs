// Jackson Coxson
// lockdown - interact with the lockdown service: get/set values, enter
// recovery mode.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::{use_idevice_state, IdeviceState};

#[component]
pub fn Page() -> impl IntoView {
    let state = use_idevice_state();
    let start_session = RwSignal::<bool>::new(true);
    let domain = RwSignal::<String>::new(String::new());

    view! {
        <Title text="lockdown - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"lockdown"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Read and write lockdown values. Protected keys require a TLS session, which uses the device's pairing file."
                </p>
            </div>

            <fieldset class="space-y-2 rounded border border-stone-200 p-3 dark:border-stone-700">
                <legend class="px-1 text-xs font-bold uppercase tracking-wide text-stone-500 dark:text-stone-400">
                    "Common"
                </legend>
                <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                    <input
                        type="checkbox"
                        prop:checked=move || start_session.get()
                        on:change=move |ev| {
                            start_session.set(leptos::prelude::event_target_checked(&ev))
                        }
                    />
                    "Start TLS session (required for protected keys / set / recovery)"
                </label>
                <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                    "Domain (optional):"
                    <input
                        type="text"
                        class="flex-1 rounded border border-stone-300 bg-white px-2 py-1 text-sm dark:border-stone-600 dark:bg-stone-800"
                        placeholder="e.g. com.apple.mobile.wireless_lockdown"
                        prop:value=move || domain.get()
                        on:input=move |ev| domain.set(leptos::prelude::event_target_value(&ev))
                    />
                </label>
            </fieldset>

            <GetSection state start_session domain />
            <SetSection state start_session domain />
            <RecoverySection state start_session />
        </div>
    }
}

#[component]
fn GetSection(
    state: IdeviceState,
    start_session: RwSignal<bool>,
    domain: RwSignal<String>,
) -> impl IntoView {
    let key = RwSignal::<String>::new(String::new());
    let output = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        error.set(None);
        output.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            let key = key.get_untracked();
            let domain = domain.get_untracked();
            let want_session = start_session.get_untracked();
            wasm_bindgen_futures::spawn_local(async move {
                match run_get(state, want_session, key, domain).await {
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
            let _ = (state, key, domain, start_session);
        }
    };

    view! {
        <Section title="Get value">
            <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                "Key (empty = all):"
                <input
                    type="text"
                    class="flex-1 rounded border border-stone-300 bg-white px-2 py-1 text-sm dark:border-stone-600 dark:bg-stone-800"
                    placeholder="e.g. ProductVersion"
                    prop:value=move || key.get()
                    on:input=move |ev| key.set(leptos::prelude::event_target_value(&ev))
                />
            </label>
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get()
            >
                {move || if busy.get() { "Running..." } else { "Get" }}
            </button>
            <ErrorBlock error />
            <Show when=move || output.with(|o| o.is_some())>
                <pre class="max-h-[40vh] overflow-auto rounded border border-stone-200 bg-stone-50 p-3 text-xs leading-snug dark:border-stone-700 dark:bg-stone-900 dark:text-stone-200">
                    {move || output.get().unwrap_or_default()}
                </pre>
            </Show>
        </Section>
    }
}

#[component]
fn SetSection(
    state: IdeviceState,
    start_session: RwSignal<bool>,
    domain: RwSignal<String>,
) -> impl IntoView {
    let key = RwSignal::<String>::new(String::new());
    let value = RwSignal::<String>::new(String::new());
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        error.set(None);
        status.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            let k = key.get_untracked();
            let v = value.get_untracked();
            let domain = domain.get_untracked();
            let want_session = start_session.get_untracked();
            wasm_bindgen_futures::spawn_local(async move {
                match run_set(state, want_session, k, v, domain).await {
                    Ok(()) => status.set(Some("Successfully set.".to_string())),
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
            let _ = (state, key, value, domain, start_session);
        }
    };

    view! {
        <Section title="Set value">
            <p class="text-xs text-stone-500 dark:text-stone-400">
                "Sets a string value. Many keys are read-only; the device will return an error if it refuses."
            </p>
            <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                "Key:"
                <input
                    type="text"
                    class="flex-1 rounded border border-stone-300 bg-white px-2 py-1 text-sm dark:border-stone-600 dark:bg-stone-800"
                    prop:value=move || key.get()
                    on:input=move |ev| key.set(leptos::prelude::event_target_value(&ev))
                />
            </label>
            <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                "Value:"
                <input
                    type="text"
                    class="flex-1 rounded border border-stone-300 bg-white px-2 py-1 text-sm dark:border-stone-600 dark:bg-stone-800"
                    prop:value=move || value.get()
                    on:input=move |ev| value.set(leptos::prelude::event_target_value(&ev))
                />
            </label>
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get() || key.with(|k| k.is_empty())
            >
                {move || if busy.get() { "Running..." } else { "Set" }}
            </button>
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
fn RecoverySection(state: IdeviceState, start_session: RwSignal<bool>) -> impl IntoView {
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        // Recovery reboots the device. Confirm before firing - this is a
        // destructive (well, disruptive) action.
        #[cfg(target_arch = "wasm32")]
        {
            let confirmed = web_sys::window()
                .and_then(|w| {
                    w.confirm_with_message(
                        "This will reboot the device into recovery mode. Continue?",
                    )
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
            let want_session = start_session.get_untracked();
            wasm_bindgen_futures::spawn_local(async move {
                match run_recovery(state, want_session).await {
                    Ok(()) => status.set(Some(
                        "Recovery command sent. The device should reboot into recovery mode."
                            .to_string(),
                    )),
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
            let _ = (state, start_session);
        }
    };

    view! {
        <Section title="Recovery mode">
            <div class="rounded border border-amber-300 bg-amber-50 p-2 text-xs text-amber-900 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-100">
                "Reboots the device into recovery mode."
            </div>
            <button
                class="rounded bg-red-600 px-3 py-1.5 text-sm text-white hover:bg-red-700 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get()
            >
                {move || if busy.get() { "Sending..." } else { "Enter recovery" }}
            </button>
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

#[cfg(target_arch = "wasm32")]
async fn open_with_optional_session(
    state: &IdeviceState,
    start_session: bool,
) -> Result<idevice::services::lockdown::LockdownClient, String> {
    use crate::idevice_tools::transport::{load_pairing_file, open_lockdown};
    let mut lockdown = open_lockdown().await?;
    if start_session {
        let pairing = load_pairing_file(state)?;
        tracing::info!("starting TLS session");
        lockdown
            .start_session(&pairing)
            .await
            .map_err(|e| format!("start_session: {e:?}"))?;
    }
    Ok(lockdown)
}

#[cfg(target_arch = "wasm32")]
async fn run_get(
    state: IdeviceState,
    start_session: bool,
    key: String,
    domain: String,
) -> Result<String, String> {
    let mut lockdown = open_with_optional_session(&state, start_session).await?;
    let key_opt = if key.is_empty() {
        None
    } else {
        Some(key.as_str())
    };
    let domain_opt = if domain.is_empty() {
        None
    } else {
        Some(domain.as_str())
    };
    let value = lockdown
        .get_value(key_opt, domain_opt)
        .await
        .map_err(|e| format!("get_value: {e:?}"))?;
    let mut buf = Vec::new();
    plist::to_writer_xml(&mut buf, &value).map_err(|e| format!("plist serialize: {e:?}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
async fn run_set(
    state: IdeviceState,
    start_session: bool,
    key: String,
    value: String,
    domain: String,
) -> Result<(), String> {
    let mut lockdown = open_with_optional_session(&state, start_session).await?;
    let domain_opt = if domain.is_empty() {
        None
    } else {
        Some(domain.as_str())
    };
    let v = plist::Value::String(value);
    lockdown
        .set_value(key, v, domain_opt)
        .await
        .map_err(|e| format!("set_value: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
async fn run_recovery(state: IdeviceState, start_session: bool) -> Result<(), String> {
    let mut lockdown = open_with_optional_session(&state, start_session).await?;
    lockdown
        .enter_recovery()
        .await
        .map_err(|e| format!("enter_recovery: {e:?}"))
}
