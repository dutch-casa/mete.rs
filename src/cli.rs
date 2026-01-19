use clap::{Parser, Subcommand};
use colored::*;

struct MetricInfo {
    title: &'static str,
    desc: &'static str,
    levels: &'static [(&'static str, &'static str, u8)], // (range, label, color: 0=green,1=yellow,2=red)
    notes: &'static [&'static str],
}

impl MetricInfo {
    fn lookup(key: &str) -> Option<Self> {
        Some(match key {
            "mi" | "maintainability" | "maintainability-index" => Self {
                title: "Maintainability Index (MI)",
                desc: "Measures code maintainability on a scale of 0-100.\n  Higher values indicate easier to maintain code.",
                levels: &[("85-100", "Excellent", 0), ("65-84", "Good", 1), ("50-64", "Moderate", 2), ("0-49", "Poor", 2)],
                notes: &["Calculated from: cyclomatic complexity, LOC, Halstead volume"],
            },
            "cc" | "cyclomatic" | "cyclomatic-complexity" => Self {
                title: "Cyclomatic Complexity (CC)",
                desc: "Measures linearly independent paths through code.\n  Lower values indicate simpler, easier to test code.",
                levels: &[("1-5", "Simple", 0), ("6-10", "Moderate", 1), ("11-20", "Complex", 2), ("21+", "Very Complex", 2)],
                notes: &["Each branch (if, for, while) adds 1. Base is 1."],
            },
            "loc" | "lines" => Self {
                title: "Lines of Code (LOC)",
                desc: "Count of physical lines in source file.",
                levels: &[("1-25", "Small", 0), ("26-50", "Medium", 1), ("51-100", "Large", 2), ("101+", "Very Large", 2)],
                notes: &[],
            },
            "depth" | "nesting" => Self {
                title: "Nesting Depth",
                desc: "Maximum nesting level of control structures.\n  Lower values indicate flatter, more readable code.",
                levels: &[("1-2", "Flat", 0), ("3-4", "Moderate", 1), ("5-6", "Deep", 2), ("7+", "Very Deep", 2)],
                notes: &[],
            },
            "dup" | "duplicates" | "duplication" => Self {
                title: "Code Duplication",
                desc: "Identifies structurally similar code blocks.\n  Duplication increases maintenance burden.",
                levels: &[("0%", "None", 0), ("1-3%", "Low", 1), ("4-6%", "Moderate", 2), ("7%+", "High", 2)],
                notes: &["Ratio = duplicate_blocks / total_loc"],
            },
            "fan" | "fan-in" | "fan-out" => Self {
                title: "Fan-in / Fan-out",
                desc: "Fan-in: modules importing this. Fan-out: modules this imports.",
                levels: &[("I > O", "Stable", 0), ("O > I", "Volatile", 2)],
                notes: &["Stability = fan_out / (fan_in + fan_out)"],
            },
            _ => return None,
        })
    }

    fn print(&self) {
        println!("{}", self.title.cyan().bold());
        println!();
        for line in self.desc.lines() {
            println!("  {}", line);
        }
        println!();
        for (range, label, color) in self.levels {
            let dot = match color {
                0 => "●".green(),
                1 => "●".yellow(),
                _ => "●".red(),
            };
            println!("  {} {:7} {}", dot, range, label);
        }
        if !self.notes.is_empty() {
            println!();
            for note in self.notes {
                println!("  {}", note);
            }
        }
    }
}

const DEFAULT_CONFIG: &str = r#"# Mete configuration
pattern = "**/*"
format = "table"
sort_by = "mi"
sort_order = "asc"
verbose = false
quiet = false
"#;

/// Structural metrics engine - analyze code quality with joy
#[derive(Parser, Debug)]
#[command(name = "mete")]
#[command(version, about, long_about = None)]
#[command(author, version)]
pub struct Cli {
    /// Programming language (auto-detect if not provided)
    #[arg(short, long, global = true)]
    pub language: Option<String>,

