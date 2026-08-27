use tauri::AppHandle;

use crate::error::CommandResult;
use crate::updater::{self, UpdateCheckDto};

#[tauri::command]
pub async fn updater_check(app: AppHandle) -> CommandResult<UpdateCheckDto> {
    updater::check_for_update(Some(&app)).await
}

#[tauri::command]
pub async fn updater_apply_if_available(app: AppHandle) -> CommandResult<UpdateCheckDto> {
    updater::apply_if_available(&app).await
}
