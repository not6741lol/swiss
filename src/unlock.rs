//! `swiss unlock` — find out which process is holding a file, and optionally kill it.
//!
//! Backed by the Windows Restart Manager API. On other platforms the subcommand
//! still exists (so `--help` stays consistent) but reports that it is unavailable.

use anyhow::Result;
use clap::Args;

#[derive(Debug, Args)]
pub struct UnlockArgs {
    /// The file or folder that is locked.
    pub path: String,

    /// Kill the processes holding the lock.
    #[arg(long, short)]
    pub kill: bool,

    /// Answer "yes" to the confirmation prompt.
    #[arg(long, short = 'y')]
    pub yes: bool,
}

pub fn run(args: UnlockArgs) -> Result<()> {
    platform::run(args)
}

#[cfg(windows)]
mod platform {
    use super::UnlockArgs;
    use anyhow::Result;

    pub fn run(args: UnlockArgs) -> Result<()> {
        let action = if args.kill { "unlock and kill" } else { "inspect locks on" };
        anyhow::bail!(
            "`swiss unlock` is not implemented yet — would {action} `{}`",
            args.path
        )
    }
}

#[cfg(not(windows))]
mod platform {
    use super::UnlockArgs;
    use anyhow::Result;

    pub fn run(_args: UnlockArgs) -> Result<()> {
        anyhow::bail!(
            "`swiss unlock` is only available on Windows (it relies on the Restart Manager API)"
        )
    }
}
