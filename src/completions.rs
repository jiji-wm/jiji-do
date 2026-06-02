//! Shell completions for jiji-do.
//!
//! [`run`] generates static completions from the clap surface and writes them
//! to stdout. It returns before any capability probe, so no compositor socket
//! or external tool is needed.
//!
//! Because verbs are modelled as subcommands in [`crate::cli::Cmd`], both the
//! fish and bash generators enumerate verb names in their output.

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::Shell;

use crate::cli::Cli;

/// Generate shell completions for `shell` and write them to stdout.
///
/// # Errors
///
/// Returns an error if stdout cannot be written.
pub fn run(shell: Shell) -> Result<()> {
    clap_complete::generate(
        shell,
        &mut Cli::command(),
        "jiji-do",
        &mut std::io::stdout(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both fish and bash completions must enumerate verb names, because verbs
    /// are subcommands. Fish emits subcommand-tree completions that include each
    /// verb name; bash similarly enumerates subcommand names.
    #[test]
    fn completions_are_non_empty_and_contain_known_verb() {
        let mut fish_buf = Vec::new();
        clap_complete::generate(Shell::Fish, &mut Cli::command(), "jiji-do", &mut fish_buf);
        let fish_output =
            String::from_utf8(fish_buf).expect("fish completions must be valid UTF-8");
        assert!(
            fish_output.contains("switch-activity"),
            "fish completions must enumerate the registered verb switch-activity"
        );

        let mut bash_buf = Vec::new();
        clap_complete::generate(Shell::Bash, &mut Cli::command(), "jiji-do", &mut bash_buf);
        let bash_output =
            String::from_utf8(bash_buf).expect("bash completions must be valid UTF-8");
        assert!(
            bash_output.contains("switch-activity"),
            "bash completions must enumerate the registered verb switch-activity"
        );
    }
}
