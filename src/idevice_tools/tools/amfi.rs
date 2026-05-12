// Jackson Coxson
// amfi - developer-mode controls via Apple Mobile File Integrity.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::idevice_tools::state::use_idevice_state;
#[cfg(target_arch = "wasm32")]
use crate::idevice_tools::state::IdeviceState;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DevModeAction {
    Show,
    Enable,
    Accept,
}

impl DevModeAction {
    fn label(self) -> &'static str {
        match self {
            DevModeAction::Show => "Show in Settings",
            DevModeAction::Enable => "Enable",
            DevModeAction::Accept => "Accept dialogue",
        }
    }

    fn busy_label(self) -> &'static str {
        match self {
            DevModeAction::Show => "Showing...",
            DevModeAction::Enable => "Enabling...",
            DevModeAction::Accept => "Accepting...",
        }
    }

    fn success_message(self) -> &'static str {
        match self {
            DevModeAction::Show => "Developer Mode option revealed in Settings.",
            DevModeAction::Enable => {
                "Enable command sent. The device will reboot to apply the change."
            }
            DevModeAction::Accept => "Accept dialogue triggered on the device.",
        }
    }
}

#[component]
pub fn Page() -> impl IntoView {
    view! {
        <Title text="amfi - idevice tools" />
        <div class="space-y-5">
            <div class="space-y-2">
                <h1 class="text-xl font-bold dark:text-stone-100">"amfi"</h1>
                <p class="text-sm text-stone-700 dark:text-stone-300">
                    "Control Developer Mode and trust app signers via Apple Mobile File Integrity."
                </p>
            </div>

            <DeveloperModeSection />
            <TrustSection />
        </div>
    }
}

