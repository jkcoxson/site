// Jackson Coxson

use leptos::*;

use leptos_router::Redirect;
use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use std::sync::Arc;
#[cfg(feature = "ssr")]
use tokio::sync::Mutex;

use crate::{
    app::{Footer, NavBar},
    error_template::{AppError, ErrorTemplate},
};

#[cfg(feature = "ssr")]
use crate::forge::Forge;

#[component]
/// Shows the Forge file explorer
pub fn ForgeComponent() -> impl IntoView {
    view! {
        <NavBar/>
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
                    <h2>{move || leptos_router::use_location().pathname.get()}</h2>
                    {match resource.get() {
                        Some(data) => {
                            match data.clone() {
                                Ok(d) => {
                                    match d {
                                        PrintReturn::File => {
                                            let path = leptos_router::use_location()
                                                .pathname
                                                .get()
                                                .replacen("/forge", "/cdn", 1);
                                            view! { <Redirect path=path/> }.into_view()
                                        }
                                        PrintReturn::Dir((dirs, files)) => {
                                            view! {

                                                <div class="file-browser">
                                                    <ul class="folders">
                                                    <Back/>
                                                        {dirs
                                                            .into_iter()
                                                            .map(|n| view! { <Folder name=n/> })
                                                            .collect::<Vec<_>>()}
                                                    </ul>
                                                    <ul class="files">
                                                        {files
                                                            .into_iter()
                                                            .map(|n| view! { <File name=n/> })
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
                                            view! { <ErrorTemplate outside_errors/> }.into_view()
                                        }
                                        _ => {
                                            let mut outside_errors = Errors::default();
                                            outside_errors
                                                .insert_with_default_key(AppError::InternalServerError);
                                            view! { <ErrorTemplate outside_errors/> }.into_view()
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

        <Footer/>
    }
}

#[derive(Deserialize, Serialize, Clone)]
pub enum PrintReturn {
    File,
    Dir((Vec<String>, Vec<String>)),
}

#[server(PrintTree, "/api")]
pub async fn print_tree(request: Vec<String>) -> Result<PrintReturn, ServerFnError> {
    let state = expect_context::<Arc<Mutex<Forge>>>();

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
                leptos_axum::redirect(format!("/cdn/{}", borrowed_request.join("/")).as_str());
                return Ok(PrintReturn::File);
            }
            _ => return Err(e.into()),
        },
    };

    Ok(PrintReturn::Dir(data))
}

#[component]
fn Folder(name: String) -> impl IntoView {
    view! {
        <li class="folder">
            <a class="folder-icon"></a>
            <a
                class="folder-name"
                href=format!("{}/{}", leptos_router::use_location().pathname.get_untracked(), name)
            >
                {name}
            </a>
        </li>
    }
}

#[component]
fn Back() -> impl IntoView {
    view! {
        <li class="folder">
            <a class="folder-icon"></a>
            <a
                class="folder-name"
                href={move || {format!("/{}", {let path = leptos_router::use_location().pathname.get();
                    let mut path = path
                        .split("/")
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<&str>>();
                    path.pop();path.join("/")})}}
            >
                ".."
            </a>
        </li>
    }
}

#[component]
fn File(name: String) -> impl IntoView {
    view! {
        <li class="file">
            <span class="file-icon"></span>
            <a
                class="file-name"
                href=format!("{}/{}", leptos_router::use_location().pathname.get_untracked(), name)
                    .replacen("/forge", "/cdn", 1)
                rel="external"
            >
                {name}
            </a>
        </li>
    }
}
