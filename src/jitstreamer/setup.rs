// Jackson Coxson

use leptos::{logging::error, prelude::*};
use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};
use web_sys::{js_sys, Blob, Url};

use super::EXTERNAL_JITSTREAMER_API;

#[component]
pub fn Setup() -> impl IntoView {
    let (res, set_res) = signal(None);
    let (api, set_api) = signal(EXTERNAL_JITSTREAMER_API.to_string());
    view! {
        <div class="shadow-inner p-6">
            <h2 class="text-2xl font-bold mb-4">Setup</h2>
            <Suspense>
                {move || match res.get() {
                    Some(Ok(())) => {
                        view! {
                            <div
                                class="bg-green-100 dark:bg-green-800 border-l-4 border-green-500 text-green-700 dark:text-green-100 p-4 mb-4"
                                role="alert"
                            >
                                <p class="font-bold">Success</p>
                                <p>Your pairing file has been uploaded.</p>
                            </div>
                        }
                            .into_any()
                    }
                    Some(Err(err)) => {
                        view! {
                            <div
                                class="bg-red-100 dark:bg-red-800 border-l-4 border-red-500 text-red-700 dark:text-white p-4 mb-4"
                                role="alert"
                            >
                                <p class="font-bold">Error</p>
                                <p>{err}</p>
                            </div>
                        }
                            .into_any()
                    }
                    None => ().into_any(),
                }}
            </Suspense>
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
            <p class="mb-4">Upload the pairing file you generated in the previous step.</p>
            <select
                on:change:target=move |ev| {
                    set_api.set(ev.target().value().parse().unwrap());
                }
                prop:value=move || api.get().to_string()
            >
                <option value=EXTERNAL_JITSTREAMER_API>"JitStreamer Main (Utah)"</option>
                <option value="https://jitstreamer-de-api.jkcoxson.com">"JitStreamer (Germany)"</option>
                // <option value="https://jitstreamer-api.sidestore.io">"SideStore"</option>
            </select>
            <form class="shadow-md rounded px-8 pt-8 pb-8 my-4 w-full max-w-md">
                <input
                    type="file"
                    name="pairing_file"
                    accept=".plist,.mobiledevicepairing"
                    class="mb-4 border rounded w-full py-2 px-3"
                    on:input=move |ev| {
                        let files = ev
                            .target()
                            .unwrap()
                            .unchecked_ref::<web_sys::HtmlInputElement>()
                            .files()
                            .unwrap();
                        leptos::task::spawn_local(async move {
                            match upload_pairing_file(files, api.get().as_str()).await {
                                Ok(conf) => {
                                    println!("Received conf: {conf:?}");
                                    set_res.set(Some(Ok(())));
                                    download_file("jitstreamer.conf", conf).unwrap_throw();
                                }
                                Err(err) => {
                                    error!("Error uploading file: {err}");
                                    set_res.set(Some(Err(err)));
                                }
                            }
                        })
                    }
                />
            </form>
            <p class="mb-4">
                The pairing file should download automatically. Transfer it to your iOS device.
            </p>
            <h3 class="text-xl font-bold mb-4">4. Connect to JitStreamer</h3>
            <p class="mb-4">
                Open the Wireguard app on your phone and import the configuration file you downloaded.
            </p>
            <h3 class="text-xl font-bold mb-4">5. Download the shortcut</h3>
            <a
                href="https://www.icloud.com/shortcuts/c19d3ca719074dc09e140e3e3298b771"
                target="_blank"
                class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 rounded"
            >
                {"Download Shortcut"}
            </a>
        </div>
    }
}

async fn upload_pairing_file(files: web_sys::FileList, target: &str) -> Result<Vec<u8>, String> {
    let files = gloo::file::FileList::from(files);
    let file = match files.first() {
        Some(file) => file,
        None => return Err("No file selected".to_string()),
    };
    let data = match gloo::file::futures::read_as_bytes(file).await {
        Ok(data) => data,
        Err(err) => return Err(format!("Error reading file: {err}")),
    };

    let client = reqwest::Client::new();
    let pairing_file = match client
        .post(format!("{}/register", target))
        .body(data)
        .send()
        .await
    {
        Ok(res) => res,
        Err(err) => return Err(format!("Error uploading file: {err}")),
    };
    if pairing_file.status() != 200 {
        return Err(format!(
            "Error uploading file: {}",
            pairing_file.text().await.unwrap()
        ));
    }
    let pairing_file = match pairing_file.bytes().await {
        Ok(bytes) => bytes,
        Err(err) => return Err(format!("Error reading response: {err}")),
    };

    Ok(pairing_file.to_vec())
}

pub fn download_file(file_name: &str, data: Vec<u8>) -> Result<(), JsValue> {
    // Safely copy the Vec<u8> data into a Uint8Array.
    let array = js_sys::Uint8Array::new_with_length(data.len() as u32);
    array.copy_from(&data[..]);

    // Create a Blob from the Uint8Array.
    let blob_parts = js_sys::Array::new();
    blob_parts.push(&array);
    let blob = Blob::new_with_u8_array_sequence(&blob_parts)?;

    // Create an object URL for the Blob.
    let url = Url::create_object_url_with_blob(&blob)?;

    // Create an <a> element and set its href to the object URL.
    let document = web_sys::window().unwrap().document().unwrap();
    let a = document
        .create_element("a")?
        .dyn_into::<web_sys::HtmlAnchorElement>()?;
    a.set_href(&url);
    a.set_download(file_name);

    // Programmatically click the <a> element to trigger the download.
    a.click();

    // Clean up by revoking the object URL.
    Url::revoke_object_url(&url)?;

    Ok(())
}
