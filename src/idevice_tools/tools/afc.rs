// Jackson Coxson
// afc - a Finder-style browser for Apple File Conduit. Models the desktop
// afc_finder workflow:
//   1. Pick a connection mode (Root jail / Documents / Container / Crash
//      reports), optionally selecting an installed app for the house-arrest
//      modes via InstallationProxy.
//   2. Connect once; cache the AfcClient in a thread-local so subsequent
//      directory listings reuse the same TLS lockdown→AFC stack.
//   3. Browse with an editable path bar, sortable columns, double-click to
//      enter directories, single-click to select. Upload/Download/Delete/
//      New Folder act on the selection.
//
// Folder upload/download is intentionally omitted: WebUSB has no folder
// picker, and triggering N save dialogs for a recursive get is hostile.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::{use_idevice_state, IdeviceState};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum AfcMode {
    #[default]
    Root,
    Documents,
    Container,
    CrashReports,
}

impl AfcMode {
    const ALL: &'static [AfcMode] = &[
        AfcMode::Root,
        AfcMode::Documents,
        AfcMode::Container,
        AfcMode::CrashReports,
    ];

    fn label(self) -> &'static str {
        match self {
            AfcMode::Root => "AFC Jail",
            AfcMode::Documents => "App Documents",
            AfcMode::Container => "App Container",
            AfcMode::CrashReports => "Crash Reports",
        }
    }

    fn needs_bundle(self) -> bool {
        matches!(self, AfcMode::Documents | AfcMode::Container)
    }

    fn jail_root(self) -> &'static str {
        match self {
            AfcMode::Documents => "/Documents",
            _ => "/",
        }
    }
}

