// Jackson Coxson

use crate::app::{Footer, NavBar};
#[cfg(feature = "ssr")]
use crate::context::Context;
use crate::error_template::AppError;
use crate::error_template::ErrorTemplate;
use leptos::*;

#[component]
pub fn BrowseView() -> impl IntoView {
    let once = create_resource(|| (), |_| async move { get_posts(None, None).await });
    view! {
        <NavBar />
        <div class="flex justify-center">
            <div class="m-6 flex w-5/6 flex-col md:w-3/4">
                <h1 class="m-6">"Blog Posts"</h1>
                <hr />
                <Suspense fallback=move || {
                    view! { <h2>"Loading..."</h2> }
                }>
                    {move || match once.get() {
                        Some(posts) => {
                            match posts {
                                Ok(posts) => {
                                    view! {
                                        <div>
                                            {posts
                                                .into_iter()
                                                .map(|p| view! { <PostPreviewComponent preview=p /> })
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
                                    view! { <ErrorTemplate outside_errors /> }.into_view()
                                }
                            }
                        }
                        None => view! { <h2>"Loading..."</h2> }.into_view(),
                    }}

                </Suspense>
            </div>
        </div>
        <br />
        <Footer />
    }
}

#[component]
fn PostPreviewComponent(preview: crate::blog::structures::PostPreview) -> impl IntoView {
    view! {
        <a
            href=format!("/blog/{}", preview.slug)
            class="flex items-start border-b p-4 transition hover:bg-gray-100 dark:hover:bg-gray-800"
        >
            <div class="">
                {if let Some(i) = preview.image_path {
                    view! { <img src=i alt="Post Image" class="mr-4 h-96 w-full object-cover" /> }
                        .into_view()
                } else {
                    view! {}.into_view()
                }} <div class="flex-grow">
                    <h3 class="mb-1 text-lg font-semibold">{preview.post_name}</h3>
                    <p class="mb-1 text-gray-600 dark:text-gray-200">{preview.sneak_peak}</p>
                </div> <div class="text-sm text-gray-500">
                    <small>{preview.relative_date}</small>
                </div>
            </div>
        </a>
    }
}

#[server(GetPosts, "/api", "Url", "get_posts")]
pub async fn get_posts(
    page: Option<u16>,
    limit: Option<u16>,
) -> Result<Vec<super::structures::PostPreview>, ServerFnError> {
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
LEFT JOIN categories ON posts.category = categories.id
ORDER BY posts.date_published DESC
LIMIT ?,?;
"#,
    )
    .bind(page.unwrap_or(0))
    .bind(match limit {
        Some(l) => l + 1,
        None => u16::MAX,
    })
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
                date_published: p.date_published,
                relative_date: format_relative_time(p.date_published),
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
    previews.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    Ok(previews)
}

#[cfg(feature = "ssr")]
fn format_relative_time(dt: sqlx::types::chrono::NaiveDateTime) -> String {
    let now = sqlx::types::chrono::Local::now().naive_utc();
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
