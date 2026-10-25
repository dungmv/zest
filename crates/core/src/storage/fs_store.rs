use crate::models::HttpMethod;
use crate::storage::fs_ops::{
    create_group, create_request, delete_group, delete_request, move_group, move_request,
    path_to_forward_slash, rename_group, rename_request, reorder_folder,
};
use crate::storage::models::{
    EnvironmentFile, FolderFile, FolderNode, LocFile, LocalState, PinsFile, RequestAuth,
    RequestFile, RequestNode, WorkspaceMeta, WorkspaceTree,
};
use crate::storage::serialize::{atomic_write_str, to_yaml_string, with_retry};
use crate::storage::slug::loc_path_for_yaml;
use crate::storage::traits::WorkspaceStore;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub struct FsWorkspaceStore {
    root_dir: PathBuf,
    suppressed_paths: Arc<Mutex<HashSet<PathBuf>>>,
    suppress_tx: std::sync::mpsc::Sender<PathBuf>,
}

impl FsWorkspaceStore {
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        let suppressed_paths = Arc::new(Mutex::new(HashSet::new()));
        let (suppress_tx, suppress_rx) = std::sync::mpsc::channel::<PathBuf>();
        let sup_worker = suppressed_paths.clone();

        std::thread::Builder::new()
            .name("workspace-suppress-cleaner".to_string())
            .spawn(move || {
                let mut deadlines: std::collections::VecDeque<(PathBuf, std::time::Instant)> =
                    std::collections::VecDeque::new();
                loop {
                    let now = std::time::Instant::now();
                    while let Some((_, deadline)) = deadlines.front() {
                        if *deadline <= now {
                            if let Some((expired_path, _)) = deadlines.pop_front() {
                                if !deadlines.iter().any(|(p, _)| p == &expired_path) {
                                    let mut set =
                                        sup_worker.lock().unwrap_or_else(|e| e.into_inner());
                                    set.remove(&expired_path);
                                }
                            }
                        } else {
                            break;
                        }
                    }

                    let timeout = if let Some((_, earliest)) = deadlines.front() {
                        earliest.saturating_duration_since(now)
                    } else {
                        std::time::Duration::from_secs(3600)
                    };

                    match suppress_rx.recv_timeout(timeout) {
                        Ok(path) => {
                            deadlines.push_back((
                                path,
                                std::time::Instant::now() + std::time::Duration::from_millis(1500),
                            ));
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            })
            .ok();

        Self {
            root_dir: root_dir.into(),
            suppressed_paths,
            suppress_tx,
        }
    }

    pub fn suppress_path(&self, path: &Path) {
        let mut set = self
            .suppressed_paths
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        set.insert(path.to_path_buf());
        let _ = self.suppress_tx.send(path.to_path_buf());
    }

    pub fn suppressed_paths(&self) -> Arc<Mutex<HashSet<PathBuf>>> {
        self.suppressed_paths.clone()
    }

    pub fn collections_dir(&self) -> PathBuf {
        self.root_dir.join("collections")
    }

    pub fn environments_dir(&self) -> PathBuf {
        self.root_dir.join("environments")
    }

    pub fn local_dir(&self) -> PathBuf {
        self.root_dir.join(".local")
    }

    pub fn secrets_dir(&self) -> PathBuf {
        self.local_dir().join("secrets")
    }

    fn scan_folder(
        &self,
        folder_path: &Path,
        rel_folder: Option<&str>,
    ) -> Result<Option<FolderNode>> {
        if !folder_path.exists() || !folder_path.is_dir() {
            return Ok(None);
        }

        let dir_name = folder_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("folder")
            .to_string();

        let folder_rel_str = match rel_folder {
            Some(parent) => format!("{}/{}", parent, dir_name),
            None => dir_name.clone(),
        };

        let meta_file = folder_path.join("_folder.yaml");
        let folder_meta = if meta_file.exists() {
            let raw = std::fs::read_to_string(&meta_file).unwrap_or_default();
            serde_yaml::from_str::<FolderFile>(&raw).unwrap_or_else(|_| FolderFile {
                name: dir_name.clone(),
                order: Vec::new(),
                headers: Vec::new(),
                auth: RequestAuth::Inherit,
            })
        } else {
            FolderFile {
                name: dir_name.clone(),
                order: Vec::new(),
                headers: Vec::new(),
                auth: RequestAuth::Inherit,
            }
        };

        let mut req_map: HashMap<String, RequestNode> = HashMap::new();
        let mut found_stems = Vec::new();
        let mut subfolder_nodes = Vec::new();
        let mut loc_files = Vec::new();

        if let Ok(entries) = std::fs::read_dir(folder_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let sub_name = entry.file_name().to_string_lossy().to_string();
                    if !sub_name.starts_with('.') {
                        if let Ok(Some(sub)) = self.scan_folder(&path, Some(&folder_rel_str)) {
                            subfolder_nodes.push(sub);
                        }
                    }
                    continue;
                }

                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                if ext == "loc" {
                    loc_files.push(path.clone());
                    continue;
                }

                // Scanner treats every *.yaml in collections/ except _folder.yaml as a request; ignores every other file.
                if ext != "yaml" && ext != "yml" || file_name == "_folder.yaml" {
                    continue;
                }

                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("request")
                    .to_string();

                let rel_req_path = path
                    .strip_prefix(self.collections_dir())
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

                let raw_content = std::fs::read_to_string(&path).unwrap_or_default();
                let (req_file, parse_error) =
                    match serde_yaml::from_str::<RequestFile>(&raw_content) {
                        Ok(f) => (f, None),
                        Err(e) => (
                            RequestFile {
                                name: stem.clone(),
                                ..Default::default()
                            },
                            Some(e.to_string()),
                        ),
                    };

                let node = RequestNode {
                    slug: stem.clone(),
                    rel_path: rel_req_path,
                    file: req_file,
                    last_status_code: last_status,
                    parse_error,
                };

                req_map.insert(stem.clone(), node);
                found_stems.push(stem);
            }
        }

        // Orphan .loc (no matching yaml) is ignored by the scanner and deleted lazily when its folder is scanned
        for loc in loc_files {
            let stem = loc.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if !req_map.contains_key(stem) {
                let _ = std::fs::remove_file(loc);
            }
        }

        // Order requests:
        // Follow _folder.yaml `order:`. Requests missing from it are appended alphabetically.
        // Entries pointing to missing files are ignored silently.
        let mut ordered_requests = Vec::with_capacity(req_map.len());
        let mut included_stems = std::collections::HashSet::with_capacity(folder_meta.order.len());

        for stem in &folder_meta.order {
            if let Some(req) = req_map.remove(stem) {
                ordered_requests.push(req);
                included_stems.insert(stem.clone());
            }
        }

        let mut remaining_stems: Vec<String> = req_map.keys().cloned().collect();
        remaining_stems.sort();
        for stem in remaining_stems {
            if let Some(req) = req_map.remove(&stem) {
                ordered_requests.push(req);
            }
        }

        subfolder_nodes.sort_by(|a, b| a.dir_name.cmp(&b.dir_name));

        Ok(Some(FolderNode {
            dir_name,
            rel_path: folder_rel_str,
            file: folder_meta,
            requests: ordered_requests,
            subfolders: subfolder_nodes,
        }))
    }
}

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
            default_environment: Some("local".to_string()),
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
            atomic_write_str(&gitignore_path, &content)?;
        }

        // Check if git repo detected and .local or *.loc is tracked
        let git_dir = self.root_dir.join(".git");
        let mut tracked_leak = false;
        if git_dir.exists() {
            let mut cmd = std::process::Command::new("git");
            cmd.arg("ls-files").arg(".local").arg("*.loc");
            cmd.current_dir(&self.root_dir);
            cmd.env("GIT_TERMINAL_PROMPT", "0");
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000);
            }
            if let Ok(output) = cmd.output() {
                if output.status.success() {
                    let text = String::from_utf8_lossy(&output.stdout);
                    if !text.trim().is_empty() {
                        tracked_leak = true;
                    }
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
        let meta: WorkspaceMeta = serde_yaml::from_str(&raw)?;
        Ok(meta)
    }

    fn save_workspace_meta(&self, meta: &WorkspaceMeta) -> Result<()> {
        let path = self.root_dir.join("workspace.yaml");
        let yaml = to_yaml_string(meta)?;
        atomic_write_str(&path, &yaml)?;
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

                if ext != "yaml" && ext != "yml" || file_name == "_folder.yaml" {
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
        let file = serde_yaml::from_str(&raw)?;
        Ok(file)
    }

    fn save_folder_meta(&self, folder_rel: &str, meta: &FolderFile) -> Result<()> {
        let dir = self.collections_dir().join(folder_rel);
        std::fs::create_dir_all(&dir)?;
        let yaml = to_yaml_string(meta)?;
        atomic_write_str(&dir.join("_folder.yaml"), &yaml)?;
        Ok(())
    }

    fn read_request(&self, rel_path: &str) -> Result<RequestFile> {
        let path = self.collections_dir().join(rel_path);
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read request file at {:?}", path))?;
        let req: RequestFile = serde_yaml::from_str(&raw)?;
        Ok(req)
    }

    fn write_request(&self, rel_path: &str, req: &RequestFile) -> Result<()> {
        let path = self.collections_dir().join(rel_path);
        self.suppress_path(&path);
        let yaml = to_yaml_string(req)?;
        atomic_write_str(&path, &yaml)?;
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
        let (slug, path) = create_request(&self.collections_dir(), folder_rel, name, req)?;
        self.suppress_path(&path);
        Ok((slug, path_to_forward_slash(&path)))
    }

    fn delete_request(&self, rel_path: &str) -> Result<()> {
        let path = self.collections_dir().join(rel_path);
        self.suppress_path(&path);
        delete_request(&self.collections_dir(), Path::new(rel_path))
    }

    fn rename_request(&self, rel_path: &str, new_name: &str) -> Result<String> {
        let old_p = self.collections_dir().join(rel_path);
        let old_loc = loc_path_for_yaml(&old_p);
        self.suppress_path(&old_p);
        self.suppress_path(&old_loc);
        let new_path = rename_request(&self.collections_dir(), Path::new(rel_path), new_name)?;
        let full_new_yaml = self.collections_dir().join(&new_path);
        let full_new_loc = loc_path_for_yaml(&full_new_yaml);
        self.suppress_path(&full_new_yaml);
        self.suppress_path(&full_new_loc);
        Ok(path_to_forward_slash(&new_path))
    }

    fn move_request(&self, rel_path: &str, target_folder_rel: Option<&str>) -> Result<String> {
        let old_p = self.collections_dir().join(rel_path);
        let old_loc = loc_path_for_yaml(&old_p);
        self.suppress_path(&old_p);
        self.suppress_path(&old_loc);
        let new_path = move_request(
            &self.collections_dir(),
            Path::new(rel_path),
            target_folder_rel,
        )?;
        let full_new_yaml = self.collections_dir().join(&new_path);
        let full_new_loc = loc_path_for_yaml(&full_new_yaml);
        self.suppress_path(&full_new_yaml);
        self.suppress_path(&full_new_loc);
        Ok(path_to_forward_slash(&new_path))
    }

    fn create_group(
        &self,
        parent_folder_rel: Option<&str>,
        name: &str,
    ) -> Result<(String, String)> {
        let (dir_name, path) = create_group(&self.collections_dir(), parent_folder_rel, name)?;
        Ok((dir_name, path_to_forward_slash(&path)))
    }

    fn delete_group(&self, folder_rel: &str) -> Result<()> {
        delete_group(&self.collections_dir(), folder_rel)
    }

    fn rename_group(&self, folder_rel: &str, new_name: &str) -> Result<String> {
        let new_path = rename_group(&self.collections_dir(), folder_rel, new_name)?;
        Ok(path_to_forward_slash(&new_path))
    }

    fn move_group(&self, folder_rel: &str, target_parent_rel: Option<&str>) -> Result<String> {
        let new_path = move_group(&self.collections_dir(), folder_rel, target_parent_rel)?;
        Ok(path_to_forward_slash(&new_path))
    }

    fn reorder_folder(&self, folder_rel: Option<&str>, order: &[String]) -> Result<()> {
        reorder_folder(&self.collections_dir(), folder_rel, order)
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
                        list.push(stem.to_string());
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
        self.suppress_path(&path);
        let yaml = to_yaml_string(&committed_env)?;
        atomic_write_str(&path, &yaml)?;

        self.save_secrets(name, &secrets)?;

        Ok(())
    }

    fn delete_environment(&self, name: &str) -> Result<()> {
        let path = self.environments_dir().join(format!("{}.yaml", name));
        self.suppress_path(&path);
        if path.exists() {
            let _ = with_retry(|| std::fs::remove_file(&path));
        }
        let secret_path = self.secrets_dir().join(format!("{}.yaml", name));
        if secret_path.exists() {
            let _ = with_retry(|| std::fs::remove_file(&secret_path));
        }
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
        self.suppress_path(&old_path);
        self.suppress_path(&new_path);
        if old_path.exists() {
            with_retry(|| std::fs::rename(&old_path, &new_path))?;
        }

        let old_sec = self.secrets_dir().join(format!("{}.yaml", old_name));
        let new_sec = self.secrets_dir().join(format!("{}.yaml", new_name));
        if old_sec.exists() {
            let _ = with_retry(|| std::fs::rename(&old_sec, &new_sec));
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
        atomic_write_str(&path, &yaml)?;
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
            let _ = atomic_write_str(&loc_path, &json);
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
        atomic_write_str(&path, &yaml)?;
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
        atomic_write_str(&path, &yaml)?;
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
        atomic_write_str(&path, &json)?;
        Ok(())
    }

    fn suppressed_paths(
        &self,
    ) -> std::sync::Arc<std::sync::Mutex<std::collections::HashSet<std::path::PathBuf>>> {
        self.suppressed_paths.clone()
    }
}