#[derive(Clone, Debug)]
struct AfcItem {
    name: String,
    size: usize,
    is_dir: bool,
    is_link: bool,
    link_target: Option<String>,
    modified: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum SortColumn {
    #[default]
    Name,
    Size,
    Modified,
}

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum SortDirection {
    #[default]
    Asc,
    Desc,
}

#[derive(Clone)]
struct InstalledApp {
    name: String,
    bundle_id: String,
}

#[component]
pub fn Page() -> impl IntoView {
    let state = use_idevice_state();

    let mode = RwSignal::<AfcMode>::new(AfcMode::Root);
    let bundle_id = RwSignal::<String>::new(String::new());
    let installed_apps = RwSignal::<Option<Result<Vec<InstalledApp>, String>>>::new(None);
    let apps_loading = RwSignal::<bool>::new(false);

    let connected = RwSignal::<bool>::new(false);
    let current_path = RwSignal::<String>::new("/".to_string());
    let listing = RwSignal::<Option<Result<Vec<AfcItem>, String>>>::new(None);
    let selected = RwSignal::<Option<String>>::new(None);
    let sort_col = RwSignal::<SortColumn>::new(SortColumn::Name);
    let sort_dir = RwSignal::<SortDirection>::new(SortDirection::Asc);

    let busy = RwSignal::<bool>::new(false);
    let status = RwSignal::<String>::new(String::new());

    let new_folder_open = RwSignal::<bool>::new(false);
    let new_folder_name = RwSignal::<String>::new(String::new());

    view! {
        <Title text="afc - idevice tools" />
        <div class="space-y-3">
            <h1 class="text-xl font-bold dark:text-stone-100">"afc"</h1>
            <Show
                when=move || connected.get()
                fallback=move || {
                    view! {
                        <ConnectView
                            state
                            mode
                            bundle_id
                            installed_apps
                            apps_loading
                            connected
                            current_path
                            listing
                            busy
                            status
                        />
                    }
                }
            >
                <ExplorerView
                    state
                    mode
                    bundle_id
                    connected
                    current_path
                    listing
                    selected
                    sort_col
                    sort_dir
                    busy
                    status
                    new_folder_open
                    new_folder_name
                />
            </Show>
            <StatusBar status busy />
        </div>
    }
}

#[component]
fn ConnectView(
    state: IdeviceState,
    mode: RwSignal<AfcMode>,
    bundle_id: RwSignal<String>,
    installed_apps: RwSignal<Option<Result<Vec<InstalledApp>, String>>>,
    apps_loading: RwSignal<bool>,
    connected: RwSignal<bool>,
    current_path: RwSignal<String>,
    listing: RwSignal<Option<Result<Vec<AfcItem>, String>>>,
    busy: RwSignal<bool>,
    status: RwSignal<String>,
) -> impl IntoView {
    let on_connect = move |_| {
        let m = mode.get_untracked();
        let b = bundle_id.get_untracked();
        if m.needs_bundle() && b.is_empty() {
            status.set("Pick or type a bundle ID first.".to_string());
            return;
        }
        busy.set(true);
        status.set("Connecting...".to_string());
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match connect_afc(&state, m, b).await {
                    Ok(()) => {
                        let root = m.jail_root().to_string();
                        current_path.set(root.clone());
                        connected.set(true);
                        status.set("Connected.".to_string());
                        match list_dir(&root).await {
                            Ok(items) => listing.set(Some(Ok(items))),
                            Err(e) => {
                                state.push_log(format!("ERROR: {e}"));
                                listing.set(Some(Err(e)));
                            }
                        }
                    }
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        status.set(format!("Failed to connect: {e}"));
                    }
                }
                busy.set(false);
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            busy.set(false);
            let _ = (state, connected, current_path, listing);
        }
    };

    let on_refresh_apps = move |_| {
        apps_loading.set(true);
        installed_apps.set(None);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match fetch_installed_apps(&state).await {
                    Ok(apps) => installed_apps.set(Some(Ok(apps))),
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        installed_apps.set(Some(Err(e)));
                    }
                }
                apps_loading.set(false);
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            apps_loading.set(false);
            let _ = state;
        }
    };

    let needs_bundle = move || mode.with(|m| m.needs_bundle());

    view! {
        <p class="text-sm text-stone-700 dark:text-stone-300">
            "Pick a connection mode and connect to start browsing. Sessions live for as long as this tab holds the WebUSB claim."
        </p>

        <fieldset class="space-y-2 rounded border border-stone-200 p-3 dark:border-stone-700">
            <legend class="px-1 text-xs font-bold uppercase tracking-wide text-stone-500 dark:text-stone-400">
                "Mode"
            </legend>
            <div class="flex flex-wrap gap-4">
                {AfcMode::ALL
                    .iter()
                    .copied()
                    .map(|m| {
                        view! {
                            <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                                <input
                                    type="radio"
                                    name="afc-mode"
                                    prop:checked=move || mode.get() == m
                                    on:change=move |_| {
                                        mode.set(m);
                                        bundle_id.set(String::new());
                                    }
                                />
                                {m.label()}
                            </label>
                        }
                    })
                    .collect_view()}
            </div>
        </fieldset>

        <Show when=needs_bundle>
            <fieldset class="space-y-2 rounded border border-stone-200 p-3 dark:border-stone-700">
                <legend class="px-1 text-xs font-bold uppercase tracking-wide text-stone-500 dark:text-stone-400">
                    "Application"
                </legend>
                <div class="flex items-center gap-2">
                    <input
                        type="text"
                        class="flex-1 rounded border border-stone-300 bg-white px-2 py-1 font-mono text-sm dark:border-stone-600 dark:bg-stone-800"
                        placeholder="App name or bundle ID"
                        prop:value=move || bundle_id.get()
                        on:input=move |ev| bundle_id.set(leptos::prelude::event_target_value(&ev))
                    />
                    <button
                        class="rounded bg-stone-200 px-3 py-1.5 text-sm hover:bg-stone-300 disabled:opacity-50 dark:bg-stone-700 dark:text-stone-100 dark:hover:bg-stone-600"
                        on:click=on_refresh_apps
                        disabled=move || apps_loading.get()
                    >
                        {move || { if apps_loading.get() { "Loading..." } else { "Load apps" } }}
                    </button>
                </div>
                <AppList installed_apps apps_loading bundle_id />
            </fieldset>
        </Show>

        <button
            class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
            on:click=on_connect
            disabled=move || { busy.get() || (needs_bundle() && bundle_id.with(|b| b.is_empty())) }
        >
            {move || if busy.get() { "Connecting..." } else { "Connect" }}
        </button>
    }
}

