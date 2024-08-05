// Jackson Coxson

use crate::app::{Footer, NavBar};
#[cfg(feature = "ssr")]
use crate::context::Context;
use crate::error_template::AppError;
use crate::error_template::ErrorTemplate;
use leptos::*;

#[component]
pub fn BrowseView() -> impl IntoView {
    let once = create_resource(|| (), |_| async move { get_posts().await });
    view! {
        <NavBar/>
        <div class="container">
            <h1>"Blog Posts"</h1>
            <Suspense fallback=move || {
                view! { <h2>"Loading..."</h2> }
            }>
                {move || match once.get() {
                    Some(posts) => {
                        match posts {
                            Ok(posts) => {
                                view! {
                                    <div class="list-group">
                                        {posts
                                            .into_iter()
                                            .map(|p| view! { <PostPreviewComponent preview=p/> })
                                            .collect::<Vec<_>>()
                                            .into_view()}

                                    </div>
                                }
                                    .into_view()
                            }
                            Err(e) => {
                                println!("Error fetching posts: {e:?}");
                                let mut outside_errors = Errors::default();
                                outside_errors
                                    .insert_with_default_key(AppError::InternalServerError);
                                view! { <ErrorTemplate outside_errors/> }.into_view()
                            }
                        }
                    }
                    None => view! { <h2>"Loading..."</h2> }.into_view(),
                }}

            </Suspense>
        </div>
        <br/>
        <Footer/>
    }
}

#[component]
fn PostPreviewComponent(preview: crate::blog::structures::PostPreview) -> impl IntoView {
    view! {
        <a
            href=format!("/blog/{}", preview.slug)
            class="list-group-item list-group-item-action post"
        >
            <div class="row">
                <img
                    src="https://via.placeholder.com/100"
                    alt="Post Image"
                    class="col-auto post-img"
                />
                <div class="col">
                    <h3 class="mb-1">{preview.post_name}</h3>
                    <p class="mb-1">{preview.sneak_peak}</p>
                </div>
                <div class="col-auto float-right">
                    <small>{preview.date_published}</small>
                </div>
            </div>
        </a>
    }
}

#[server(GetPosts, "/api", "Url", "get_posts")]
async fn get_posts() -> Result<Vec<super::structures::PostPreview>, ServerFnError> {
    let state = expect_context::<Context>();

    let posts = match sqlx::query_as::<_, crate::blog::structures::raw::RawPostPreview>(
        r#"
SELECT
    posts.post_name,
    posts.slug,
    posts.sneak_peak,
    posts.image_path,
    posts.published,
    posts.date_published,
    posts.date_updated,
    posts.category,
    categories.category_name
FROM posts
LEFT JOIN categories ON posts.category = categories.id;
"#,
    )
    .fetch_all(&state.sql_pool)
    .await
    {
        Ok(p) => p,
        Err(e) => {
            println!("Error fetching post previews from the database: {:?}", e);
            return Err(ServerFnError::ServerError(e.to_string()));
        }
    };

    let mut set = tokio::task::JoinSet::new();
    posts.into_iter().for_each(|p| {
        let pool = state.sql_pool.clone();
        set.spawn(async move {
            let tags = match sqlx::query_as::<_, super::structures::Tag>(
                r#"
SELECT 
    tags.id, 
    tags.tag_name 
FROM post_tags 
LEFT JOIN tags ON post_tags.tag_id = tags.id 
WHERE post_tags.slug = ?;"#,
            )
            .bind(&p.slug)
            .fetch_all(&pool)
            .await
            {
                Ok(t) => t,
                Err(e) => {
                    println!("Unable to fetch the tags for post {}: {e:?}", &p.slug);
                    Vec::new()
                }
            };
            super::structures::PostPreview {
                post_name: p.post_name,
                slug: p.slug,
                sneak_peak: p.sneak_peak,
                image_path: p.image_path,
                published: p.published.unwrap_or(false),
                date_published: format_relative_time(p.date_published),
                date_updated: p.date_updated.map(format_relative_time),
                category: match p.category {
                    Some(c) => Some(super::structures::Category {
                        id: c,
                        category_name: p.category_name.unwrap(),
                    }),
                    None => None,
                },
                tags,
            }
        });
    });

    let mut previews = Vec::with_capacity(set.len());
    while let Some(preview) = set.join_next().await {
        match preview {
            Ok(p) => {
                if p.published {
                    previews.push(p)
                }
                continue;
            }
            Err(e) => {
                println!("Unable to join preview future! {e:?}");
            }
        }
    }
    Ok(previews)
}

#[cfg(feature = "ssr")]
fn format_relative_time(dt: sqlx::types::chrono::NaiveDateTime) -> String {
    let now = sqlx::types::chrono::Utc::now().naive_utc();
    let duration = now.signed_duration_since(dt);

    if duration.num_minutes() < 1 {
        "Just now".to_string()
    } else if duration.num_hours() < 1 {
        format!("{} minutes ago", duration.num_minutes())
    } else if duration.num_hours() == 1 {
        "An hour ago".to_string()
    } else if duration.num_days() < 1 {
        format!("{} hours ago", duration.num_hours())
    } else if duration.num_days() == 1 {
        "Yesterday".to_string()
    } else if duration.num_days() < 7 {
        format!("{} days ago", duration.num_days())
    } else if duration.num_days() < 14 {
        "Last week".to_string()
    } else if duration.num_days() < 31 {
        format!("{} weeks ago", duration.num_days() / 7)
    } else if duration.num_days() < 61 {
        "Last month".to_string()
    } else if duration.num_days() < 365 {
        format!("{} months ago", duration.num_days() / 30)
    } else if duration.num_days() < 730 {
        "Last year".to_string()
    } else {
        format!("{} years ago", duration.num_days() / 365)
    }
}
