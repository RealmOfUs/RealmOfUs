//! rou keeps native Git path and revision habits; it discovers the realm from cwd.
use clap::{Args, CommandFactory, Parser, Subcommand};
use rou_core::selection::Selection;
use rou_git::{
    Context, Repository, Result, build::BuildOptions, ensure, lock::WriteLock, snapshot::Mode,
};
use serde_json::Value;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

mod help;
mod report;

#[derive(Parser)]
#[command(
    name = "rou",
    version,
    about = "Distributed Realm authoring with native Git and Git LFS",
    long_about = help::OVERVIEW,
    after_help = "Any other command is forwarded to Git unchanged; run \"git help <command>\".",
    after_long_help = help::TOPICS
)]
struct Cli {
    /// Print the machine-readable JSON report for built-in commands.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Debug, Clone, Default)]
struct Select {
    /// Select all entities and their authoring/runtime dependencies.
    #[arg(long, conflicts_with_all = ["chunk", "entity"], long_help = help::ALL)]
    all: bool,
    /// Select x,y,z or x,y,z..x,y,z, including both endpoints; repeatable.
    #[arg(long, value_name = "SELECTOR", allow_hyphen_values = true, long_help = help::CHUNK)]
    chunk: Vec<String>,
    /// Select an entity ID; repeatable and combinable with --chunk.
    #[arg(long, value_name = "ID", long_help = help::ENTITY)]
    entity: Vec<String>,
}
impl Select {
    fn value(&self) -> Selection {
        Selection {
            all: self.all,
            chunks: self.chunk.clone(),
            entities: self.entity.clone(),
        }
    }
    fn chosen(&self) -> Option<Selection> {
        let value = self.value();
        if value.is_empty() { None } else { Some(value) }
    }
}

#[derive(Args)]
struct Raw {
    /// Native Git diff arguments, or --realm [--staged] [A [B]].
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<OsString>,
}

#[derive(Subcommand)]
enum Sparse {
    /// Replace the local selection and obtain its LFS dependencies.
    #[command(long_about = help::SPARSE_SET, after_long_help = help::SPARSE_SET_EXAMPLES)]
    Set(Select),
    /// Extend the local selection and obtain its LFS dependencies.
    #[command(long_about = help::SPARSE_ADD, after_long_help = help::SPARSE_ADD_EXAMPLES)]
    Add(Select),
    /// Print saved selectors without changing files.
    #[command(long_about = help::SPARSE_LIST, after_long_help = "Example:\n  rou sparse list")]
    List,
}

#[derive(Subcommand)]
enum Command {
    /// Create Git/LFS templates without staging or committing.
    #[command(long_about = help::INIT, after_long_help = help::INIT_EXAMPLES)]
    Init {
        /// Directory to initialize; must be the Git root if already a repository.
        #[arg(default_value = ".")]
        directory: PathBuf,
    },
    /// Clone metadata, keeping unselected LFS payloads cold.
    #[command(long_about = help::CLONE, after_long_help = help::CLONE_EXAMPLES)]
    Clone {
        /// Git remote URL or local repository path.
        source: String,
        /// Destination; defaults to the source's last path component without .git.
        directory: Option<PathBuf>,
        #[command(flatten)]
        select: Select,
    },
    /// Manage local entity/chunk selections and LFS dependencies.
    #[command(visible_alias = "sparse-checkout", long_about = help::SPARSE, after_long_help = help::SPARSE_EXAMPLES)]
    Sparse {
        #[command(subcommand)]
        action: Sparse,
    },
    /// Switch or create a branch and reapply the local selection.
    #[command(long_about = help::SWITCH, after_long_help = help::SWITCH_EXAMPLES)]
    Switch {
        /// Create and switch to this new branch, starting at REVISION or HEAD.
        #[arg(short = 'c')]
        branch: Option<String>,
        /// Existing branch to switch to, or starting commit when using -c.
        revision: Option<String>,
        #[command(flatten)]
        select: Select,
    },
    /// Switch branches, inspect a detached commit, or replace the selection.
    #[command(long_about = help::CHECKOUT, after_long_help = help::CHECKOUT_EXAMPLES)]
    Checkout {
        /// Create and switch to this new branch, starting at REVISION or HEAD.
        #[arg(short = 'b')]
        branch: Option<String>,
        /// Local branch, tag or commit to check out [default: HEAD].
        revision: Option<String>,
        #[command(flatten)]
        select: Select,
    },
    /// Show Git status and local selection without running validation.
    #[command(long_about = help::STATUS, after_long_help = help::STATUS_EXAMPLES)]
    Status,
    /// Show native Git differences or a --realm change summary.
    #[command(long_about = help::DIFF, after_long_help = help::DIFF_EXAMPLES)]
    Diff(Raw),
    /// Build frozen inputs and write per-model GLB/records to runtime/.
    #[command(long_about = help::BUILD, after_long_help = help::BUILD_EXAMPLES)]
    Build {
        /// Read inputs from the Git index; outputs still go to the worktree.
        #[arg(long, visible_alias = "cached")]
        staged: bool,
        /// Number of successful .rou/build/run-* snapshots to retain.
        #[arg(
            long,
            default_value_t = 1,
            value_name = "N",
            long_help = "Keep the newest N successful build snapshots under .rou/build/. Zero removes all successful snapshots, including this run. Failed runs are retained for diagnosis. The JSON report sets run to null when N is zero. This does not remove the published runtime/ outputs."
        )]
        keep_build_runs: usize,
    },
    /// Validate a commit (HEAD by default) or the Git index.
    #[command(long_about = help::VALIDATE, after_long_help = help::VALIDATE_EXAMPLES)]
    Validate {
        /// Validate the Git index instead of a commit; conflicts with REVISION.
        #[arg(long, visible_alias = "cached", conflicts_with = "revision")]
        staged: bool,
        /// Commit, tag or branch to validate [default: HEAD].
        revision: Option<String>,
    },
    /// Validate the staged world, create a Git commit and check its result.
    #[command(long_about = help::COMMIT, after_long_help = help::COMMIT_EXAMPLES)]
    Commit {
        /// Required nonempty commit message, passed to Git.
        #[arg(short, long)]
        message: String,
        /// Override Git's author, for example 'Name <name@example.com>'.
        #[arg(long)]
        author: Option<String>,
    },
    /// Validate and merge a revision, leaving divergent merges pending.
    #[command(long_about = help::MERGE, after_long_help = help::MERGE_EXAMPLES)]
    Merge {
        /// Abort the pending merge using Git and refresh the local selection.
        #[arg(long, conflicts_with_all = ["revision", "ff_only"])]
        abort: bool,
        /// Refuse divergent history; only fast-forward or already-merged is allowed.
        #[arg(long)]
        ff_only: bool,
        /// Branch, tag or commit to merge; required unless --abort is used.
        revision: Option<String>,
    },
    /// Fetch a remote branch and integrate it only by fast-forward.
    #[command(long_about = help::PULL, after_long_help = help::PULL_EXAMPLES)]
    Pull {
        /// Explicitly request the default; pull always uses fast-forward only.
        #[arg(long)]
        ff_only: bool,
        /// Git remote [default: current branch's remote, then origin].
        remote: Option<String>,
        /// Remote branch [default: configured upstream branch, then local branch].
        branch: Option<String>,
    },
    /// Validate and push a local branch together with required LFS history.
    #[command(long_about = help::PUSH, after_long_help = help::PUSH_EXAMPLES)]
    Push {
        /// Record the same-name upstream after a successful push.
        #[arg(short = 'u', long = "set-upstream")]
        upstream: bool,
        /// Git remote [default: current branch's remote, then origin].
        remote: Option<String>,
        /// Local branch to push to the same branch name on REMOTE [default: current branch].
        branch: Option<String>,
    },
    /// Export a fixed Git version or an explicit built worktree draft.
    #[command(long_about = help::VIEW, after_long_help = help::VIEW_EXAMPLES)]
    View {
        /// Commit, tag or branch to export [default: HEAD].
        revision: Option<String>,
        /// Bundle directory, relative to the calling directory.
        #[arg(
            long,
            value_name = "DIRECTORY",
            long_help = "Destination directory, relative to the calling directory. Defaults to <realm-root>/.rou/view/<commit-oid>, or .rou/view/worktree with --worktree. An existing bundle or empty directory may be replaced. Ancestor directory symlinks such as /tmp are allowed and resolved before writing; the reported catalog path uses that resolved parent. Bundle-root and internal symlinks, unrelated files and the authoring root are rejected."
        )]
        output: Option<PathBuf>,
        #[command(flatten)]
        select: Select,
        /// Export an already-built working draft; conflicts with REVISION.
        #[arg(long, conflicts_with = "revision")]
        worktree: bool,
    },
    #[command(external_subcommand)]
    Native(Vec<OsString>),
}

