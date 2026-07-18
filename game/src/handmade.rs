use crate::game_api::{GameMemory, Input};
use logger::log_info;
pub mod game_api;

pub type GameUpdateAndRenderFn = extern "C" fn(memory: &mut GameMemory, input: &Input);
#[unsafe(no_mangle)]
pub extern "C" fn game_update_and_render(memory: &mut GameMemory, input: &Input) {
    log_info!("GAMEUPDATEANDRENDER {}", memory.permanent_storage_size);
}
