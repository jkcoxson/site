// Jackson Coxson

use crate::app::App;
use crate::context::Context;
use axum::extract::Request;
use axum::response::Response as AxumResponse;
use axum::{
    body::Body,
    extract::State,
    http::{Response, StatusCode},
    response::IntoResponse,
};
use leptos::prelude::*;
use leptos_axum::render_app_to_stream_with_context;

pub async fn file_and_error_handler(
    State(options): State<LeptosOptions>,
    req: Request<Body>,
    context: Context,
) -> AxumResponse {
    let pkg_root = options.site_root.clone();
    let (parts, body) = req.into_parts();

    let mut static_parts = parts.clone();
    static_parts.headers.clear();
    if let Some(encodings) = parts.headers.get("accept-encoding") {
        static_parts
            .headers
            .insert("accept-encoding", encodings.clone());
    }

    let path: Vec<&str> = static_parts.uri.path().split('/').collect();
    if path.len() > 2 && path[1] == "cdn" {
        let context = context.clone();
        if let Ok(f) = context
            .forge
            .get()
            .lock()
            .await
            .get(path[2..].to_vec(), None)
        {
            match f {
                crate::forge::ForgeReturnType::File(f) => {
                    // Serve the file
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("content-type", f.1)
                        .body(Body::from(f.0))
                        .unwrap()
                }
                crate::forge::ForgeReturnType::Dir => Response::builder()
                    .status(StatusCode::TEMPORARY_REDIRECT)
                    .header("location", format!("/forge/{}", path[2..].join("/")))
                    .body(Body::empty())
                    .unwrap(),
            }
        } else {
            let handler = render_app_to_stream_with_context(
                move || {
                    provide_context(context.clone());
                },
                App,
            );
            handler(Request::from_parts(parts, body))
                .await
                .into_response()
        }
    } else {
        let res = get_static_file(Request::from_parts(static_parts, Body::empty()), &pkg_root)
            .await
            .unwrap();

        if res.status() == StatusCode::OK {
            res.into_response()
        } else {
            let handler = render_app_to_stream_with_context(
                move || {
                    provide_context(context.clone());
                },
                App,
            );
            handler(Request::from_parts(parts, body))
                .await
                .into_response()
        }
    }
}

async fn get_static_file(
    request: Request<Body>,
    root: &str,
) -> Result<Response<Body>, (StatusCode, String)> {
    // `ServeDir` implements `tower::Service` so we can call it with `tower::ServiceExt::oneshot`
    // This path is relative to the cargo root
    match tower::ServiceExt::oneshot(
        tower_http::services::ServeDir::new(root)
            .precompressed_gzip()
            .precompressed_br(),
        request,
    )
    .await
    {
        Ok(res) => Ok(res.into_response()),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Error serving files: {err}"),
        )),
    }
}
