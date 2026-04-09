use std::{
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Seek, SeekFrom},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::models::{AgentError, LogEnvelope, LogSourceConfig, LogStartPosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    #[cfg(unix)]
    device_id: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume_serial_number: u64,
    #[cfg(windows)]
    file_index: u64,
    #[cfg(not(any(unix, windows)))]
    len: u64,
    #[cfg(not(any(unix, windows)))]
    modified_nanos: u128,
}

#[derive(Debug)]
pub struct LogTailer {
    agent_id: String,
    source: String,
    primary_path: PathBuf,
    active_file: File,
    active_identity: FileIdentity,
    cursor: u64,
    line_number: u64,
    pending_bytes: Vec<u8>,
}

impl LogTailer {
    pub fn new(
        agent_id: impl Into<String>,
        source: impl Into<String>,
        config: LogSourceConfig,
    ) -> Result<Self, AgentError> {
        let mut active_file = OpenOptions::new().read(true).open(&config.primary_path)?;
        let active_identity = Self::read_file_identity(&config.primary_path)?;
        let cursor = match config.start_position {
            LogStartPosition::Beginning => 0,
            LogStartPosition::End => active_file.seek(SeekFrom::End(0))?,
        };

        Ok(Self {
            agent_id: agent_id.into(),
            source: source.into(),
            primary_path: config.primary_path,
            active_file,
            active_identity,
            cursor,
            line_number: 0,
            pending_bytes: Vec::new(),
        })
    }

    pub fn poll(&mut self) -> Result<Vec<LogEnvelope>, AgentError> {
        let mut entries = self.read_available_lines()?;

        if self.should_switch_to_new_primary()? {
            self.switch_to_new_primary()?;
            entries.extend(self.read_available_lines()?);
        }

        Ok(entries)
    }

    fn read_available_lines(&mut self) -> Result<Vec<LogEnvelope>, AgentError> {
        let mut reader = BufReader::new(&mut self.active_file);
        reader.seek(SeekFrom::Start(self.cursor))?;

        let mut buffered_line = std::mem::take(&mut self.pending_bytes);
        let mut entries = Vec::new();

        loop {
            let mut chunk = Vec::new();
            let bytes_read = reader.read_until(b'\n', &mut chunk)?;
            if bytes_read == 0 {
                self.pending_bytes = buffered_line;
                break;
            }

            self.cursor += bytes_read as u64;
            buffered_line.extend_from_slice(&chunk);

            if !buffered_line.ends_with(b"\n") {
                continue;
            }

            let line_bytes = Self::trim_line_break(&buffered_line);
            let raw_line = String::from_utf8(line_bytes.to_vec())
                .map_err(|_| AgentError::NotUtf8(self.primary_path.display().to_string()))?;

            self.line_number += 1;
            entries.push(LogEnvelope {
                agent_id: self.agent_id.clone(),
                source: self.source.clone(),
                cursor: self.cursor.to_string(),
                line_number: self.line_number,
                raw_line,
                observed_at: Self::observed_at_now(),
            });
            buffered_line.clear();
        }

        Ok(entries)
    }

    fn should_switch_to_new_primary(&self) -> Result<bool, AgentError> {
        if !self.primary_path.exists() {
            return Ok(false);
        }

        let metadata = fs::metadata(&self.primary_path)?;
        let primary_identity = Self::file_identity_from_metadata(&metadata);

        if primary_identity != self.active_identity {
            return Ok(true);
        }

        Ok(metadata.len() < self.cursor)
    }

    fn switch_to_new_primary(&mut self) -> Result<(), AgentError> {
        let active_file = OpenOptions::new().read(true).open(&self.primary_path)?;
        let active_identity = Self::read_file_identity(&self.primary_path)?;

        self.active_file = active_file;
        self.active_identity = active_identity;
        self.cursor = 0;
        self.pending_bytes.clear();

        Ok(())
    }

    fn trim_line_break(bytes: &[u8]) -> &[u8] {
        if bytes.ends_with(b"\r\n") {
            &bytes[..bytes.len() - 2]
        } else if bytes.ends_with(b"\n") {
            &bytes[..bytes.len() - 1]
        } else {
            bytes
        }
    }

    fn observed_at_now() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis().to_string())
            .unwrap_or_else(|_| "0".to_string())
    }

    fn read_file_identity(path: &std::path::Path) -> Result<FileIdentity, AgentError> {
        let metadata = fs::metadata(path)?;
        Ok(Self::file_identity_from_metadata(&metadata))
    }

    #[cfg(unix)]
    fn file_identity_from_metadata(metadata: &fs::Metadata) -> FileIdentity {
        use std::os::unix::fs::MetadataExt;

        FileIdentity {
            device_id: metadata.dev(),
            inode: metadata.ino(),
        }
    }

    #[cfg(windows)]
    fn file_identity_from_metadata(metadata: &fs::Metadata) -> FileIdentity {
        use std::os::windows::fs::MetadataExt;

        let file_index =
            ((metadata.file_index_high() as u64) << 32) | metadata.file_index_low() as u64;

        FileIdentity {
            volume_serial_number: metadata.volume_serial_number().unwrap_or_default() as u64,
            file_index,
        }
    }

    #[cfg(not(any(unix, windows)))]
    fn file_identity_from_metadata(metadata: &fs::Metadata) -> FileIdentity {
        let modified_nanos = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|value| value.as_nanos())
            .unwrap_or(0);

        FileIdentity {
            len: metadata.len(),
            modified_nanos,
        }
    }
}