enum Output {
    Json(Value),
    Text(String),
    Exit(i32),
}

fn run(command: Command, mut machine: bool) -> Result<Output> {
    let kind = report::kind(&command);
    if let Command::Init { directory } = command {
        let repo = Repository::init(&directory)?;
        return finish(&repo, Output::Json(repo.status_json()?), kind, machine);
    }
    if let Command::Clone {
        source,
        directory,
        select,
    } = command
    {
        let directory = directory.unwrap_or_else(|| {
            PathBuf::from(
                source
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or("realm")
                    .trim_end_matches(".git"),
            )
        });
        let repo = Repository::clone(&source, &directory)?;
        if let Some(selection) = select.chosen() {
            let lock = WriteLock::acquire(&repo)?;
            lock.install_signal_cleanup()?;
            repo.sparse("set", selection)?;
        }
        return finish(&repo, Output::Json(repo.status_json()?), kind, machine);
    }
    let repo = Repository::discover(Path::new("."))?;
    let mutating = match &command {
        Command::Status
        | Command::Validate { .. }
        | Command::View { .. }
        | Command::Diff(_)
        | Command::Sparse {
            action: Sparse::List,
        } => false,
        Command::Native(args) => {
            let read_only = args
                .first()
                .is_some_and(|c| ["log", "show", "reflog"].iter().any(|v| c == v))
                || (args.first().is_some_and(|c| c == "lfs")
                    && args.get(1).is_some_and(|c| {
                        ["env", "ls-files", "status", "version"]
                            .iter()
                            .any(|v| c == v)
                    }));
            !read_only
        }
        _ => true,
    };
    let _lock = if mutating {
        let lock = WriteLock::acquire(&repo)?;
        lock.install_signal_cleanup()?;
        Some(lock)
    } else {
        None
    };
    let result = match command {
        Command::Status => {
            if machine {
                Output::Json(repo.status_json()?)
            } else {
                Output::Text(report::status(&repo)?)
            }
        }
        Command::Sparse { action } => match action {
            Sparse::List => Output::Json(serde_json::to_value(repo.selection()?)?),
            Sparse::Set(select) => sparse_output(repo.sparse("set", select.value())?, machine)?,
            Sparse::Add(select) => sparse_output(repo.sparse("add", select.value())?, machine)?,
        },
        Command::Switch {
            branch,
            revision,
            select,
        } => Output::Json(repo.switch(
            revision.as_deref().unwrap_or("HEAD"),
            branch.as_deref(),
            false,
            select.chosen().as_ref(),
        )?),
        Command::Checkout {
            branch,
            revision,
            select,
        } => {
            if branch.is_none() && revision.is_none() && select.chosen().is_some() {
                sparse_output(repo.sparse("set", select.value())?, machine)?
            } else {
                Output::Json(repo.switch(
                    revision.as_deref().unwrap_or("HEAD"),
                    branch.as_deref(),
                    true,
                    select.chosen().as_ref(),
                )?)
            }
        }
        Command::Build {
            staged,
            keep_build_runs,
        } => Output::Json(serde_json::to_value(repo.build(&BuildOptions {
            staged,
            keep_build_runs,
        })?)?),
        Command::Validate { staged, revision } => {
            let mode = if staged {
                Mode::Index
            } else {
                Mode::Commit(repo.resolve(revision.as_deref().unwrap_or("HEAD"))?)
            };
            Output::Json(serde_json::to_value(repo.validate(mode, true)?.report)?)
        }
        Command::Commit { message, author } => {
            Output::Json(repo.commit(&message, author.as_deref())?)
        }
        Command::Merge {
            abort,
            ff_only,
            revision,
        } => Output::Json(repo.merge(revision.as_deref(), ff_only, abort)?),
        Command::Pull {
            ff_only: _,
            remote,
            branch,
        } => Output::Json(repo.pull(remote.as_deref(), branch.as_deref())?),
        Command::Push {
            upstream,
            remote,
            branch,
        } => Output::Json(repo.push(remote.as_deref(), branch.as_deref(), upstream)?),
        Command::View {
            revision,
            output,
            select,
            worktree,
        } => {
            let mode = if worktree {
                Mode::Worktree
            } else {
                Mode::Commit(repo.resolve(revision.as_deref().unwrap_or("HEAD"))?)
            };
            let report = repo.view(mode, output.as_deref(), select.chosen().as_ref())?;
            if machine {
                Output::Json(serde_json::to_value(report)?)
            } else {
                Output::Text(report::view(&report))
            }
        }
        Command::Diff(raw) => {
            let mut realm = false;
            let mut paths = false;
            let mut args = Vec::new();
            for arg in raw.args {
                if arg == "--" {
                    paths = true;
                }
                if arg == "--json" && !paths {
                    machine = true;
                } else if arg == "--realm" && !paths {
                    realm = true;
                } else {
                    args.push(arg);
                }
            }
            if realm {
                Output::Json(repo.domain_diff(&args)?)
            } else {
                args.insert(0, "diff".into());
                Output::Exit(repo.from_caller(&args)?)
            }
        }
        Command::Native(args) => {
            let name = args
                .first()
                .and_then(|v| v.to_str())
                .context("missing Git command")?;
            ensure!(
                "invalid_argument",
                ![
                    "checkout",
                    "switch",
                    "commit",
                    "merge",
                    "pull",
                    "push",
                    "diff",
                    "status",
                    "sparse-checkout",
                    "init",
                    "clone"
                ]
                .contains(&name),
                "{name} is reserved; run rou {name} --help"
            );
            Output::Exit(repo.from_caller(&args)?)
        }
        Command::Init { .. } | Command::Clone { .. } => {
            unreachable!("handled before repository discovery")
        }
    };
    finish(&repo, result, kind, machine)
}

