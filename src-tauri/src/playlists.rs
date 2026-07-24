//! Vela-native, cross-source playlists.
//!
//! The store is deliberately separate from `config.json`. Playlist entries
//! keep full item snapshots so mixed-source routing, artwork, episode labels,
//! and explicit playback verbs do not need lossy reconstruction.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::source::ItemDto;

const SCHEMA_VERSION: u32 = 1;
const MAX_NAME_CHARS: usize = 120;
static PLAYLISTS_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PlaylistFile {
    pub schema_version: u32,
    pub playlists: Vec<Playlist>,
}

impl Default for PlaylistFile {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            playlists: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub items: Vec<PlaylistEntry>,
    pub created_ms: u64,
    pub updated_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntry {
    /// Stable even when the same title intentionally appears more than once.
    pub id: String,
    pub item: ItemDto,
    /// Retained display context after the source configuration is removed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistSummary {
    pub id: String,
    pub name: String,
    pub item_count: usize,
    pub created_ms: u64,
    pub updated_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistView {
    pub id: String,
    pub name: String,
    pub items: Vec<PlaylistEntryView>,
    pub created_ms: u64,
    pub updated_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntryView {
    pub id: String,
    pub item: ItemDto,
    pub source_name: Option<String>,
    /// Read-time routing availability only. A configured server can still go
    /// offline between render and Play; playback discovers and skips that.
    pub available: bool,
}

fn paths() -> Result<(PathBuf, PathBuf), String> {
    let data = crate::storage::config_dir_file("playlists.json")
        .map_err(|error| format!("playlist store unavailable: {error}"))?;
    let lock = crate::storage::config_dir_file("playlists.lock")
        .map_err(|error| format!("playlist lock unavailable: {error}"))?;
    Ok((data, lock))
}

fn validate_file(file: &PlaylistFile) -> Result<(), String> {
    if file.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "unsupported playlist schema version {}",
            file.schema_version
        ));
    }
    Ok(())
}

fn sanitize_legacy_artwork_in(file: &mut PlaylistFile) -> bool {
    let mut changed = false;
    for playlist in &mut file.playlists {
        for entry in &mut playlist.items {
            changed |= crate::artwork::sanitize_item_artwork(&mut entry.item);
        }
    }
    changed
}

fn load_at(path: &Path) -> Result<PlaylistFile, String> {
    let mut file: PlaylistFile = crate::storage::load_json(path)
        .map_err(|error| format!("could not read playlists: {error}"))?;
    validate_file(&file)?;
    sanitize_legacy_artwork_in(&mut file);
    Ok(file)
}

fn update_at<R>(
    data_path: &Path,
    lock_path: &Path,
    mutate: impl FnOnce(&mut PlaylistFile) -> Result<R, String>,
) -> Result<R, String> {
    crate::storage::update_json(
        "playlists",
        &PLAYLISTS_LOCK,
        data_path,
        lock_path,
        |file: &mut PlaylistFile| {
            validate_file(file)?;
            sanitize_legacy_artwork_in(file);
            let output = mutate(file)?;
            sanitize_legacy_artwork_in(file);
            Ok(output)
        },
    )
}

fn update<R>(mutate: impl FnOnce(&mut PlaylistFile) -> Result<R, String>) -> Result<R, String> {
    let (data, lock) = paths()?;
    update_at(&data, &lock, mutate)
}

fn rekey_source_id_in(file: &mut PlaylistFile, old_source_id: &str, new_source_id: &str) {
    for playlist in &mut file.playlists {
        for entry in &mut playlist.items {
            crate::source::rekey_item_source(&mut entry.item, old_source_id, new_source_id);
        }
    }
}

