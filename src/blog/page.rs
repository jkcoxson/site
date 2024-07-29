// Jackson Coxson

use crate::app::{Footer, NavBar};
#[cfg(feature = "ssr")]
use crate::context::Context;
use crate::error_template::AppError;
use crate::error_template::ErrorTemplate;
use leptos::*;
use leptos_router::use_params_map;
#[cfg(feature = "ssr")]
use sqlx::{mysql::MySqlRow, Row};

#[component]
pub fn PageView() -> impl IntoView {
    let params = use_params_map();
    let data = create_resource(
        move || params.with(|p| p.get("id").cloned().unwrap_or_default()),
        move |id| async move { id },
    );
    let once = create_resource(
        move || params.get(),
        |d| async move { get_post_content(d.get("id").cloned().unwrap_or_default()).await },
    );
    view! {
        <NavBar/>
        <div class="container">
            <Suspense fallback=move || {
                view! { <h2>"Loading..."</h2> }
            }>

                {match once.get() {
                    Some(data) => {
                        match data {
                            Ok(data) => view! {
                                <div inner_html=data/>
                            }.into_view(),
                            Err(e) => {
                                println!("Unable to get data! {e:?}");
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
                    None => view! { "Loading..." }.into_view(),
                }}

            </Suspense>
        </div>
        <br/>
        <Footer/>
    }
}

#[server(GetPostContent)]
async fn get_post_content(slug: String) -> Result<String, ServerFnError> {
    let state = expect_context::<Context>();
    let post = match sqlx::query_as::<_, (String,)>(
        r#"
     SELECT
         file_path
     FROM posts
     WHERE slug = ?;
             "#,
    )
    .bind(slug)
    .fetch_one(&state.sql_pool)
    .await
    {
        Ok(p) => p.0,
        Err(e) => match e {
            sqlx::Error::RowNotFound => return Err(ServerFnError::Request("".to_string())),
            _ => return Err(ServerFnError::ServerError(e.to_string())),
        },
    };
    let file = match tokio::fs::read_to_string(post).await {
        Ok(f) => f,
        Err(e) => return Err(ServerFnError::ServerError(e.to_string())),
    };
    markdown::CompileOptions::gfm();
    let options = markdown::Options {
        parse: markdown::ParseOptions::gfm(),
        compile: markdown::CompileOptions {
            allow_dangerous_html: true,
            allow_dangerous_protocol: true,
            gfm_footnote_clobber_prefix: Some("".to_string()),
            gfm_tagfilter: true,
            ..markdown::CompileOptions::default()
        },
    };
    Ok(match markdown::to_html_with_options(&file, &options) {
        Ok(o) => o,
        Err(e) => return Err(ServerFnError::ServerError(e.to_string())),
    })
}