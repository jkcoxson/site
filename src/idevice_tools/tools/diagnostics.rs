// Jackson Coxson
// diagnostics - DiagnosticsRelay service: IORegistry, MobileGestalt, power.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SimpleQuery {
    GasGauge,
    Nand,
    Wifi,
    All,
}

impl SimpleQuery {
    fn label(self) -> &'static str {
        match self {
            SimpleQuery::GasGauge => "Gas gauge",
            SimpleQuery::Nand => "NAND",
            SimpleQuery::Wifi => "Wi-Fi",
            SimpleQuery::All => "All diagnostics",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PowerAction {
    Sleep,
    Restart,
    Shutdown,
    Goodbye,
}

impl PowerAction {
    fn label(self) -> &'static str {
        match self {
            PowerAction::Sleep => "Sleep",
            PowerAction::Restart => "Restart",
            PowerAction::Shutdown => "Shutdown",
            PowerAction::Goodbye => "Goodbye (close session)",
        }
    }

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    fn confirm(self) -> Option<&'static str> {
        match self {
            PowerAction::Sleep => Some("Put the device to sleep?"),
            PowerAction::Restart => Some("Restart the device?"),
            PowerAction::Shutdown => Some("Power off the device?"),
            PowerAction::Goodbye => None,
        }
    }
}

#[component]
pub fn Page() -> impl IntoView {
    view! {
        <Title text="diagnostics - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"diagnostics"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Query the diagnostics relay: IORegistry, MobileGestalt, and battery / NAND / Wi-Fi info."
                </p>
            </div>
            <IORegistrySection />
            <MobileGestaltSection />
            <SimpleQuerySection />
            <PowerSection />
        </div>
    }
}

