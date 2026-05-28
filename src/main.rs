mod capabilities;
mod cli;
mod error;
mod menu;
mod niri;
mod proc;
mod registry;
mod snapshot;
mod verbs;

use capabilities::Capabilities;
use clap::Parser;
use error::DoError;
use snapshot::Snapshot;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = cli::Cli::parse();
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("jiji-do: {e:#}");
            // Map known capability errors to 69; everything else to 1.
            let code = e
                .downcast_ref::<DoError>()
                .map(|d| d.exit_code())
                .unwrap_or(1);
            ExitCode::from(code as u8)
        }
    }
}

fn run(args: cli::Cli) -> anyhow::Result<()> {
    let caps = Capabilities::probe();

    if args.debug {
        print_debug(caps);
        return Ok(());
    }

    // The socket is the irreducible prerequisite: snapshot capture and every
    // verb need it. Gate here so a missing socket is a clean 69, not a generic
    // capture failure (exit 1).
    if !caps.contains(Capabilities::NIRI_SOCKET) {
        return Err(DoError::MissingCapability(
            "niri socket unavailable: $NIRI_SOCKET unset or unreachable".into(),
        )
        .into());
    }

    // Snapshot captured BEFORE any picker opens (menu or verb-internal).
    let snapshot = Snapshot::capture(caps)?;

    match args.verb {
        // Direct dispatch.
        Some(name) => {
            let verb =
                registry::find(&name).ok_or_else(|| anyhow::anyhow!("unknown verb: {name}"))?;
            if !verb.is_enabled(caps) {
                return Err(DoError::MissingCapability(format!(
                    "{name} requires {:?}; run with --debug to see what's missing",
                    verb.requires
                ))
                .into());
            }
            (verb.dispatch)(&snapshot)
        }
        // Menu.
        None => {
            if !caps.contains(Capabilities::FUZZEL) {
                return Err(DoError::MissingCapability(
                    "fuzzel not on $PATH (required to render the menu)".into(),
                )
                .into());
            }
            let enabled = registry::enabled_for_menu(caps);
            match menu::render_menu(&enabled)? {
                Some(verb) => (verb.dispatch)(&snapshot),
                None => Ok(()), // cancelled
            }
        }
    }
}

fn print_debug(caps: Capabilities) {
    println!("capabilities: {caps:?}");
    for verb in registry::REGISTRY {
        if verb.is_enabled(caps) {
            println!("  {}: kept", verb.name);
        } else {
            let missing = verb.requires.difference(caps);
            println!("  {}: filtered (missing: {missing:?})", verb.name);
        }
    }
}