    /// File pattern for directories (e.g., "*.rs", "**/*.py")
    #[arg(short, long, global = true, default_value = "**/*")]
    pub pattern: String,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Suppress non-error output
    #[arg(short = 'q', long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Analyze files and directories
    Analyze {
        /// Source file or directory to analyze
        #[arg(value_name = "PATH")]
        path: String,

        /// Programming language (auto-detect if not provided)
        #[arg(short, long)]
        language: Option<String>,

        /// File pattern for directories (e.g., "*.rs", "**/*.py")
        #[arg(short, long, default_value = "**/*")]
        pattern: String,

        /// Only show files with MI below threshold (0-100)
        #[arg(long)]
        threshold: Option<f64>,

        /// Sort by field (mi, cc, cog, loc, depth, functions, dups, path)
        #[arg(short, long, default_value = "mi")]
        sort_by: String,

        /// Sort order (asc = worst first for MI)
        #[arg(long, default_value = "asc")]
        sort_order: String,

        /// Maximum complexity to show (0 = no limit)
        #[arg(long)]
        max_complexity: Option<u32>,

        /// Maximum nesting depth to show (0 = no limit)
        #[arg(long)]
        max_depth: Option<u32>,

        /// Output format
        #[arg(short, long, default_value = "table", value_parser = ["table", "json", "csv", "summary"])]
        format: String,

        /// Show Maintainability Index (hidden by default)
        #[arg(long)]
        mi: bool,
    },

    /// AI-friendly refactoring targets (prioritized by impact)
    Targets {
        /// Source file or directory to analyze
        #[arg(value_name = "PATH")]
        path: String,

        /// Programming language (auto-detect if not provided)
        #[arg(short, long)]
        language: Option<String>,

        /// File pattern for directories
        #[arg(short, long, default_value = "**/*")]
        pattern: String,

        /// Maximum targets to return
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Minimum complexity threshold
        #[arg(long, default_value = "5")]
        min_cc: u32,
    },

    /// Show detailed function-level metrics
    Functions {
        /// Source file or directory to analyze
        #[arg(value_name = "PATH")]
        path: String,

        /// Programming language (auto-detect if not provided)
        #[arg(short, long)]
        language: Option<String>,

        /// File pattern for directories
        #[arg(short, long, default_value = "**/*")]
        pattern: String,

        /// Show only complex functions (CC > 10 or CC/LOC > 0.3)
        #[arg(long)]
        complex: bool,

        /// Show only large functions (LOC > 50)
        #[arg(long)]
        large: bool,

        /// Show only deeply nested functions (depth > 3)
        #[arg(long)]
        deep: bool,

        /// Minimum complexity to show
        #[arg(long)]
        min_complexity: Option<u32>,

        /// Minimum LOC to show
        #[arg(long)]
        min_loc: Option<u32>,

        /// Sort by field (cc, cog, loc, depth, name, path)
        #[arg(short, long, default_value = "cc")]
        sort_by: String,

        /// Sort order (desc = worst first for CC)
        #[arg(long, default_value = "desc")]
        sort_order: String,

        /// Output format
        #[arg(short, long, default_value = "table", value_parser = ["table", "json", "csv"])]
        format: String,
    },

    /// Find duplicate code blocks
    Duplicates {
        /// Source file or directory to analyze
        #[arg(value_name = "PATH")]
        path: String,

        /// Programming language (auto-detect if not provided)
        #[arg(short, long)]
        language: Option<String>,

        /// File pattern for directories
        #[arg(short, long, default_value = "**/*")]
        pattern: String,

        /// Minimum instances to consider as duplicate
        #[arg(short, long, default_value = "2")]
        min_instances: u32,

        /// Show code snippets for each duplicate
        #[arg(long)]
        show_code: bool,

        /// Output format
        #[arg(short, long, default_value = "table", value_parser = ["table", "json", "csv"])]
        format: String,

        /// Similarity threshold for cross-file duplicates (0.0-1.0)
        #[arg(short, long)]
        threshold: Option<f32>,

        /// Enable cross-file duplicate detection
        #[arg(long)]
        cross_file: bool,

        /// Minimum lines of code to consider (filters trivial functions)
        #[arg(long, default_value = "5")]
        min_loc: u32,

        /// Include anonymous functions/closures (excluded by default)
        #[arg(long)]
        include_anonymous: bool,
    },

    /// Show metrics explanation
    Explain {
        /// Metric to explain
        #[arg(value_name = "METRIC")]
        metric: Option<String>,
    },

    /// Measure structural entropy (syntactic complexity)
    Entropy {
        /// Source file or directory to analyze
        #[arg(value_name = "PATH")]
        path: String,

        /// Programming language (auto-detect if not provided)
        #[arg(short, long)]
        language: Option<String>,

        /// File pattern for directories
        #[arg(short, long, default_value = "**/*")]
        pattern: String,

        /// Maximum number of results to show (0 = no limit)
        #[arg(short, long)]
        top_n: Option<usize>,

        /// Output format
        #[arg(short, long, default_value = "table", value_parser = ["table", "json", "csv"])]
        format: String,
    },

