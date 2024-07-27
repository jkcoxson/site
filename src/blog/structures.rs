// Jackson Coxson

use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use sqlx::types::chrono::NaiveDateTime;

#[cfg(feature = "ssr")]
pub mod raw {
    use serde::{Deserialize, Serialize};
    use sqlx::types::chrono::NaiveDateTime;

    pub struct RawPost {
        pub post_name: String,
        pub file_path: String,
        pub slug: String,
        pub sneak_peak: Option<String>,
        pub image_path: Option<String>,
        pub published: Option<bool>,
        pub date_published: NaiveDateTime,
        pub date_updated: Option<NaiveDateTime>,
        pub category: Option<i32>,
    }

    #[derive(sqlx::FromRow)]
    pub struct RawPostPreview {
        pub post_name: String,
        pub slug: String,
        pub sneak_peak: Option<String>,
        pub image_path: Option<String>,
        pub published: Option<bool>,
        pub date_published: NaiveDateTime,
        pub date_updated: Option<NaiveDateTime>,
        pub category: Option<i32>,
        pub category_name: Option<String>,
    }

    pub struct RawPostTags {
        pub id: i32,
        pub slug: String,
        pub tag_id: i32,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PostPreview {
    pub post_name: String,
    pub slug: String,
    pub sneak_peak: Option<String>,
    pub image_path: Option<String>,
    pub published: bool,
    pub date_published: String,
    pub date_updated: Option<String>,
    pub category: Option<Category>,
    pub tags: Vec<Tag>,
}

pub struct Post {
    pub post_name: String,
    pub content: String,
    pub slug: String,
    pub date_published: String,
    pub date_updated: String,
    pub category: Category,
    pub tags: Vec<Tag>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Category {
    pub id: i32,
    pub category_name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "ssr", derive(sqlx::FromRow))]
pub struct Tag {
    pub id: i32,
    pub tage_name: String,
}
