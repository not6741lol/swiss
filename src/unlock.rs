//! `swiss unlock` — find out which process is holding a file, and optionally kill it.
//!
//! Backed by the Windows Restart Manager API. On other platforms the data API
//! reports that it is unavailable, so a front-end can say so up front rather than
//! failing once the user has already picked a file.

use anyhow::{bail, Result};
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

/// True when `swiss unlock` can actually do something on this platform.
pub const SUPPORTED: bool = cfg!(windows);

/// A process holding the target open.
#[derive(Debug, Clone)]
pub struct Holder {
    /// Process id.
    pub pid: u32,
    /// Application name as reported by the Restart Manager.
    pub name: String,
    /// Raw `RM_APP_TYPE` value.
    pub app_type: i32,
}

impl Holder {
    /// Human-readable name for the application type.
    pub fn type_label(&self) -> &'static str {
        match self.app_type {
            1 => "main window",
            2 => "other window",
            3 => "service",
            4 => "explorer",
            5 => "console",
            1000 => "critical",
            _ => "unknown",
        }
    }
}

/// The result of asking who holds a file open.
#[derive(Debug)]
pub struct Lock {
    /// The path that was queried: canonical, without the `\\?\` verbatim prefix.
    pub path: String,
    /// The processes holding it open. Empty means nothing was found.
    pub holders: Vec<Holder>,
}

/// Ask the Restart Manager which processes are holding `path` open.
pub fn who_holds(path: &str) -> Result<Lock> {
    platform::who_holds(path)
}

/// Terminate one process by id.
pub fn terminate(pid: u32) -> Result<()> {
    platform::terminate(pid)
}

pub fn run(args: UnlockArgs) -> Result<()> {
    let lock = who_holds(&args.path)?;
    let shown = &lock.path;

    if lock.holders.is_empty() {
        println!("Nothing is holding `{shown}`.");
        return Ok(());
    }

    println!("{} process(es) are holding `{shown}`:\n", lock.holders.len());
    for holder in &lock.holders {
        println!(
            "  pid {:>7}  {:<32}  {}",
            holder.pid,
            holder.name,
            holder.type_label()
        );
    }

    if !args.kill {
        println!("\nRe-run with `--kill` to close them.");
        return Ok(());
    }

    if !args.yes && !confirm("\nKill them?")? {
        println!("Aborted.");
        return Ok(());
    }

    let mut killed = 0usize;
    let mut failed = 0usize;
    for holder in &lock.holders {
        match terminate(holder.pid) {
            Ok(()) => {
                killed += 1;
                println!("  terminated {} (pid {})", holder.name, holder.pid);
            }
            Err(err) => {
                failed += 1;
                eprintln!("  could not terminate pid {}: {err}", holder.pid);
            }
        }
    }

    println!("\nDone — {killed} terminated, {failed} failed.");
    if failed > 0 {
        bail!("some processes could not be closed (try running as administrator)");
    }
    Ok(())
}

