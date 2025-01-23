// Jackson Coxson

use leptos::prelude::*;
use leptos_meta::Title;
use serde::{Deserialize, Serialize};

use crate::app::{Footer, NavBar};

mod setup;

pub const EXTERNAL_JITSTREAMER_API: &str = "https://jitstreamer-api.jkcoxson.com";
pub const INTERNAL_JITSTREAMER_API: &str = "http://[fd00::]:9172";

#[derive(Debug, Serialize, Deserialize)]
struct VersionRequest {
    version: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BasicResponse {
    ok: bool,
}

#[component]
pub fn Page() -> impl IntoView {
    view! {
        <Title text="JitStreamer" />
        <NavBar />
        <div class="flex justify-center">
            <div class="flex flex-col items-center">
                <h1 class="m-6">"JitStreamer"</h1>

                <setup::Setup />
            </div>
        </div>
        <Footer />
    }
}