#[component]
fn DeveloperModeSection() -> impl IntoView {
    let state = use_idevice_state();
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy_action = RwSignal::<Option<DevModeAction>>::new(None);
    let dev_mode_status = RwSignal::<Option<bool>>::new(None);

    let run_action = move |action: DevModeAction| {
        error.set(None);
        status.set(None);
        busy_action.set(Some(action));
        let success_msg = action.success_message().to_string();
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_dev_mode_action(state, action).await {
                    Ok(()) => status.set(Some(success_msg)),
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        error.set(Some(e));
                    }
                }
                busy_action.set(None);
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (state, action, success_msg);
            busy_action.set(None);
        }
    };

    let on_status = move |_| {
        error.set(None);
        status.set(None);
        dev_mode_status.set(None);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_status(state).await {
                    Ok(b) => dev_mode_status.set(Some(b)),
                    Err(e) => {
                        state.push_log(format!("ERROR: {e}"));
                        error.set(Some(e));
                    }
                }
            });
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = state;
    };

    let is_busy = move || busy_action.get().is_some();

    let action_button = move |action: DevModeAction| {
        view! {
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=move |_| run_action(action)
                disabled=is_busy
            >
                {move || {
                    if busy_action.get() == Some(action) {
                        action.busy_label()
                    } else {
                        action.label()
                    }
                }}
            </button>
        }
    };

    view! {
        <Section title="Developer Mode">
            <p class="text-xs text-stone-500 dark:text-stone-400">
                "Show the Developer Mode option in Settings -> Privacy & Security. Show is the only action that works with a passcode set."
                "Enable will reboot the device, and accept will finish enablingn developer mode."
            </p>
            <div class="flex flex-wrap gap-2">
                {action_button(DevModeAction::Show)} {action_button(DevModeAction::Enable)}
                {action_button(DevModeAction::Accept)}
            </div>
            <div class="flex flex-wrap items-center gap-3">
                <button
                    class="rounded border border-blue-500 px-3 py-1.5 text-sm text-blue-600 hover:bg-blue-50 disabled:opacity-50 dark:text-blue-300 dark:hover:bg-stone-700"
                    on:click=on_status
                    disabled=is_busy
                >
                    "Check status"
                </button>
                <Show when=move || dev_mode_status.with(|s| s.is_some())>
                    <span class="text-sm dark:text-stone-200">
                        "Developer Mode: "
                        <span class=move || {
                            if dev_mode_status.get() == Some(true) {
                                "font-semibold text-green-700 dark:text-green-300"
                            } else {
                                "font-semibold text-stone-700 dark:text-stone-300"
                            }
                        }>
                            {move || {
                                if dev_mode_status.get() == Some(true) {
                                    "enabled"
                                } else {
                                    "disabled"
                                }
                            }}
                        </span>
                    </span>
                </Show>
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
fn TrustSection() -> impl IntoView {
    let state = use_idevice_state();
    let uuid = RwSignal::<String>::new(String::new());
    let status = RwSignal::<Option<String>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let busy = RwSignal::<bool>::new(false);

    let on_run = move |_| {
        let u = uuid.get_untracked();
        if u.is_empty() {
            return;
        }
        error.set(None);
        status.set(None);
        busy.set(true);
        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                match run_trust(state, u).await {
                    Ok(true) => status.set(Some("Signer trusted.".to_string())),
                    Ok(false) => status.set(Some(
                        "Trust call returned `false` - the device did not record the signer as trusted."
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
            let _ = (state, u);
            busy.set(false);
        }
    };

    view! {
        <Section title="Trust signer">
            <p class="text-xs text-stone-500 dark:text-stone-400">
                "Trust a profile UUID. Not much is known about how this works, it doesn't seem to work on production iOS."
            </p>
            <label class="flex items-center gap-2 text-sm dark:text-stone-200">
                "Profile UUID:"
                <input
                    type="text"
                    class="flex-1 rounded border border-stone-300 bg-white px-2 py-1 font-mono text-sm dark:border-stone-600 dark:bg-stone-800 dark:text-stone-100"
                    placeholder="00000000-0000-0000-0000-000000000000"
                    prop:value=move || uuid.get()
                    on:input=move |ev| uuid.set(leptos::prelude::event_target_value(&ev))
                />
            </label>
            <button
                class="rounded bg-blue-500 px-3 py-1.5 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
                on:click=on_run
                disabled=move || busy.get() || uuid.with(|u| u.is_empty())
            >
                {move || if busy.get() { "Trusting..." } else { "Trust" }}
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
async fn open_amfi(state: &IdeviceState) -> Result<idevice::services::amfi::AmfiClient, String> {
    use idevice::{amfi::AmfiClient, IdeviceService};
    let provider = crate::idevice_tools::transport::build_provider(state)?;
    AmfiClient::connect(&provider)
        .await
        .map_err(|e| format!("AmfiClient::connect: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
async fn run_dev_mode_action(state: IdeviceState, action: DevModeAction) -> Result<(), String> {
    let mut amfi = open_amfi(&state).await?;
    match action {
        DevModeAction::Show => amfi
            .reveal_developer_mode_option_in_ui()
            .await
            .map_err(|e| format!("reveal_developer_mode_option_in_ui: {e:?}")),
        DevModeAction::Enable => amfi
            .enable_developer_mode()
            .await
            .map_err(|e| format!("enable_developer_mode: {e:?}")),
        DevModeAction::Accept => amfi
            .accept_developer_mode()
            .await
            .map_err(|e| format!("accept_developer_mode: {e:?}")),
    }
}

#[cfg(target_arch = "wasm32")]
async fn run_status(state: IdeviceState) -> Result<bool, String> {
    let mut amfi = open_amfi(&state).await?;
    amfi.get_developer_mode_status()
        .await
        .map_err(|e| format!("get_developer_mode_status: {e:?}"))
}

#[cfg(target_arch = "wasm32")]
async fn run_trust(state: IdeviceState, uuid: String) -> Result<bool, String> {
    let mut amfi = open_amfi(&state).await?;
    amfi.trust_app_signer(uuid)
        .await
        .map_err(|e| format!("trust_app_signer: {e:?}"))
}