pub(crate) fn migrate_source_id_at(
    data_path: &Path,
    lock_path: &Path,
    old_source_id: &str,
    new_source_id: &str,
) -> Result<(), String> {
    match std::fs::symlink_metadata(data_path) {
        Ok(_) => update_at(data_path, lock_path, |file| {
            rekey_source_id_in(file, old_source_id, new_source_id);
            Ok(())
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("could not inspect playlists: {error}")),
    }
}

/// Re-key persisted Vela playlist routing during the one-shot multi-Plex
/// migration. A genuinely absent playlist store stays absent; a corrupt or
/// inaccessible one fails closed so config retains its retry marker.
pub(crate) fn migrate_source_id(
    old_source_id: &str,
    new_source_id: &str,
) -> Result<(), String> {
    let (data, lock) = paths()?;
    migrate_source_id_at(&data, &lock, old_source_id, new_source_id)
}

/// Rewrite valid legacy playlist artwork snapshots without exposing a corrupt
/// playlist file as an app-wide settings fault. Reads remain sanitized even if
/// this best-effort upgrade write cannot complete.
pub(crate) fn sanitize_legacy_artwork() -> Result<(), String> {
    let (data, lock) = paths()?;
    match std::fs::symlink_metadata(&data) {
        Ok(metadata) if metadata.file_type().is_file() => {
            let mut current: PlaylistFile = crate::storage::load_json(&data)
                .map_err(|error| format!("could not read playlists: {error}"))?;
            validate_file(&current)?;
            if sanitize_legacy_artwork_in(&mut current) {
                update_at(&data, &lock, |_| Ok(()))?;
            }
            Ok(())
        }
        Ok(_) => Err("playlist store is not a regular file".to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("could not inspect playlists: {error}")),
    }
}

pub fn load() -> Result<PlaylistFile, String> {
    let (data, _) = paths()?;
    load_at(&data)
}

pub fn list() -> Result<Vec<PlaylistSummary>, String> {
    Ok(load()?
        .playlists
        .into_iter()
        .map(|playlist| PlaylistSummary {
            id: playlist.id,
            name: playlist.name,
            item_count: playlist.items.len(),
            created_ms: playlist.created_ms,
            updated_ms: playlist.updated_ms,
        })
        .collect())
}

pub fn get(id: &str) -> Result<Playlist, String> {
    load()?
        .playlists
        .into_iter()
        .find(|playlist| playlist.id == id)
        .ok_or_else(|| "playlist not found".to_string())
}

fn validated_name(name: String) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("playlist name cannot be empty".to_string());
    }
    if trimmed.chars().count() > MAX_NAME_CHARS {
        return Err(format!(
            "playlist name cannot exceed {MAX_NAME_CHARS} characters"
        ));
    }
    Ok(trimmed.to_string())
}

fn create_in(
    file: &mut PlaylistFile,
    id: String,
    name: String,
    now_ms: u64,
) -> Result<Playlist, String> {
    let playlist = Playlist {
        id,
        name: validated_name(name)?,
        items: Vec::new(),
        created_ms: now_ms,
        updated_ms: now_ms,
    };
    file.playlists.push(playlist.clone());
    Ok(playlist)
}

fn rename_in(
    file: &mut PlaylistFile,
    id: &str,
    name: String,
    now_ms: u64,
) -> Result<Playlist, String> {
    let name = validated_name(name)?;
    let playlist = file
        .playlists
        .iter_mut()
        .find(|playlist| playlist.id == id)
        .ok_or_else(|| "playlist not found".to_string())?;
    playlist.name = name;
    playlist.updated_ms = now_ms;
    Ok(playlist.clone())
}

fn delete_in(file: &mut PlaylistFile, id: &str) -> Result<(), String> {
    let before = file.playlists.len();
    file.playlists.retain(|playlist| playlist.id != id);
    if file.playlists.len() == before {
        return Err("playlist not found".to_string());
    }
    Ok(())
}

fn validate_item(item: &ItemDto) -> Result<(), String> {
    if item.rating_key.split_once(':').is_none() || item.source_id.trim().is_empty() {
        return Err("playlist items need a routable source key".to_string());
    }
    if matches!(item.media_type.as_deref(), Some("show" | "season")) {
        return Err("only playable items can be added to a playlist".to_string());
    }
    Ok(())
}

fn add_items_in(
    file: &mut PlaylistFile,
    id: &str,
    entries: Vec<PlaylistEntry>,
    now_ms: u64,
) -> Result<Playlist, String> {
    if entries.is_empty() {
        return Err("choose at least one item".to_string());
    }
    for entry in &entries {
        validate_item(&entry.item)?;
    }
    let playlist = file
        .playlists
        .iter_mut()
        .find(|playlist| playlist.id == id)
        .ok_or_else(|| "playlist not found".to_string())?;
    playlist.items.extend(entries);
    playlist.updated_ms = now_ms;
    Ok(playlist.clone())
}

