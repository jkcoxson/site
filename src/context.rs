// Jackson Coxson

use std::sync::Arc;

use sqlx::{MySql, Pool};

use crate::forge::buffer::ForgeRing;
use crate::games::server::GameState;

#[derive(Clone)]
pub struct Context {
    pub forge: ForgeRing,
    pub sql_pool: Pool<MySql>,
    pub game_state: Arc<GameState>,
}
