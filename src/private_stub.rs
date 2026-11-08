// Jackson Coxson
//
// Stand-in for src/private/ when that repo isn't checked out. It must expose
// the same items as src/private/mod.rs.

use leptos::prelude::*;

/// No private routes.
#[component(transparent)]
pub fn PrivateRoutes() -> impl leptos_router::MatchNestedRoutes + Clone {}

/// No private server routes.
#[cfg(feature = "ssr")]
pub fn router<S: Clone + Send + Sync + 'static>() -> axum::Router<S> {
    axum::Router::new()
}
