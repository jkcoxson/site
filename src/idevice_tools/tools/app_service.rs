// Jackson Coxson

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[derive(Clone)]
struct AppRow {
    bundle_id: String,
    name: String,
    version: String,
    flags: String,
}

#[derive(Clone)]
struct ProcessRow {
    pid: u32,
    path: String,
}

#[component]
pub fn Page() -> impl IntoView {
    view! {
        <Title text="app_service - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"app_service"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "RemoteXPC app service: requires iOS 17+ (CoreDeviceProxy)."
                </p>
            </div>
            <ListAppsSection />
            <ProcessesSection />
            <LaunchSection />
            <UninstallSection />
            <SignalSection />
        </div>
    }
}

#[component]
fn ListAppsSection() -> impl IntoView {
    let state = use_idevice_state();
    let rows = RwSignal::<Option<Vec<AppRow>>>::new(None);
    let busy = RwSignal::<bool>::new(false);
    let error = RwSignal::<Option<String>>::new(None);

    let on_run = move |_| {
        error.set(None);
        rows.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_list_apps(state).await {
                    Ok(list) => rows.set(Some(list)),
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
        <Section title="List apps">
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get()
            >
                {move || if busy.get() { "Listing..." } else { "List" }}
            </button>
            <ErrorBlock error />
            <Show when=move || rows.with(|r| r.is_some())>
                <div class="max-h-[40vh] overflow-auto rounded border border-stone-200 dark:border-stone-700">
                    <table class="w-full text-xs">
                        <thead class="sticky top-0 bg-stone-100 dark:bg-stone-800 dark:text-stone-200">
                            <tr>
                                <th class="px-2 py-1 text-left">"Name"</th>
                                <th class="px-2 py-1 text-left">"Bundle ID"</th>
                                <th class="px-2 py-1 text-left">"Version"</th>
                                <th class="px-2 py-1 text-left">"Flags"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                rows.get()
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|r| {
                                        view! {
                                            <tr class="border-b border-stone-100 dark:border-stone-800 dark:text-stone-100">
                                                <td class="px-2 py-1 truncate">{r.name}</td>
                                                <td class="px-2 py-1 truncate font-mono">{r.bundle_id}</td>
                                                <td class="px-2 py-1 font-mono">{r.version}</td>
                                                <td class="px-2 py-1 font-mono text-stone-500 dark:text-stone-400">
                                                    {r.flags}
                                                </td>
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
fn ProcessesSection() -> impl IntoView {
    let state = use_idevice_state();
    let rows = RwSignal::<Option<Vec<ProcessRow>>>::new(None);
    let busy = RwSignal::<bool>::new(false);
    let error = RwSignal::<Option<String>>::new(None);

    let on_run = move |_| {
        error.set(None);
        rows.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_processes(state).await {
                    Ok(list) => rows.set(Some(list)),
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
        <Section title="Processes">
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get()
            >
                {move || if busy.get() { "Loading..." } else { "List processes" }}
            </button>
            <ErrorBlock error />
            <Show when=move || rows.with(|r| r.is_some())>
                <div class="max-h-[40vh] overflow-auto rounded border border-stone-200 dark:border-stone-700">
                    <table class="w-full text-xs">
                        <thead class="sticky top-0 bg-stone-100 dark:bg-stone-800 dark:text-stone-200">
                            <tr>
                                <th class="px-2 py-1 text-left">"PID"</th>
                                <th class="px-2 py-1 text-left">"Path"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || {
                                rows.get()
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|r| {
                                        view! {
                                            <tr class="border-b border-stone-100 font-mono dark:border-stone-800 dark:text-stone-100">
                                                <td class="px-2 py-1">{r.pid}</td>
                                                <td class="px-2 py-1 truncate">{r.path}</td>
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
fn LaunchSection() -> impl IntoView {
    let state = use_idevice_state();
    let bundle = RwSignal::<String>::new(String::new());
    let kill_existing = RwSignal::<bool>::new(false);
    let start_suspended = RwSignal::<bool>::new(false);
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        let id = bundle.get_untracked();
        if id.is_empty() {
            return;
        }
        let ks = kill_existing.get_untracked();
        let ss = start_suspended.get_untracked();
        error.set(None);
        status.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_launch(state, id, ks, ss).await {
                    Ok(pid) => status.set(Some(format!("Launched PID {pid}."))),
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
            let _ = (state, id, ks, ss);
            busy.set(false);
        }
    };

    view! {
        <Section title="Launch app">
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
            </div>
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get() || bundle.with(|b| b.is_empty())
            >
                {move || if busy.get() { "Launching..." } else { "Launch" }}
            </button>
            <ErrorBlock error />
            <StatusBlock status />
        </Section>
    }
}

#[component]
fn UninstallSection() -> impl IntoView {
    let state = use_idevice_state();
    let bundle = RwSignal::<String>::new(String::new());
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        let id = bundle.get_untracked();
        if id.is_empty() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let confirmed = web_sys::window()
                .and_then(|w| w.confirm_with_message(&format!("Uninstall {id}?")).ok())
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
                match run_uninstall(state, id.clone()).await {
                    Ok(()) => status.set(Some(format!("Uninstalled {id}."))),
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
            let _ = (state, id);
            busy.set(false);
        }
    };

    view! {
        <Section title="Uninstall">
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
            <button
                class="rounded bg-red-600 px-3 py-1.5 text-sm text-white hover:bg-red-700 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get() || bundle.with(|b| b.is_empty())
            >
                {move || if busy.get() { "Uninstalling..." } else { "Uninstall" }}
            </button>
            <ErrorBlock error />
            <StatusBlock status />
        </Section>
    }
}

#[component]
fn SignalSection() -> impl IntoView {
    let state = use_idevice_state();
    let pid = RwSignal::<String>::new(String::new());
    let signal = RwSignal::<String>::new("9".to_string());
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        let Ok(p) = pid.get_untracked().parse::<u32>() else {
            error.set(Some("PID must be a number.".to_string()));
            return;
        };
        let Ok(s) = signal.get_untracked().parse::<u32>() else {
            error.set(Some("Signal must be a number.".to_string()));
            return;
        };
        error.set(None);
        status.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_signal(state, p, s).await {
                    Ok(()) => status.set(Some(format!("Sent signal {s} to PID {p}."))),
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
            let _ = (state, p, s);
            busy.set(false);
        }
    };

    view! {
        <Section title="Send signal">
            <div class="flex flex-wrap items-center gap-2">
                <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                    "PID:"
                    <input
                        type="text"
                        inputmode="numeric"
                        class="w-24 rounded border border-stone-300 bg-white px-2 py-1 font-mono text-sm dark:border-stone-600 dark:bg-stone-800"
                        prop:value=move || pid.get()
                        on:input=move |ev| pid.set(leptos::prelude::event_target_value(&ev))
                    />
                </label>
                <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                    "Signal:"
                    <input
                        type="text"
                        inputmode="numeric"
                        class="w-20 rounded border border-stone-300 bg-white px-2 py-1 font-mono text-sm dark:border-stone-600 dark:bg-stone-800"
                        prop:value=move || signal.get()
                        on:input=move |ev| signal.set(leptos::prelude::event_target_value(&ev))
                    />
                </label>
                <button
                    class="rounded bg-red-600 px-3 py-1.5 text-sm text-white hover:bg-red-700 disabled:opacity-50"
                    on:click=on_run
                    disabled=move || busy.get()
                >
                    {move || if busy.get() { "Sending..." } else { "Send" }}
                </button>
            </div>
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
async fn open_app_service(
    state: &IdeviceState,
) -> Result<
    (
        idevice::tcp::handle::AdapterHandle,
        idevice::rsd::RsdHandshake,
        idevice::core_device::AppServiceClient<Box<dyn idevice::ReadWrite>>,
    ),
    String,
> {
    use idevice::{core_device::AppServiceClient, RsdService};
    let (mut adapter, mut handshake) = crate::idevice_tools::transport::open_rsd(state).await?;
    let asc = AppServiceClient::connect_rsd(&mut adapter, &mut handshake)
        .await
        .map_err(|e| format!("AppServiceClient::connect_rsd: {e:?}"))?;
    Ok((adapter, handshake, asc))
}

#[cfg(target_arch = "wasm32")]
async fn run_list_apps(state: IdeviceState) -> Result<Vec<AppRow>, String> {
    let (_a, _h, mut asc) = open_app_service(&state).await?;
    let apps = asc
        .list_apps(true, true, true, true, true)
        .await
        .map_err(|e| format!("list_apps: {e:?}"))?;
    let mut out: Vec<AppRow> = apps
        .into_iter()
        .map(|a| {
            let mut flags = Vec::new();
            if a.is_first_party {
                flags.push("first-party");
            }
            if a.is_developer_app {
                flags.push("dev");
            }
            if a.is_internal {
                flags.push("internal");
            }
            if a.is_hidden {
                flags.push("hidden");
            }
            if a.is_app_clip {
                flags.push("clip");
            }
            if a.is_removable {
                flags.push("removable");
            }
            AppRow {
                name: a.name,
                bundle_id: a.bundle_identifier,
                version: a.version.or(a.bundle_version).unwrap_or_default(),
                flags: flags.join(", "),
            }
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

#[cfg(target_arch = "wasm32")]
async fn run_processes(state: IdeviceState) -> Result<Vec<ProcessRow>, String> {
    let (_a, _h, mut asc) = open_app_service(&state).await?;
    let list = asc
        .list_processes()
        .await
        .map_err(|e| format!("list_processes: {e:?}"))?;
    let mut out: Vec<ProcessRow> = list
        .into_iter()
        .map(|p| ProcessRow {
            pid: p.pid,
            path: p.executable_url.map(|u| u.relative).unwrap_or_default(),
        })
        .collect();
    out.sort_by_key(|p| p.pid);
    Ok(out)
}

#[cfg(target_arch = "wasm32")]
async fn run_launch(
    state: IdeviceState,
    bundle: String,
    kill_existing: bool,
    start_suspended: bool,
) -> Result<u32, String> {
    let (_a, _h, mut asc) = open_app_service(&state).await?;
    let res = asc
        .launch_application(
            bundle,
            &[],
            kill_existing,
            start_suspended,
            None,
            None,
            None,
        )
        .await
        .map_err(|e| format!("launch_application: {e:?}"))?;
    Ok(res.pid)
}

#[cfg(target_arch = "wasm32")]
async fn run_uninstall(state: IdeviceState, bundle: String) -> Result<(), String> {
    let (_a, _h, mut asc) = open_app_service(&state).await?;
    asc.uninstall_app(bundle)
        .await
        .map_err(|e| format!("uninstall_app: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
async fn run_signal(state: IdeviceState, pid: u32, signal: u32) -> Result<(), String> {
    let (_a, _h, mut asc) = open_app_service(&state).await?;
    asc.send_signal(pid, signal)
        .await
        .map_err(|e| format!("send_signal: {e:?}"))?;
    Ok(())
}
