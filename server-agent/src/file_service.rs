use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::config::FileServiceConfig;
use crate::models::{AgentError, FileTreeEntry, ReadFileResult, WriteFileResult};
use crate::path_policy::PathPolicy;

#[derive(Debug, Clone)]
pub struct FileService {
    path_policy: PathPolicy,
    max_file_size: u64,
    allowed_extensions: Option<HashSet<String>>,
}

#[derive(Debug, Clone, Copy)]
enum NewlineStyle {
    Lf,
    Crlf,
}

impl FileService {
    pub fn new(path_policy: PathPolicy, config: FileServiceConfig) -> Self {
        let allowed_extensions = config.allowed_extensions.map(|v| {
            v.into_iter()
                .map(|ext| Self::normalize_extension_rule(&ext))
                .collect()
        });
        Self {
            path_policy,
            max_file_size: config.max_file_size,
            allowed_extensions,
        }
    }

    pub fn list_tree(&self, logical_dir: &str) -> Result<Vec<FileTreeEntry>, AgentError> {
        let root_dir = self
            .path_policy
            .resolve_existing_logical_path(logical_dir)?;
        if !root_dir.is_dir() {
            return Err(AgentError::AccessDenied(logical_dir.to_string()));
        }

        let mut result = Vec::new();
        let mut stack = vec![root_dir];
        while let Some(current_dir) = stack.pop() {
            for entry in fs::read_dir(current_dir)? {
                let entry = entry?;
                let real_path = self
                    .path_policy
                    .resolve_existing_local_path(&entry.path())?;
                let metadata = fs::metadata(&real_path)?;
                let logical_path = self.path_policy.local_to_logical(&real_path)?;
                let is_dir = metadata.is_dir();
                result.push(FileTreeEntry {
                    logical_path,
                    is_dir,
                    size: if metadata.is_file() {
                        Some(metadata.len())
                    } else {
                        None
                    },
                });
                if is_dir {
                    stack.push(real_path);
                }
            }
        }
        result.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
        Ok(result)
    }

    pub fn read_text_file(&self, logical_path: &str) -> Result<ReadFileResult, AgentError> {
        let normalized_logical = PathPolicy::normalize_logical_path(logical_path)?;
        let real_path = self
            .path_policy
            .resolve_existing_logical_path(&normalized_logical)?;
        let metadata = fs::metadata(&real_path)?;
        if !metadata.is_file() {
            return Err(AgentError::AccessDenied(normalized_logical));
        }

        self.ensure_extension_allowed(&normalized_logical, &real_path)?;
        self.ensure_size_limit(&normalized_logical, metadata.len())?;

        let raw = fs::read(&real_path)?;
        self.ensure_size_limit(&normalized_logical, raw.len() as u64)?;
        let content = String::from_utf8(raw.clone())
            .map_err(|_| AgentError::NotUtf8(normalized_logical.clone()))?;
        let version = Self::build_version(&raw, &metadata);
        Ok(ReadFileResult {
            logical_path: normalized_logical,
            content,
            version,
        })
    }

    pub fn write_text_file(
        &self,
        logical_path: &str,
        new_content: &str,
        expected_version: Option<&str>,
    ) -> Result<WriteFileResult, AgentError> {
        let normalized_logical = PathPolicy::normalize_logical_path(logical_path)?;
        let write_target = self
            .path_policy
            .resolve_write_target_path(&normalized_logical)?;

        if let Some(expected) = expected_version {
            let actual = self.current_version(&write_target)?;
            if actual != expected {
                return Err(AgentError::VersionConflict {
                    path: normalized_logical,
                    expected: expected.to_string(),
                    actual,
                });
            }
        }

        self.ensure_extension_allowed(logical_path, &write_target)?;
        let style = self.detect_newline_style(&write_target)?;
        let normalized_content = Self::apply_newline_style(new_content, style);
        let raw = normalized_content.as_bytes();
        self.ensure_size_limit(logical_path, raw.len() as u64)?;
        fs::write(&write_target, raw)?;

        let metadata = fs::metadata(&write_target)?;
        let version = Self::build_version(raw, &metadata);
        Ok(WriteFileResult {
            logical_path: PathPolicy::normalize_logical_path(logical_path)?,
            version,
        })
    }

    fn normalize_extension_rule(input: &str) -> String {
        let mut ext = input.trim().to_ascii_lowercase();
        if !ext.is_empty() && !ext.starts_with('.') {
            ext.insert(0, '.');
        }
        ext
    }

    fn ensure_extension_allowed(
        &self,
        logical_path: &str,
        local_path: &Path,
    ) -> Result<(), AgentError> {
        let Some(allowed_extensions) = &self.allowed_extensions else {
            return Ok(());
        };

        let extension = match local_path.extension() {
            Some(ext) => {
                let ext = ext
                    .to_str()
                    .ok_or_else(|| AgentError::PathEncoding(local_path.display().to_string()))?;
                format!(".{}", ext.to_ascii_lowercase())
            }
            None => String::new(),
        };
        if allowed_extensions.contains(&extension) {
            return Ok(());
        }

        Err(AgentError::ExtensionNotAllowed {
            path: logical_path.to_string(),
            extension,
        })
    }

    fn ensure_size_limit(&self, logical_path: &str, actual_size: u64) -> Result<(), AgentError> {
        if actual_size > self.max_file_size {
            return Err(AgentError::FileTooLarge {
                path: logical_path.to_string(),
                max_size: self.max_file_size,
                actual_size,
            });
        }
        Ok(())
    }

    fn build_version(content: &[u8], metadata: &fs::Metadata) -> String {
        // 基于内容和修改时间生成稳定版本号。
        let content_hash = Self::fnv1a64(content);
        let modified_nanos = metadata
            .modified()
            .ok()
            .and_then(|v| v.duration_since(UNIX_EPOCH).ok())
            .map(|v| v.as_nanos())
            .unwrap_or(0);
        format!("{content_hash:016x}-{modified_nanos:032x}")
    }

    fn fnv1a64(content: &[u8]) -> u64 {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in content {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash
    }

    fn current_version(&self, local_path: &Path) -> Result<String, AgentError> {
        if !local_path.exists() {
            return Ok("missing".to_string());
        }
        let real_path = self.path_policy.resolve_existing_local_path(local_path)?;
        let metadata = fs::metadata(&real_path)?;
        if !metadata.is_file() {
            return Err(AgentError::AccessDenied(real_path.display().to_string()));
        }
        let content = fs::read(&real_path)?;
        Ok(Self::build_version(&content, &metadata))
    }

    fn detect_newline_style(&self, local_path: &Path) -> Result<NewlineStyle, AgentError> {
        if !local_path.exists() {
            return Ok(NewlineStyle::Lf);
        }
        let real_path = self.path_policy.resolve_existing_local_path(local_path)?;
        let content = fs::read(real_path)?;
        if content.windows(2).any(|v| v == b"\r\n") {
            return Ok(NewlineStyle::Crlf);
        }
        Ok(NewlineStyle::Lf)
    }

    fn apply_newline_style(content: &str, style: NewlineStyle) -> String {
        let lf_normalized = content.replace("\r\n", "\n");
        match style {
            NewlineStyle::Lf => lf_normalized,
            NewlineStyle::Crlf => lf_normalized.replace('\n', "\r\n"),
        }
    }
}
