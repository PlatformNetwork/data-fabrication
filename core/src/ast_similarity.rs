//! AST Similarity Detection Types and Normalizer
//!
//! Provides types and functions for detecting code plagiarism through 
//! AST structural comparison.

use crate::error::{DataFabricationError, Result};
use rustpython_parser::ast::{self, Expr, Ranged, Stmt, Suite};
use rustpython_parser::Parse;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;

/// Error type for similarity operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimilarityError {
    InvalidScore { value: u8 },
    EmptySubmission,
    ParseError { message: String },
}

impl fmt::Display for SimilarityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScore { value } => write!(f, "Invalid similarity score: {}. Must be 0-100", value),
            Self::EmptySubmission => write!(f, "Cannot compare empty submission"),
            Self::ParseError { message } => write!(f, "Parse error: {}", message),
        }
    }
}

impl std::error::Error for SimilarityError {}

/// A similarity score between 0 and 100
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct SimilarityScore(pub u8);

impl SimilarityScore {
    pub fn new(value: u8) -> std::result::Result<Self, SimilarityError> {
        if value > 100 { Err(SimilarityError::InvalidScore { value }) }
        else { Ok(Self(value)) }
    }
    pub fn value(&self) -> u8 { self.0 }
    pub fn as_f64(&self) -> f64 { self.0 as f64 / 100.0 }
}

impl fmt::Display for SimilarityScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}%", self.0) }
}

/// Result of comparing two submissions
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    pub score: SimilarityScore,
    pub submission_a: usize,
    pub submission_b: usize,
}

impl ComparisonResult {
    pub fn new(score: SimilarityScore, a: usize, b: usize) -> Self {
        Self { score, submission_a: a, submission_b: b }
    }
}

/// Status of plagiarism detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlagiarismStatus {
    Clean,
    Suspicious,
    Plagiarized,
}

impl PlagiarismStatus {
    pub fn from_score(score: SimilarityScore) -> Self {
        if score.0 >= 80 { Self::Plagiarized }
        else if score.0 >= 50 { Self::Suspicious }
        else { Self::Clean }
    }
}

impl fmt::Display for PlagiarismStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Clean => write!(f, "Clean"),
            Self::Suspicious => write!(f, "Suspicious"),
            Self::Plagiarized => write!(f, "Plagiarized"),
        }
    }
}

/// Normalized AST for comparison
#[derive(Debug, Clone)]
pub struct NormalizedAst {
    pub source: String,
    pub node_sequence: Vec<String>,
}

impl NormalizedAst {
    pub fn new(source: String) -> Self { Self { source, node_sequence: Vec::new() } }
    pub fn with_nodes(source: String, nodes: Vec<String>) -> Self {
        Self { source, node_sequence: nodes }
    }
}

/// SHA-256 hash of AST structure
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructureHash(pub [u8; 32]);

impl StructureHash {
    pub fn new(bytes: [u8; 32]) -> Self { Self(bytes) }
    pub fn as_hex(&self) -> String { self.0.iter().map(|b| format!("{:02x}", b)).collect() }
    pub fn prefix_u64(&self) -> u64 {
        u64::from_be_bytes([
            self.0[0], self.0[1], self.0[2], self.0[3],
            self.0[4], self.0[5], self.0[6], self.0[7],
        ])
    }
}

impl fmt::Display for StructureHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}", self.as_hex()) }
}

/// Report from plagiarism detection
#[derive(Debug, Clone)]
pub enum PlagiarismReport {
    NoSubmissions,
    InsufficientData,
    Results { comparisons: Vec<ComparisonResult>, suspicious: Vec<usize>, plagiarized: Vec<usize> },
    ParseError { submission_index: usize, message: String },
}

impl fmt::Display for PlagiarismReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSubmissions => write!(f, "No submissions"),
            Self::InsufficientData => write!(f, "Insufficient data"),
            Self::Results { comparisons, suspicious, plagiarized } => {
                write!(f, "{} comparisons, {} suspicious, {} plagiarized", 
                    comparisons.len(), suspicious.len(), plagiarized.len())
            }
            Self::ParseError { submission_index, message } => {
                write!(f, "Parse error #{}: {}", submission_index, message)
            }
        }
    }
}

/// Cluster of similar submissions
#[derive(Debug, Clone)]
pub struct SubmissionCluster {
    pub hash_prefix: u64,
    pub submission_indices: Vec<usize>,
}

// ============================================================================
// AST NORMALIZER
// ============================================================================

