use crate::config::SourceConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static CONNECTIONS_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ConnectionsConfig {
    pub(crate) sources: Vec<SourceConfig>,
}

impl ConnectionsConfig {
    pub(crate) fn validate(&self) -> Result<(), String> {
        let mut ids = HashSet::with_capacity(self.sources.len());
        for source in &self.sources {
            validate_source(source)?;
            if !ids.insert(source.id.as_str()) {
                return Err("duplicate connection id".to_string());
            }
        }
        Ok(())
    }

    pub(crate) fn upsert(&mut self, source: SourceConfig) -> Result<(), String> {
        validate_source(&source)?;
        self.sources.retain(|held| held.id != source.id);
        self.sources.push(source);
        self.validate()
    }
}

pub(crate) fn validate_source(source: &SourceConfig) -> Result<(), String> {
    if source.id.trim().is_empty() || source.id.contains(':') {
        return Err("invalid connection id".to_string());
    }
    if source.name.trim().is_empty() {
        return Err("invalid connection name".to_string());
    }

    let nonempty = |value: &Option<String>| {
        value
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    };
    match source.kind.as_str() {
        "plex" => {
            if !nonempty(&source.access_token)
                || !nonempty(&source.device_id)
                || source.api_key.is_some()
                || source.user_id.is_some()
            {
                return Err("invalid Plex connection".to_string());
            }
            if source
                .machine_identifier
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            {
                return Err("invalid Plex server identity".to_string());
            }
            if source.machine_identifier.is_some() {
                let endpoint = url::Url::parse(&source.base_url)
                    .map_err(|_| "invalid pinned Plex endpoint".to_string())?;
                if endpoint.scheme() != "https"
                    || endpoint.host_str().is_none()
                    || endpoint.port_or_known_default().is_none()
                {
                    return Err("invalid pinned Plex endpoint".to_string());
                }
            }
        }
        "jellyfin" | "emby" => {
            if nonempty(&source.access_token) == nonempty(&source.api_key)
                || !nonempty(&source.user_id)
                || !nonempty(&source.device_id)
                || source.machine_identifier.is_some()
            {
                return Err("invalid Jellyfin/Emby connection".to_string());
            }
            let endpoint = url::Url::parse(&source.base_url)
                .map_err(|_| "invalid Jellyfin/Emby endpoint".to_string())?;
            if !matches!(endpoint.scheme(), "http" | "https") || endpoint.host_str().is_none() {
                return Err("invalid Jellyfin/Emby endpoint".to_string());
            }
        }
        _ => return Err("unknown connection kind".to_string()),
    }
    Ok(())
}

fn connections_path() -> io::Result<PathBuf> {
    crate::config::config_dir_file("connections.json")
}

pub(crate) fn load_at(path: &Path) -> io::Result<ConnectionsConfig> {
    let connections: ConnectionsConfig = crate::storage::load_json(path)?;
    connections
        .validate()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid connections document"))?;
    Ok(connections)
}

pub(crate) fn update<T, F>(mutate: F) -> Result<T, String>
where
    F: FnOnce(&mut ConnectionsConfig) -> Result<T, String>,
{
    crate::durable::ensure_commands_ready()?;
    let result = update_internal(mutate);
    if result.is_err() {
        if let Ok(path) = connections_path() {
            if let Err(error) = load_at(&path) {
                crate::durable::report_connections_fault(&error);
            }
        }
    }
    result
}

pub(crate) fn update_internal<T, F>(mutate: F) -> Result<T, String>
where
    F: FnOnce(&mut ConnectionsConfig) -> Result<T, String>,
{
    let path = connections_path().map_err(|error| format!("connections unavailable: {error}"))?;
    let lock_path = crate::config::config_dir_file("connections.lock")
        .map_err(|error| format!("connections lock unavailable: {error}"))?;
    update_at(&path, &lock_path, mutate)
}

pub(crate) fn update_at<T, F>(path: &Path, lock_path: &Path, mutate: F) -> Result<T, String>
where
    F: FnOnce(&mut ConnectionsConfig) -> Result<T, String>,
{
    crate::storage::update_json(
        "connections",
        &CONNECTIONS_LOCK,
        path,
        lock_path,
        |connections: &mut ConnectionsConfig| {
            connections.validate()?;
            let output = mutate(connections)?;
            connections.validate()?;
            Ok(output)
        },
    )
}