#[component]
fn AppList(
    installed_apps: RwSignal<Option<Result<Vec<InstalledApp>, String>>>,
    apps_loading: RwSignal<bool>,
    bundle_id: RwSignal<String>,
) -> impl IntoView {
    view! {
        {move || match installed_apps.get() {
            Some(Ok(apps)) => {
                let filter = bundle_id.get().to_lowercase();
                let filtered: Vec<InstalledApp> = apps
                    .into_iter()
                    .filter(|a| {
                        filter.is_empty() || a.name.to_lowercase().contains(&filter)
                            || a.bundle_id.to_lowercase().contains(&filter)
                    })
                    .collect();
                view! {
                    <div class="space-y-1">
                        <p class="text-xs text-stone-500 dark:text-stone-400">
                            {format!("{} apps", filtered.len())}
                        </p>
                        <ul class="max-h-64 overflow-auto rounded border border-stone-200 bg-stone-50 dark:border-stone-700 dark:bg-stone-900">
                            {filtered
                                .into_iter()
                                .map(|app| {
                                    let bid_a = app.bundle_id.clone();
                                    let bid_c = app.bundle_id.clone();
                                    view! {
                                        <li>
                                            <button
                                                class=move || {
                                                    let base = "flex w-full items-center justify-between gap-3 px-2 py-1 text-left text-sm hover:bg-stone-200 dark:hover:bg-stone-700";
                                                    if bundle_id.with(|b| b == &bid_a) {
                                                        format!("{base} bg-blue-200 dark:bg-blue-800")
                                                    } else {
                                                        base.to_string()
                                                    }
                                                }
                                                on:click=move |_| bundle_id.set(bid_c.clone())
                                            >
                                                <span class="truncate dark:text-stone-100">{app.name}</span>
                                                <span class="truncate font-mono text-xs text-stone-500 dark:text-stone-400">
                                                    {app.bundle_id}
                                                </span>
                                            </button>
                                        </li>
                                    }
                                })
                                .collect_view()}
                        </ul>
                    </div>
                }
                    .into_any()
            }
            Some(Err(e)) => {
                view! {
                    <div class="rounded bg-red-100 p-2 text-sm text-red-700 dark:bg-red-900 dark:text-red-200">
                        {e}
                    </div>
                }
                    .into_any()
            }
            None => {
                let text = if apps_loading.get() {
                    "Loading installed apps..."
                } else {
                    "Click \"Load apps\" to fetch the installed app list."
                };
                view! { <p class="text-xs italic text-stone-500 dark:text-stone-400">{text}</p> }
                    .into_any()
            }
        }}
    }
}

