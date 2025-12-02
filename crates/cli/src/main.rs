//! LLM Auto Optimizer CLI
//!
//! Production-grade command-line interface for managing LLM Auto Optimizer.

use clap::{CommandFactory, Parser, Subcommand};
use colored::Colorize;
use llm_optimizer_cli::{
    client::{ClientConfig, RestClient},
    commands::{
        AdminCommand, ConfigCommand, IntegrationCommand, MetricsCommand, OptimizeCommand,
        RunCommand, ServiceCommand, UtilCommand,
    },
    interactive,
    output::{get_formatter, OutputFormat},
    CliConfig, CliResult,
};
use std::time::Duration;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(
    name = "llm-optimizer",
    version,
    about = "LLM Auto Optimizer - Intelligent cost optimization for LLM applications",
    long_about = "Production-grade CLI tool for managing LLM Auto Optimizer.\n\n\
                  Features:\n\
                  - Create and manage optimizations\n\
                  - Monitor performance and costs\n\
                  - Configure integrations\n\
                  - Real-time metrics and analytics"
)]
struct Cli {
    /// API base URL
    #[arg(
        long,
        env = "LLM_OPTIMIZER_API_URL",
        global = true,
        help = "API base URL"
    )]
    api_url: Option<String>,

    /// API key for authentication
    #[arg(
        long,
        env = "LLM_OPTIMIZER_API_KEY",
        global = true,
        help = "API key for authentication"
    )]
    api_key: Option<String>,

    /// Output format
    #[arg(
        short,
        long,
        global = true,
        value_name = "FORMAT",
        help = "Output format (table, json, yaml, csv)"
    )]
    output: Option<OutputFormat>,

    /// Verbose output
    #[arg(short, long, global = true, help = "Enable verbose output")]
    verbose: bool,

    /// Configuration file
    #[arg(
        short,
        long,
        global = true,
        value_name = "FILE",
        help = "Path to configuration file"
    )]
    config: Option<std::path::PathBuf>,

    /// Request timeout in seconds
    #[arg(
        long,
        global = true,
        default_value = "30",
        help = "Request timeout in seconds"
    )]
    timeout: u64,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Service management operations
    #[command(name = "service", about = "Manage the LLM Auto Optimizer service")]
    Service {
        #[command(subcommand)]
        command: ServiceCommand,
    },

    /// Optimization operations
    #[command(name = "optimize", about = "Create and manage optimizations")]
    Optimize {
        #[command(subcommand)]
        command: OptimizeCommand,
    },

    /// Configuration management
    #[command(name = "config", about = "Manage configuration settings")]
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },

    /// Metrics and analytics
    #[command(name = "metrics", about = "Query metrics and analytics")]
    Metrics {
        #[command(subcommand)]
        command: MetricsCommand,
    },

    /// Integration management
    #[command(name = "integration", about = "Manage external integrations")]
    Integration {
        #[command(subcommand)]
        command: IntegrationCommand,
    },

    /// Admin operations
    #[command(name = "admin", about = "Administrative operations")]
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },

    /// Initialize CLI configuration
    #[command(name = "init", about = "Initialize CLI configuration")]
    Init {
        /// API URL
        #[arg(long, default_value = "http://localhost:8080")]
        api_url: String,

        /// API key
        #[arg(long)]
        api_key: Option<String>,

        /// Force overwrite existing configuration
        #[arg(short, long)]
        force: bool,
    },

    /// Generate shell completions
    #[command(name = "completions", about = "Generate shell completions")]
    Completions {
        /// Shell type
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Run system diagnostics
    #[command(name = "doctor", about = "Run system diagnostics")]
    Doctor,

    /// Interactive mode
    #[command(name = "interactive", about = "Start interactive mode")]
    Interactive,

    /// Run operations (benchmarks, etc.)
    #[command(name = "run", about = "Run benchmarks and other operations")]
    Run {
        #[command(subcommand)]
        command: RunCommand,
    },
}

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{} {}", "Error:".red().bold(), e);
        std::process::exit(1);
    }
}

