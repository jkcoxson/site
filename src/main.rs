// Jackson Coxson

#[cfg(feature = "ssr")]
#[tokio::main]
async fn main() {
    use std::sync::Arc;

    use axum::Router;
    use jkcoxson::app::*;
    use jkcoxson::fileserv::file_and_error_handler;
    use leptos::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};
    use tokio::sync::Mutex;

    let conf = get_configuration(None).await.unwrap();
    let leptos_options = conf.leptos_options;
    let addr = leptos_options.site_addr;
    let routes = generate_route_list(App);

    // Create a new file forge
    let path = std::env::current_dir()
        .expect("Unable to get the current path")
        .join("forge");
    let forge = Arc::new(Mutex::new(
        jkcoxson::forge::Forge::new(path, 0).expect("Unable to create a new file forge"),
    ));
    let context = forge.clone();

    // build our application with a route
    let app = Router::new()
        .leptos_routes_with_context(
            &leptos_options,
            routes,
            move || provide_context(context.clone()),
            App,
        )
        .fallback(|state, req| file_and_error_handler(state, req, forge))
        .with_state(leptos_options);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    logging::log!("listening on http://{}", &addr);
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
