// Jackson Coxson

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[cfg(feature = "ssr")]
pub mod raw {
    use sqlx::types::chrono::NaiveDateTime;

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
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PostPreview {
    pub post_name: String,
    pub slug: String,
    pub sneak_peak: Option<String>,
    pub image_path: Option<String>,
    pub published: bool,
    pub date_published: NaiveDateTime,
    pub relative_date: String,
    pub date_updated: Option<String>,
    pub category: Option<Category>,
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
