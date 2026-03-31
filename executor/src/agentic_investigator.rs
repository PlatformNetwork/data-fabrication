//! Agentic Plagiarism Investigator
//!
//! LLM-powered agent that investigates plagiarism using function calls
//! in an isolated sandbox environment.

use plagiarism_sdk::AgentWorkspace;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};

/// Tool that the agent can call
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "name", content = "args")]
pub enum AgentTool {
    ReadFile { path: String },
    Grep { pattern: String },
    Find { name: String },
    ListDir { path: Option<String> },
    AnalyzeAst { path: String },
    Diff { file_a: String, file_b: String },
    SubmitVerdict { verdict: PlagiarismVerdict },
}

/// Result of investigating plagiarism
#[derive(Debug, Clone, Serialize)]
pub struct AgenticInvestigation {
    pub verdict: PlagiarismVerdict,
    pub audit_trail: Vec<AuditEntry>,
    pub duration_seconds: f64,
    pub iterations: u32,
}

/// Entry in the audit trail
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub tool: String,
    pub args: serde_json::Value,
    pub result: String,
    pub timestamp: DateTime<Utc>,
}

/// Verdict from the investigation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlagiarismVerdict {
    pub is_plagiarism: bool,
    pub confidence: f64,
    pub reasoning: String,
    pub audit: AuditDetails,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditDetails {
    pub structural_match: f64,
    pub logic_flow_similarity: bool,
    pub variable_patterns: String,
    pub comments_analysis: String,
    pub code_origin: String,
    pub recommendation: String,
}

/// Configuration for the investigator
pub struct InvestigatorConfig {
    pub max_iterations: u32,
    pub timeout_seconds: u64,
    pub llm_endpoint: String,
    pub llm_model: String,
}

impl Default for InvestigatorConfig {
    fn default() -> Self {
        Self {
            max_iterations: 20,
            timeout_seconds: 60,
            llm_endpoint: "http://localhost:11434/api/chat".to_string(),
            llm_model: "llama3".to_string(),
        }
    }
}

/// The agentic investigator
pub struct AgenticInvestigator {
    config: InvestigatorConfig,
    client: reqwest::Client,
}

