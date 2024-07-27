// Jackson Coxson

use sqlx::{MySql, Pool};

use crate::forge::buffer::ForgeRing;

#[derive(Clone)]
pub struct Context {
    pub forge: ForgeRing,
    pub sql_pool: Pool<MySql>,
}
