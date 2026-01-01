use clap::{Parser, Subcommand};
use colored::*;

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

        /// Sort by field (mi, cc, loc, depth, functions, dups)
        #[arg(short, long, default_value = "path")]
        sort_by: String,

        /// Sort order
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

        /// Sort by field (name, loc, cc, depth)
        #[arg(short, long, default_value = "path")]
        sort_by: String,

        /// Sort order
        #[arg(long, default_value = "asc")]
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
                self.cli.verbose,
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
            }) => duplicates::run_duplicates(
                path,
                language.as_deref(),
                pattern,
                *min_instances,
                *show_code,
                format,
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
            None => {
                eprintln!("{}", "Error: command required".red());
                self.print_help();
                std::process::exit(1);
            }
        }
    }

    fn explain(&self, metric: &Option<String>) {
        match metric.as_deref() {
            Some("mi") | Some("maintainability") | Some("maintainability-index") => {
                println!("{}", "Maintainability Index (MI)".cyan().bold());
                println!();
                println!("  Measures code maintainability on a scale of 0-100.");
                println!("  Higher values indicate easier to maintain code.");
                println!();
                println!("  {} 85-100: Excellent", "●".green());
                println!("  {} 65-84:  Good", "●".yellow());
                println!("  {} 50-64:  Moderate", "●".bright_red());
                println!("  {} 0-49:   Poor", "●".red());
                println!();
                println!("  Calculated from:");
                println!("    - Cyclomatic complexity");
                println!("    - Lines of code");
                println!("    - Halstead volume");
            }
            Some("cc") | Some("cyclomatic") | Some("cyclomatic-complexity") => {
                println!("{}", "Cyclomatic Complexity (CC)".cyan().bold());
                println!();
                println!("  Measures number of linearly independent paths through code.");
                println!("  Lower values indicate simpler, easier to test code.");
                println!();
                println!("  {} 1-5:   Simple", "●".green());
                println!("  {} 6-10:  Moderate", "●".yellow());
                println!("  {} 11-20: Complex", "●".bright_red());
                println!("  {} 21+:   Very Complex", "●".red());
                println!();
                println!("  Each branch (if, for, while, etc.) increments complexity by 1.");
                println!("  Base complexity is 1 for any function.");
            }
            Some("loc") | Some("lines") => {
                println!("{}", "Lines of Code (LOC)".cyan().bold());
                println!();
                println!("  Count of physical lines in source file.");
                println!("  Lower values generally correlate with easier maintenance.");
                println!();
                println!("  Function-level metrics:");
                println!("    {} 1-25:   Small", "●".green());
                println!("    {} 26-50:  Medium", "●".yellow());
                println!("    {} 51-100: Large", "●".bright_red());
                println!("    {} 101+:   Very Large", "●".red());
            }
            Some("depth") | Some("nesting") => {
                println!("{}", "Nesting Depth".cyan().bold());
                println!();
                println!("  Maximum nesting level of control structures.");
                println!("  Lower values indicate flatter, more readable code.");
                println!();
                println!("  {} 1-2:   Flat", "●".green());
                println!("  {} 3-4:   Moderate", "●".yellow());
                println!("  {} 5-6:   Deep", "●".bright_red());
                println!("  {} 7+:    Very Deep", "●".red());
            }
            Some("dup") | Some("duplicates") | Some("duplication") => {
                println!("{}", "Code Duplication".cyan().bold());
                println!();
                println!("  Identifies structurally similar code blocks.");
                println!("  Duplication increases maintenance burden.");
                println!();
                println!("  {} 0%:    No Duplication", "●".green());
                println!("    {} 1-3%:  Low", "●".yellow());
                println!("    {} 4-6%:  Moderate", "●".bright_red());
                println!("    {} 7%+:   High", "●".red());
                println!();
                println!("  Duplication ratio = duplicate_blocks / total_loc");
            }
            Some("fan") | Some("fan-in") | Some("fan-out") => {
                println!("{}", "Fan-in / Fan-out".cyan().bold());
                println!();
                println!("  Fan-in:  Number of modules that import this module");
                println!("  Fan-out: Number of modules that this module imports");
                println!();
                println!("  Stability index = fan_out / (fan_in + fan_out)");
                println!();
                println!("  {} Stable (I > O): Less likely to change", "●".green());
                println!("    {} Volatile (O > I): More likely to change", "●".red());
            }
            None | Some(_) => {
                println!("{}", "Code Quality Metrics".cyan().bold());
                println!();
                println!("  {} maintainability-index", "mi".green());
                println!("  {} cyclomatic-complexity", "cc".green());
                println!("  {} lines-of-code", "loc".green());
                println!("  {} nesting-depth", "depth".green());
                println!("  {} code-duplication", "dup".green());
                println!("  {} fan-in-fan-out", "fan".green());
                println!();
                println!(
                    "  Run {} for details on any metric.",
                    "`mete explain <metric>`".cyan()
                );
            }
        }
    }

    fn init_config(&self, path: &str) {
        let config = r#"# Mete configuration file
# Generated by mete init

# Default language (comment out to auto-detect)
# language = "rust"

# Default file pattern for directories
pattern = "**/*"

# Output format: table, json, csv, summary
format = "table"

# Only show files with MI below threshold
# threshold = 70.0

# Default sort field for analyze: path, mi, cc, loc, depth, functions, dups
sort_by = "path"

# Default sort order: asc, desc
sort_order = "asc"

# Maximum complexity to show (0 = no limit)
# max_complexity = 10

# Maximum nesting depth to show (0 = no limit)
# max_depth = 5

# Verbose output
verbose = false

# Suppress non-error output
quiet = false
"#;

        match std::fs::write(path, config) {
            Ok(_) => println!("{} {}", "Created configuration file:".green(), path),
            Err(e) => eprintln!("{} {}: {}", "Failed to create config file".red(), path, e),
        }
    }

    fn print_help(&self) {
        println!("{}", "Usage: mete [OPTIONS] <COMMAND>".cyan());
        println!();
        println!("Commands:");
        println!(
            "  {}       Analyze files and directories",
            "analyze".green()
        );
        println!(
            "  {}    Show detailed function-level metrics",
            "functions".green()
        );
        println!("  {}    Find duplicate code blocks", "duplicates".green());
        println!("  {}        Show metrics explanation", "explain".green());
        println!("  {}         Generate configuration file", "init".green());
        println!();
        println!("Run {} for more information.", "`mete help`".cyan());
    }
}
