//! Native Git process transport, raw path bytes and immutable object batching.
use crate::{Context, Result, ensure};
use bstr::{BString, ByteSlice};
use std::{
    cell::RefCell,
    ffi::{OsStr, OsString},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio},
};

/// Construct Git with repository-routing environment overrides removed and LFS smudge disabled.
pub fn command(directory: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.current_dir(directory);
    for key in [
        "GIT_DIR",
        "GIT_COMMON_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        cmd.env_remove(key);
    }
    cmd.env("GIT_LFS_SKIP_SMUDGE", "1")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .env("LC_ALL", "C");
    cmd
}

/// Capture a Git subprocess in `directory`, optionally providing stdin.
/// Nonzero exit status is returned for the caller to interpret.
pub fn execute<I, S>(directory: &Path, args: I, input: Option<&[u8]>) -> Result<Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut cmd = command(directory);
    cmd.args(args);
    execute_command(cmd, input)
}

/// Execute a prepared command, concurrently feeding stdin to avoid pipe deadlock.
pub fn execute_command(mut cmd: Command, input: Option<&[u8]>) -> Result<Output> {
    if input.is_some() {
        cmd.stdin(Stdio::piped());
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().context("start Git")?;
    if let Some(input) = input {
        // Concurrent writes avoid a pipe deadlock when both stdin and stdout are large.
        let mut stdin = child.stdin.take().expect("requested Git stdin");
        return std::thread::scope(|scope| {
            let writer = scope.spawn(move || stdin.write_all(input));
            let result = child.wait_with_output().context("wait for Git")?;
            let written = writer
                .join()
                .map_err(|_| anyhow::anyhow!("Git input writer panicked"))?;
            if result.status.success() {
                written?;
            }
            Ok(result)
        });
    }
    Ok(child.wait_with_output()?)
}

/// Return stdout on success, or a typed [`crate::error::GitFailure`] retaining diagnostics.
pub fn checked(output: Output) -> Result<Vec<u8>> {
    if !output.status.success() {
        return Err(crate::error::GitFailure {
            exit_code: output.status.code(),
            message: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            ),
        }
        .into());
    }
    Ok(output.stdout)
}

/// Convert raw Git path bytes to a platform path without lossy UTF-8 replacement.
pub fn os_path(bytes: &[u8]) -> Result<PathBuf> {
    Ok(PathBuf::from(bytes.to_os_str()?))
}
/// Return the native encoded path bytes used by Git.
pub fn bytes_path(path: &Path) -> Result<BString> {
    Ok(BString::from(
        path.as_os_str()
            .to_str()
            .map(str::as_bytes)
            .map(Vec::from)
            .unwrap_or_else(|| path.as_os_str().as_encoded_bytes().to_vec()),
    ))
}
/// Decode UTF-8 Git output and remove trailing newlines.
pub fn text(bytes: &[u8]) -> Result<&str> {
    Ok(std::str::from_utf8(bytes)?.trim_end_matches('\n'))
}