#[component]
fn ExplorerView(
    state: IdeviceState,
    mode: RwSignal<AfcMode>,
    bundle_id: RwSignal<String>,
    connected: RwSignal<bool>,
    current_path: RwSignal<String>,
    listing: RwSignal<Option<Result<Vec<AfcItem>, String>>>,
    selected: RwSignal<Option<String>>,
    sort_col: RwSignal<SortColumn>,
    sort_dir: RwSignal<SortDirection>,
    busy: RwSignal<bool>,
    status: RwSignal<String>,
    new_folder_open: RwSignal<bool>,
    new_folder_name: RwSignal<String>,
) -> impl IntoView {
    let path_input = RwSignal::<String>::new(current_path.get_untracked());
    Effect::new(move |_| {
        path_input.set(current_path.get());
    });

    let navigate_to: Callback<String> = Callback::new(move |path: String| {
        busy.set(true);
        selected.set(None);
        status.set(format!("Loading {path}..."));
        #[cfg(target_arch = "wasm32")]
        {
            let p = path.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match list_dir(&p).await {
                    Ok(items) => {
                        current_path.set(p);
                        listing.set(Some(Ok(items)));
                        status.set("Ready.".to_string());
                    }
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        listing.set(Some(Err(e.clone())));
                        status.set(format!("Error: {e}"));
                    }
                }
                busy.set(false);
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            busy.set(false);
            let _ = (path, state);
        }
    });

    let on_disconnect = move |_| {
        #[cfg(target_arch = "wasm32")]
        drop_afc();
        connected.set(false);
        listing.set(None);
        selected.set(None);
        status.set("Disconnected.".to_string());
    };

    let on_up = move |_| {
        let jail = mode.get_untracked().jail_root();
        let here = current_path.get_untracked();
        if here == jail {
            return;
        }
        navigate_to.run(parent_path(&here, jail));
    };

    let on_go = move |_| navigate_to.run(path_input.get_untracked());

    let on_refresh = move |_| navigate_to.run(current_path.get_untracked());

    let on_new_folder_open = move |_| {
        new_folder_name.set(String::new());
        new_folder_open.set(true);
    };

    let create_folder_action: Callback<()> = Callback::new(move |_| {
        let name = new_folder_name.get_untracked();
        if name.is_empty() {
            return;
        }
        let path = join_path(&current_path.get_untracked(), &name);
        busy.set(true);
        new_folder_open.set(false);
        status.set(format!("Creating {path}..."));
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_mkdir(&path).await {
                    Ok(()) => {
                        status.set(format!("Created {path}"));
                        busy.set(false);
                        navigate_to.run(current_path.get_untracked());
                    }
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        status.set(format!("Error: {e}"));
                        busy.set(false);
                    }
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            busy.set(false);
            let _ = (state, path);
        }
    });

    let on_delete = move |_| {
        let Some(name) = selected.get_untracked() else {
            return;
        };
        #[cfg(target_arch = "wasm32")]
        {
            let prompt = format!("Delete {name}? This cannot be undone.");
            let confirmed = web_sys::window()
                .and_then(|w| w.confirm_with_message(&prompt).ok())
                .unwrap_or(false);
            if !confirmed {
                return;
            }
        }
        let path = join_path(&current_path.get_untracked(), &name);
        busy.set(true);
        status.set(format!("Deleting {path}..."));
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_remove(&path).await {
                    Ok(()) => {
                        status.set(format!("Deleted {path}"));
                        selected.set(None);
                        busy.set(false);
                        navigate_to.run(current_path.get_untracked());
                    }
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        status.set(format!("Error: {e}"));
                        busy.set(false);
                    }
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            busy.set(false);
            let _ = (state, path);
        }
    };

    let on_download = move |_| {
        let Some(name) = selected.get_untracked() else {
            return;
        };
        let item = listing.with_untracked(|l| {
            l.as_ref()
                .and_then(|r| r.as_ref().ok())
                .and_then(|items| items.iter().find(|i| i.name == name).cloned())
        });
        let Some(item) = item else { return };
        if item.is_dir {
            status.set("Folder download isn't supported in the browser.".to_string());
            return;
        }
        let path = join_path(&current_path.get_untracked(), &name);
        busy.set(true);
        status.set(format!("Downloading {path}..."));
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_download(&path, &name).await {
                    Ok(size) => status.set(format!("Downloaded {name} ({size} bytes)")),
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        status.set(format!("Error: {e}"));
                    }
                }
                busy.set(false);
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            busy.set(false);
            let _ = (state, path, name);
        }
    };

    let upload_ref = NodeRef::<leptos::html::Input>::new();
    let on_upload_click = move |_| {
        if let Some(input) = upload_ref.get_untracked() {
            #[cfg(target_arch = "wasm32")]
            {
                use wasm_bindgen::JsCast;
                let el: &web_sys::HtmlInputElement = input.unchecked_ref();
                el.click();
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = input;
            }
        }
    };

    let on_upload_change = move |_ev: leptos::ev::Event| {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            let Some(input) = upload_ref.get_untracked() else {
                return;
            };
            let el: &web_sys::HtmlInputElement = input.unchecked_ref();
            let Some(file) = el.files().and_then(|fl| fl.item(0)) else {
                return;
            };
            let dest = join_path(&current_path.get_untracked(), &file.name());
            busy.set(true);
            status.set(format!("Uploading to {dest}..."));
            wasm_bindgen_futures::spawn_local(async move {
                match run_upload(file, &dest).await {
                    Ok(size) => {
                        status.set(format!("Uploaded {size} bytes to {dest}"));
                        busy.set(false);
                        navigate_to.run(current_path.get_untracked());
                    }
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        status.set(format!("Error: {e}"));
                        busy.set(false);
                    }
                }
            });
            el.set_value("");
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            busy.set(false);
            let _ = state;
        }
    };

    let mode_label = move || {
        let m = mode.get();
        if m.needs_bundle() {
            format!("{} - {}", m.label(), bundle_id.get())
        } else {
            m.label().to_string()
        }
    };

    view! {
        <div class="flex flex-wrap items-center gap-2 text-sm">
            <span class="font-mono text-xs text-stone-600 dark:text-stone-300">{mode_label}</span>
            <button
                class="ml-auto rounded bg-stone-200 px-2 py-1 text-xs hover:bg-stone-300 dark:bg-stone-700 dark:text-stone-100 dark:hover:bg-stone-600"
                on:click=on_disconnect
            >
                "Disconnect"
            </button>
        </div>

        <div class="flex items-center gap-2">
            <button
                class="rounded bg-stone-200 px-2 py-1 text-sm hover:bg-stone-300 disabled:opacity-50 dark:bg-stone-700 dark:text-stone-100 dark:hover:bg-stone-600"
                on:click=on_up
                disabled=move || { busy.get() || current_path.get() == mode.get().jail_root() }
                title="Up"
            >
                "↑"
            </button>
            <input
                type="text"
                class="flex-1 rounded border border-stone-300 bg-white px-2 py-1 font-mono text-sm dark:border-stone-600 dark:bg-stone-800 dark:text-stone-100"
                prop:value=move || path_input.get()
                on:input=move |ev| path_input.set(leptos::prelude::event_target_value(&ev))
                on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                    if ev.key() == "Enter" {
                        navigate_to.run(path_input.get_untracked());
                    }
                }
            />
            <button
                class="rounded bg-blue-500 px-3 py-1 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_go
                disabled=move || busy.get()
            >
                "Go"
            </button>
            <button
                class="rounded bg-stone-200 px-2 py-1 text-sm hover:bg-stone-300 disabled:opacity-50 dark:bg-stone-700 dark:text-stone-100 dark:hover:bg-stone-600"
                on:click=on_refresh
                disabled=move || busy.get()
                title="Refresh"
            >
                "⟳"
            </button>
        </div>

        <div class="flex flex-wrap items-center gap-2">
            <button
                class="rounded bg-blue-500 px-3 py-1 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_upload_click
                disabled=move || busy.get()
            >
                "Upload file..."
            </button>
            <input type="file" node_ref=upload_ref on:change=on_upload_change class="hidden" />
            <button
                class="rounded bg-blue-500 px-3 py-1 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_new_folder_open
                disabled=move || busy.get()
            >
                "New folder..."
            </button>
            <button
                class="rounded bg-blue-500 px-3 py-1 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_download
                disabled=move || busy.get() || selected.with(|s| s.is_none())
            >
                "Download"
            </button>
            <button
                class="rounded bg-red-600 px-3 py-1 text-sm text-white hover:bg-red-700 disabled:opacity-50"
                on:click=on_delete
                disabled=move || busy.get() || selected.with(|s| s.is_none())
            >
                "Delete"
            </button>
        </div>

        <Show when=move || new_folder_open.get()>
            <div class="flex items-center gap-2 rounded border border-stone-300 bg-stone-50 p-2 dark:border-stone-600 dark:bg-stone-900">
                <span class="text-sm dark:text-stone-200">"New folder name:"</span>
                <input
                    type="text"
                    class="flex-1 rounded border border-stone-300 bg-white px-2 py-1 font-mono text-sm dark:border-stone-600 dark:bg-stone-800 dark:text-stone-100"
                    prop:value=move || new_folder_name.get()
                    on:input=move |ev| new_folder_name.set(leptos::prelude::event_target_value(&ev))
                    on:keydown=move |ev: leptos::ev::KeyboardEvent| {
                        if ev.key() == "Enter" {
                            create_folder_action.run(());
                        }
                    }
                />
                <button
                    class="rounded bg-blue-500 px-3 py-1 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                    on:click=move |_| create_folder_action.run(())
                    disabled=move || busy.get() || new_folder_name.with(|n| n.is_empty())
                >
                    "Create"
                </button>
                <button
                    class="rounded bg-stone-200 px-3 py-1 text-sm hover:bg-stone-300 dark:bg-stone-700 dark:text-stone-100 dark:hover:bg-stone-600"
                    on:click=move |_| new_folder_open.set(false)
                >
                    "Cancel"
                </button>
            </div>
        </Show>

        <FileTable listing selected sort_col sort_dir current_path navigate_to=navigate_to />
    }
}

