use super::types::ResolverStore;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn resolver_path(config_dir: &Path) -> PathBuf {
    config_dir.join("resolver.json")
}

pub fn load(path: &Path) -> Result<ResolverStore> {
    if !path.exists() {
        return Ok(ResolverStore::default());
    }
    let data = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read resolver store '{}'", path.display()))?;
    serde_json::from_str(&data)
        .with_context(|| format!("Invalid resolver store at '{}'", path.display()))
}

/// Persist the store using a temp-file + rename for atomicity.
pub fn save(path: &Path, store: &ResolverStore) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create directory '{}'", parent.display())
            })?;
        }
    }
    let data =
        serde_json::to_string_pretty(store).context("Failed to serialize resolver store")?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &data)
        .with_context(|| format!("Failed to write resolver store to '{}'", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .with_context(|| format!("Failed to commit resolver store to '{}'", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::types::{AliasEntry, ProviderEntry};

    #[test]
    fn round_trip_empty_store() {
        let dir = std::env::temp_dir().join(format!(
            "fio-resolver-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = resolver_path(&dir);
        let store = ResolverStore::default();
        save(&path, &store).unwrap();
        let loaded = load(&path).unwrap();
        assert!(loaded.providers.is_empty());
        assert!(loaded.actions.is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn round_trip_with_data() {
        let dir = std::env::temp_dir().join(format!(
            "fio-resolver-test-data-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = resolver_path(&dir);

        let mut store = ResolverStore::default();
        store.providers.insert(
            "linear".to_string(),
            ProviderEntry::new(vec!["linear".to_string(), "ln".to_string()]),
        );
        store.actions.insert(
            "create_tickets".to_string(),
            AliasEntry::new(vec!["create tickets".to_string()]),
        );

        save(&path, &store).unwrap();
        let loaded = load(&path).unwrap();

        assert!(loaded.providers.contains_key("linear"));
        let prov = &loaded.providers["linear"];
        assert!(prov.alias.aliases.contains(&"ln".to_string()));
        assert!(loaded.actions.contains_key("create_tickets"));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_missing_file_returns_default() {
        let path = std::env::temp_dir().join("fio-resolver-nonexistent-xyz.json");
        let store = load(&path).unwrap();
        assert!(store.providers.is_empty());
    }

    #[test]
    fn load_corrupt_file_returns_error() {
        let dir = std::env::temp_dir().join(format!(
            "fio-resolver-corrupt-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = resolver_path(&dir);
        std::fs::write(&path, "{not-json").unwrap();
        let err = load(&path).unwrap_err().to_string();
        assert!(
            err.contains("Invalid resolver store"),
            "unexpected error: {err}"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
