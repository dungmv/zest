use super::fs_store::FsWorkspaceStore;
use crate::models::HttpMethod;
use crate::storage::fingerprints::DiskFingerprints;
use crate::storage::fs_ops::{
    create_group, create_request, delete_group, delete_request, move_group, move_request,
    path_to_forward_slash, rename_group, rename_request, reorder_folder,
};
use crate::storage::models::{
    EnvironmentFile, FolderFile, LocFile, LocalState, PinsFile, RequestAuth, RequestFile,
    RequestNode, WorkspaceMeta, WorkspaceTree,
};
use crate::storage::serialize::{to_yaml_string, with_retry};
use crate::storage::slug::loc_path_for_yaml;
use crate::storage::traits::WorkspaceStore;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// The `WorkspaceStore` implementation for [`FsWorkspaceStore`], kept in its
/// own module so each storage file stays under the 650 LOC budget.
impl WorkspaceStore for FsWorkspaceStore {
    fn workspace_root(&self) -> &Path {
        &self.root_dir
    }

    fn init_workspace(&self, name: &str) -> Result<()> {
        std::fs::create_dir_all(&self.root_dir)?;
        std::fs::create_dir_all(self.collections_dir())?;
        std::fs::create_dir_all(self.environments_dir())?;
        std::fs::create_dir_all(self.local_dir())?;
        std::fs::create_dir_all(self.secrets_dir())?;

        // 1. workspace.yaml
        let meta = WorkspaceMeta {
            version: 1,
            name: name.to_string(),
            headers: Vec::new(),
            auth: RequestAuth::None,
        };
        self.save_workspace_meta(&meta)?;

        // 2. .gitignore
        self.ensure_gitignore()?;

        // 3. environments/local.yaml
        let local_env = EnvironmentFile::default();
        self.save_environment("local", &local_env)?;

        Ok(())
    }

    fn ensure_gitignore(&self) -> Result<bool> {
        let gitignore_path = self.root_dir.join(".gitignore");
        let mut content = if gitignore_path.exists() {
            std::fs::read_to_string(&gitignore_path).unwrap_or_default()
        } else {
            String::new()
        };

        let has_local = content
            .lines()
            .any(|l| l.trim() == ".local/" || l.trim() == ".local");
        let has_loc = content.lines().any(|l| l.trim() == "*.loc");

        let mut modified = false;
        if !has_local {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str(".local/\n");
            modified = true;
        }

        if !has_loc {
            if !content.is_empty() && !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str("*.loc\n");
            modified = true;
        }

        if modified || !gitignore_path.exists() {
            self.writer.write_str(&gitignore_path, &content)?;
        }

        // Check if git repo detected and .local or *.loc is tracked
        let git_dir = self.root_dir.join(".git");
        let mut tracked_leak = false;
        if git_dir.exists() {
            if let Ok(output) =
                crate::git::run_git_cmd(&self.root_dir, &["ls-files", ".local", "*.loc"])
            {
                if !output.trim().is_empty() {
                    tracked_leak = true;
                }
            }
        }

        Ok(tracked_leak)
    }

    fn load_workspace_meta(&self) -> Result<WorkspaceMeta> {
        let path = self.root_dir.join("workspace.yaml");
        if !path.exists() {
            return Ok(WorkspaceMeta::default());
        }
        let raw = std::fs::read_to_string(&path)?;
        self.writer.record_read(&path, raw.as_bytes());
        let meta: WorkspaceMeta = serde_yaml::from_str(&raw)?;
        Ok(meta)
    }

    fn save_workspace_meta(&self, meta: &WorkspaceMeta) -> Result<()> {
        let path = self.root_dir.join("workspace.yaml");
        let yaml = to_yaml_string(meta)?;
        self.writer.write_str(&path, &yaml)?;
        Ok(())
    }