#[component]
fn FileTable(
    listing: RwSignal<Option<Result<Vec<AfcItem>, String>>>,
    selected: RwSignal<Option<String>>,
    sort_col: RwSignal<SortColumn>,
    sort_dir: RwSignal<SortDirection>,
    current_path: RwSignal<String>,
    navigate_to: Callback<String>,
) -> impl IntoView {
    let toggle_sort = move |col: SortColumn| {
        if sort_col.get_untracked() == col {
            sort_dir.update(|d| {
                *d = match d {
                    SortDirection::Asc => SortDirection::Desc,
                    SortDirection::Desc => SortDirection::Asc,
                }
            });
        } else {
            sort_col.set(col);
            sort_dir.set(SortDirection::Asc);
        }
    };

    let header_arrow = move |col: SortColumn| {
        if sort_col.get() != col {
            return "";
        }
        match sort_dir.get() {
            SortDirection::Asc => " ▲",
            SortDirection::Desc => " ▼",
        }
    };

    let sorted = move || -> Option<Result<Vec<AfcItem>, String>> {
        let listing = listing.get()?;
        match listing {
            Err(e) => Some(Err(e)),
            Ok(mut items) => {
                let col = sort_col.get();
                let dir = sort_dir.get();
                items.sort_by(|a, b| {
                    let ord = match col {
                        SortColumn::Name => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                        SortColumn::Size => a.size.cmp(&b.size),
                        SortColumn::Modified => a.modified.cmp(&b.modified),
                    };
                    match dir {
                        SortDirection::Asc => ord,
                        SortDirection::Desc => ord.reverse(),
                    }
                });
                Some(Ok(items))
            }
        }
    };

    view! {
        <div class="rounded border border-stone-200 dark:border-stone-700">
            <div class="grid grid-cols-[1fr_120px_180px] gap-2 border-b border-stone-200 bg-stone-100 px-2 py-1 text-xs font-semibold dark:border-stone-700 dark:bg-stone-800 dark:text-stone-200">
                <button
                    class="text-left hover:underline"
                    on:click=move |_| toggle_sort(SortColumn::Name)
                >
                    {move || format!("Name{}", header_arrow(SortColumn::Name))}
                </button>
                <button
                    class="text-left hover:underline"
                    on:click=move |_| toggle_sort(SortColumn::Size)
                >
                    {move || format!("Size{}", header_arrow(SortColumn::Size))}
                </button>
                <button
                    class="text-left hover:underline"
                    on:click=move |_| toggle_sort(SortColumn::Modified)
                >
                    {move || format!("Modified{}", header_arrow(SortColumn::Modified))}
                </button>
            </div>
            <div class="max-h-[55vh] overflow-auto">
                {move || match sorted() {
                    Some(Ok(items)) => {
                        if items.is_empty() {
                            view! {
                                <p class="p-2 text-sm italic text-stone-500 dark:text-stone-400">
                                    "(empty)"
                                </p>
                            }
                                .into_any()
                        } else {
                            view! {
                                <ul>
                                    {items
                                        .into_iter()
                                        .map(|item| {
                                            let icon = if item.is_dir {
                                                "📁"
                                            } else if item.is_link {
                                                "🔗"
                                            } else {
                                                "📄"
                                            };
                                            let size_str = if item.is_dir {
                                                "—".to_string()
                                            } else {
                                                format_size(item.size)
                                            };
                                            let modified = item
                                                .modified
                                                .clone()
                                                .unwrap_or_else(|| "—".to_string());
                                            let n_sel = item.name.clone();
                                            let n_click = item.name.clone();
                                            let n_dblclick = item.name.clone();
                                            let is_dir = item.is_dir;
                                            view! {
                                                <li>
                                                    <button
                                                        class=move || {
                                                            let base = "grid w-full grid-cols-[1fr_120px_180px] gap-2 px-2 py-1 text-left text-sm hover:bg-stone-100 dark:hover:bg-stone-800 dark:text-stone-100";
                                                            let is_selected = selected
                                                                .with(|s| s.as_deref() == Some(n_sel.as_str()));
                                                            if is_selected {
                                                                format!("{base} bg-blue-200 dark:bg-blue-800")
                                                            } else {
                                                                base.to_string()
                                                            }
                                                        }
                                                        on:click=move |_| selected.set(Some(n_click.clone()))
                                                        on:dblclick=move |_| {
                                                            if is_dir {
                                                                let p = join_path(
                                                                    &current_path.get_untracked(),
                                                                    &n_dblclick,
                                                                );
                                                                navigate_to.run(p);
                                                            }
                                                        }
                                                    >
                                                        <span class="truncate font-mono">
                                                            {format!("{icon} {}", item.name)}
                                                            {item
                                                                .link_target
                                                                .as_deref()
                                                                .map(|t| format!(" → {t}"))
                                                                .unwrap_or_default()}
                                                        </span>
                                                        <span class="font-mono text-xs text-stone-600 dark:text-stone-400">
                                                            {size_str}
                                                        </span>
                                                        <span class="font-mono text-xs text-stone-600 dark:text-stone-400">
                                                            {modified}
                                                        </span>
                                                    </button>
                                                </li>
                                            }
                                        })
                                        .collect_view()}
                                </ul>
                            }
                                .into_any()
                        }
                    }
                    Some(Err(e)) => {
                        view! { <div class="p-2 text-sm text-red-700 dark:text-red-300">{e}</div> }
                            .into_any()
                    }
                    None => {
                        view! {
                            <p class="p-2 text-sm italic text-stone-500 dark:text-stone-400">
                                "Loading..."
                            </p>
                        }
                            .into_any()
                    }
                }}
            </div>
        </div>
    }
}

