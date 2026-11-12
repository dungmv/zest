use crate::storage::fingerprints::DiskFingerprints;
use crate::storage::fs_ops::path_to_forward_slash;
use crate::storage::models::{
    FolderFile, FolderNode, LocFile, RequestAuth, RequestFile, RequestNode,
};
use crate::storage::serialize::AtomicWriter;
use crate::storage::slug::loc_path_for_yaml;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct FsWorkspaceStore {
    pub(super) root_dir: PathBuf,
    // Widened to `pub(super)` so the `WorkspaceStore` impl in
    // `fs_store_ops.rs` can reach fingerprints and the write-behind writer.
    pub(super) fingerprints: Arc<DiskFingerprints>,
    pub(super) writer: AtomicWriter,
}

impl FsWorkspaceStore {
    pub fn new(root_dir: impl Into<PathBuf>) -> Self {
        let root_dir = root_dir.into();
        let fingerprints = Arc::new(DiskFingerprints::new());
        let tmp_dir = root_dir.join(".local").join("tmp");
        let writer = AtomicWriter::new(tmp_dir, fingerprints.clone());
        Self {
            root_dir,
            fingerprints,
            writer,
        }
    }

    pub fn disk_fingerprints(&self) -> Arc<DiskFingerprints> {
        self.fingerprints.clone()
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

    pub(super) fn scan_folder(
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
            self.writer.record_read(&meta_file, raw.as_bytes());
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

                // Scanner treats every *.yaml in collections/ except _folder.yaml and dot-files as a request.
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
                self.writer.record_read(&path, raw_content.as_bytes());
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