fn sparse_output(report: rou_git::sparse::SparseReport, machine: bool) -> Result<Output> {
    let value = serde_json::to_value(&report)?;
    if machine {
        Ok(Output::Json(value))
    } else {
        Ok(Output::Text(report::sparse(
            &value,
            report.materialized,
            report.already_present,
        )))
    }
}

fn finish(repo: &Repository, output: Output, kind: &str, machine: bool) -> Result<Output> {
    if machine {
        return Ok(output);
    }
    use std::io::Write;
    for native in repo.take_output() {
        std::io::stdout().write_all(&native.stdout)?;
        std::io::stdout().flush()?;
        std::io::stderr().write_all(&native.stderr)?;
    }
    let kind = if kind == "merge"
        && !repo
            .status(["rev-parse", "--verify", "--quiet", "MERGE_HEAD"])?
            .status
            .success()
    {
        "integrated"
    } else {
        kind
    };
    Ok(match output {
        Output::Json(value) => Output::Text(report::render(kind, &value)),
        other => other,
    })
}

fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    let native_help = args.get(1).is_some_and(|arg| arg == "help")
        && args
            .get(2)
            .and_then(|arg| arg.to_str())
            .is_some_and(|name| {
                !name.starts_with('-') && Cli::command().find_subcommand(name).is_none()
            });
    let cli = if native_help {
        Cli {
            json: false,
            command: Command::Native(args[1..].to_vec()),
        }
    } else {
        Cli::parse()
    };
    let result = run(cli.command, cli.json);
    match result {
        Ok(Output::Json(value)) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).expect("JSON result")
            );
        }
        Ok(Output::Text(text)) => print!("{text}"),
        Ok(Output::Exit(code)) => std::process::exit(code),
        Err(error) => {
            eprintln!("rou [{}]: {error:#}", rou_git::error::code(&error));
            std::process::exit(1);
        }
    }
}
