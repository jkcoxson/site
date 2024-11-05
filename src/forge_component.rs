// Jackson Coxson

use leptos::*;

use serde::{Deserialize, Serialize};

use crate::{
    app::{Footer, NavBar},
    error_template::{AppError, ErrorTemplate},
};

#[cfg(feature = "ssr")]
use crate::context::Context;

#[component]
/// Shows the Forge file explorer
pub fn ForgeComponent() -> impl IntoView {
    view! {
        <NavBar />
        <div class="flex flex-col justify-center text-center items-center">
            <h1>Forge</h1>

            {
                let resource = create_resource(
                    move || leptos_router::use_location().pathname.get(),
                    |route| async move {
                        let split_route = route
                            .split('/')
                            .map(|r| r.to_string())
                            .collect::<Vec<String>>();
                        println!("loading data from API");
                        print_tree(split_route).await
                    },
                );
                view! {
                    <Transition fallback=move || {
                        view! { <h2>"Loading..."</h2> }
                    }>
                        <h2 class="text-sky-500">
                            {move || leptos_router::use_location().pathname.get()}
                        </h2>
                        {match resource.get() {
                            Some(data) => {
                                match data.clone() {
                                    Ok(d) => {
                                        match d {
                                            PrintReturn::File => {
                                                crate::reload();
                                                view! { "Reloading..." }.into_view()
                                            }
                                            PrintReturn::Dir((dirs, files)) => {
                                                view! {
                                                    <div class="w-5/6 md:w-1/3 lg:1/4 bg-gray-200 rounded-t-xl">
                                                        <ul>
                                                            <Back />
                                                            {dirs
                                                                .into_iter()
                                                                .map(|n| view! { <Folder name=n /> })
                                                                .collect::<Vec<_>>()}
                                                        </ul>
                                                        <ul>
                                                            {files
                                                                .into_iter()
                                                                .map(|n| view! { <File name=n /> })
                                                                .collect::<Vec<_>>()}
                                                        </ul>
                                                    </div>
                                                }
                                                    .into_view()
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        match e {
                                            ServerFnError::Request(_) => {
                                                let mut outside_errors = Errors::default();
                                                outside_errors.insert_with_default_key(AppError::NotFound);
                                                view! { <ErrorTemplate outside_errors /> }.into_view()
                                            }
                                            _ => {
                                                let mut outside_errors = Errors::default();
                                                outside_errors
                                                    .insert_with_default_key(AppError::InternalServerError);
                                                view! { <ErrorTemplate outside_errors /> }.into_view()
                                            }
                                        }
                                    }
                                }
                            }
                            None => {
                                view! {
                                    // every time `count` changes, this will run

                                    <h2>Loading...</h2>
                                }
                                    .into_view()
                            }
                        }}

                    </Transition>
                }
            }
        </div>

        <Footer />
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub enum PrintReturn {
    File,
    Dir((Vec<String>, Vec<String>)),
}

#[server(PrintTree, "/api")]
pub async fn print_tree(request: Vec<String>) -> Result<PrintReturn, ServerFnError> {
    let state = expect_context::<Context>();
    let state = state.forge.get();

    let borrowed_request: Vec<&str> = request
        .iter()
        .filter(|s| !s.is_empty())
        .map(|r| r.as_str())
        .collect();
    let data = match state.lock().await.view(borrowed_request[1..].to_vec()) {
        Ok(data) => data,
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => {
                return Err(ServerFnError::Request("File not found".to_string()));
            }
            std::io::ErrorKind::InvalidData => {
                println!("Redirecting from: {borrowed_request:?}");
                leptos_axum::redirect(format!("/cdn/{}", borrowed_request[1..].join("/")).as_str());
                return Ok(PrintReturn::File);
            }
            _ => return Err(e.into()),
        },
    };

    Ok(PrintReturn::Dir(data))
}

#[component]
fn Folder(name: String) -> impl IntoView {
    let mut current_path = leptos_router::use_location().pathname.get_untracked();
    if current_path.ends_with('/') {
        current_path.truncate(current_path.len() - 1)
    }
    view! {
        <li class="p-2 m-4 outline outline-2 rounded-md hover:bg-blue-400 flex justify-center items-center">
            <a class="flex w-full h-full" href=format!("{}/{}", current_path, name)>
                <div class="w-8 h-8 bg-green-700 rounded-2xl flex-shrink-0 flex items-center justify-center mr-2"></div>
                <p>{name}</p>
            </a>
        </li>
    }
}

#[component]
fn Back() -> impl IntoView {
    view! {
        <li class="p-2 m-4 outline outline-2 rounded-md hover:bg-blue-400 flex justify-center items-center">
            <a
                class="flex w-full h-full"
                href=move || {
                    format!(
                        "/{}",
                        {
                            let path = leptos_router::use_location().pathname.get();
                            let mut path = path
                                .split("/")
                                .filter(|s| !s.is_empty())
                                .collect::<Vec<&str>>();
                            path.pop();
                            path.join("/")
                        },
                    )
                }
            >

                <div class="-icon w-8 h-8 bg-green-700 rounded-2xl flex-shrink-0 flex items-center justify-center mr-2"></div>
                <p>".."</p>
            </a>
        </li>
    }
}

#[component]
fn File(name: String) -> impl IntoView {
    let mut current_path = leptos_router::use_location().pathname.get_untracked();
    if current_path.ends_with('/') {
        current_path.truncate(current_path.len() - 1)
    }
    view! {
        <li class="p-2 m-4 outline outline-2 rounded-md hover:bg-blue-400 flex justify-center items-center">
            <a
                class="flex w-full h-full"
                href=format!("{}/{}", current_path, name).replacen("/forge", "/cdn", 1)
                rel="external"
            >
                <div class="w-8 h-8 bg-blue-700 rounded-2xl flex-shrink-0 flex items-center justify-center mr-2"></div>
                <p>{name}</p>
            </a>
        </li>
    }
}