    /// Generate configuration file
    Init {
        /// Configuration file path
        #[arg(short, long, default_value = ".mete.toml")]
        path: String,
    },

    /// Run as MCP server (stdio transport)
    #[cfg(feature = "mcp")]
    Mcp {},
}

pub struct CliRunner {
    pub cli: Cli,
}

impl CliRunner {
    pub fn new() -> Self {
        Self { cli: Cli::parse() }
    }

    pub fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        use crate::commands::*;

        match &self.cli.command {
            Some(Commands::Explain { metric }) => {
                self.explain(metric);
                Ok(())
            }
            Some(Commands::Init { path }) => {
                self.init_config(path);
                Ok(())
            }
            Some(Commands::Analyze {
                path,
                language,
                pattern,
                threshold,
                sort_by,
                sort_order,
                max_complexity,
                max_depth,
                format,
                mi,
            }) => analyze::run_analyze(
                path,
                language.as_deref(),
                pattern,
                *threshold,
                sort_by,
                sort_order,
                *max_complexity,
                *max_depth,
                format,
                *mi,
                self.cli.verbose,
                self.cli.quiet,
            ),
            Some(Commands::Targets {
                path,
                language,
                pattern,
                limit,
                min_cc,
            }) => targets::run_targets(
                path,
                language.as_deref(),
                pattern,
                *limit,
                *min_cc,
                self.cli.quiet,
            ),
            Some(Commands::Functions {
                path,
                language,
                pattern,
                complex,
                large,
                deep,
                min_complexity,
                min_loc,
                sort_by,
                sort_order,
                format,
            }) => functions::run_functions(
                path,
                language.as_deref(),
                pattern,
                *complex,
                *large,
                *deep,
                *min_complexity,
                *min_loc,
                sort_by,
                sort_order,
                format,
                self.cli.verbose,
                self.cli.quiet,
            ),
            Some(Commands::Duplicates {
                path,
                language,
                pattern,
                min_instances,
                show_code,
                format,
                threshold,
                cross_file,
                min_loc,
                include_anonymous,
            }) => duplicates::run_duplicates(
                path,
                language.as_deref(),
                pattern,
                *min_instances,
                *show_code,
                format,
                *threshold,
                *cross_file,
                *min_loc,
                *include_anonymous,
                self.cli.verbose,
                self.cli.quiet,
            ),
            Some(Commands::Entropy {
                path,
                language,
                pattern,
                top_n,
                format,
            }) => entropy::run_entropy(
                path,
                language.as_deref(),
                pattern,
                *top_n,
                format,
                self.cli.verbose,
                self.cli.quiet,
            ),
            #[cfg(feature = "mcp")]
            Some(Commands::Mcp {}) => {
                // Handled in main.rs before run() is called
                unreachable!("MCP command should be handled in main()")
            }
            None => {
                eprintln!("{}", "Error: command required".red());
                self.print_help();
                std::process::exit(1);
            }
        }
    }

    fn explain(&self, metric: &Option<String>) {
        let info = metric.as_deref().and_then(MetricInfo::lookup);
        match info {
            Some(m) => m.print(),
            None => Self::print_metric_list(),
        }
    }

    fn print_metric_list() {
        println!("{}", "Code Quality Metrics".cyan().bold());
        println!();
        for (short, long) in [
            ("mi", "maintainability-index"),
            ("cc", "cyclomatic-complexity"),
            ("loc", "lines-of-code"),
            ("depth", "nesting-depth"),
            ("dup", "code-duplication"),
            ("fan", "fan-in-fan-out"),
        ] {
            println!("  {} {}", short.green(), long);
        }
        println!();
        println!("  Run {} for details.", "`mete explain <metric>`".cyan());
    }

    fn init_config(&self, path: &str) {
        match std::fs::write(path, DEFAULT_CONFIG) {
            Ok(_) => println!("{} {}", "Created:".green(), path),
            Err(e) => eprintln!("{} {}: {}", "Failed:".red(), path, e),
        }
    }

    fn print_help(&self) {
        println!("{}", "Usage: mete [OPTIONS] <COMMAND>".cyan());
        println!();
        println!("Commands:");
        for (cmd, desc) in [
            ("analyze", "Analyze files and directories"),
            ("functions", "Show function-level metrics"),
            ("duplicates", "Find duplicate code blocks"),
            ("entropy", "Measure structural entropy"),
            ("explain", "Show metrics explanation"),
            ("init", "Generate configuration file"),
        ] {
            println!("  {:<12} {}", cmd.green(), desc);
        }
        println!();
        println!("Run {} for more information.", "`mete help`".cyan());
    }
}
