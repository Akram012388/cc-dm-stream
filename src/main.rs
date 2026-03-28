mod bus;
mod theme;
mod types;
mod watcher;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "cc-dm-stream",
    version,
    about = "Live streaming TUI for the cc-dm coordination bus"
)]
pub struct Cli {
    /// Filter to a specific project (skips welcome screen)
    #[arg(long)]
    pub project: Option<String>,
}

fn main() {
    let _cli = Cli::parse();
    // Phase 1 stub — full event loop comes in Phase 7
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_no_args_defaults() {
        let cli = Cli::parse_from(["cc-dm-stream"]);
        assert!(cli.project.is_none());
    }

    #[test]
    fn cli_project_flag() {
        let cli = Cli::parse_from(["cc-dm-stream", "--project", "foo"]);
        assert_eq!(cli.project, Some("foo".to_string()));
    }
}
