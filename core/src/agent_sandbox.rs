//! Agent Sandbox - Isolated workspace for agentic plagiarism investigation
//!
//! Security model:
//! - Whitelist approach: everything forbidden by default
//! - Path validation via canonicalization
//! - No shell execution
//! - No network access

use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Errors from sandbox operations
#[derive(Debug, Clone, thiserror::Error)]
pub enum SandboxError {
    #[error("Path traversal attempt blocked: {0}")]
    PathTraversalAttempt(String),

    #[error("Path is outside workspace: {0}")]
    OutsideWorkspace(String),

    #[error("Forbidden file extension: {0}")]
    ForbiddenExtension(String),

    #[error("File too large: {0} bytes")]
    FileTooLarge(usize),

    #[error("IO error: {0}")]
    IoError(String),

    #[error("Unknown tool: {0}")]
    UnknownTool(String),
}

/// Configuration for the agent sandbox
#[derive(Debug, Clone)]
pub struct SandboxConfig {
    pub allowed_extensions: Vec<String>,
    pub max_file_size: usize,
    pub timeout_seconds: u64,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            allowed_extensions: vec![".py".to_string(), ".txt".to_string(), ".md".to_string()],
            max_file_size: 1024 * 1024, // 1 MB
            timeout_seconds: 60,
        }
    }
}

/// Isolated workspace for agent investigation
pub struct AgentWorkspace {
    temp_dir: TempDir,
    config: SandboxConfig,
}

impl AgentWorkspace {
    /// Create a new isolated workspace
    pub fn new() -> Result<Self, SandboxError> {
        let temp_dir = TempDir::new().map_err(|e| SandboxError::IoError(e.to_string()))?;

        Ok(Self {
            temp_dir,
            config: SandboxConfig::default(),
        })
    }

    /// Get the workspace root path
    pub fn root(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Validate that a path is safe to access
    pub fn validate_path(&self, path: &str) -> Result<PathBuf, SandboxError> {
        // 1. Create the full path
        let full_path = self.temp_dir.path().join(path);

        // 2. Canonicalize (resolves .. and symlinks)
        let canonical = full_path
            .canonicalize()
            .map_err(|_| SandboxError::OutsideWorkspace(path.to_string()))?;

        // 3. Check it's still under workspace root
        if !canonical.starts_with(self.temp_dir.path()) {
            return Err(SandboxError::PathTraversalAttempt(path.to_string()));
        }

        // 4. Check extension whitelist
        if let Some(ext) = canonical.extension() {
            let ext_str = format!(".{}", ext.to_string_lossy());
            if !self.config.allowed_extensions.contains(&ext_str) {
                return Err(SandboxError::ForbiddenExtension(ext_str));
            }
        }

        Ok(canonical)
    }

    /// Write a file to the workspace
    pub fn write_file(&self, name: &str, content: &str) -> Result<(), SandboxError> {
        let path = self.temp_dir.path().join(name);
        std::fs::write(&path, content).map_err(|e| SandboxError::IoError(e.to_string()))?;
        Ok(())
    }

    /// Read a file from the workspace
    pub fn read_file(&self, path: &str) -> Result<String, SandboxError> {
        let validated = self.validate_path(path)?;

        let metadata =
            std::fs::metadata(&validated).map_err(|e| SandboxError::IoError(e.to_string()))?;

        if metadata.len() as usize > self.config.max_file_size {
            return Err(SandboxError::FileTooLarge(metadata.len() as usize));
        }

        std::fs::read_to_string(&validated).map_err(|e| SandboxError::IoError(e.to_string()))
    }

    /// List files in the workspace
    pub fn list_files(&self) -> Result<Vec<String>, SandboxError> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(self.temp_dir.path())
            .map_err(|e| SandboxError::IoError(e.to_string()))?
        {
            let entry = entry.map_err(|e| SandboxError::IoError(e.to_string()))?;
            if entry
                .file_type()
                .map_err(|e| SandboxError::IoError(e.to_string()))?
                .is_file()
            {
                files.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        Ok(files)
    }

    /// Grep for a pattern in workspace files
    pub fn grep(&self, pattern: &str) -> Result<Vec<String>, SandboxError> {
        use regex::Regex;

        let re = Regex::new(pattern).map_err(|e| SandboxError::IoError(e.to_string()))?;

        let mut results = Vec::new();
        for file in self.list_files()? {
            if let Ok(content) = self.read_file(&file) {
                for (line_num, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        results.push(format!("{}:{}:{}", file, line_num + 1, line));
                    }
                }
            }
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_creation() {
        let workspace = AgentWorkspace::new().unwrap();
        assert!(workspace.root().exists());
    }

    #[test]
    fn test_write_and_read() {
        let workspace = AgentWorkspace::new().unwrap();
        workspace.write_file("test.py", "print('hello')").unwrap();
        let content = workspace.read_file("test.py").unwrap();
        assert_eq!(content, "print('hello')");
    }

    #[test]
    fn test_blocks_path_traversal() {
        let workspace = AgentWorkspace::new().unwrap();
        workspace.write_file("test.py", "x = 1").unwrap();

        let result = workspace.validate_path("../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_blocks_absolute_path() {
        let workspace = AgentWorkspace::new().unwrap();
        let result = workspace.validate_path("/etc/shadow");
        assert!(result.is_err());
    }

    #[test]
    fn test_allows_workspace_files() {
        let workspace = AgentWorkspace::new().unwrap();
        workspace.write_file("code.py", "x = 1").unwrap();
        let result = workspace.validate_path("code.py");
        assert!(result.is_ok());
    }
}