#[component]
fn IORegistrySection() -> impl IntoView {
    let state = use_idevice_state();
    let plane = RwSignal::<String>::new(String::new());
    let entry = RwSignal::<String>::new(String::new());
    let class = RwSignal::<String>::new(String::new());
    let output = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        error.set(None);
        output.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            let p = plane.get_untracked();
            let n = entry.get_untracked();
            let c = class.get_untracked();
            wasm_bindgen_futures::spawn_local(async move {
                match run_ioregistry(state, p, n, c).await {
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
            let _ = (state, plane, entry, class);
            busy.set(false);
        }
    };

    view! {
        <Section title="IORegistry">
            <p class="text-xs text-stone-500 dark:text-stone-400">
                "Walk the IORegistry tree. Leave fields empty to fetch the default plane."
            </p>
            <div class="grid grid-cols-1 gap-2 sm:grid-cols-3">
                <TextField label="Plane" value=plane placeholder="IODeviceTree" />
                <TextField label="Entry name" value=entry placeholder="" />
                <TextField label="Entry class" value=class placeholder="" />
            </div>
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get()
            >
                {move || if busy.get() { "Running..." } else { "Query" }}
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
fn MobileGestaltSection() -> impl IntoView {
    let state = use_idevice_state();
    let keys = RwSignal::<String>::new(String::new());
    let output = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        error.set(None);
        output.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            let k = keys.get_untracked();
            wasm_bindgen_futures::spawn_local(async move {
                match run_mobilegestalt(state, k).await {
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
            let _ = (state, keys);
            busy.set(false);
        }
    };

    view! {
        <Section title="MobileGestalt">
            <p class="text-xs text-stone-500 dark:text-stone-400">
                "Comma-separated list of MobileGestalt keys."
            </p>
            <TextField label="Keys" value=keys placeholder="ProductName,UniqueDeviceID" />
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get() || keys.with(|k| k.trim().is_empty())
            >
                {move || if busy.get() { "Running..." } else { "Query" }}
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
fn SimpleQuerySection() -> impl IntoView {
    let state = use_idevice_state();
    let output = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<Option<SimpleQuery>>::new(None);

    let run_query = move |q: SimpleQuery| {
        error.set(None);
        output.set(None);
        busy.set(Some(q));
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_simple(state, q).await {
                    Ok(xml) => output.set(Some(xml)),
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        error.set(Some(e));
                    }
                }
                busy.set(None);
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (state, q);
            busy.set(None);
        }
    };

    let button = move |q: SimpleQuery| {
        view! {
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=move |_| run_query(q)
                disabled=move || busy.get().is_some()
            >
                {move || if busy.get() == Some(q) { "Loading..." } else { q.label() }}
            </button>
        }
    };

    view! {
        <Section title="Quick queries">
            <div class="flex flex-wrap gap-2">
                {button(SimpleQuery::GasGauge)} {button(SimpleQuery::Nand)}
                {button(SimpleQuery::Wifi)} {button(SimpleQuery::All)}
            </div>
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
fn PowerSection() -> impl IntoView {
    let state = use_idevice_state();
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let run = move |action: PowerAction| {
        #[cfg(target_arch = "wasm32")]
        {
            if let Some(msg) = action.confirm() {
                let confirmed = web_sys::window()
                    .and_then(|w| w.confirm_with_message(msg).ok())
                    .unwrap_or(false);
                if !confirmed {
                    return;
                }
            }
        }
        error.set(None);
        status.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_power(state, action).await {
                    Ok(()) => status.set(Some(format!("{} sent.", action.label()))),
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
            let _ = (state, action);
            busy.set(false);
        }
    };

    let danger_btn = move |a: PowerAction| {
        view! {
            <button
                class="rounded bg-red-600 px-3 py-1.5 text-sm text-white hover:bg-red-700 disabled:opacity-50"
                on:click=move |_| run(a)
                disabled=move || busy.get()
            >
                {a.label()}
            </button>
        }
    };

    view! {
        <Section title="Power">
            <div class="rounded border border-amber-300 bg-amber-50 p-2 text-xs text-amber-900 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-100">
                "Sleep / Restart / Shutdown interrupt the device. Goodbye closes the diagnostics session."
            </div>
            <div class="flex flex-wrap gap-2">
                {danger_btn(PowerAction::Sleep)} {danger_btn(PowerAction::Restart)}
                {danger_btn(PowerAction::Shutdown)}
                <button
                    class="rounded border border-stone-400 px-3 py-1.5 text-sm hover:bg-stone-100 disabled:opacity-50 dark:border-stone-500 dark:text-stone-100 dark:hover:bg-stone-700"
                    on:click=move |_| run(PowerAction::Goodbye)
                    disabled=move || busy.get()
                >
                    {PowerAction::Goodbye.label()}
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

#[component]
fn TextField(
    label: &'static str,
    value: RwSignal<String>,
    #[prop(optional)] placeholder: &'static str,
) -> impl IntoView {
    view! {
        <label class="flex flex-col gap-1 text-sm dark:text-stone-200">
            <span class="text-xs text-stone-500 dark:text-stone-400">{label}</span>
            <input
                type="text"
                class="rounded border border-stone-300 bg-white px-2 py-1 text-sm dark:border-stone-600 dark:bg-stone-800"
                placeholder=placeholder
                prop:value=move || value.get()
                on:input=move |ev| value.set(leptos::prelude::event_target_value(&ev))
            />
        </label>
    }
}

// --- wasm-only backends ---------------------------------------------------

#[cfg(target_arch = "wasm32")]
async fn open_diag(
    state: &IdeviceState,
) -> Result<idevice::services::diagnostics_relay::DiagnosticsRelayClient, String> {
    use idevice::{IdeviceService, diagnostics_relay::DiagnosticsRelayClient};
    let provider = crate::idevice_tools::transport::build_provider(state)?;
    DiagnosticsRelayClient::connect(&provider)
        .await
        .map_err(|e| format!("DiagnosticsRelayClient::connect: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
fn dict_to_xml(d: Option<plist::Dictionary>) -> Result<String, String> {
    let d = d.ok_or_else(|| "(empty response)".to_string())?;
    let mut buf = Vec::new();
    plist::to_writer_xml(&mut buf, &plist::Value::Dictionary(d))
        .map_err(|e| format!("plist serialize: {e:?}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
async fn run_ioregistry(
    state: IdeviceState,
    plane: String,
    entry: String,
    class: String,
) -> Result<String, String> {
    let mut c = open_diag(&state).await?;
    fn opt(s: &str) -> Option<&str> {
        if s.is_empty() { None } else { Some(s) }
    }
    let res = c
        .ioregistry(opt(&plane), opt(&entry), opt(&class))
        .await
        .map_err(|e| format!("ioregistry: {e:?}"))?;
    dict_to_xml(res)
}

#[cfg(target_arch = "wasm32")]
async fn run_mobilegestalt(state: IdeviceState, keys: String) -> Result<String, String> {
    let mut c = open_diag(&state).await?;
    let keys: Vec<String> = keys
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let res = c
        .mobilegestalt(if keys.is_empty() { None } else { Some(keys) })
        .await
        .map_err(|e| format!("mobilegestalt: {e:?}"))?;
    dict_to_xml(res)
}

#[cfg(target_arch = "wasm32")]
async fn run_simple(state: IdeviceState, q: SimpleQuery) -> Result<String, String> {
    let mut c = open_diag(&state).await?;
    let res = match q {
        SimpleQuery::GasGauge => c.gasguage().await.map_err(|e| format!("gasguage: {e:?}"))?,
        SimpleQuery::Nand => c.nand().await.map_err(|e| format!("nand: {e:?}"))?,
        SimpleQuery::Wifi => c.wifi().await.map_err(|e| format!("wifi: {e:?}"))?,
        SimpleQuery::All => c.all().await.map_err(|e| format!("all: {e:?}"))?,
    };
    dict_to_xml(res)
}

#[cfg(target_arch = "wasm32")]
async fn run_power(state: IdeviceState, action: PowerAction) -> Result<(), String> {
    let mut c = open_diag(&state).await?;
    match action {
        PowerAction::Sleep => c.sleep().await.map_err(|e| format!("sleep: {e:?}")),
        PowerAction::Restart => c.restart().await.map_err(|e| format!("restart: {e:?}")),
        PowerAction::Shutdown => c.shutdown().await.map_err(|e| format!("shutdown: {e:?}")),
        PowerAction::Goodbye => c.goodbye().await.map_err(|e| format!("goodbye: {e:?}")),
    }
}
