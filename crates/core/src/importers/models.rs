use crate::models::{
    Auth, EnvironmentVariable, HttpMethod, KeyValuePair, Protocol, RequestBody, ScriptConfig,
};
use crate::storage::models::RequestExampleFile;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImportFormat {
    OpenApi3,
    PostmanCollection,
}

impl ImportFormat {
    pub fn display_name(&self) -> &'static str {
        match self {
            ImportFormat::OpenApi3 => "OpenAPI 3.1 / 3.0 (YAML/JSON)",
            ImportFormat::PostmanCollection => "Postman Collection v2.1 (JSON)",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedRequest {
    pub name: String,
    pub method: HttpMethod,
    pub url: String,
    pub protocol: Protocol,
    pub params: Vec<KeyValuePair>,
    pub headers: Vec<KeyValuePair>,
    pub auth: Auth,
    pub body: RequestBody,
    pub script: Option<ScriptConfig>,
    pub examples: Vec<RequestExampleFile>,
    /// Nested folder names path, e.g. ["Users", "Auth"]
    pub folder_path: Vec<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedFolder {
    /// Full path hierarchy of folder, e.g. ["Users"] or ["Users", "Auth"]
    pub path: Vec<String>,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedEnvironment {
    pub name: String,
    pub variables: Vec<EnvironmentVariable>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportResult {
    pub title: String,
    pub description: Option<String>,
    pub format: ImportFormat,
    pub folders: Vec<ImportedFolder>,
    pub requests: Vec<ImportedRequest>,
    pub environments: Vec<ImportedEnvironment>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOptions {
    /// Target folder relative path within existing workspace, or None for root
    pub target_folder_rel: Option<String>,
    /// Whether to wrap all imported items into a dedicated top-level folder
    pub create_enclosing_folder: bool,
    /// Custom name for enclosing folder (defaults to API / Collection title)
    pub enclosing_folder_name: Option<String>,
    /// Whether to import extracted environments
    pub import_environments: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            target_folder_rel: None,
            create_enclosing_folder: true,
            enclosing_folder_name: None,
            import_environments: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportSummary {
    pub total_requests: usize,
    pub total_folders: usize,
    pub total_environments: usize,
    pub created_request_paths: Vec<String>,
    pub created_folder_paths: Vec<String>,
}
