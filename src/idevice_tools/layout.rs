// Jackson Coxson
// Sidebar layout: top "device strip" + left tool list + content Outlet.

use leptos::prelude::*;
use leptos_router::components::{Outlet, A};

use crate::app::{Footer, NavBar};

use super::state::{use_idevice_state, IdeviceState, LogLevel};
use super::tools::TOOLS;

#[derive(Clone, PartialEq, Eq)]
enum BrowserSupport {
    Unknown,
    #[allow(dead_code)]
    Supported,
    #[allow(dead_code)]
    Unsupported(String),
}

#[component]
pub fn Layout() -> impl IntoView {
    super::state::provide_idevice_state();
    let state = use_idevice_state();
    let support = RwSignal::new(BrowserSupport::Unknown);

    #[cfg(target_arch = "wasm32")]
    Effect::new(move |_| {
        support.set(detect_browser_support());

        super::logging::install();
        super::logging::attach_sink(state.log);
        super::logging::set_level(state.log_level.get_untracked());
    });

    Effect::new(move |_| {
        let level = state.log_level.get();
        #[cfg(target_arch = "wasm32")]
        {
            super::logging::set_level(level);
            super::state::save_log_level(level);
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = level;
    });

    view! {
        <NavBar />
        {move || match support.get() {
            BrowserSupport::Supported => {
                view! {
                    <DeviceStrip state />
                    <div class="container mx-auto flex flex-col gap-4 px-4 py-6 lg:flex-row">
                        <Sidebar />
                        <section class="flex-1 min-w-0">
                            <Outlet />
                            <LogPanel state />
                        </section>
                    </div>
                }
                    .into_any()
            }
            BrowserSupport::Unsupported(msg) => {
                view! { <UnsupportedBanner message=msg /> }.into_any()
            }
            BrowserSupport::Unknown => view! { <LoadingPlaceholder /> }.into_any(),
        }}
        <Footer />
    }
}

#[component]
fn UnsupportedBanner(message: String) -> impl IntoView {
    view! {
        <div class="container mx-auto px-4 py-12">
            <div class="mx-auto max-w-2xl space-y-4 rounded border border-amber-300 bg-amber-50 p-6 text-amber-900 dark:border-amber-700 dark:bg-amber-900/30 dark:text-amber-100">
                <h1 class="text-2xl font-bold">"idevice tools"</h1>
                <p class="font-semibold">"This browser can't run the tools."</p>
                <p class="text-sm">{message}</p>
                <p class="text-sm">
                    "The tools talk to iOS devices over WebUSB, which currently requires "
                    <strong>"Chrome (or a Chromium based browser) on macOS or Linux"</strong>
                    ". Firefox and Safari don't ship WebUSB; Windows blocks the mux interface at the driver level."
                </p>
            </div>
        </div>
    }
}

#[component]
fn LoadingPlaceholder() -> impl IntoView {
    view! {
        <div class="container mx-auto px-4 py-12 text-center text-stone-500 dark:text-stone-400">
            "Loading..."
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
fn detect_browser_support() -> BrowserSupport {
    use wasm_bindgen::JsValue;

    let window = match web_sys::window() {
        Some(w) => w,
        None => return BrowserSupport::Unsupported("No window object available.".into()),
    };
    let nav = window.navigator();

    let has_usb = js_sys::Reflect::has(nav.as_ref(), &JsValue::from_str("usb")).unwrap_or(false);

    let platform = js_sys::Reflect::get(nav.as_ref(), &JsValue::from_str("platform"))
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();
    let ua = nav.user_agent().unwrap_or_default();
    let is_windows = platform.starts_with("Win");

    web_sys::console::log_1(&JsValue::from_str(&format!(
        "[idevice-tools] detect: has_usb={has_usb} platform={platform:?} \
         is_windows={is_windows} ua={ua:?}"
    )));

    if is_windows {
        return BrowserSupport::Unsupported(
            "Windows isn't supported - the WinUSB driver stack prevents claiming the Apple mux interface.".into(),
        );
    }

    BrowserSupport::Supported
}

#[component]
fn DeviceStrip(state: IdeviceState) -> impl IntoView {
    let connected = Memo::new(move |_| state.device.get().is_some());
    let paired = Memo::new(move |_| state.pairing_xml.get().is_some());

    let device_label = move || match state.device.get() {
        Some(d) => format!(
            "{}  ({:04x}:{:04x})",
            if d.serial.is_empty() {
                "(no serial)".to_string()
            } else {
                d.serial
            },
            d.vid,
            d.pid,
        ),
        None => "No device connected".to_string(),
    };

    let on_connect = move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            let state = state;
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = super::transport::connect_iphone(state).await {
                    state.push_log(format!("ERROR: {e}"));
                }
            });
        }
    };

    let on_pair = move |_| {
        #[cfg(target_arch = "wasm32")]
        {
            let state = state;
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = super::transport::pair_device(state).await {
                    state.push_log(format!("ERROR: {e}"));
                }
            });
        }
    };

    view! {
        <div class="border-b border-stone-300 bg-stone-100 px-4 py-3 dark:border-stone-700 dark:bg-stone-800">
            <div class="container mx-auto flex flex-wrap items-center gap-3">
                <div class="flex flex-col">
                    <span class="text-xs uppercase tracking-wide text-stone-500 dark:text-stone-400">
                        Device
                    </span>
                    <span class="font-mono text-sm dark:text-stone-100">{device_label}</span>
                </div>
                <div class="ml-auto flex flex-wrap items-center gap-2">
                    <StatusBadge label="Connected" ok=Signal::derive(move || connected.get()) />
                    <StatusBadge label="Paired" ok=Signal::derive(move || paired.get()) />
                    <button
                        class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                        on:click=on_connect
                        disabled=move || connected.get()
                    >
                        "Connect device"
                    </button>
                    <button
                        class="rounded border border-blue-500 px-3 py-1.5 text-sm text-blue-600 hover:bg-blue-50 disabled:opacity-50 dark:text-blue-300 dark:hover:bg-stone-700"
                        on:click=on_pair
                        disabled=move || !connected.get()
                    >
                        "Pair device"
                    </button>
                    <LogLevelDropdown state />
                </div>
            </div>
        </div>
    }
}