async fn run() -> CliResult<()> {
    let cli = Cli::parse();

    // Initialize tracing
    init_tracing(cli.verbose);

    // Load configuration
    let mut config = load_config(&cli)?;

    // Override with CLI arguments
    if let Some(api_url) = cli.api_url {
        config.api_url = api_url;
    }
    if let Some(api_key) = cli.api_key {
        config.api_key = Some(api_key);
    }
    if let Some(output) = cli.output {
        config.output_format = output;
    }
    if cli.verbose {
        config.verbose = true;
    }
    config.timeout = cli.timeout;

    // Get output formatter
    let formatter = get_formatter(config.output_format);

    // Handle commands that don't require API client
    if let Some(Commands::Init {
        api_url,
        api_key,
        force,
    }) = &cli.command
    {
        let cmd = UtilCommand::Init {
            api_url: api_url.clone(),
            api_key: api_key.clone(),
            force: *force,
        };
        return cmd.execute(None).await;
    }

    if let Some(Commands::Completions { shell }) = &cli.command {
        use clap_complete::generate;
        let mut cmd = build_cli();
        let bin_name = cmd.get_name().to_string();
        generate(*shell, &mut cmd, bin_name, &mut std::io::stdout());
        return Ok(());
    }

    // Create API client
    let client_config = ClientConfig {
        base_url: config.api_url.clone(),
        api_key: config.api_key.clone(),
        timeout: Duration::from_secs(config.timeout),
    };

    let client = RestClient::new(client_config)?;

    // Handle doctor command
    if let Some(Commands::Doctor) = &cli.command {
        let cmd = UtilCommand::Doctor;
        return cmd.execute(Some(&client)).await;
    }

    // Handle interactive mode
    if let Some(Commands::Interactive) = &cli.command {
        return interactive::run_interactive_mode(&client, &formatter).await;
    }

    // If no command provided, show help
    let Some(command) = cli.command else {
        Cli::command()
            .print_help()
            .map_err(|e| llm_optimizer_cli::CliError::Io(e))?;
        return Ok(());
    };

    // Execute command
    match command {
        Commands::Service { command } => {
            command.execute(&client, &formatter).await?;
        }
        Commands::Optimize { command } => {
            command.execute(&client, &formatter).await?;
        }
        Commands::Config { command } => {
            command.execute(&client, &formatter).await?;
        }
        Commands::Metrics { command } => {
            command.execute(&client, &formatter).await?;
        }
        Commands::Integration { command } => {
            command.execute(&client, &formatter).await?;
        }
        Commands::Admin { command } => {
            command.execute(&client, &formatter).await?;
        }
        Commands::Run { command } => {
            command.execute(&formatter).await?;
        }
        Commands::Init { .. } | Commands::Completions { .. } | Commands::Doctor | Commands::Interactive => {
            // Already handled above
        }
    }

    Ok(())
}

/// Load configuration from file or create default
fn load_config(cli: &Cli) -> CliResult<CliConfig> {
    if let Some(config_path) = &cli.config {
        // Load from specified file
        CliConfig::from_file(config_path)
    } else if let Some(default_config) = CliConfig::default_config_file() {
        // Try to load from default location
        if default_config.exists() {
            Ok(CliConfig::from_file(&default_config).unwrap_or_default())
        } else {
            Ok(CliConfig::default())
        }
    } else {
        Ok(CliConfig::default())
    }
}

/// Initialize tracing/logging
fn init_tracing(verbose: bool) {
    let filter = if verbose {
        tracing_subscriber::EnvFilter::new("llm_optimizer_cli=debug,info")
    } else {
        tracing_subscriber::EnvFilter::new("llm_optimizer_cli=warn")
    };

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();
}

// Re-export build_cli for completions
pub fn build_cli() -> clap::Command {
    <Cli as clap::CommandFactory>::command()
}