/// Ask a yes/no question on the terminal.
fn confirm(prompt: &str) -> Result<bool> {
    use std::io::{self, Write};

    print!("{prompt} [y/N] ");
    io::stdout().flush().ok();

    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

#[cfg(windows)]
mod platform {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::path::PathBuf;

    use anyhow::{bail, Context, Result};
    use windows::core::{PCWSTR, PWSTR};
    use windows::Win32::Foundation::{CloseHandle, ERROR_MORE_DATA, ERROR_SUCCESS};
    use windows::Win32::System::RestartManager::{
        RmEndSession, RmGetList, RmRegisterResources, RmStartSession, CCH_RM_SESSION_KEY,
        RM_PROCESS_INFO,
    };
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    use super::{Holder, Lock};

    /// A Restart Manager session. Ends automatically when dropped.
    struct Session(u32);

    impl Session {
        fn start() -> Result<Self> {
            let mut handle: u32 = 0;
            // RmStartSession writes a CCH_RM_SESSION_KEY-wide key into this buffer.
            let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
            let status = unsafe { RmStartSession(&mut handle, None, PWSTR(key.as_mut_ptr())) };
            if status != ERROR_SUCCESS {
                bail!(
                    "could not start a Restart Manager session (error {})",
                    status.0
                );
            }
            Ok(Session(handle))
        }
    }

    impl Drop for Session {
        fn drop(&mut self) {
            unsafe {
                let _ = RmEndSession(self.0);
            }
        }
    }

    pub fn who_holds(path: &str) -> Result<Lock> {
        let target = resolve(path)?;
        let shown = pretty(&target);
        let session = Session::start()?;

        let wide = to_wide(target.as_os_str());
        let files = [PCWSTR(wide.as_ptr())];

        let status = unsafe { RmRegisterResources(session.0, Some(&files[..]), None, None) };
        if status != ERROR_SUCCESS {
            bail!(
                "could not hand `{shown}` to the Restart Manager (error {})",
                status.0
            );
        }

        Ok(Lock {
            path: shown,
            holders: query_holders(&session)?,
        })
    }

    pub fn terminate(pid: u32) -> Result<()> {
        kill(pid)
    }

    /// Strip the `\\?\` verbatim prefix that `canonicalize` adds, so output stays readable.
    fn pretty(path: &std::path::Path) -> String {
        let text = path.display().to_string();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            format!(r"\\{rest}")
        } else if let Some(rest) = text.strip_prefix(r"\\?\") {
            rest.to_string()
        } else {
            text
        }
    }

    fn resolve(path: &str) -> Result<PathBuf> {
        let raw = PathBuf::from(path);
        if !raw.exists() {
            bail!("`{path}` does not exist");
        }
        Ok(std::fs::canonicalize(&raw).unwrap_or(raw))
    }

    fn query_holders(session: &Session) -> Result<Vec<Holder>> {
        let mut needed: u32 = 0;
        let mut count: u32 = 0;
        let mut reasons: u32 = 0;

        // First pass: how many processes are involved?
        let status = unsafe { RmGetList(session.0, &mut needed, &mut count, None, &mut reasons) };
        if status == ERROR_SUCCESS && needed == 0 {
            return Ok(Vec::new());
        }
        if status != ERROR_MORE_DATA && status != ERROR_SUCCESS {
            bail!(
                "could not list the processes holding the file (error {})",
                status.0
            );
        }

        let mut buffer: Vec<RM_PROCESS_INFO> =
            (0..needed).map(|_| RM_PROCESS_INFO::default()).collect();
        let mut count = needed;

        // Second pass: fill the buffer.
        let status = unsafe {
            RmGetList(
                session.0,
                &mut needed,
                &mut count,
                Some(buffer.as_mut_ptr()),
                &mut reasons,
            )
        };
        if status != ERROR_SUCCESS {
            bail!("could not read the process list (error {})", status.0);
        }

        buffer.truncate(count as usize);

        Ok(buffer
            .into_iter()
            .map(|info| Holder {
                pid: info.Process.dwProcessId,
                name: from_wide(&info.strAppName),
                app_type: info.ApplicationType.0,
            })
            .collect())
    }

    fn kill(pid: u32) -> Result<()> {
        unsafe {
            let handle = OpenProcess(PROCESS_TERMINATE, false, pid)
                .with_context(|| format!("could not open pid {pid}"))?;
            let result = TerminateProcess(handle, 1);
            let _ = CloseHandle(handle);
            result.with_context(|| format!("could not terminate pid {pid}"))?;
        }
        Ok(())
    }

    fn to_wide(value: &OsStr) -> Vec<u16> {
        value.encode_wide().chain(std::iter::once(0)).collect()
    }

    fn from_wide(buffer: &[u16]) -> String {
        let end = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        String::from_utf16_lossy(&buffer[..end])
    }
}

#[cfg(not(windows))]
mod platform {
    use anyhow::Result;

    use super::Lock;

    pub fn who_holds(_path: &str) -> Result<Lock> {
        anyhow::bail!(
            "`swiss unlock` is only available on Windows (it relies on the Restart Manager API)"
        )
    }

    pub fn terminate(_pid: u32) -> Result<()> {
        anyhow::bail!(
            "`swiss unlock` is only available on Windows (it relies on the Restart Manager API)"
        )
    }
}