#[component]
fn LogLevelDropdown(state: IdeviceState) -> impl IntoView {
    let on_change = move |ev: leptos::ev::Event| {
        let value = leptos::prelude::event_target_value(&ev);
        if let Some(level) = LogLevel::level_from_str(&value) {
            state.log_level.set(level);
        }
    };
    view! {
        <label class="flex items-center gap-1 text-xs text-stone-600 dark:text-stone-300">
            <select
                class="rounded border border-stone-300 bg-white px-2 py-1 text-xs dark:border-stone-600 dark:bg-stone-800"
                on:change=on_change
                prop:value=move || state.log_level.get().as_str().to_string()
            >
                {LogLevel::ALL
                    .iter()
                    .copied()
                    .map(|l| {
                        view! { <option value=l.as_str()>{l.as_str()}</option> }
                    })
                    .collect_view()}
            </select>
        </label>
    }
}

#[component]
fn StatusBadge(label: &'static str, ok: Signal<bool>) -> impl IntoView {
    let class = move || {
        if ok.get() {
            "rounded-full bg-green-100 px-2 py-0.5 text-xs font-semibold text-green-700 dark:bg-green-900 dark:text-green-200"
        } else {
            "rounded-full bg-stone-200 px-2 py-0.5 text-xs font-semibold text-stone-500 dark:bg-stone-700 dark:text-stone-400"
        }
    };
    view! { <span class=class>{label}</span> }
}

#[component]
fn Sidebar() -> impl IntoView {
    view! {
        <aside class="w-full shrink-0 lg:w-56">
            <nav class="flex flex-col gap-1 rounded border border-stone-200 bg-white p-2 text-sm dark:border-stone-700 dark:bg-stone-800">
                <h3 class="px-2 py-1 text-xs font-bold uppercase tracking-wide text-stone-500 dark:text-stone-400">
                    Tools
                </h3>
                {TOOLS
                    .iter()
                    .map(|t| {
                        view! {
                            <A
                                href=format!("/idevice-tools/{}", t.slug)
                                attr:class="rounded px-2 py-1 text-stone-800 hover:bg-stone-100 dark:text-stone-100 dark:hover:bg-stone-700"
                            >
                                {t.name}
                            </A>
                        }
                    })
                    .collect_view()}
            </nav>
        </aside>
    }
}

#[component]
fn LogPanel(state: IdeviceState) -> impl IntoView {
    let has_lines = Memo::new(move |_| !state.log.with(|v| v.is_empty()));
    let on_clear = move |_| state.clear_log();

    view! {
        <Show when=move || has_lines.get()>
            <div class="mt-6 rounded border border-stone-200 bg-stone-50 dark:border-stone-700 dark:bg-stone-900">
                <div class="flex items-center justify-between border-b border-stone-200 px-3 py-1.5 dark:border-stone-700">
                    <span class="text-xs font-bold uppercase tracking-wide text-stone-500 dark:text-stone-400">
                        Log
                    </span>
                    <button
                        class="text-xs text-blue-600 hover:underline dark:text-blue-300"
                        on:click=on_clear
                    >
                        "Clear"
                    </button>
                </div>
                <pre class="max-h-64 overflow-auto p-3 text-xs leading-snug text-stone-700 dark:text-stone-200">
                    {move || state.log.with(|v| v.join("\n"))}
                </pre>
            </div>
        </Show>
    }
}

#[component]
pub fn ToolHome() -> impl IntoView {
    view! {
        <div class="space-y-4">
            <h1 class="text-2xl font-bold dark:text-stone-100">"idevice tools"</h1>
            <p class="text-stone-700 dark:text-stone-300">
                "Browser-native iOS device tools. Connect a device above, optionally pair it, then pick a tool from the sidebar."
            </p>
            <p class="text-xs text-stone-500 dark:text-stone-400">
                "Requires a browser with WebUSB support (Chrome / Edge). Pairing files are stored in this browser only."
            </p>
            <div class="grid gap-3 sm:grid-cols-2">
                {TOOLS
                    .iter()
                    .map(|t| {
                        view! {
                            <A
                                href=format!("/idevice-tools/{}", t.slug)
                                attr:class="block rounded border border-stone-200 bg-white p-3 hover:border-blue-400 hover:bg-blue-50 dark:border-stone-700 dark:bg-stone-800 dark:hover:bg-stone-700"
                            >
                                <h3 class="font-semibold dark:text-stone-100">{t.name}</h3>
                                <p class="text-xs text-stone-600 dark:text-stone-400">
                                    {t.description}
                                </p>
                            </A>
                        }
                    })
                    .collect_view()}
            </div>
        </div>
    }
}