#[derive(Debug, Clone)]
/// Discovered native Git worktree and caller context; contains no cached mutable ref state.
pub struct Repository {
    /// Canonical native Git worktree root.
    pub root: PathBuf,
    /// Canonical directory used for caller-relative Git forwarding.
    pub caller: PathBuf,
    /// Absolute Git directory, including linked-worktree layouts.
    pub git_dir: PathBuf,
    pub(crate) output: RefCell<Vec<Output>>,
}
impl Repository {
    /// Discover the worktree root and actual Git directory from a canonical caller directory.
    pub fn discover(directory: &Path) -> Result<Self> {
        let caller = directory.canonicalize()?;
        let root = checked(execute(&caller, ["rev-parse", "--show-toplevel"], None)?)?;
        let root = os_path(root.strip_suffix(b"\n").unwrap_or(&root))?;
        let git_dir = checked(execute(&root, ["rev-parse", "--absolute-git-dir"], None)?)?;
        Ok(Self {
            root,
            caller,
            output: RefCell::default(),
            git_dir: os_path(git_dir.strip_suffix(b"\n").unwrap_or(&git_dir))?,
        })
    }
    /// Run checked Git at the repository root and return stdout.
    pub fn run<I, S>(&self, args: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        checked(execute(&self.root, args, None)?)
    }
    /// Run an operation and retain its successful stdout/stderr for host presentation.
    /// The returned bytes remain stdout only, preserving machine report fields.
    pub fn run_visible<I, S>(&self, args: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = execute(&self.root, args, None)?;
        let stdout = checked(output.clone())?;
        self.output.borrow_mut().push(output);
        Ok(stdout)
    }
    /// Drain successful operation output, preserving each native stream as bytes.
    pub fn take_output(&self) -> Vec<Output> {
        self.output.take()
    }
    /// Run checked Git at the repository root with byte-oriented stdin.
    pub fn input<I, S>(&self, args: I, input: &[u8]) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        checked(execute(&self.root, args, Some(input))?)
    }
    /// Run Git at the repository root without treating a nonzero exit as an error.
    pub fn status<I, S>(&self, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        execute(&self.root, args, None)
    }
    /// Forward native Git arguments from the caller directory with inherited standard streams.
    /// Preserve the native exit code, using 1 when terminated by a signal.
    pub fn from_caller(&self, args: &[OsString]) -> Result<i32> {
        Ok(command(&self.caller)
            .args(args)
            .status()?
            .code()
            .unwrap_or(1))
    }
    /// Resolve a non-option revision to a full immutable commit OID.
    pub fn resolve(&self, revision: &str) -> Result<String> {
        ensure!(
            "invalid_revision",
            !revision.is_empty() && !revision.starts_with('-'),
            "invalid revision"
        );
        Ok(text(&self.run([
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{revision}^{{commit}}"),
        ])?)?
        .to_owned())
    }
    /// Return the current commit OID, or `None` for an unborn HEAD.
    pub fn head(&self) -> Result<Option<String>> {
        let output = self.status(["rev-parse", "--verify", "HEAD"])?;
        if output.status.success() {
            Ok(Some(text(&output.stdout)?.to_owned()))
        } else {
            Ok(None)
        }
    }
    /// List commit parents, or current HEAD plus pending merge parents for index/worktree validation.
    pub fn parents(&self, commit: Option<&str>) -> Result<Vec<String>> {
        if let Some(commit) = commit {
            let raw = self.run(["rev-list", "--parents", "-n", "1", commit])?;
            return Ok(text(&raw)?
                .split_whitespace()
                .skip(1)
                .map(str::to_owned)
                .collect());
        }
        let mut parents: Vec<_> = self.head()?.into_iter().collect();
        let merge = self.git_dir.join("MERGE_HEAD");
        if merge.exists() {
            parents.extend(std::fs::read_to_string(merge)?.lines().map(str::to_owned));
        }
        parents.sort();
        parents.dedup();
        Ok(parents)
    }
    /// Read the current branch remote, falling back to `origin`.
    pub fn default_remote(&self) -> Result<String> {
        let current = self.status(["symbolic-ref", "--short", "-q", "HEAD"])?;
        if current.status.success() {
            let name = format!("branch.{}.remote", text(&current.stdout)?);
            let remote = self.status(["config", "--get", &name])?;
            if remote.status.success() {
                return Ok(text(&remote.stdout)?.to_owned());
            }
        }
        Ok("origin".into())
    }
    /// Start a persistent Git blob reader for this repository.
    pub fn batch(&self) -> Result<Batch> {
        Batch::new(&self.root)
    }
}

/// A persistent cat-file process. Requests use immutable OIDs, never user path text.
pub struct Batch {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}
impl Batch {
    /// Start `git cat-file --batch`; the process is killed and reaped on drop.
    pub fn new(root: &Path) -> Result<Self> {
        let mut child = command(root)
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        Ok(Self {
            input: child.stdin.take().expect("piped stdin"),
            output: BufReader::new(child.stdout.take().expect("piped stdout")),
            child,
        })
    }
    /// Read one blob within `max` bytes, returning `None` for a missing object.
    /// Reject non-blobs and drain oversized responses so subsequent requests stay synchronized.
    pub fn read(&mut self, object: &str, max: usize) -> Result<Option<Vec<u8>>> {
        ensure!(
            "invalid_argument",
            !object.contains(['\n', '\r', '\0']),
            "invalid object query"
        );
        writeln!(self.input, "{object}")?;
        self.input.flush()?;
        let mut header = String::new();
        self.output.read_line(&mut header)?;
        let parts: Vec<_> = header.split_whitespace().collect();
        if parts.len() == 2 && parts[1] == "missing" {
            return Ok(None);
        }
        ensure!(
            "invalid_git_response",
            parts.len() == 3 && parts[1] == "blob",
            "expected a Git blob: {header}"
        );
        let size: usize = parts[2].parse()?;
        // Drain rejected responses too, so a failed candidate cannot desynchronize the pipe.
        if size > max {
            std::io::copy(
                &mut self.output.by_ref().take(size as u64 + 1),
                &mut std::io::sink(),
            )?;
            crate::bail!("byte_limit", "Git blob exceeds byte budget: {size} > {max}");
        }
        let mut data = vec![0; size];
        self.output.read_exact(&mut data)?;
        let mut separator = [0];
        self.output.read_exact(&mut separator)?;
        ensure!(
            "invalid_git_response",
            separator == [b'\n'],
            "invalid cat-file response"
        );
        Ok(Some(data))
    }
}
impl Drop for Batch {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
