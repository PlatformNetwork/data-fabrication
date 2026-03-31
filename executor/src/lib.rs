//! Python execution engines for data-fabrication harnesses.

mod error;
mod executor;
mod llm_inference;

pub use error::{ExecutorError, ExecutorResult};
pub use executor::{ExecutionResult, PythonExecutor};
pub use llm_inference::{
    LlmInference, LlmInferenceConfig, LlmInferenceError, PlagiarismVerdict, 
    PlagiarismAudit, AuditDetails,
};

// Re-export similarity types from core for convenience
#[cfg(feature = "std")]
pub use data_fabrication_core::{
    SimilarityScore, ComparisonResult, PlagiarismStatus, NormalizedAst,
    StructureHash, PlagiarismReport, SimilarityError,
    normalize_ast, hash_structure, compare_structures, check_plagiarism,
};