const BUILTINS: &[&str] = &[
    "print", "len", "range", "str", "int", "float", "list", "dict", "set", "tuple",
    "bool", "None", "True", "False", "if", "else", "elif", "for", "while", "def",
    "class", "return", "yield", "import", "from", "as", "try", "except", "finally",
    "with", "lambda", "and", "or", "not", "in", "is", "assert", "raise", "break",
    "continue", "pass", "global", "nonlocal", "del",
];

/// Normalize Python source code for similarity comparison
pub fn normalize_ast(source: &str) -> Result<NormalizedAst> {
    let ast = Suite::parse(source, "<similarity>").map_err(|e| {
        DataFabricationError::SchemaError {
            message: format!("Failed to parse Python: {}", e),
            line: None,
        }
    })?;

    let mut normalizer = AstNormalizer::new();
    normalizer.walk_statements(&ast);
    
    Ok(NormalizedAst::with_nodes(source.to_string(), normalizer.node_sequence))
}

/// Compute the structure hash of a normalized AST
pub fn hash_structure(ast: &NormalizedAst) -> StructureHash {
    let mut hasher = Sha256::new();
    for node in &ast.node_sequence {
        hasher.update(node.as_bytes());
        hasher.update(b"\0");
    }
    let result = hasher.finalize();
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&result);
    StructureHash(hash)
}

/// AST normalizer that walks the tree
struct AstNormalizer {
    var_counter: usize,
    var_map: HashMap<String, String>,
    node_sequence: Vec<String>,
}

impl AstNormalizer {
    fn new() -> Self {
        Self { var_counter: 0, var_map: HashMap::new(), node_sequence: Vec::new() }
    }

    fn normalize_var(&mut self, name: &str) -> String {
        if is_builtin(name) || name.starts_with('_') {
            return name.to_string();
        }
        self.var_map.entry(name.to_string())
            .or_insert_with(|| {
                let normalized = format!("var_{}", self.var_counter);
                self.var_counter += 1;
                normalized
            })
            .clone()
    }

    fn walk_statements(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.walk_statement(stmt);
        }
    }

    fn walk_statement(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::FunctionDef(f) => {
                self.node_sequence.push("FunctionDef".to_string());
                let name = self.normalize_var(f.name.as_str()); self.node_sequence.push(name);
                for arg in &f.args.args {
                    let name = self.normalize_var(arg.def.arg.as_str()); self.node_sequence.push(name);
                }
                self.walk_statements(&f.body);
            }
            Stmt::ClassDef(c) => {
                self.node_sequence.push("ClassDef".to_string());
                let name = self.normalize_var(c.name.as_str()); self.node_sequence.push(name);
                self.walk_statements(&c.body);
            }
            Stmt::Return(r) => {
                self.node_sequence.push("Return".to_string());
                if let Some(e) = &r.value { self.walk_expr(e); }
            }
            Stmt::Assign(a) => {
                self.node_sequence.push("Assign".to_string());
                for target in &a.targets { self.walk_expr(target); }
                self.walk_expr(&a.value);
            }
            Stmt::AugAssign(a) => {
                self.node_sequence.push("AugAssign".to_string());
                self.walk_expr(&a.target);
                self.walk_expr(&a.value);
            }
            Stmt::For(f) => {
                self.node_sequence.push("For".to_string());
                self.walk_expr(&f.target);
                self.walk_expr(&f.iter);
                self.walk_statements(&f.body);
                self.walk_statements(&f.orelse);
            }
            Stmt::While(w) => {
                self.node_sequence.push("While".to_string());
                self.walk_expr(&w.test);
                self.walk_statements(&w.body);
                self.walk_statements(&w.orelse);
            }
            Stmt::If(i) => {
                self.node_sequence.push("If".to_string());
                self.walk_expr(&i.test);
                self.walk_statements(&i.body);
                self.walk_statements(&i.orelse);
            }
            Stmt::Expr(e) => {
                if !is_docstring(&e.value) {
                    self.node_sequence.push("Expr".to_string());
                    self.walk_expr(&e.value);
                }
            }
            Stmt::Import(i) => {
                self.node_sequence.push("Import".to_string());
                for alias in &i.names {
                    self.node_sequence.push(format!("import:{}", alias.name));
                }
            }
            Stmt::ImportFrom(i) => {
                self.node_sequence.push("ImportFrom".to_string());
                if let Some(m) = &i.module {
                    self.node_sequence.push(format!("from:{}", m));
                }
            }
            Stmt::Try(t) => {
                self.node_sequence.push("Try".to_string());
                self.walk_statements(&t.body);
                for handler in &t.handlers {
                    if let ast::ExceptHandler::ExceptHandler(h) = handler {
                        self.walk_statements(&h.body);
                    }
                }
                self.walk_statements(&t.orelse);
                self.walk_statements(&t.finalbody);
            }
            Stmt::With(w) => {
                self.node_sequence.push("With".to_string());
                for item in &w.items {
                    self.walk_expr(&item.context_expr);
                    if let Some(o) = &item.optional_vars { self.walk_expr(o); }
                }
                self.walk_statements(&w.body);
            }
            _ => {
                self.node_sequence.push("Stmt".to_string());
            }
        }
    }

    fn walk_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Name(n) => {
                let normalized = self.normalize_var(n.id.as_str());
                self.node_sequence.push(format!("Name({})", normalized));
            }
            Expr::Constant(_) => {
                self.node_sequence.push("Constant".to_string());
            }
            Expr::Call(c) => {
                self.node_sequence.push("Call".to_string());
                self.walk_expr(&c.func);
                for arg in &c.args { self.walk_expr(arg); }
                for kw in &c.keywords { self.walk_expr(&kw.value); }
            }
            Expr::Attribute(a) => {
                self.node_sequence.push("Attribute".to_string());
                self.walk_expr(&a.value);
                self.node_sequence.push(format!(".{}", a.attr));
            }
            Expr::BinOp(b) => {
                self.node_sequence.push("BinOp".to_string());
                self.walk_expr(&b.left);
                self.walk_expr(&b.right);
            }
            Expr::UnaryOp(u) => {
                self.node_sequence.push("UnaryOp".to_string());
                self.walk_expr(&u.operand);
            }
            Expr::Compare(c) => {
                self.node_sequence.push("Compare".to_string());
                self.walk_expr(&c.left);
                for comp in &c.comparators { self.walk_expr(comp); }
            }
            Expr::List(l) => {
                self.node_sequence.push("List".to_string());
                for e in &l.elts { self.walk_expr(e); }
            }
            Expr::Dict(d) => {
                self.node_sequence.push("Dict".to_string());
                for (k, v) in d.keys.iter().zip(d.values.iter()) {
                    if let Some(k) = k { self.walk_expr(k); }
                    self.walk_expr(v);
                }
            }
            Expr::Tuple(t) => {
                self.node_sequence.push("Tuple".to_string());
                for e in &t.elts { self.walk_expr(e); }
            }
            Expr::Subscript(s) => {
                self.node_sequence.push("Subscript".to_string());
                self.walk_expr(&s.value);
                self.walk_expr(&s.slice);
            }
            Expr::Lambda(l) => {
                self.node_sequence.push("Lambda".to_string());
                self.walk_expr(&l.body);
            }
            Expr::ListComp(l) => {
                self.node_sequence.push("ListComp".to_string());
                self.walk_expr(&l.elt);
            }
            Expr::GeneratorExp(g) => {
                self.node_sequence.push("GeneratorExp".to_string());
                self.walk_expr(&g.elt);
            }
            _ => {
                self.node_sequence.push("Expr".to_string());
            }
        }
    }
}

