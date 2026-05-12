// Jackson Coxson

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[derive(Clone, Copy, PartialEq, Eq)]
enum InstallAction {
    Install,
    Upgrade,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AppType {
    User,
    System,
    Any,
}

impl AppType {
    fn as_str(self) -> &'static str {
        match self {
            AppType::User => "User",
            AppType::System => "System",
            AppType::Any => "Any",
        }
    }
}

#[derive(Clone, Debug)]
struct AppEntry {
    bundle_id: String,
    display_name: String,
    version: String,
}

#[component]
pub fn Page() -> impl IntoView {
    view! {
        <Title text="installation_proxy - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"installation_proxy"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Install signed .ipa packages, list and browse installed apps, or uninstall by bundle ID."
                </p>
            </div>
            <InstallSection />
            <LookupSection />
            <BrowseSection />
            <UninstallSection />
        </div>
    }
}

#[component]
fn InstallSection() -> impl IntoView {
    let state = use_idevice_state();
    let file_name = RwSignal::<Option<String>>::new(None);
    let file_size = RwSignal::<Option<usize>>::new(None);
    let busy = RwSignal::<bool>::new(false);
    let status = RwSignal::<String>::new(String::new());
    let error = RwSignal::<Option<String>>::new(None);
    let progress = RwSignal::<u64>::new(0);
    let input_ref = NodeRef::<leptos::html::Input>::new();

    let on_file_change = move |_ev: leptos::ev::Event| {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let Some(input) = input_ref.get_untracked() else {
                return;
            };
            let el: &web_sys::HtmlInputElement = input.unchecked_ref();
            match el.files().and_then(|fl| fl.item(0)) {
                Some(f) => {
                    file_name.set(Some(f.name()));
                    file_size.set(Some(f.size() as usize));
                }
                None => {
                    file_name.set(None);
                    file_size.set(None);
                }
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = input_ref;
    };

    let run_action = move |action: InstallAction| {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let file = input_ref.get_untracked().and_then(|input| {
                let el: &web_sys::HtmlInputElement = input.unchecked_ref();
                el.files().and_then(|fl| fl.item(0))
            });
            let Some(file) = file else {
                error.set(Some("Pick a .ipa file first.".to_string()));
                return;
            };
            error.set(None);
            busy.set(true);
            progress.set(0);
            let verb = match action {
                InstallAction::Install => "Installing",
                InstallAction::Upgrade => "Upgrading",
            };
            status.set(format!("{verb}: reading file..."));
            wasm_bindgen_futures::spawn_local(async move {
                match run_install(state, action, file, status, progress).await {
                    Ok(()) => {
                        status.set(format!("{verb} succeeded."));
                        progress.set(100);
                    }
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        error.set(Some(e));
                        status.set(format!("{verb} failed."));
                    }
                }
                busy.set(false);
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (action, state, status, error, progress, input_ref);
        }
    };

    view! {
        <Section title="Install / Upgrade">
            <p class="text-xs text-stone-500 dark:text-stone-400">
                "Uploads to AFC `PublicStaging/idevice.ipa` and hands the path to InstallationProxy."
            </p>
            <label class="flex flex-col gap-1 text-sm dark:text-stone-200">
                ".ipa file:"
                <input
                    type="file"
                    accept=".ipa,application/octet-stream"
                    node_ref=input_ref
                    on:change=on_file_change
                    class="text-sm"
                    disabled=move || busy.get()
                /> <Show when=move || file_name.with(|n| n.is_some())>
                    <span class="font-mono text-xs text-stone-500 dark:text-stone-400">
                        {move || {
                            format!(
                                "{} ({})",
                                file_name.get().unwrap_or_default(),
                                file_size.get().map(format_size).unwrap_or_default(),
                            )
                        }}
                    </span>
                </Show>
            </label>
            <div class="flex flex-wrap gap-2">
                <button
                    class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                    on:click=move |_| run_action(InstallAction::Install)
                    disabled=move || busy.get() || file_name.with(|n| n.is_none())
                >
                    {move || if busy.get() { "Working..." } else { "Install" }}
                </button>
                <button
                    class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                    on:click=move |_| run_action(InstallAction::Upgrade)
                    disabled=move || busy.get() || file_name.with(|n| n.is_none())
                >
                    {move || if busy.get() { "Working..." } else { "Upgrade" }}
                </button>
            </div>
            <Show when=move || busy.get() || (progress.get() > 0)>
                <div class="space-y-1">
                    <div class="h-2 w-full overflow-hidden rounded bg-stone-200 dark:bg-stone-700">
                        <div
                            class="h-full bg-blue-500 transition-all"
                            style:width=move || format!("{}%", progress.get().min(100))
                        ></div>
                    </div>
                    <p class="text-xs font-mono text-stone-600 dark:text-stone-400">
                        {move || format!("{}%", progress.get().min(100))}
                    </p>
                </div>
            </Show>
            <Show when=move || !status.with(String::is_empty)>
                <p class="text-sm text-stone-700 dark:text-stone-300">{move || status.get()}</p>
            </Show>
            <ErrorBlock error />
        </Section>
    }
}

#[component]
fn LookupSection() -> impl IntoView {
    let state = use_idevice_state();
    let app_type = RwSignal::<AppType>::new(AppType::User);
    let apps = RwSignal::<Option<Vec<AppEntry>>>::new(None);
    let busy = RwSignal::<bool>::new(false);
    let error = RwSignal::<Option<String>>::new(None);

    let on_run = move |_| {
        error.set(None);
        busy.set(true);
        apps.set(None);
        #[cfg(target_arch = "wasm32")]
        {
            let ty = app_type.get_untracked();
            wasm_bindgen_futures::spawn_local(async move {
                match run_lookup(state, ty).await {
                    Ok(list) => apps.set(Some(list)),
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
            let _ = (state, app_type);
        }
    };

    let radio = move |t: AppType| {
        view! {
            <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                <input
                    type="radio"
                    name="instproxy-app-type"
                    prop:checked=move || app_type.get() == t
                    on:change=move |_| app_type.set(t)
                />
                {t.as_str()}
            </label>
        }
    };

    view! {
        <Section title="Lookup apps">
            <div class="flex flex-wrap items-center gap-4">
                {radio(AppType::User)} {radio(AppType::System)} {radio(AppType::Any)}
                <button
                    class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                    on:click=on_run
                    disabled=move || busy.get()
                >
                    {move || if busy.get() { "Loading..." } else { "List" }}
                </button>
            </div>
            <ErrorBlock error />
            <Show when=move || apps.with(|a| a.is_some())>
                <AppTable apps />
            </Show>
        </Section>
    }
}

#[component]
fn AppTable(apps: RwSignal<Option<Vec<AppEntry>>>) -> impl IntoView {
    view! {
        <div class="rounded border border-stone-200 dark:border-stone-700">
            <div class="grid grid-cols-[1fr_1.5fr_100px] gap-2 border-b border-stone-200 bg-stone-100 px-2 py-1 text-xs font-semibold dark:border-stone-700 dark:bg-stone-800 dark:text-stone-200">
                <span>"Name"</span>
                <span>"Bundle ID"</span>
                <span>"Version"</span>
            </div>
            <div class="max-h-[40vh] overflow-auto">
                {move || {
                    let list = apps.get().unwrap_or_default();
                    if list.is_empty() {
                        view! {
                            <p class="p-2 text-sm italic text-stone-500 dark:text-stone-400">
                                "(no apps)"
                            </p>
                        }
                            .into_any()
                    } else {
                        view! {
                            <ul>
                                {list
                                    .into_iter()
                                    .map(|app| {
                                        view! {
                                            <li class="grid grid-cols-[1fr_1.5fr_100px] gap-2 border-b border-stone-100 px-2 py-1 text-sm last:border-b-0 dark:border-stone-800 dark:text-stone-100">
                                                <span class="truncate">{app.display_name}</span>
                                                <span class="truncate font-mono text-xs">
                                                    {app.bundle_id}
                                                </span>
                                                <span class="truncate font-mono text-xs">
                                                    {app.version}
                                                </span>
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
        </div>
    }
}

#[component]
fn BrowseSection() -> impl IntoView {
    let state = use_idevice_state();
    let output = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);
    let error = RwSignal::<Option<String>>::new(None);

    let on_run = move |_| {
        error.set(None);
        output.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_browse(state).await {
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
        }
    };

    view! {
        <Section title="Browse (raw)">
            <p class="text-xs text-stone-500 dark:text-stone-400">
                "Streams every dictionary InstallationProxy returns for the `Browse` command. Useful when you need fields beyond display name / bundle ID / version."
            </p>
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get()
            >
                {move || if busy.get() { "Browsing..." } else { "Browse" }}
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
fn UninstallSection() -> impl IntoView {
    let state = use_idevice_state();
    let bundle_id = RwSignal::<String>::new(String::new());
    let busy = RwSignal::<bool>::new(false);
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);

    let on_run = move |_| {
        let bid = bundle_id.get_untracked();
        if bid.is_empty() {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let confirmed = web_sys::window()
                .and_then(|w| {
                    w.confirm_with_message(&format!(
                        "Uninstall {bid}? Local app data will be deleted from the device."
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
                match run_uninstall(state, bid.clone()).await {
                    Ok(()) => status.set(Some(format!("Uninstalled {bid}."))),
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
            let _ = (state, bid);
        }
    };

    view! {
        <Section title="Uninstall">
            <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                "Bundle ID:"
                <input
                    type="text"
                    class="flex-1 rounded border border-stone-300 bg-white px-2 py-1 font-mono text-sm dark:border-stone-600 dark:bg-stone-800 dark:text-stone-100"
                    placeholder="com.example.MyApp"
                    prop:value=move || bundle_id.get()
                    on:input=move |ev| bundle_id.set(leptos::prelude::event_target_value(&ev))
                />
            </label>
            <button
                class="rounded bg-red-600 px-3 py-1.5 text-sm text-white hover:bg-red-700 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get() || bundle_id.with(|b| b.is_empty())
            >
                {move || if busy.get() { "Uninstalling..." } else { "Uninstall" }}
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

fn format_size(bytes: usize) -> String {
    let b = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", b / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", b / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", b / (1024.0 * 1024.0 * 1024.0))
    }
}

// --- wasm-only backends ---------------------------------------------------

#[cfg(target_arch = "wasm32")]
async fn run_install(
    state: IdeviceState,
    action: InstallAction,
    file: web_sys::File,
    status: RwSignal<String>,
    progress: RwSignal<u64>,
) -> Result<(), String> {
    use idevice::{
        afc::{opcode::AfcFopenMode, AfcClient},
        installation_proxy::InstallationProxyClient,
        IdeviceService,
    };

    let verb = match action {
        InstallAction::Install => "Installing",
        InstallAction::Upgrade => "Upgrading",
    };

    status.set(format!("{verb}: reading file..."));
    let bytes = read_file_bytes(&file).await?;
    let size = bytes.len();
    state.push_log(format!("Read {size} bytes from local file"));

    let provider = crate::idevice_tools::transport::build_provider(&state)?;

    status.set(format!("{verb}: uploading to PublicStaging..."));
    let mut afc = AfcClient::connect(&provider)
        .await
        .map_err(|e| format!("AfcClient::connect: {e:?}"))?;
    ensure_public_staging(&mut afc).await?;
    let remote_path = "PublicStaging/idevice.ipa";
    let mut fd = afc
        .open(remote_path.to_string(), AfcFopenMode::WrOnly)
        .await
        .map_err(|e| format!("open({remote_path}): {e:?}"))?;
    fd.write_entire(&bytes)
        .await
        .map_err(|e| format!("write_entire: {e:?}"))?;
    fd.close().await.map_err(|e| format!("close: {e:?}"))?;
    drop(afc);
    state.push_log(format!("Uploaded {size} bytes to {remote_path}"));

    status.set(format!("{verb}: starting InstallationProxy..."));
    let mut inst = InstallationProxyClient::connect(&provider)
        .await
        .map_err(|e| format!("InstallationProxyClient::connect: {e:?}"))?;

    let cb = move |(pct, _): (u64, ())| async move {
        progress.set(pct);
    };

    match action {
        InstallAction::Install => inst
            .install_with_callback(remote_path.to_string(), None, cb, ())
            .await
            .map_err(|e| format!("install: {e:?}")),
        InstallAction::Upgrade => inst
            .upgrade_with_callback(remote_path.to_string(), None, cb, ())
            .await
            .map_err(|e| format!("upgrade: {e:?}")),
    }
}

#[cfg(target_arch = "wasm32")]
async fn run_lookup(state: IdeviceState, ty: AppType) -> Result<Vec<AppEntry>, String> {
    use idevice::{installation_proxy::InstallationProxyClient, IdeviceService};

    let provider = crate::idevice_tools::transport::build_provider(&state)?;
    let mut inst = InstallationProxyClient::connect(&provider)
        .await
        .map_err(|e| format!("InstallationProxyClient::connect: {e:?}"))?;
    let apps = inst
        .get_apps(Some(ty.as_str()), None)
        .await
        .map_err(|e| format!("get_apps: {e:?}"))?;
    let mut out: Vec<AppEntry> = apps
        .into_iter()
        .map(|(bundle_id, info)| {
            let dict = info.as_dictionary();
            let display_name = dict
                .and_then(|d| d.get("CFBundleDisplayName"))
                .and_then(|v| v.as_string())
                .or_else(|| {
                    dict.and_then(|d| d.get("CFBundleName"))
                        .and_then(|v| v.as_string())
                })
                .unwrap_or(&bundle_id)
                .to_string();
            let version = dict
                .and_then(|d| d.get("CFBundleShortVersionString"))
                .and_then(|v| v.as_string())
                .or_else(|| {
                    dict.and_then(|d| d.get("CFBundleVersion"))
                        .and_then(|v| v.as_string())
                })
                .unwrap_or("")
                .to_string();
            AppEntry {
                bundle_id,
                display_name,
                version,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    Ok(out)
}

#[cfg(target_arch = "wasm32")]
async fn run_browse(state: IdeviceState) -> Result<String, String> {
    use idevice::{installation_proxy::InstallationProxyClient, IdeviceService};

    let provider = crate::idevice_tools::transport::build_provider(&state)?;
    let mut inst = InstallationProxyClient::connect(&provider)
        .await
        .map_err(|e| format!("InstallationProxyClient::connect: {e:?}"))?;
    let values = inst
        .browse(None)
        .await
        .map_err(|e| format!("browse: {e:?}"))?;
    let aggregate = plist::Value::Array(values);
    let mut buf = Vec::new();
    plist::to_writer_xml(&mut buf, &aggregate).map_err(|e| format!("plist serialize: {e:?}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
async fn run_uninstall(state: IdeviceState, bundle_id: String) -> Result<(), String> {
    use idevice::{installation_proxy::InstallationProxyClient, IdeviceService};

    let provider = crate::idevice_tools::transport::build_provider(&state)?;
    let mut inst = InstallationProxyClient::connect(&provider)
        .await
        .map_err(|e| format!("InstallationProxyClient::connect: {e:?}"))?;
    inst.uninstall(bundle_id, None)
        .await
        .map_err(|e| format!("uninstall: {e:?}"))
}

/// `idevice::utils::installation::helpers::ensure_public_staging` is gated to
/// non-wasm, so we inline its trivial body here.
#[cfg(target_arch = "wasm32")]
async fn ensure_public_staging(afc: &mut idevice::afc::AfcClient) -> Result<(), String> {
    match afc.get_file_info("PublicStaging").await {
        Ok(_) => Ok(()),
        Err(_) => afc
            .mk_dir("PublicStaging")
            .await
            .map_err(|e| format!("mk_dir(PublicStaging): {e:?}")),
    }
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
