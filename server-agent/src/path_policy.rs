use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::config::WorkspaceRootConfig;
use crate::models::AgentError;

#[derive(Debug, Clone)]
pub struct PathPolicy {
    roots: HashMap<String, PathBuf>,
}

impl PathPolicy {
    pub fn new(roots: &[WorkspaceRootConfig]) -> Result<Self, AgentError> {
        let mut map = HashMap::new();
        for root in roots {
            Self::validate_segment(&root.name).map_err(|_| {
                AgentError::InvalidConfig(format!("workspace root name is invalid: {}", root.name))
            })?;
            if map.contains_key(&root.name) {
                return Err(AgentError::InvalidConfig(format!(
                    "duplicate workspace root: {}",
                    root.name
                )));
            }
            if !root.local_root.is_absolute() {
                return Err(AgentError::InvalidConfig(format!(
                    "workspace root path must be absolute: {}",
                    root.local_root.display()
                )));
            }
            let canonical_root = fs::canonicalize(&root.local_root).map_err(|err| {
                AgentError::InvalidConfig(format!(
                    "workspace root path is invalid: {}, error: {}",
                    root.local_root.display(),
                    err
                ))
            })?;
            map.insert(root.name.clone(), canonical_root);
        }
        if map.is_empty() {
            return Err(AgentError::InvalidConfig(
                "at least one workspace root is required".into(),
            ));
        }
        Ok(Self { roots: map })
    }

    pub fn normalize_logical_path(input: &str) -> Result<String, AgentError> {
        let normalized = input.replace('\\', "/");
        if !normalized.starts_with('/') {
            return Err(AgentError::InvalidLogicalPath(input.to_string()));
        }
        let mut parts = Vec::new();
        for part in normalized.split('/') {
            if part.is_empty() {
                continue;
            }
            if Self::validate_segment(part).is_err() {
                return Err(AgentError::AccessDenied(input.to_string()));
            }
            parts.push(part);
        }
        if parts.is_empty() {
            return Err(AgentError::InvalidLogicalPath(input.to_string()));
        }
        Ok(format!("/{}", parts.join("/")))
    }

    pub fn logical_to_local(&self, logical_path: &str) -> Result<PathBuf, AgentError> {
        let normalized = Self::normalize_logical_path(logical_path)?;
        let parts: Vec<&str> = normalized.trim_start_matches('/').split('/').collect();
        let root_name = parts
            .first()
            .ok_or_else(|| AgentError::InvalidLogicalPath(logical_path.to_string()))?;
        let root_path = self
            .roots
            .get(*root_name)
            .ok_or_else(|| AgentError::UnknownRoot(root_name.to_string()))?;

        let mut local = root_path.clone();
        for part in parts.iter().skip(1) {
            local.push(part);
        }
        Ok(local)
    }

    pub fn local_to_logical(&self, local_path: &Path) -> Result<String, AgentError> {
        if let Some(logical) = self.local_to_logical_from_candidate(local_path)? {
            return Ok(logical);
        }
        if let Ok(canonical) = fs::canonicalize(local_path) {
            if let Some(logical) = self.local_to_logical_from_candidate(&canonical)? {
                return Ok(logical);
            }
        }
        Err(AgentError::AccessDenied(local_path.display().to_string()))
    }

    pub fn resolve_existing_logical_path(&self, logical_path: &str) -> Result<PathBuf, AgentError> {
        let local = self.logical_to_local(logical_path)?;
        self.resolve_existing_local_path(&local)
    }

    pub fn resolve_existing_local_path(&self, local_path: &Path) -> Result<PathBuf, AgentError> {
        let real_path = fs::canonicalize(local_path)?;
        if self.find_root_for_real_path(&real_path).is_none() {
            return Err(AgentError::AccessDenied(real_path.display().to_string()));
        }
        Ok(real_path)
    }

    pub fn resolve_write_target_path(&self, logical_path: &str) -> Result<PathBuf, AgentError> {
        let local = self.logical_to_local(logical_path)?;
        if local.exists() {
            return self.resolve_existing_local_path(&local);
        }
        let parent = local
            .parent()
            .ok_or_else(|| AgentError::InvalidLogicalPath(logical_path.to_string()))?;
        let file_name = local
            .file_name()
            .ok_or_else(|| AgentError::InvalidLogicalPath(logical_path.to_string()))?;
        let real_parent = self.resolve_existing_local_path(parent)?;
        Ok(real_parent.join(file_name))
    }

    fn validate_segment(segment: &str) -> Result<(), AgentError> {
        if segment.trim().is_empty() || segment == "." || segment == ".." {
            return Err(AgentError::InvalidLogicalPath(segment.to_string()));
        }
        if segment.contains('/') || segment.contains('\\') || segment.contains('\0') {
            return Err(AgentError::InvalidLogicalPath(segment.to_string()));
        }
        Ok(())
    }

    fn roots_longest_first(&self) -> Vec<(&String, &PathBuf)> {
        let mut roots: Vec<(&String, &PathBuf)> = self.roots.iter().collect();
        roots.sort_by_key(|(_, path)| std::cmp::Reverse(path.components().count()));
        roots
    }

    fn find_root_for_real_path(&self, real_path: &Path) -> Option<(&str, &PathBuf)> {
        for (root_name, root_path) in self.roots_longest_first() {
            if real_path.starts_with(root_path) {
                return Some((root_name.as_str(), root_path));
            }
        }
        None
    }

    fn local_to_logical_from_candidate(
        &self,
        candidate: &Path,
    ) -> Result<Option<String>, AgentError> {
        for (root_name, root_path) in self.roots_longest_first() {
            if let Ok(relative) = candidate.strip_prefix(root_path) {
                let mut logical_parts = Vec::new();
                for component in relative.components() {
                    match component {
                        Component::Normal(value) => {
                            let value = value.to_str().ok_or_else(|| {
                                AgentError::PathEncoding(candidate.display().to_string())
                            })?;
                            if Self::validate_segment(value).is_err() {
                                return Err(AgentError::AccessDenied(
                                    candidate.display().to_string(),
                                ));
                            }
                            logical_parts.push(value.to_string());
                        }
                        Component::CurDir => {}
                        _ => return Err(AgentError::AccessDenied(candidate.display().to_string())),
                    }
                }
                if logical_parts.is_empty() {
                    return Ok(Some(format!("/{root_name}")));
                }
                return Ok(Some(format!("/{root_name}/{}", logical_parts.join("/"))));
            }
        }
        Ok(None)
    }
}
