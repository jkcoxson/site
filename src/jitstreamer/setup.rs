// Jackson Coxson

use leptos::prelude::*;

#[component]
pub fn Setup() -> impl IntoView {
    view! {
        <div class="shadow-inner p-6">
            <h2 class="text-2xl font-bold mb-4">Setup</h2>
            <h3 class="text-xl font-bold mb-4">1. Download Wireguard from the App Store</h3>
            <a
                href="https://apps.apple.com/us/app/wireguard/id1441195209?ls=1"
                class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
                target="_blank"
            >
                {"Download Wireguard"}
            </a>
            <h3 class="text-xl font-bold mb-4 pt-6">2. Generate a Pairing File</h3>
            <p class="mb-4">
                Go to
                <a
                    href="https://github.com/osy/Jitterbug/releases"
                    target="_blank"
                    class="text-sky-500"
                >
                    Jitterbug Pair
                </a>and download the version for your computer.
            </p>
            <p class="mb-4">
                Run the program with your iOS device connected to your computer.
                It will save a file to your computer.
            </p>
            <h3 class="text-xl font-bold mb-4">3. Upload the Pairing File</h3>
            <a
                href="https://www.icloud.com/shortcuts/3d2b0b9c981440029d909b9028ccab1c"
                target="_blank"
                class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
            >
                {"Download Shortcut"}
            </a>
            <p>Run this shortcut and it will add a configuration to Wireguard</p>
            <h3 class="text-xl font-bold mb-4">4. Download the shortcut</h3>
            <a
                href="https://www.icloud.com/shortcuts/13eb20918ad34998a30f9f1422c26196"
                target="_blank"
                class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
            >
                {"Download Shortcut"}
            </a>
        </div>
    }
}
