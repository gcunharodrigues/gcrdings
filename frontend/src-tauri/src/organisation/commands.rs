use tauri::{AppHandle, Runtime};

use super::models::{validate_name, Folder, OrganisationError, Tag};
use super::repository::{FolderRepository, TagRepository};
use crate::state::AppState;

#[tauri::command]
pub async fn api_list_folders<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Folder>, OrganisationError> {
    FolderRepository::list(state.db_manager.pool()).await
}

#[tauri::command]
pub async fn api_create_folder<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
) -> Result<String, OrganisationError> {
    let name = validate_name(&name)?;
    FolderRepository::create(state.db_manager.pool(), &name, parent_id.as_deref()).await
}

#[tauri::command]
pub async fn api_rename_folder<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    folder_id: String,
    name: String,
) -> Result<(), OrganisationError> {
    let name = validate_name(&name)?;
    FolderRepository::rename(state.db_manager.pool(), &folder_id, &name).await
}

#[tauri::command]
pub async fn api_move_folder<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    folder_id: String,
    parent_id: Option<String>,
) -> Result<(), OrganisationError> {
    FolderRepository::move_to(state.db_manager.pool(), &folder_id, parent_id.as_deref()).await
}

#[tauri::command]
pub async fn api_delete_folder<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    folder_id: String,
) -> Result<(), OrganisationError> {
    FolderRepository::delete(state.db_manager.pool(), &folder_id).await
}

#[tauri::command]
pub async fn api_set_session_folder<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    folder_id: Option<String>,
) -> Result<(), OrganisationError> {
    FolderRepository::assign_session(state.db_manager.pool(), &meeting_id, folder_id.as_deref())
        .await
}

#[tauri::command]
pub async fn api_list_tags<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<Tag>, OrganisationError> {
    TagRepository::list(state.db_manager.pool()).await
}

#[tauri::command]
pub async fn api_create_tag<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    name: String,
    color: Option<String>,
) -> Result<String, OrganisationError> {
    let name = validate_name(&name)?;
    TagRepository::create(state.db_manager.pool(), &name, color.as_deref()).await
}

#[tauri::command]
pub async fn api_rename_tag<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    tag_id: String,
    name: String,
    color: Option<String>,
) -> Result<(), OrganisationError> {
    let name = validate_name(&name)?;
    TagRepository::rename(state.db_manager.pool(), &tag_id, &name, color.as_deref()).await
}

#[tauri::command]
pub async fn api_delete_tag<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    tag_id: String,
) -> Result<(), OrganisationError> {
    TagRepository::delete(state.db_manager.pool(), &tag_id).await
}

#[tauri::command]
pub async fn api_get_session_tags<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<Tag>, OrganisationError> {
    TagRepository::tags_for_session(state.db_manager.pool(), &meeting_id).await
}

#[tauri::command]
pub async fn api_attach_tag<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    tag_id: String,
) -> Result<(), OrganisationError> {
    TagRepository::attach(state.db_manager.pool(), &meeting_id, &tag_id).await
}

#[tauri::command]
pub async fn api_detach_tag<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    tag_id: String,
) -> Result<(), OrganisationError> {
    TagRepository::detach(state.db_manager.pool(), &meeting_id, &tag_id).await
}