fn remove_item_in(
    file: &mut PlaylistFile,
    id: &str,
    entry_id: &str,
    now_ms: u64,
) -> Result<Playlist, String> {
    let playlist = file
        .playlists
        .iter_mut()
        .find(|playlist| playlist.id == id)
        .ok_or_else(|| "playlist not found".to_string())?;
    let before = playlist.items.len();
    playlist.items.retain(|entry| entry.id != entry_id);
    if playlist.items.len() == before {
        return Err("playlist item not found".to_string());
    }
    playlist.updated_ms = now_ms;
    Ok(playlist.clone())
}

fn reorder_in(
    file: &mut PlaylistFile,
    id: &str,
    entry_id: &str,
    to_index: usize,
    now_ms: u64,
) -> Result<Playlist, String> {
    let playlist = file
        .playlists
        .iter_mut()
        .find(|playlist| playlist.id == id)
        .ok_or_else(|| "playlist not found".to_string())?;
    let from_index = playlist
        .items
        .iter()
        .position(|entry| entry.id == entry_id)
        .ok_or_else(|| "playlist item not found".to_string())?;
    if to_index >= playlist.items.len() {
        return Err("playlist position is out of range".to_string());
    }
    if from_index != to_index {
        let entry = playlist.items.remove(from_index);
        playlist.items.insert(to_index, entry);
        playlist.updated_ms = now_ms;
    }
    Ok(playlist.clone())
}

pub fn create(name: String, now_ms: u64) -> Result<Playlist, String> {
    let id = uuid::Uuid::new_v4().to_string();
    update(move |file| create_in(file, id, name, now_ms))
}

pub fn rename(id: String, name: String, now_ms: u64) -> Result<Playlist, String> {
    update(move |file| rename_in(file, &id, name, now_ms))
}

pub fn delete(id: String) -> Result<(), String> {
    update(move |file| delete_in(file, &id))
}

pub fn add_items(
    id: String,
    items: Vec<(ItemDto, Option<String>)>,
    now_ms: u64,
) -> Result<Playlist, String> {
    let entries = items
        .into_iter()
        .map(|(item, source_name)| PlaylistEntry {
            id: uuid::Uuid::new_v4().to_string(),
            item,
            source_name,
        })
        .collect();
    update(move |file| add_items_in(file, &id, entries, now_ms))
}

pub fn remove_item(id: String, entry_id: String, now_ms: u64) -> Result<Playlist, String> {
    update(move |file| remove_item_in(file, &id, &entry_id, now_ms))
}

pub fn reorder(
    id: String,
    entry_id: String,
    to_index: usize,
    now_ms: u64,
) -> Result<Playlist, String> {
    update(move |file| reorder_in(file, &id, &entry_id, to_index, now_ms))
}