    fn scan_tree(&self) -> Result<WorkspaceTree> {
        let coll_dir = self.collections_dir();
        if !coll_dir.exists() {
            std::fs::create_dir_all(&coll_dir)?;
            return Ok(WorkspaceTree::default());
        }

        let mut root_requests = Vec::new();
        let mut folders = Vec::new();
        let mut loc_files = Vec::new();
        let mut req_map: HashMap<String, RequestNode> = HashMap::new();

        if let Ok(entries) = std::fs::read_dir(&coll_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let folder_name = entry.file_name().to_string_lossy().to_string();
                    if !folder_name.starts_with('.') {
                        if let Ok(Some(folder_node)) = self.scan_folder(&path, None) {
                            folders.push(folder_node);
                        }
                    }
                    continue;
                }

                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if ext == "loc" {
                    loc_files.push(path);
                    continue;
                }

                if (ext != "yaml" && ext != "yml")
                    || file_name == "_folder.yaml"
                    || file_name.starts_with('.')
                {
                    continue;
                }

                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("request")
                    .to_string();

                let rel_path = path
                    .strip_prefix(&coll_dir)
                    .map(path_to_forward_slash)
                    .unwrap_or_else(|_| path_to_forward_slash(&path));

                let loc_path = loc_path_for_yaml(&path);
                let last_status = if loc_path.exists() {
                    std::fs::read_to_string(&loc_path)
                        .ok()
                        .and_then(|c| serde_json::from_str::<LocFile>(&c).ok())
                        .map(|l| l.response.status)
                } else {
                    None
                };

                let raw = std::fs::read_to_string(&path).unwrap_or_default();
                self.writer.record_read(&path, raw.as_bytes());
                let (req_file, parse_error) = match serde_yaml::from_str::<RequestFile>(&raw) {
                    Ok(f) => (f, None),
                    Err(e) => (
                        RequestFile {
                            name: stem.clone(),
                            ..Default::default()
                        },
                        Some(e.to_string()),
                    ),
                };

                req_map.insert(
                    stem.clone(),
                    RequestNode {
                        slug: stem,
                        rel_path,
                        file: req_file,
                        last_status_code: last_status,
                        parse_error,
                    },
                );
            }
        }

        // Clean up orphan locs in root collections
        for loc in loc_files {
            let stem = loc.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if !req_map.contains_key(stem) {
                let _ = std::fs::remove_file(loc);
            }
        }

        let mut root_stems: Vec<String> = req_map.keys().cloned().collect();
        root_stems.sort();
        root_requests.reserve(root_stems.len());
        for s in root_stems {
            if let Some(r) = req_map.remove(&s) {
                root_requests.push(r);
            }
        }

        folders.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));

        Ok(WorkspaceTree {
            root_requests,
            folders,
        })
    }

    fn load_folder_meta(&self, folder_rel: &str) -> Result<FolderFile> {
        let meta_file = self.collections_dir().join(folder_rel).join("_folder.yaml");
        if !meta_file.exists() {
            return Ok(FolderFile {
                name: folder_rel.to_string(),
                order: Vec::new(),
                headers: Vec::new(),
                auth: RequestAuth::Inherit,
            });
        }
        let raw = std::fs::read_to_string(&meta_file)?;
        self.writer.record_read(&meta_file, raw.as_bytes());
        let file = serde_yaml::from_str(&raw)?;
        Ok(file)
    }

    fn save_folder_meta(&self, folder_rel: &str, meta: &FolderFile) -> Result<()> {
        let dir = self.collections_dir().join(folder_rel);
        std::fs::create_dir_all(&dir)?;
        let yaml = to_yaml_string(meta)?;
        self.writer.write_str(&dir.join("_folder.yaml"), &yaml)?;
        Ok(())
    }

    fn read_request(&self, rel_path: &str) -> Result<RequestFile> {
        let path = self.collections_dir().join(rel_path);
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read request file at {:?}", path))?;
        self.writer.record_read(&path, raw.as_bytes());
        let req: RequestFile = serde_yaml::from_str(&raw)?;
        Ok(req)
    }

    fn write_request(&self, rel_path: &str, req: &RequestFile) -> Result<()> {
        let path = self.collections_dir().join(rel_path);
        let yaml = to_yaml_string(req)?;
        self.writer.write_str(&path, &yaml)?;
        Ok(())
    }

    fn create_request(
        &self,
        folder_rel: Option<&str>,
        name: &str,
        method: HttpMethod,
        url: &str,
    ) -> Result<(String, String)> {
        let req = RequestFile {
            name: name.to_string(),
            method,
            url: url.to_string(),
            ..Default::default()
        };
        let (slug, path) =
            create_request(&self.writer, &self.collections_dir(), folder_rel, name, req)?;
        Ok((slug, path_to_forward_slash(&path)))
    }

    fn delete_request(&self, rel_path: &str) -> Result<()> {
        delete_request(&self.writer, &self.collections_dir(), Path::new(rel_path))
    }

    fn rename_request(&self, rel_path: &str, new_name: &str) -> Result<String> {
        let new_path = rename_request(
            &self.writer,
            &self.collections_dir(),
            Path::new(rel_path),
            new_name,
        )?;
        Ok(path_to_forward_slash(&new_path))
    }

    fn move_request(&self, rel_path: &str, target_folder_rel: Option<&str>) -> Result<String> {
        let new_path = move_request(
            &self.writer,
            &self.collections_dir(),
            Path::new(rel_path),
            target_folder_rel,
        )?;
        Ok(path_to_forward_slash(&new_path))
    }

    fn create_group(
        &self,
        parent_folder_rel: Option<&str>,
        name: &str,
    ) -> Result<(String, String)> {
        let (dir_name, path) = create_group(
            &self.writer,
            &self.collections_dir(),
            parent_folder_rel,
            name,
        )?;
        Ok((dir_name, path_to_forward_slash(&path)))
    }

    fn delete_group(&self, folder_rel: &str) -> Result<()> {
        delete_group(&self.collections_dir(), folder_rel)
    }

    fn rename_group(&self, folder_rel: &str, new_name: &str) -> Result<String> {
        let new_path = rename_group(&self.writer, &self.collections_dir(), folder_rel, new_name)?;
        Ok(path_to_forward_slash(&new_path))
    }

    fn move_group(&self, folder_rel: &str, target_parent_rel: Option<&str>) -> Result<String> {
        let new_path = move_group(&self.collections_dir(), folder_rel, target_parent_rel)?;
        Ok(path_to_forward_slash(&new_path))
    }

    fn reorder_folder(&self, folder_rel: Option<&str>, order: &[String]) -> Result<()> {
        reorder_folder(&self.writer, &self.collections_dir(), folder_rel, order)
    }

    fn list_environments(&self) -> Result<Vec<String>> {
        let env_dir = self.environments_dir();
        if !env_dir.exists() {
            return Ok(Vec::new());
        }

        let mut list = Vec::new();
        for entry in std::fs::read_dir(env_dir)?.flatten() {
            let path = entry.path();
            if path.is_file() {
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
                if ext == "yaml" || ext == "yml" {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if stem != "globals" && !stem.starts_with('.') {
                            list.push(stem.to_string());
                        }
                    }
                }
            }
        }
        list.sort();
        Ok(list)
    }

    fn load_environment(&self, name: &str) -> Result<EnvironmentFile> {
        let path = self.environments_dir().join(format!("{}.yaml", name));
        if !path.exists() {
            return Ok(EnvironmentFile::default());
        }
        let raw = std::fs::read_to_string(&path)?;
        self.writer.record_read(&path, raw.as_bytes());
        let mut env: EnvironmentFile = serde_yaml::from_str(&raw)?;

        // Inject secrets from .local/secrets/<name>.yaml
        let secrets = self.load_secrets(name).unwrap_or_default();
        for v in &mut env.variables {
            if v.secret {
                if let Some(secret_val) = secrets.get(&v.name) {
                    v.value = Some(secret_val.clone());
                }
            }
        }

        Ok(env)
    }

    fn save_environment(&self, name: &str, env: &EnvironmentFile) -> Result<()> {
        std::fs::create_dir_all(self.environments_dir())?;
        std::fs::create_dir_all(self.secrets_dir())?;

        let mut committed_env = env.clone();
        let mut secrets = HashMap::new();

        // Secret split:
        // variables with secret: true have value: None in committed file,
        // and secret value is written only to .local/secrets/<name>.yaml
        for v in &mut committed_env.variables {
            if v.secret {
                if let Some(val) = v.value.take() {
                    secrets.insert(v.name.clone(), val);
                }
            }
        }

        let path = self.environments_dir().join(format!("{}.yaml", name));
        let yaml = to_yaml_string(&committed_env)?;
        self.writer.write_str(&path, &yaml)?;

        self.save_secrets(name, &secrets)?;

        Ok(())
    }

    fn delete_environment(&self, name: &str) -> Result<()> {
        let path = self.environments_dir().join(format!("{}.yaml", name));
        let _ = self.writer.remove_file(&path);
        let secret_path = self.secrets_dir().join(format!("{}.yaml", name));
        let _ = self.writer.remove_file(&secret_path);
        Ok(())
    }

    fn rename_environment(&self, old_name: &str, new_name: &str) -> Result<()> {
        let old_path = self.environments_dir().join(format!("{}.yaml", old_name));
        let new_path = self.environments_dir().join(format!("{}.yaml", new_name));
        // Never silently overwrite an existing environment: `fs::rename` would
        // replace the sibling file on both Windows and Unix.
        let new_sec = self.secrets_dir().join(format!("{}.yaml", new_name));
        if new_path != old_path && (new_path.exists() || new_sec.exists()) {
            anyhow::bail!("Environment '{}' already exists", new_name);
        }
        if old_path.exists() {
            self.writer.rename_file(&old_path, &new_path)?;
        }

        let old_sec = self.secrets_dir().join(format!("{}.yaml", old_name));
        if old_sec.exists() {
            let _ = self.writer.rename_file(&old_sec, &new_sec);
        }
        Ok(())
    }

    fn load_secrets(&self, env_name: &str) -> Result<HashMap<String, String>> {
        let path = self.secrets_dir().join(format!("{}.yaml", env_name));
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let raw = match std::fs::read_to_string(&path) {
            Ok(r) => r,
            Err(_) => return Ok(HashMap::new()),
        };

        match serde_yaml::from_str::<HashMap<String, String>>(&raw) {
            Ok(map) => Ok(map),
            Err(e) => {
                // Corrupt .local/secrets/*.yaml: rename to *.bak, user data that cannot be regenerated
                let bak_path = self.secrets_dir().join(format!("{}.yaml.bak", env_name));
                let _ = with_retry(|| std::fs::rename(&path, &bak_path));
                Err(anyhow::anyhow!(
                    "Corrupt secret file for environment '{}' ({}), backed up to {}.yaml.bak",
                    env_name,
                    e,
                    env_name
                ))
            }
        }
    }

    fn save_secrets(&self, env_name: &str, secrets: &HashMap<String, String>) -> Result<()> {
        std::fs::create_dir_all(self.secrets_dir())?;
        let path = self.secrets_dir().join(format!("{}.yaml", env_name));
        let yaml = to_yaml_string(secrets)?;
        self.writer.write_str(&path, &yaml)?;
        Ok(())
    }

    fn load_response_cache(&self, rel_path: &str) -> Result<Option<LocFile>> {
        let yaml_path = self.collections_dir().join(rel_path);
        let loc_path = loc_path_for_yaml(&yaml_path);
        if !loc_path.exists() {
            return Ok(None);
        }

        // Corrupt .loc: ignore silently
        let raw = match std::fs::read_to_string(&loc_path) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };
        let file = serde_json::from_str::<LocFile>(&raw).ok();
        Ok(file)
    }

    fn save_response_cache(&self, rel_path: &str, cache: &LocFile) -> Result<()> {
        let yaml_path = self.collections_dir().join(rel_path);
        let loc_path = loc_path_for_yaml(&yaml_path);

        // Truncate body at 512KB with truncated: true; binary bodies store metadata only
        let mut final_cache = cache.clone();
        if final_cache.response.binary {
            final_cache.response.body = None;
        } else if let Some(body) = &final_cache.response.body {
            const MAX_BODY_BYTES: usize = 512 * 1024;
            if body.len() > MAX_BODY_BYTES {
                let truncated_body = body[..MAX_BODY_BYTES].to_string();
                final_cache.response.body = Some(truncated_body);
                final_cache.response.truncated = true;
            }
        }

        if let Ok(json) = serde_json::to_string_pretty(&final_cache) {
            let _ = self.writer.write_str(&loc_path, &json);
        }
        Ok(())
    }

    fn load_pins(&self) -> Result<Vec<String>> {
        let path = self.local_dir().join("pins.yaml");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = match std::fs::read_to_string(&path) {
            Ok(r) => r,
            Err(_) => return Ok(Vec::new()),
        };
        // Corrupt pins.yaml: ignore and use defaults silently
        let file = serde_yaml::from_str::<PinsFile>(&raw).unwrap_or_default();
        Ok(file.pins)
    }

    fn save_pins(&self, pins: &[String]) -> Result<()> {
        std::fs::create_dir_all(self.local_dir())?;
        let path = self.local_dir().join("pins.yaml");
        let file = PinsFile {
            pins: pins.to_vec(),
        };
        let yaml = to_yaml_string(&file)?;
        self.writer.write_str(&path, &yaml)?;
        Ok(())
    }

    fn load_state(&self) -> Result<LocalState> {
        let path = self.local_dir().join("state.yaml");
        if !path.exists() {
            return Ok(LocalState::default());
        }
        let raw = match std::fs::read_to_string(&path) {
            Ok(r) => r,
            Err(_) => return Ok(LocalState::default()),
        };
        // Corrupt state.yaml: ignore and use defaults silently
        let state = serde_yaml::from_str::<LocalState>(&raw).unwrap_or_default();
        Ok(state)
    }

    fn save_state(&self, state: &LocalState) -> Result<()> {
        std::fs::create_dir_all(self.local_dir())?;
        let path = self.local_dir().join("state.yaml");
        let yaml = to_yaml_string(state)?;
        self.writer.write_str(&path, &yaml)?;
        Ok(())
    }

    fn load_cookies(&self) -> Result<serde_json::Value> {
        let path = self.local_dir().join("cookies.json");
        if !path.exists() {
            return Ok(serde_json::json!([]));
        }
        let raw = match std::fs::read_to_string(&path) {
            Ok(r) => r,
            Err(_) => return Ok(serde_json::json!([])),
        };
        // Corrupt cookies.json: ignore and use defaults silently
        let val = serde_json::from_str::<serde_json::Value>(&raw).unwrap_or(serde_json::json!([]));
        Ok(val)
    }

    fn save_cookies(&self, cookies: &serde_json::Value) -> Result<()> {
        std::fs::create_dir_all(self.local_dir())?;
        let path = self.local_dir().join("cookies.json");
        let json = serde_json::to_string_pretty(cookies)?;
        self.writer.write_str(&path, &json)?;
        Ok(())
    }

    fn disk_fingerprints(&self) -> Arc<DiskFingerprints> {
        self.fingerprints.clone()
    }
}
