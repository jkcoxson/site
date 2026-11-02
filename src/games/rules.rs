// Jackson Coxson
// The "How to play" button every game puts next to its title: a round book
// icon that opens the rules in an overlay. Each game supplies the rules as
// children, written as <section>s; `RulesSection` gives them a heading.

use leptos::prelude::*;

/// A round "?" book button that opens `children` in a rules overlay. Drop it
/// anywhere (lobby, single player, game room) to give players a rule book.
#[component]
pub fn RulesButton(
    /// The overlay's heading, e.g. "How to play Five Crowns".
    title: &'static str,
    children: ChildrenFn,
) -> impl IntoView {
    let show = RwSignal::new(false);
    view! {
        <button
            title="How to play"
            aria-label="How to play"
            on:click=move |_| show.set(true)
            class="relative flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-stone-100 text-stone-600 hover:bg-stone-200 dark:bg-stone-700 dark:text-stone-200 dark:hover:bg-stone-600 transition-colors shadow-sm"
        >
            <svg
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                class="h-5 w-5"
            >
                <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
            </svg>
            <span class="absolute inset-0 flex items-center justify-center text-[10px] font-bold leading-none">
                "?"
            </span>
        </button>
        {move || {
            if !show.get() {
                return view! { <span /> }.into_any();
            }
            view! {
                <div
                    class="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black/60 p-4 backdrop-blur-sm"
                    on:click=move |_| show.set(false)
                >
                    <div
                        class="my-6 w-full max-w-2xl rounded-2xl bg-stone-800 text-stone-200 shadow-2xl ring-1 ring-stone-700"
                        on:click=move |ev| ev.stop_propagation()
                    >
                        // Header, sticky so the close button is always reachable.
                        <div class="sticky top-0 flex items-center justify-between rounded-t-2xl border-b border-stone-700 bg-stone-800/95 px-5 py-3 backdrop-blur">
                            <h2 class="text-lg font-bold text-white">{title}</h2>
                            <button
                                title="Close"
                                aria-label="Close"
                                on:click=move |_| show.set(false)
                                class="flex h-8 w-8 items-center justify-center rounded-full text-stone-400 hover:bg-stone-700 hover:text-white"
                            >
                                <svg
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2"
                                    stroke-linecap="round"
                                    class="h-5 w-5"
                                >
                                    <line x1="6" y1="6" x2="18" y2="18" />
                                    <line x1="18" y1="6" x2="6" y2="18" />
                                </svg>
                            </button>
                        </div>

                        <div class="space-y-6 px-5 py-5 text-sm leading-relaxed">
                            {children()}
                            <div class="flex justify-end pt-1">
                                <button
                                    on:click=move |_| show.set(false)
                                    class="rounded-xl bg-blue-500 px-5 py-2 font-semibold text-white hover:bg-blue-600 touch-manipulation"
                                >
                                    "Got it"
                                </button>
                            </div>
                        </div>
                    </div>
                </div>
            }
                .into_any()
        }}
    }
}

/// One headed section of a rules overlay.
#[component]
pub fn RulesSection(heading: &'static str, children: Children) -> impl IntoView {
    view! {
        <section class="space-y-2">
            <h3 class="font-semibold text-white">{heading}</h3>
            {children()}
        </section>
    }
}