pub fn view(playlist: Playlist, live_sources: &HashSet<String>) -> PlaylistView {
    PlaylistView {
        id: playlist.id,
        name: playlist.name,
        items: playlist
            .items
            .into_iter()
            .map(|entry| {
                let key_source = entry.item.rating_key.split_once(':');
                let available = key_source.is_some_and(|(source, raw)| {
                    !raw.is_empty()
                        && source == entry.item.source_id
                        && live_sources.contains(source)
                });
                PlaylistEntryView {
                    id: entry.id,
                    item: entry.item,
                    source_name: entry.source_name,
                    available,
                }
            })
            .collect(),
        created_ms: playlist.created_ms,
        updated_ms: playlist.updated_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::ItemDto;
    use std::fs;

    fn item(source: &str, key: &str, title: &str) -> ItemDto {
        ItemDto {
            rating_key: format!("{source}:{key}"),
            title: title.to_string(),
            year: None,
            summary: None,
            duration_ms: Some(60_000),
            media_type: Some("movie".to_string()),
            poster: None,
            series_poster: None,
            backdrop: None,
            view_offset_ms: None,
            played: Some(false),
            last_watched_at_ms: None,
            added_at_ms: None,
            index: None,
            parent_index: None,
            grandparent_title: None,
            parent_title: None,
            parent_rating_key: None,
            grandparent_rating_key: None,
            source_id: source.to_string(),
            provider_ids: vec![],
            backing: None,
            canonical_id: None,
            watch_key: None,
            detail_key: None,
        }
    }

    fn temp_paths(label: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("vela-playlists-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        (
            root.join("playlists.json"),
            root.join("playlists.lock"),
            root,
        )
    }

    fn entry(id: &str, source: &str, key: &str, title: &str) -> PlaylistEntry {
        PlaylistEntry {
            id: id.to_string(),
            item: item(source, key, title),
            source_name: Some(source.to_uppercase()),
        }
    }

    #[test]
    fn ordinary_playlist_write_removes_legacy_plex_artwork_tokens() {
        let (data, lock, root) = temp_paths("artwork-sanitize");
        let token = "synthetic-playlist-artwork-token";
        let mut saved_entry = entry("one", "plex-a", "1", "Legacy");
        saved_entry.item.poster = Some(format!(
            "https://plex.example/photo/:/transcode?width=300&height=450&\
             url=%2Flibrary%2Fmetadata%2F1%2Fthumb%2F2&X-Plex-Token={token}"
        ));
        crate::storage::save_json(
            &data,
            &PlaylistFile {
                playlists: vec![Playlist {
                    id: "p1".to_string(),
                    name: "Legacy".to_string(),
                    items: vec![saved_entry],
                    created_ms: 1,
                    updated_ms: 1,
                }],
                ..Default::default()
            },
        )
        .unwrap();

        update_at(&data, &lock, |_| Ok(())).unwrap();
        let saved = fs::read_to_string(&data).unwrap();
        assert!(!saved.contains(token));
        assert!(saved.contains(crate::artwork::ARTWORK_MARKER_PREFIX));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn crud_uses_stable_entries_allows_duplicates_and_checks_bounds() {
        let mut file = PlaylistFile::default();
        let created = create_in(&mut file, "p1".into(), "  Voyage  ".into(), 1).unwrap();
        assert_eq!(created.name, "Voyage");
        assert_eq!(created.created_ms, 1);

        let renamed = rename_in(&mut file, "p1", "New name".into(), 2).unwrap();
        assert_eq!(renamed.name, "New name");
        assert_eq!(renamed.updated_ms, 2);

        let added = add_items_in(
            &mut file,
            "p1",
            vec![
                entry("one", "a", "1", "One"),
                entry("two", "b", "2", "Two"),
                entry("again", "a", "1", "One"),
            ],
            3,
        )
        .unwrap();
        assert_eq!(added.items.len(), 3);
        assert_eq!(
            added.items[0].item.rating_key,
            added.items[2].item.rating_key
        );
        assert_ne!(added.items[0].id, added.items[2].id);

        let moved = reorder_in(&mut file, "p1", "again", 1, 4).unwrap();
        assert_eq!(
            moved
                .items
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec!["one", "again", "two"]
        );
        let moved_back = reorder_in(&mut file, "p1", "again", 0, 5).unwrap();
        assert_eq!(
            moved_back
                .items
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec!["again", "one", "two"]
        );

        let before = serde_json::to_vec(&file).unwrap();
        assert!(reorder_in(&mut file, "p1", "again", 3, 6).is_err());
        assert_eq!(serde_json::to_vec(&file).unwrap(), before);

        let removed = remove_item_in(&mut file, "p1", "one", 7).unwrap();
        assert_eq!(
            removed
                .items
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec!["again", "two"]
        );
        assert!(remove_item_in(&mut file, "p1", "missing", 8).is_err());
        delete_in(&mut file, "p1").unwrap();
        assert!(file.playlists.is_empty());
        assert!(delete_in(&mut file, "p1").is_err());
    }

    #[test]
    fn invalid_names_and_unplayable_items_do_not_mutate_the_store() {
        let mut file = PlaylistFile::default();
        assert!(create_in(&mut file, "p".into(), "   ".into(), 1).is_err());
        assert!(file.playlists.is_empty());
        create_in(&mut file, "p".into(), "Good".into(), 1).unwrap();

        let before = serde_json::to_vec(&file).unwrap();
        let mut show = entry("show", "a", "show", "Show");
        show.item.media_type = Some("show".to_string());
        assert!(add_items_in(&mut file, "p", vec![show], 2).is_err());
        assert_eq!(serde_json::to_vec(&file).unwrap(), before);
        assert!(add_items_in(&mut file, "p", vec![], 2).is_err());
        assert_eq!(serde_json::to_vec(&file).unwrap(), before);
    }

    #[test]
    fn store_round_trips_entries_and_order() {
        let (data, lock, root) = temp_paths("crud");
        let created = update_at(&data, &lock, |file| {
            let playlist = Playlist {
                id: "p1".to_string(),
                name: "Draft".to_string(),
                items: vec![],
                created_ms: 1,
                updated_ms: 1,
            };
            file.playlists.push(playlist.clone());
            Ok(playlist)
        })
        .unwrap();
        assert_eq!(created.name, "Draft");

        update_at(&data, &lock, |file| {
            let playlist = &mut file.playlists[0];
            playlist.name = "Voyage".to_string();
            playlist.items = vec![
                PlaylistEntry {
                    id: "one".to_string(),
                    item: item("a", "1", "One"),
                    source_name: Some("A".to_string()),
                },
                PlaylistEntry {
                    id: "two".to_string(),
                    item: item("b", "2", "Two"),
                    source_name: Some("B".to_string()),
                },
                PlaylistEntry {
                    id: "again".to_string(),
                    item: item("a", "1", "One"),
                    source_name: Some("A".to_string()),
                },
            ];
            let moved = playlist.items.remove(2);
            playlist.items.insert(1, moved);
            playlist.items.retain(|entry| entry.id != "one");
            playlist.updated_ms = 2;
            Ok(())
        })
        .unwrap();

        let loaded = load_at(&data).unwrap();
        assert_eq!(loaded.playlists[0].name, "Voyage");
        assert_eq!(
            loaded.playlists[0]
                .items
                .iter()
                .map(|entry| entry.id.as_str())
                .collect::<Vec<_>>(),
            vec!["again", "two"]
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unavailable_entries_are_retained_and_marked_in_place() {
        let playlist = Playlist {
            id: "p".to_string(),
            name: "Mixed".to_string(),
            items: vec![
                PlaylistEntry {
                    id: "live".to_string(),
                    item: item("live", "1", "Live"),
                    source_name: None,
                },
                PlaylistEntry {
                    id: "gone".to_string(),
                    item: item("gone", "2", "Gone"),
                    source_name: Some("Removed server".to_string()),
                },
            ],
            created_ms: 1,
            updated_ms: 1,
        };
        let view = view(playlist, &HashSet::from(["live".to_string()]));
        assert_eq!(view.items.len(), 2, "curated entries are never filtered");
        assert!(view.items[0].available);
        assert!(!view.items[1].available);
        assert_eq!(view.items[1].item.title, "Gone");
    }

    #[test]
    fn malformed_or_future_store_fails_closed_without_rewriting() {
        let (data, lock, root) = temp_paths("fail-closed");
        fs::write(&data, b"{not json").unwrap();
        let before = fs::read(&data).unwrap();
        assert!(update_at(&data, &lock, |_| Ok(())).is_err());
        assert_eq!(fs::read(&data).unwrap(), before);

        fs::write(&data, br#"{"schemaVersion":2,"playlists":[]}"#).unwrap();
        let before = fs::read(&data).unwrap();
        assert!(update_at(&data, &lock, |_| Ok(())).is_err());
        assert_eq!(fs::read(&data).unwrap(), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn read_only_playback_snapshot_leaves_store_byte_identical() {
        let (data, lock, root) = temp_paths("read-only");
        update_at(&data, &lock, |file| {
            file.playlists.push(Playlist {
                id: "p".to_string(),
                name: "Untouched".to_string(),
                items: vec![],
                created_ms: 1,
                updated_ms: 1,
            });
            Ok(())
        })
        .unwrap();
        let before = fs::read(&data).unwrap();
        let loaded = load_at(&data).unwrap();
        assert_eq!(loaded.playlists[0].name, "Untouched");
        assert_eq!(fs::read(&data).unwrap(), before);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&data).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
