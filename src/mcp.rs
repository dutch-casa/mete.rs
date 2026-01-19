//! MCP server implementation for mete.
//!
//! Exposes mete's static code analysis as an MCP server for AI coding assistants.

use rmcp::{
    handler::server::tool::schema_for_type,
    model::{
        CallToolRequestParam, CallToolResult, Content, Implementation, ListToolsResult,
        PaginatedRequestParam, ProtocolVersion, ServerCapabilities, ServerInfo, Tool,
        ToolsCapability,
    },
    service::{Peer, RequestContext, RoleServer},
    Error as McpError,
};
use std::sync::Arc;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::commands::common::{analyze_directory, analyze_file};
use mete::data::SingleFileResult;
use mete::lang::Language;

// ============================================================================
// Parameter structs for MCP tools
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnalyzeParams {
    /// Absolute path to file or directory to analyze
    pub path: String,
    /// Programming language (auto-detect if omitted)
    pub language: Option<String>,
    /// Glob pattern for directory scan (default: "**/*")
    pub pattern: Option<String>,
    /// Only show files with MI below this threshold (0-100)
    pub threshold: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct TargetsParams {
    /// Absolute path to file or directory to analyze
    pub path: String,
    /// Programming language (auto-detect if omitted)
    pub language: Option<String>,
    /// Glob pattern for directory scan (default: "**/*")
    pub pattern: Option<String>,
    /// Maximum number of targets to return (default: 20)
    pub limit: Option<usize>,
    /// Minimum cyclomatic complexity threshold (default: 5)
    pub min_cc: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct FunctionsParams {
    /// Absolute path to file or directory to analyze
    pub path: String,
    /// Programming language (auto-detect if omitted)
    pub language: Option<String>,
    /// Glob pattern for directory scan (default: "**/*")
    pub pattern: Option<String>,
    /// Show only complex functions (CC > 10 or CC/LOC > 0.3)
    pub complex: Option<bool>,
    /// Show only large functions (LOC > 50)
    pub large: Option<bool>,
    /// Show only deeply nested functions (depth > 3)
    pub deep: Option<bool>,
    /// Minimum complexity to show
    pub min_complexity: Option<u32>,
    /// Minimum LOC to show
    pub min_loc: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DuplicatesParams {
    /// Absolute path to file or directory to analyze
    pub path: String,
    /// Programming language (auto-detect if omitted)
    pub language: Option<String>,
    /// Glob pattern for directory scan (default: "**/*")
    pub pattern: Option<String>,
    /// Similarity threshold for cross-file duplicates (0.0-1.0, default: 0.8)
    pub threshold: Option<f32>,
    /// Enable cross-file duplicate detection
    pub cross_file: Option<bool>,
    /// Minimum lines of code to consider (default: 5)
    pub min_loc: Option<u32>,
    /// Include anonymous functions/closures
    pub include_anonymous: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EntropyParams {
    /// Absolute path to file or directory to analyze
    pub path: String,
    /// Programming language (auto-detect if omitted)
    pub language: Option<String>,
    /// Glob pattern for directory scan (default: "**/*")
    pub pattern: Option<String>,
    /// Maximum number of results to show
    pub top_n: Option<usize>,
}

// ============================================================================
// Result structs for JSON serialization
// ============================================================================

#[derive(Debug, Serialize)]
struct AnalyzeResult {
    files: Vec<FileMetrics>,
    summary: AnalyzeSummary,
}

#[derive(Debug, Serialize)]
struct FileMetrics {
    path: String,
    loc: u32,
    cc_max: u32,
    cognitive_max: u32,
    depth_max: u32,
    mi: u8,
    function_count: usize,
    dup_count: u32,
}

#[derive(Debug, Serialize)]
struct AnalyzeSummary {
    total_files: usize,
    total_loc: u32,
    avg_mi: f64,
    max_cc: u32,
    max_depth: u32,
}

#[derive(Debug, Serialize)]
struct RefactorTarget {
    file: String,
    name: String,
    lines: LineRange,
    metrics: TargetMetrics,
    priority: f64,
    reason: String,
}

#[derive(Debug, Serialize)]
struct LineRange {
    start: u32,
    end: u32,
}

#[derive(Debug, Serialize)]
struct TargetMetrics {
    cc: u32,
    cognitive: u32,
    loc: u32,
    depth: u32,
}

#[derive(Debug, Serialize)]
struct TargetsResult {
    targets: Vec<RefactorTarget>,
    count: usize,
}

#[derive(Debug, Serialize)]
struct FunctionMetrics {
    name: Option<String>,
    file: String,
    start_line: u32,
    end_line: u32,
    loc: u32,
    cc: u32,
    cognitive: u32,
    depth: u32,
    fingerprint: u64,
}

#[derive(Debug, Serialize)]
struct FunctionsResult {
    functions: Vec<FunctionMetrics>,
    count: usize,
}

#[derive(Debug, Serialize)]
struct DuplicateInstance {
    file: String,
    name: Option<String>,
    start_line: u32,
    end_line: u32,
    similarity: f32,
}

#[derive(Debug, Serialize)]
struct DuplicateGroup {
    fingerprint: u64,
    similarity: f32,
    instances: Vec<DuplicateInstance>,
}

#[derive(Debug, Serialize)]
struct DuplicatesResult {
    duplicates: Vec<DuplicateGroup>,
    summary: DuplicatesSummary,
}

#[derive(Debug, Serialize)]
struct DuplicatesSummary {
    duplicate_groups: usize,
    total_instances: usize,
}

#[derive(Debug, Serialize)]
struct EntropyFileResult {
    path: String,
    entropy: f64,
    metric_mass: f64,
    node_count: u32,
    unique_types: usize,
}

#[derive(Debug, Serialize)]
struct EntropyResult {
    files: Vec<EntropyFileResult>,
    count: usize,
}

// ============================================================================
// MCP Server Implementation
// ============================================================================

#[derive(Clone)]
pub struct MeteServer {
    peer: Option<Peer<RoleServer>>,
}

impl MeteServer {
    pub fn new() -> Self {
        Self { peer: None }
    }

    fn tool_definitions() -> Vec<Tool> {
        vec![
            Tool::new(
                "analyze",
                "Analyze code quality metrics (MI, CC, cognitive complexity, depth) for files or directories",
                Arc::new(schema_for_type::<AnalyzeParams>()),
            ),
            Tool::new(
                "targets",
                "Find AI-friendly refactoring targets sorted by priority (most impactful first)",
                Arc::new(schema_for_type::<TargetsParams>()),
            ),
            Tool::new(
                "functions",
                "Get function-level metrics with optional filtering by complexity, size, or depth",
                Arc::new(schema_for_type::<FunctionsParams>()),
            ),
            Tool::new(
                "duplicates",
                "Detect duplicate code blocks across files (exact and similar matches)",
                Arc::new(schema_for_type::<DuplicatesParams>()),
            ),
            Tool::new(
                "entropy",
                "Measure structural entropy (complexity distribution via Shannon entropy)",
                Arc::new(schema_for_type::<EntropyParams>()),
            ),
        ]
    }
}

impl Default for MeteServer {
    fn default() -> Self {
        Self::new()
    }
}

impl rmcp::handler::server::ServerHandler for MeteServer {
    fn get_peer(&self) -> Option<Peer<RoleServer>> {
        self.peer.clone()
    }

    fn set_peer(&mut self, peer: Peer<RoleServer>) {
        self.peer = Some(peer);
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability { list_changed: None }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "mete".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some("Static code analysis - quality metrics, duplicates, complexity.".to_string()),
        }
    }

    async fn list_tools(
        &self,
        _request: PaginatedRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            next_cursor: None,
            tools: Self::tool_definitions(),
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let args = request.arguments.unwrap_or_default();

        match request.name.as_ref() {
            "analyze" => {
                let params: AnalyzeParams = serde_json::from_value(serde_json::Value::Object(args))
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                call_analyze(params)
            }
            "targets" => {
                let params: TargetsParams = serde_json::from_value(serde_json::Value::Object(args))
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                call_targets(params)
            }
            "functions" => {
                let params: FunctionsParams = serde_json::from_value(serde_json::Value::Object(args))
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                call_functions(params)
            }
            "duplicates" => {
                let params: DuplicatesParams = serde_json::from_value(serde_json::Value::Object(args))
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                call_duplicates(params)
            }
            "entropy" => {
                let params: EntropyParams = serde_json::from_value(serde_json::Value::Object(args))
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
                call_entropy(params)
            }
            _ => Err(McpError::invalid_params(
                format!("Unknown tool: {}", request.name),
                None,
            )),
        }
    }
}

// ============================================================================
// Tool implementations
// ============================================================================

fn call_analyze(params: AnalyzeParams) -> Result<CallToolResult, McpError> {
    let path = Path::new(&params.path);
    let pattern = params.pattern.as_deref().unwrap_or("**/*");
    let lang = params.language.as_deref().and_then(Language::from_str);

    let results = if path.is_file() {
        analyze_file(path, lang, true)
    } else if path.is_dir() {
        analyze_directory(path, lang, pattern, true)
    } else {
        return Ok(CallToolResult::error(vec![Content::text(format!(
            "Path does not exist or is not a file/directory: {}",
            params.path
        ))]));
    };

    let filtered: Vec<_> = if let Some(threshold) = params.threshold {
        results
            .into_iter()
            .filter(|r| (r.mi as f64) < threshold)
            .collect()
    } else {
        results
    };

    let result = build_analyze_result(&filtered);
    let json = serde_json::to_string_pretty(&result)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn call_targets(params: TargetsParams) -> Result<CallToolResult, McpError> {
    let path = Path::new(&params.path);
    let pattern = params.pattern.as_deref().unwrap_or("**/*");
    let lang = params.language.as_deref().and_then(Language::from_str);
    let limit = params.limit.unwrap_or(20);
    let min_cc = params.min_cc.unwrap_or(5);

    let results = if path.is_file() {
        analyze_file(path, lang, true)
    } else if path.is_dir() {
        analyze_directory(path, lang, pattern, true)
    } else {
        return Ok(CallToolResult::error(vec![Content::text(format!(
            "Path does not exist or is not a file/directory: {}",
            params.path
        ))]));
    };

    let mut targets = collect_refactor_targets(&results, min_cc);
    targets.sort_by(|a, b| {
        b.priority
            .partial_cmp(&a.priority)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    targets.truncate(limit);

    let result = TargetsResult {
        count: targets.len(),
        targets,
    };

    let json = serde_json::to_string_pretty(&result)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn call_functions(params: FunctionsParams) -> Result<CallToolResult, McpError> {
    let path = Path::new(&params.path);
    let pattern = params.pattern.as_deref().unwrap_or("**/*");
    let lang = params.language.as_deref().and_then(Language::from_str);

    let results = if path.is_file() {
        analyze_file(path, lang, true)
    } else if path.is_dir() {
        analyze_directory(path, lang, pattern, true)
    } else {
        return Ok(CallToolResult::error(vec![Content::text(format!(
            "Path does not exist or is not a file/directory: {}",
            params.path
        ))]));
    };

    let mut functions = collect_function_metrics(&results);

    if params.complex.unwrap_or(false) {
        functions.retain(|f| mete::metrics::is_complex(f.cc, f.loc));
    }
    if params.large.unwrap_or(false) {
        functions.retain(|f| mete::metrics::is_large(f.loc));
    }
    if params.deep.unwrap_or(false) {
        functions.retain(|f| mete::metrics::is_deeply_nested(f.depth));
    }
    if let Some(min_cc) = params.min_complexity {
        functions.retain(|f| f.cc >= min_cc);
    }
    if let Some(min_loc) = params.min_loc {
        functions.retain(|f| f.loc >= min_loc);
    }

    let result = FunctionsResult {
        count: functions.len(),
        functions,
    };

    let json = serde_json::to_string_pretty(&result)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn call_duplicates(params: DuplicatesParams) -> Result<CallToolResult, McpError> {
    let path = Path::new(&params.path);
    let pattern = params.pattern.as_deref().unwrap_or("**/*");
    let lang = params.language.as_deref().and_then(Language::from_str);
    let threshold = params.threshold.unwrap_or(0.8);
    let min_loc = params.min_loc.unwrap_or(5);
    let include_anonymous = params.include_anonymous.unwrap_or(false);
    let cross_file = params.cross_file.unwrap_or(false);

    let results = if path.is_file() {
        analyze_file(path, lang, true)
    } else if path.is_dir() {
        analyze_directory(path, lang, pattern, true)
    } else {
        return Ok(CallToolResult::error(vec![Content::text(format!(
            "Path does not exist or is not a file/directory: {}",
            params.path
        ))]));
    };

    let groups = if cross_file {
        find_cross_file_duplicates(&results, threshold, min_loc, include_anonymous)
    } else {
        find_within_file_duplicates(&results, min_loc, include_anonymous)
    };

    let total_instances: usize = groups.iter().map(|g| g.instances.len()).sum();

    let result = DuplicatesResult {
        summary: DuplicatesSummary {
            duplicate_groups: groups.len(),
            total_instances,
        },
        duplicates: groups,
    };

    let json = serde_json::to_string_pretty(&result)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    Ok(CallToolResult::success(vec![Content::text(json)]))
}

fn call_entropy(params: EntropyParams) -> Result<CallToolResult, McpError> {
    let path = Path::new(&params.path);
    let pattern = params.pattern.as_deref().unwrap_or("**/*");
    let lang = params.language.as_deref().and_then(Language::from_str);

    let results = if path.is_file() {
        analyze_file_entropy(path, lang)
    } else if path.is_dir() {
        analyze_directory_entropy(path, lang, pattern)
    } else {
        return Ok(CallToolResult::error(vec![Content::text(format!(
            "Path does not exist or is not a file/directory: {}",
            params.path
        ))]));
    };

    let mut sorted = results;
    sorted.sort_by(|a, b| b.metric_mass.total_cmp(&a.metric_mass));

    let files: Vec<_> = if let Some(n) = params.top_n {
        sorted.into_iter().take(n).collect()
    } else {
        sorted
    };

    let result = EntropyResult {
        count: files.len(),
        files,
    };

    let json = serde_json::to_string_pretty(&result)
        .map_err(|e| McpError::internal_error(e.to_string(), None))?;

    Ok(CallToolResult::success(vec![Content::text(json)]))
}

// ============================================================================
// Helper functions
// ============================================================================

fn build_analyze_result(results: &[SingleFileResult]) -> AnalyzeResult {
    let files: Vec<FileMetrics> = results
        .iter()
        .map(|r| FileMetrics {
            path: r.path.display().to_string(),
            loc: r.loc,
            cc_max: r.cc_max,
            cognitive_max: r.cognitive_max,
            depth_max: r.depth_max,
            mi: r.mi,
            function_count: r.functions.len(),
            dup_count: r.dup_count,
        })
        .collect();

    let total_loc: u32 = results.iter().map(|r| r.loc).sum();
    let avg_mi = if results.is_empty() {
        0.0
    } else {
        results.iter().map(|r| r.mi as f64).sum::<f64>() / results.len() as f64
    };
    let max_cc = results.iter().map(|r| r.cc_max).max().unwrap_or(0);
    let max_depth = results.iter().map(|r| r.depth_max).max().unwrap_or(0);

    AnalyzeResult {
        summary: AnalyzeSummary {
            total_files: files.len(),
            total_loc,
            avg_mi,
            max_cc,
            max_depth,
        },
        files,
    }
}

fn collect_refactor_targets(results: &[SingleFileResult], min_cc: u32) -> Vec<RefactorTarget> {
    let mut targets = Vec::new();

    for result in results {
        for func in &result.functions {
            if func.cc < min_cc {
                continue;
            }

            let priority = (func.cc as f64 * 2.0)
                + (func.cognitive as f64 * 1.5)
                + (func.loc as f64 * 0.1)
                + (func.depth as f64 * 1.0);

            let reason = determine_reason(func.cc, func.cognitive, func.loc, func.depth);

            targets.push(RefactorTarget {
                file: result.path.display().to_string(),
                name: func
                    .name
                    .clone()
                    .unwrap_or_else(|| "<anonymous>".to_string()),
                lines: LineRange {
                    start: func.start_line,
                    end: func.end_line,
                },
                metrics: TargetMetrics {
                    cc: func.cc,
                    cognitive: func.cognitive,
                    loc: func.loc,
                    depth: func.depth,
                },
                priority: (priority * 10.0).round() / 10.0,
                reason,
            });
        }
    }

    targets
}

fn determine_reason(cc: u32, cognitive: u32, loc: u32, depth: u32) -> String {
    let mut reasons = Vec::new();

    if cc > 10 {
        reasons.push(format!("high cyclomatic complexity ({})", cc));
    } else if cc > 5 {
        reasons.push(format!("moderate complexity ({})", cc));
    }

    if cognitive > 15 {
        reasons.push(format!("high cognitive load ({})", cognitive));
    }

    if loc > 50 {
        reasons.push(format!("large function ({} lines)", loc));
    }

    if depth > 4 {
        reasons.push(format!("deeply nested (depth {})", depth));
    }

    if reasons.is_empty() {
        "minor complexity".to_string()
    } else {
        reasons.join(", ")
    }
}

fn collect_function_metrics(results: &[SingleFileResult]) -> Vec<FunctionMetrics> {
    results
        .iter()
        .flat_map(|r| {
            r.functions.iter().map(|f| FunctionMetrics {
                name: f.name.clone(),
                file: r.path.display().to_string(),
                start_line: f.start_line,
                end_line: f.end_line,
                loc: f.loc,
                cc: f.cc,
                cognitive: f.cognitive,
                depth: f.depth,
                fingerprint: f.fingerprint,
            })
        })
        .collect()
}

fn find_cross_file_duplicates(
    results: &[SingleFileResult],
    threshold: f32,
    min_loc: u32,
    include_anonymous: bool,
) -> Vec<DuplicateGroup> {
    use mete::dup::DuplicateIndex;

    let mut index = DuplicateIndex::new(threshold);

    for (file_idx, result) in results.iter().enumerate() {
        for (fn_idx, func) in result.functions.iter().enumerate() {
            if func.loc < min_loc {
                continue;
            }
            if !include_anonymous && func.name.is_none() {
                continue;
            }

            let location = mete::dup::FunctionLocation {
                file_idx: file_idx as u32,
                fn_idx: fn_idx as u32,
            };

            let tokens = vec![func.fingerprint];
            index.add(location, func.fingerprint, &tokens);
        }
    }

    let groups = index.find_similar_duplicates();

    groups
        .into_iter()
        .filter(|g| g.instances.len() >= 2)
        .map(|g| {
            let instances: Vec<DuplicateInstance> = g
                .instances
                .iter()
                .map(|(loc, similarity)| {
                    let result = &results[loc.file_idx as usize];
                    let func = &result.functions[loc.fn_idx as usize];
                    DuplicateInstance {
                        file: result.path.display().to_string(),
                        name: func.name.clone(),
                        start_line: func.start_line,
                        end_line: func.end_line,
                        similarity: *similarity,
                    }
                })
                .collect();

            DuplicateGroup {
                fingerprint: results[g.canonical.file_idx as usize].functions
                    [g.canonical.fn_idx as usize]
                    .fingerprint,
                similarity: g.similarity,
                instances,
            }
        })
        .collect()
}

fn find_within_file_duplicates(
    results: &[SingleFileResult],
    min_loc: u32,
    include_anonymous: bool,
) -> Vec<DuplicateGroup> {
    use std::collections::HashMap;

    let mut all_groups: Vec<DuplicateGroup> = Vec::new();

    for result in results {
        let mut by_fingerprint: HashMap<u64, Vec<&mete::data::FunctionData>> = HashMap::new();
        for func in &result.functions {
            if func.loc < min_loc {
                continue;
            }
            if !include_anonymous && func.name.is_none() {
                continue;
            }
            by_fingerprint
                .entry(func.fingerprint)
                .or_default()
                .push(func);
        }

        for (fingerprint, funcs) in by_fingerprint {
            if funcs.len() >= 2 {
                let instances: Vec<DuplicateInstance> = funcs
                    .iter()
                    .map(|f| DuplicateInstance {
                        file: result.path.display().to_string(),
                        name: f.name.clone(),
                        start_line: f.start_line,
                        end_line: f.end_line,
                        similarity: 1.0,
                    })
                    .collect();

                all_groups.push(DuplicateGroup {
                    fingerprint,
                    similarity: 1.0,
                    instances,
                });
            }
        }
    }

    all_groups.sort_by(|a, b| b.instances.len().cmp(&a.instances.len()));
    all_groups
}

fn analyze_file_entropy(path: &Path, language: Option<Language>) -> Vec<EntropyFileResult> {
    let lang = match language.or_else(|| Language::from_path(path)) {
        Some(l) => l,
        None => return Vec::new(),
    };

    let source = match std::fs::read(path) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let tree = match parse_source(&lang, &source) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let (type_counts, node_count) = count_node_types(&tree);
    if node_count == 0 {
        return Vec::new();
    }

    let (entropy, metric_mass) = compute_entropy(&type_counts, node_count);

    vec![EntropyFileResult {
        path: path.display().to_string(),
        entropy,
        metric_mass,
        node_count,
        unique_types: type_counts.len(),
    }]
}

fn analyze_directory_entropy(
    dir: &Path,
    language: Option<Language>,
    pattern: &str,
) -> Vec<EntropyFileResult> {
    use rayon::prelude::*;

    let glob_pattern = dir.join(pattern);
    let pattern_str = glob_pattern.to_string_lossy().to_string();

    let entries = match glob::glob(&pattern_str) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let file_paths: Vec<std::path::PathBuf> = entries
        .filter_map(|entry| match entry {
            Ok(path) if path.is_file() && !mete::is_skippable(&path) => Some(path),
            _ => None,
        })
        .collect();

    file_paths
        .par_iter()
        .filter_map(|path| analyze_file_entropy(path, language).into_iter().next())
        .collect()
}

fn parse_source(lang: &Language, source: &[u8]) -> Option<tree_sitter::Tree> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&lang.tree_sitter_language()).ok()?;
    parser.parse(source, None)
}

fn count_node_types(tree: &tree_sitter::Tree) -> (std::collections::HashMap<&str, u32>, u32) {
    use std::collections::HashMap;

    let mut type_counts: HashMap<&str, u32> = HashMap::new();
    let mut node_count: u32 = 0;
    let mut cursor = tree.walk();

    loop {
        let kind = cursor.node().kind();
        *type_counts.entry(kind).or_insert(0) += 1;
        node_count += 1;

        if cursor.goto_first_child() {
            continue;
        }

        while !cursor.goto_next_sibling() {
            if !cursor.goto_parent() {
                return (type_counts, node_count);
            }
        }
    }
}

fn compute_entropy(
    type_counts: &std::collections::HashMap<&str, u32>,
    node_count: u32,
) -> (f64, f64) {
    let total = node_count as f64;
    let entropy: f64 = type_counts
        .values()
        .map(|&count| {
            let p = count as f64 / total;
            -p * p.log2()
        })
        .sum();

    let metric_mass = entropy * total.ln();
    (entropy, metric_mass)
}

// ============================================================================
// Server entry point
// ============================================================================

pub async fn run_server() -> anyhow::Result<()> {
    use rmcp::service::ServiceExt;
    use rmcp::transport::io::stdio;

    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    tracing::info!("Starting mete MCP server v{}", env!("CARGO_PKG_VERSION"));

    let server = MeteServer::new();
    let transport = stdio();

    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