impl AgenticInvestigator {
    pub fn new(config: InvestigatorConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds + 10))
            .build()?;
        Ok(Self { config, client })
    }
    
    /// Run the investigation
    pub async fn investigate(
        &self,
        code_a: &str,
        code_b: &str,
        initial_similarity: f64,
    ) -> Result<AgenticInvestigation, InvestigationError> {
        let start = Instant::now();
        
        let workspace = AgentWorkspace::new()
            .map_err(|e| InvestigationError::SandboxError(e.to_string()))?;
        
        workspace.write_file("code_a.py", code_a)
            .map_err(|e| InvestigationError::SandboxError(e.to_string()))?;
        workspace.write_file("code_b.py", code_b)
            .map_err(|e| InvestigationError::SandboxError(e.to_string()))?;
        
        let prompt = self.build_initial_prompt(initial_similarity);
        
        let result = self.run_loop(&workspace, &prompt, start).await?;
        
        Ok(result)
    }
    
    /// Investigate from ZIP artifacts using SDK's extract_zip
    pub async fn investigate_artifacts(
        &self,
        artifact_a_name: &str,
        artifact_a_data: &[u8],
        artifact_b_name: &str,
        artifact_b_data: &[u8],
        initial_similarity: f64,
    ) -> Result<AgenticInvestigation, InvestigationError> {
        let start = Instant::now();
        
        let workspace = AgentWorkspace::new()
            .map_err(|e| InvestigationError::SandboxError(e.to_string()))?;
        
        // Extract ZIP artifacts using SDK's extract_zip method
        workspace.extract_zip(artifact_a_name, artifact_a_data)
            .map_err(|e| InvestigationError::SandboxError(e.to_string()))?;
        workspace.extract_zip(artifact_b_name, artifact_b_data)
            .map_err(|e| InvestigationError::SandboxError(e.to_string()))?;
        
        // List extracted files for investigation
        let files_a = workspace.list_dir_recursive(Some(artifact_a_name))
            .map_err(|e| InvestigationError::SandboxError(e.to_string()))?;
        let files_b = workspace.list_dir_recursive(Some(artifact_b_name))
            .map_err(|e| InvestigationError::SandboxError(e.to_string()))?;
        
        let prompt = self.build_artifact_prompt(initial_similarity, &files_a, &files_b);
        
        let result = self.run_loop(&workspace, &prompt, start).await?;
        
        Ok(result)
    }
    
    fn build_initial_prompt(&self, similarity: f64) -> String {
        format!(
            r#"You are a plagiarism investigation agent with access to a sandboxed workspace.

Files available:
- code_a.py (first submission)
- code_b.py (second submission)

Structural similarity score: {:.1}%

You have these tools available:
- read_file(path): Read a file in the workspace
- grep(pattern): Search for patterns across all files
- find(name): Find files matching a name pattern
- list_dir(path): List directory contents
- analyze_ast(path): Get AST structure of a Python file
- diff(file_a, file_b): Compare two files line by line
- submit_verdict(verdict): Submit your final verdict

Investigate thoroughly. Look at:
1. Function structure and naming
2. Variable patterns
3. Comments and docstrings
4. Logic flow

When ready, call submit_verdict with your conclusion.

Start investigating now."#,
            similarity * 100.0
        )
    }
    
    fn build_artifact_prompt(&self, similarity: f64, files_a: &[String], files_b: &[String]) -> String {
        format!(
            r#"You are a plagiarism investigation agent with access to a sandboxed workspace.

Artifact A files (first submission):
{}

Artifact B files (second submission):
{}

Structural similarity score: {:.1}%

You have these tools available:
- read_file(path): Read a file in the workspace
- grep(pattern): Search for patterns across all files
- find(name): Find files matching a name pattern
- list_dir(path): List directory contents
- analyze_ast(path): Get AST structure of a Python file
- diff(file_a, file_b): Compare two files line by line
- submit_verdict(verdict): Submit your final verdict

Investigate thoroughly. Look at:
1. Function structure and naming
2. Variable patterns
3. Comments and docstrings
4. Logic flow

When ready, call submit_verdict with your conclusion.

Start investigating now."#,
            files_a.iter().map(|f| format!("  - {}", f)).collect::<Vec<_>>().join("\n"),
            files_b.iter().map(|f| format!("  - {}", f)).collect::<Vec<_>>().join("\n"),
            similarity * 100.0
        )
    }
    
    async fn run_loop(
        &self,
        _workspace: &AgentWorkspace,
        _initial_prompt: &str,
        start: Instant,
    ) -> Result<AgenticInvestigation, InvestigationError> {
        let verdict = PlagiarismVerdict {
            is_plagiarism: false,
            confidence: 0.5,
            reasoning: "Agentic investigation placeholder".to_string(),
            audit: AuditDetails {
                structural_match: 0.5,
                logic_flow_similarity: false,
                variable_patterns: "unknown".to_string(),
                comments_analysis: "Not analyzed".to_string(),
                code_origin: "Unknown".to_string(),
                recommendation: "FLAG_FOR_REVIEW".to_string(),
            },
        };
        
        Ok(AgenticInvestigation {
            verdict,
            audit_trail: vec![],
            duration_seconds: start.elapsed().as_secs_f64(),
            iterations: 1,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InvestigationError {
    #[error("Sandbox error: {0}")]
    SandboxError(String),
    
    #[error("Timeout after {0}s")]
    Timeout(u64),
    
    #[error("Max iterations reached")]
    MaxIterationsReached,
    
    #[error("LLM error: {0}")]
    LlmError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_investigator_creation() {
        let config = InvestigatorConfig::default();
        let investigator = AgenticInvestigator::new(config);
        assert!(investigator.is_ok());
    }
    
    #[tokio::test]
    async fn test_investigation_basic() {
        let config = InvestigatorConfig::default();
        let investigator = AgenticInvestigator::new(config).unwrap();
        
        let code_a = "x = 1\ny = x + 2";
        let code_b = "a = 1\nb = a + 2";
        
        let result = investigator.investigate(code_a, code_b, 0.95).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_investigate_artifacts() {
        let config = InvestigatorConfig::default();
        let investigator = AgenticInvestigator::new(config).unwrap();
        
        // Create simple ZIP files in memory
        let mut buf_a = Vec::new();
        {
            use std::io::Cursor;
            use std::io::Write;
            use zip::write::SimpleFileOptions;
            use zip::ZipWriter;
            
            let w = Cursor::new(&mut buf_a);
            let mut zip = ZipWriter::new(w);
            zip.start_file("main.py", SimpleFileOptions::default()).unwrap();
            zip.write_all(b"def hello(): pass").unwrap();
            zip.finish().unwrap();
        }
        
        let mut buf_b = Vec::new();
        {
            use std::io::Cursor;
            use std::io::Write;
            use zip::write::SimpleFileOptions;
            use zip::ZipWriter;
            
            let w = Cursor::new(&mut buf_b);
            let mut zip = ZipWriter::new(w);
            zip.start_file("main.py", SimpleFileOptions::default()).unwrap();
            zip.write_all(b"def greet(): pass").unwrap();
            zip.finish().unwrap();
        }
        
        let result = investigator.investigate_artifacts(
            "artifact_a",
            &buf_a,
            "artifact_b", 
            &buf_b,
            0.95
        ).await;
        assert!(result.is_ok());
    }
}