#[component]
fn StatusBar(status: RwSignal<String>, busy: RwSignal<bool>) -> impl IntoView {
    view! {
        <div class="flex items-center gap-2 border-t border-stone-200 pt-2 text-xs text-stone-600 dark:border-stone-700 dark:text-stone-400">
            <Show when=move || busy.get()>
                <span class="inline-block h-2 w-2 animate-pulse rounded-full bg-blue-500"></span>
            </Show>
            <span class="font-mono">{move || status.get()}</span>
        </div>
    }
}

// --- helpers --------------------------------------------------------------

fn join_path(base: &str, name: &str) -> String {
    if base.ends_with('/') {
        format!("{base}{name}")
    } else {
        format!("{base}/{name}")
    }
}

fn parent_path(path: &str, jail: &str) -> String {
    let trimmed = path.trim_end_matches('/');
    match trimmed.rsplit_once('/') {
        Some(("", _)) => "/".to_string(),
        Some((head, _)) if head.len() < jail.len() => jail.to_string(),
        Some((head, _)) => head.to_string(),
        None => jail.to_string(),
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
thread_local! {
    static AFC: std::cell::RefCell<Option<idevice::afc::AfcClient>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(target_arch = "wasm32")]
fn take_afc() -> Result<idevice::afc::AfcClient, String> {
    AFC.with(|c| c.borrow_mut().take())
        .ok_or_else(|| "No active AFC session. Connect first.".to_string())
}

#[cfg(target_arch = "wasm32")]
fn put_afc(client: idevice::afc::AfcClient) {
    AFC.with(|c| *c.borrow_mut() = Some(client));
}

#[cfg(target_arch = "wasm32")]
fn drop_afc() {
    AFC.with(|c| c.borrow_mut().take());
}

#[cfg(target_arch = "wasm32")]
async fn connect_afc(state: &IdeviceState, mode: AfcMode, bundle: String) -> Result<(), String> {
    use idevice::{
        afc::AfcClient, crashreportcopymobile::CrashReportCopyMobileClient,
        house_arrest::HouseArrestClient, IdeviceService,
    };

    drop_afc();
    let provider = crate::idevice_tools::transport::build_provider(state)?;
    let client = match mode {
        AfcMode::Root => AfcClient::connect(&provider)
            .await
            .map_err(|e| format!("AfcClient::connect: {e:?}"))?,
        AfcMode::Documents => {
            let h = HouseArrestClient::connect(&provider)
                .await
                .map_err(|e| format!("HouseArrestClient::connect: {e:?}"))?;
            h.vend_documents(bundle)
                .await
                .map_err(|e| format!("vend_documents: {e:?}"))?
        }
        AfcMode::Container => {
            let h = HouseArrestClient::connect(&provider)
                .await
                .map_err(|e| format!("HouseArrestClient::connect: {e:?}"))?;
            h.vend_container(bundle)
                .await
                .map_err(|e| format!("vend_container: {e:?}"))?
        }
        AfcMode::CrashReports => {
            let c = CrashReportCopyMobileClient::connect(&provider)
                .await
                .map_err(|e| format!("CrashReportCopyMobileClient::connect: {e:?}"))?;
            c.to_afc_client()
        }
    };
    put_afc(client);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn fetch_installed_apps(state: &IdeviceState) -> Result<Vec<InstalledApp>, String> {
    use idevice::{installation_proxy::InstallationProxyClient, IdeviceService};

    let provider = crate::idevice_tools::transport::build_provider(state)?;
    let mut client = InstallationProxyClient::connect(&provider)
        .await
        .map_err(|e| format!("InstallationProxyClient::connect: {e:?}"))?;
    let apps = client
        .get_apps(Some("User"), None)
        .await
        .map_err(|e| format!("get_apps: {e:?}"))?;
    let mut out: Vec<InstalledApp> = apps
        .into_iter()
        .map(|(bundle_id, info)| {
            let dict = info.as_dictionary();
            let name = dict
                .and_then(|d| d.get("CFBundleDisplayName"))
                .and_then(|v| v.as_string())
                .or_else(|| {
                    dict.and_then(|d| d.get("CFBundleName"))
                        .and_then(|v| v.as_string())
                })
                .unwrap_or(&bundle_id)
                .to_string();
            InstalledApp { name, bundle_id }
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(out)
}

#[cfg(target_arch = "wasm32")]
async fn list_dir(path: &str) -> Result<Vec<AfcItem>, String> {
    let mut afc = take_afc()?;
    let result: Result<Vec<AfcItem>, String> = async {
        let names = afc
            .list_dir(path.to_string())
            .await
            .map_err(|e| format!("list_dir: {e:?}"))?;
        let mut items = Vec::with_capacity(names.len());
        for name in names {
            if name == "." || name == ".." {
                continue;
            }
            let full = join_path(path, &name);
            match afc.get_file_info(full.clone()).await {
                Ok(info) => items.push(AfcItem {
                    name,
                    size: info.size,
                    is_dir: info.st_ifmt == "S_IFDIR",
                    is_link: info.st_ifmt == "S_IFLNK",
                    link_target: info.st_link_target,
                    modified: Some(info.modified.format("%Y-%m-%d %H:%M").to_string()),
                }),
                Err(e) => {
                    tracing::warn!("get_file_info({full}) failed: {e:?}");
                    items.push(AfcItem {
                        name,
                        size: 0,
                        is_dir: false,
                        is_link: false,
                        link_target: None,
                        modified: None,
                    });
                }
            }
        }
        Ok(items)
    }
    .await;
    put_afc(afc);
    result
}

#[cfg(target_arch = "wasm32")]
async fn run_mkdir(path: &str) -> Result<(), String> {
    let mut afc = take_afc()?;
    let r = afc
        .mk_dir(path.to_string())
        .await
        .map_err(|e| format!("mk_dir: {e:?}"));
    put_afc(afc);
    r
}

#[cfg(target_arch = "wasm32")]
async fn run_remove(path: &str) -> Result<(), String> {
    let mut afc = take_afc()?;
    let r = afc
        .remove_all(path.to_string())
        .await
        .map_err(|e| format!("remove_all: {e:?}"));
    put_afc(afc);
    r
}

#[cfg(target_arch = "wasm32")]
async fn run_download(path: &str, suggested_name: &str) -> Result<usize, String> {
    use idevice::afc::opcode::AfcFopenMode;

    let mut afc = take_afc()?;
    let result: Result<Vec<u8>, String> = async {
        let mut file = afc
            .open(path.to_string(), AfcFopenMode::RdOnly)
            .await
            .map_err(|e| format!("open: {e:?}"))?;
        let res = file
            .read_entire()
            .await
            .map_err(|e| format!("read_entire: {e:?}"));
        file.close().await.map_err(|e| format!("close: {e:?}"))?;
        res
    }
    .await;
    put_afc(afc);
    let bytes = result?;
    let size = bytes.len();
    save_blob(&bytes, suggested_name)?;
    Ok(size)
}

#[cfg(target_arch = "wasm32")]
async fn run_upload(file: web_sys::File, dest: &str) -> Result<usize, String> {
    use idevice::afc::opcode::AfcFopenMode;

    let bytes = read_file_bytes(&file).await?;
    let size = bytes.len();
    let mut afc = take_afc()?;
    let result: Result<(), String> = async {
        let mut remote = afc
            .open(dest.to_string(), AfcFopenMode::WrOnly)
            .await
            .map_err(|e| format!("open: {e:?}"))?;
        remote
            .write_entire(&bytes)
            .await
            .map_err(|e| format!("write_entire: {e:?}"))
    }
    .await;
    put_afc(afc);
    result.map(|_| size)
}

#[cfg(target_arch = "wasm32")]
fn save_blob(bytes: &[u8], filename: &str) -> Result<(), String> {
    use wasm_bindgen::JsCast;

    let array = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
    array.copy_from(bytes);
    let parts = js_sys::Array::new();
    parts.push(&array.buffer());
    let bag = web_sys::BlobPropertyBag::new();
    bag.set_type("application/octet-stream");
    let blob = web_sys::Blob::new_with_buffer_source_sequence_and_options(&parts, &bag)
        .map_err(|e| format!("Blob::new: {e:?}"))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob)
        .map_err(|e| format!("createObjectURL: {e:?}"))?;
    let document = web_sys::window()
        .ok_or_else(|| "no window".to_string())?
        .document()
        .ok_or_else(|| "no document".to_string())?;
    let anchor = document
        .create_element("a")
        .map_err(|e| format!("createElement(a): {e:?}"))?
        .dyn_into::<web_sys::HtmlAnchorElement>()
        .map_err(|_| "anchor cast failed".to_string())?;
    anchor.set_href(&url);
    anchor.set_download(filename);
    anchor.click();
    let _ = web_sys::Url::revoke_object_url(&url);
    Ok(())
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