pub(crate) fn lock_process() -> std::sync::MutexGuard<'static, ()> {
    CONNECTIONS_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn source(id: &str) -> SourceConfig {
        SourceConfig {
            id: id.to_string(),
            kind: "jellyfin".to_string(),
            name: "Test".to_string(),
            base_url: "http://127.0.0.1:8096".to_string(),
            access_token: Some("synthetic-token".to_string()),
            api_key: None,
            user_id: Some("synthetic-user".to_string()),
            device_id: Some("synthetic-device".to_string()),
            machine_identifier: None,
        }
    }

    fn plex_source(id: &str) -> SourceConfig {
        SourceConfig {
            id: id.to_string(),
            kind: "plex".to_string(),
            name: "Plex".to_string(),
            base_url: "https://127.0.0.1:32400".to_string(),
            access_token: Some("synthetic-plex-token".to_string()),
            api_key: None,
            user_id: None,
            device_id: Some("synthetic-plex-device".to_string()),
            machine_identifier: Some("synthetic-machine".to_string()),
        }
    }

    #[test]
    fn missing_connections_are_empty_without_creating_the_file() {
        let root =
            std::env::temp_dir().join(format!("vela-connections-missing-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("connections.json");
        assert_eq!(load_at(&path).unwrap(), ConnectionsConfig::default());
        assert!(!path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unknown_keys_and_duplicate_ids_fail_the_whole_file() {
        let root =
            std::env::temp_dir().join(format!("vela-connections-invalid-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("connections.json");
        fs::write(
            &path,
            r#"{"sources":[{"id":"jf","kind":"jellyfin","name":"Test","base_url":"http://localhost","access_token":"synthetic","user_id":"u","device_id":"d","future":true}]}"#,
        )
        .unwrap();
        assert_eq!(
            load_at(&path).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );

        let duplicate = ConnectionsConfig {
            sources: vec![source("same"), source("same")],
        };
        assert!(duplicate.validate().is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn secret_values_are_redacted_from_debug() {
        let text = format!("{:?}", source("jf"));
        assert!(!text.contains("synthetic-token"));
        assert!(!text.contains("synthetic-user"));
        assert!(!text.contains("synthetic-device"));
    }

    #[test]
    fn provider_requirements_reject_every_incomplete_or_inconsistent_row() {
        let mut invalid = Vec::new();

        let mut row = source(" ");
        invalid.push(row.clone());
        row.id = "bad:id".to_string();
        invalid.push(row.clone());
        row = source("jf");
        row.name = " ".to_string();
        invalid.push(row.clone());
        row = source("jf");
        row.kind = "future".to_string();
        invalid.push(row.clone());
        row = source("jf");
        row.access_token = None;
        invalid.push(row.clone());
        row = source("jf");
        row.api_key = Some("synthetic-second-token".to_string());
        invalid.push(row.clone());
        row = source("jf");
        row.user_id = None;
        invalid.push(row.clone());
        row = source("jf");
        row.device_id = None;
        invalid.push(row.clone());
        row = source("jf");
        row.machine_identifier = Some("not-for-jellyfin".to_string());
        invalid.push(row.clone());
        row = source("jf");
        row.base_url = "file:///tmp/media".to_string();
        invalid.push(row.clone());

        row = plex_source("plex");
        row.access_token = None;
        invalid.push(row.clone());
        row = plex_source("plex");
        row.device_id = None;
        invalid.push(row.clone());
        row = plex_source("plex");
        row.api_key = Some("synthetic-wrong-field".to_string());
        invalid.push(row.clone());
        row = plex_source("plex");
        row.user_id = Some("synthetic-wrong-user".to_string());
        invalid.push(row.clone());
        row = plex_source("plex");
        row.machine_identifier = Some(" ".to_string());
        invalid.push(row.clone());
        row = plex_source("plex");
        row.base_url = "http://127.0.0.1:32400".to_string();
        invalid.push(row);

        for row in invalid {
            assert!(validate_source(&row).is_err(), "{row:?} must be invalid");
        }
    }

    #[test]
    fn every_validated_provider_row_builds_without_network_access() {
        let jellyfin = source("jf");
        let plex = plex_source("plex");
        validate_source(&jellyfin).unwrap();
        validate_source(&plex).unwrap();
        crate::source::jellyfin::build_source(&jellyfin).unwrap();
        crate::source::plex::build_source(&plex).unwrap();
    }
}