fn is_builtin(name: &str) -> bool {
    BUILTINS.contains(&name) || name.starts_with("__")
}

fn is_docstring(expr: &Expr) -> bool {
    matches!(expr, Expr::Constant(c) if matches!(c.value, ast::Constant::Str(_)))
}

// ============================================================================
// SIMILARITY COMPARISON
// ============================================================================

/// Compare two normalized ASTs and return a similarity score
pub fn compare_structures(a: &NormalizedAst, b: &NormalizedAst) -> SimilarityScore {
    if a.node_sequence.is_empty() && b.node_sequence.is_empty() {
        return SimilarityScore(100);
    }
    if a.node_sequence.is_empty() || b.node_sequence.is_empty() {
        return SimilarityScore(0);
    }

    let lcs_len = lcs_length(&a.node_sequence, &b.node_sequence);
    let max_len = a.node_sequence.len().max(b.node_sequence.len());
    
    let score = (lcs_len as f64 / max_len as f64 * 100.0) as u8;
    SimilarityScore(score.min(100))
}

fn lcs_length(a: &[String], b: &[String]) -> usize {
    let m = a.len();
    let n = b.len();
    let mut prev = vec![0usize; n + 1];
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        for j in 1..=n {
            if a[i - 1] == b[j - 1] {
                curr[j] = prev[j - 1] + 1;
            } else {
                curr[j] = prev[j].max(curr[j - 1]);
            }
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[n]
}

/// Cluster submissions by their structure hash prefix
pub fn cluster_by_hash(submissions: &[NormalizedAst]) -> Vec<SubmissionCluster> {
    let mut hash_map: HashMap<u64, Vec<usize>> = HashMap::new();
    
    for (i, sub) in submissions.iter().enumerate() {
        let hash = hash_structure(sub);
        let prefix = hash.prefix_u64();
        hash_map.entry(prefix).or_default().push(i);
    }

    hash_map.into_iter()
        .filter(|(_, indices)| indices.len() >= 2)
        .map(|(prefix, indices)| SubmissionCluster { hash_prefix: prefix, submission_indices: indices })
        .collect()
}

/// Check plagiarism across multiple submissions
pub fn check_plagiarism(sources: &[&str]) -> Result<PlagiarismReport> {
    if sources.is_empty() {
        return Ok(PlagiarismReport::NoSubmissions);
    }
    if sources.len() < 2 {
        return Ok(PlagiarismReport::InsufficientData);
    }

    let mut normalized = Vec::new();
    for (i, source) in sources.iter().enumerate() {
        match normalize_ast(source) {
            Ok(ast) => normalized.push(ast),
            Err(e) => return Ok(PlagiarismReport::ParseError {
                submission_index: i,
                message: e.to_string(),
            }),
        }
    }

    let mut comparisons = Vec::new();
    let mut suspicious = Vec::new();
    let mut plagiarized = Vec::new();

    for i in 0..normalized.len() {
        for j in (i + 1)..normalized.len() {
            let score = compare_structures(&normalized[i], &normalized[j]);
            comparisons.push(ComparisonResult::new(score, i, j));

            match PlagiarismStatus::from_score(score) {
                PlagiarismStatus::Plagiarized => {
                    if !plagiarized.contains(&i) { plagiarized.push(i); }
                    if !plagiarized.contains(&j) { plagiarized.push(j); }
                }
                PlagiarismStatus::Suspicious => {
                    if !suspicious.contains(&i) { suspicious.push(i); }
                    if !suspicious.contains(&j) { suspicious.push(j); }
                }
                _ => {}
            }
        }
    }

    Ok(PlagiarismReport::Results { comparisons, suspicious, plagiarized })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_validation() {
        assert!(SimilarityScore::new(50).is_ok());
        assert!(SimilarityScore::new(101).is_err());
    }

    #[test]
    fn test_plagiarism_status() {
        assert_eq!(PlagiarismStatus::from_score(SimilarityScore::new(90).unwrap()), PlagiarismStatus::Plagiarized);
        assert_eq!(PlagiarismStatus::from_score(SimilarityScore::new(60).unwrap()), PlagiarismStatus::Suspicious);
        assert_eq!(PlagiarismStatus::from_score(SimilarityScore::new(30).unwrap()), PlagiarismStatus::Clean);
    }

    #[test]
    fn test_normalize_simple() {
        let code = "x = 1\ny = 2";
        let result = normalize_ast(code);
        assert!(result.is_ok());
        let ast = result.unwrap();
        assert!(!ast.node_sequence.is_empty());
    }

    #[test]
    fn test_normalize_variables() {
        let code1 = "x = 1\ny = x + 2";
        let code2 = "a = 1\nb = a + 2";
        
        let ast1 = normalize_ast(code1).unwrap();
        let ast2 = normalize_ast(code2).unwrap();
        
        assert_eq!(ast1.node_sequence, ast2.node_sequence);
    }

    #[test]
    fn test_hash_identical() {
        let code = "def foo(): return 42";
        let ast1 = normalize_ast(code).unwrap();
        let ast2 = normalize_ast(code).unwrap();
        
        assert_eq!(hash_structure(&ast1), hash_structure(&ast2));
    }

    #[test]
    fn test_compare_identical() {
        let code = "x = 1\ny = 2\nz = x + y";
        let ast = normalize_ast(code).unwrap();
        let score = compare_structures(&ast, &ast);
        assert_eq!(score.value(), 100);
    }

    #[test]
    fn test_compare_different() {
        let ast1 = normalize_ast("x = 1").unwrap();
        let ast2 = normalize_ast("def foo(): return 42").unwrap();
        let score = compare_structures(&ast1, &ast2);
        assert!(score.value() < 50);
    }

    #[test]
    fn test_check_plagiarism_empty() {
        assert!(matches!(check_plagiarism(&[]).unwrap(), PlagiarismReport::NoSubmissions));
    }

    #[test]
    fn test_check_plagiarism_single() {
        assert!(matches!(check_plagiarism(&["x = 1"]).unwrap(), PlagiarismReport::InsufficientData));
    }

    #[test]
    fn test_check_plagiarism_identical() {
        let code = "x = 1\ny = 2";
        let report = check_plagiarism(&[code, code]).unwrap();
        
        if let PlagiarismReport::Results { comparisons, plagiarized, .. } = report {
            assert_eq!(comparisons.len(), 1);
            assert_eq!(comparisons[0].score.value(), 100);
            assert_eq!(plagiarized.len(), 2);
        } else {
            panic!("Expected Results");
        }
    }
}
