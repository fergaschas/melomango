//! Plugins: sources, installing and removing them, repositories and requirements.

use super::*;

#[tauri::command]
pub(crate) fn list_sources(state: State<'_, AppState>) -> Vec<SourceInfo> {
    state.registry().list()
}

/// Every installed plugin (including metadata-only ones).
#[tauri::command]
pub(crate) fn list_plugins(state: State<'_, AppState>) -> Vec<SourceInfo> {
    state.plugins().iter().map(|p| p.info()).collect()
}

#[tauri::command]
pub(crate) async fn search(state: State<'_, AppState>, source: String, query: String) -> CmdResult<Vec<Track>> {
    let source = state.source(&source)?;
    source.search(&query).await.map_err(CmdError::from)
}

#[tauri::command]
pub(crate) async fn trending(state: State<'_, AppState>, source: String) -> CmdResult<Vec<Track>> {
    let source = state.source(&source)?;
    source.trending().await.map_err(CmdError::from)
}

#[tauri::command]
pub(crate) async fn install_plugin(app: tauri::AppHandle, path: String) -> CmdResult<SourceInfo> {
    blocking(app, move |state| {
        let id = plugin::install(path.as_ref(), &state.plugins_dir, None)?;
        state.reload();
        Ok(state.plugin(&id)?.info())
    })
    .await
}

#[tauri::command]
pub(crate) async fn uninstall_plugin(app: tauri::AppHandle, id: String) -> CmdResult<()> {
    blocking(app, move |state| {
        plugin::uninstall(&id, &state.plugins_dir, &state.plugin_data_dir)?;
        state.reload();
        Ok(())
    })
    .await
}

#[tauri::command]
pub(crate) async fn browse_plugins(state: State<'_, AppState>) -> CmdResult<Catalog> {
    Ok(catalog::browse(&state.catalog_http, &state.registries.list()).await)
}

/// The repository the installed plugin `id` came from, if installing it from `registry`
/// would replace it with another repository's plugin (the app asks first).
#[tauri::command]
pub(crate) async fn plugin_origin_change(
    app: tauri::AppHandle,
    registry: String,
    id: String,
) -> CmdResult<Option<String>> {
    blocking(app, move |state| Ok(catalog::origin_change(&state.plugins_dir, &registry, &id))).await
}

#[tauri::command]
pub(crate) async fn install_catalog_plugin(
    app: tauri::AppHandle,
    registry: String,
    id: String,
    replace_other_origin: Option<bool>,
) -> CmdResult<SourceInfo> {
    let state = app.state::<AppState>();
    let id = catalog::install(
        &state.catalog_http,
        &registry,
        &id,
        &state.plugins_dir,
        &state.plugin_data_dir,
        replace_other_origin.unwrap_or(false),
    )
    .await?;
    blocking(app.clone(), move |state| {
        state.reload();
        Ok(state.plugin(&id)?.info())
    })
    .await
}

#[tauri::command]
pub(crate) async fn list_registries(app: tauri::AppHandle) -> CmdResult<Vec<RegistryInfo>> {
    blocking(app, |state| Ok(state.registries.list())).await
}

#[tauri::command]
pub(crate) async fn add_registry(app: tauri::AppHandle, url: String) -> CmdResult<Vec<RegistryInfo>> {
    blocking(app, move |state| {
        state.registries.add(&url)?;
        Ok(state.registries.list())
    })
    .await
}

#[tauri::command]
pub(crate) async fn remove_registry(app: tauri::AppHandle, url: String) -> CmdResult<Vec<RegistryInfo>> {
    blocking(app, move |state| {
        state.registries.remove(&url)?;
        Ok(state.registries.list())
    })
    .await
}

#[tauri::command]
pub(crate) fn open_plugins_dir(app: tauri::AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    app.opener().open_path(state.plugins_dir.to_string_lossy(), None::<&str>).map_err(|e| CmdError::from(e.to_string()))
}

/// What the plugin needs installed on the computer, and whether it is there.
#[tauri::command]
pub(crate) async fn plugin_requirements(state: State<'_, AppState>, plugin_id: String) -> CmdResult<Vec<Requirement>> {
    state.plugin(&plugin_id)?.requirements().await.map_err(CmdError::from)
}

/// Lets the plugin install a requirement (e.g. yt-dlp with winget).
#[tauri::command]
pub(crate) async fn install_plugin_requirement(
    state: State<'_, AppState>,
    plugin_id: String,
    requirement_id: String,
) -> CmdResult<Vec<Requirement>> {
    state.plugin(&plugin_id)?.install_requirement(&requirement_id).await.map_err(CmdError::from)
}
