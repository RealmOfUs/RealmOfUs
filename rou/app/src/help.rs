//! Extended CLI help; execution and argument parsing remain in the binary entry point.

pub const OVERVIEW: &str = "Work with a Realm using native Git history and Git LFS payloads.\n\n\
    Run commands inside a Realm working directory; init and clone also work outside one. \
    Git paths are relative to the calling directory. Realm entity IDs and chunk coordinates \
    refer to the whole world. Built-in commands use -h for a summary or --help for \
    details and examples.";

pub const TOPICS: &str = "Workflow:
  rou init my-realm
  cd my-realm
  # Save sources and register models/entities, then:
  rou build
  rou add .
  rou commit -m 'Create a realm'
  rou view HEAD

Selection:
  rou sparse set --chunk=0,0,0..2,0,0
  rou sparse add --entity house
  rou sparse set --all
  Chunk ranges include BOTH endpoints; they are not Git commit ranges.

Any other command is forwarded to Git unchanged; run \"git help <command>\".

Details:
  rou help sparse set
  rou build --help
  rou diff --help
  rou view --help
  Wrapped commands accept only the options shown in their help.

Output:
  Realm commands default to human-readable summaries; --json prints their JSON reports.
  Native Git commands and plain diff keep Git's output and exit status.
  Realm operation errors go to stderr and exit with status 1;
  argument errors exit with status 2.";

pub const CHUNK: &str = "Select a chunk x,y,z or a box x,y,z..x,y,z, using integer chunk \
    coordinates (not metres). Use canonical decimal integers without spaces, leading \
    plus signs, leading zeroes or negative zero. Repeat --chunk to combine selections.\n\n\
    Both endpoints are INCLUDED on every axis. Each axis uses min/max independently, \
    so reversed endpoints are allowed. Overlapping selections are deduplicated. \
    A..A selects one chunk; 0,0,0..2,0,0 selects three chunks. A 3D range selects \
    the full box, not just its diagonal.\n\n\
    Negative coordinates are supported: --chunk=-1,0,-1..1,0,1 selects nine chunks. \
    All selected chunks must lie within the Realm; out-of-world ranges are rejected, \
    not clipped. At most 32768 distinct chunks may be selected per invocation.\n\n\
    Entities are selected when their world bounds intersect any selected chunk, \
    even if their origin belongs to another chunk.\n\n\
    This closed chunk selection differs from half-open spatial bounds [min,max). \
    It also differs from Git log A..B, which selects commits reachable from B \
    but not from A; Git log A..A selects no commits.";

pub const ALL: &str = "Select all current entities and their model sources, declared dependencies \
    and runtime assets. Disables Git sparse-checkout. Mutually exclusive with --chunk \
    and --entity. Other LFS files brought into the worktree may remain pointers.";

pub const ENTITY: &str = "Select an existing entity by its ID, not a model ID or path. Repeat \
    --entity to select multiple entities, or combine it with --chunk to take their union. \
    Shared models and dependencies are downloaded once. IDs use 1-80 lowercase ASCII \
    letters, digits, underscores or hyphens.";

pub const INIT: &str = "Initialize a Realm at DIRECTORY (default: current directory). \
    Creates a Git repository on main when needed, installs local LFS hooks with automatic \
    smudging disabled, and adds missing .gitignore/.gitattributes template lines.\n\n\
    Creates realm.json with a new Realm ID and the default profile if absent; an existing \
    realm.json is validated and preserved. The Realm identity and profile are immutable \
    after initialization. Creates realm/models/, realm/entities/, sources/, runtime/ \
    and docs/. Existing repositories must be initialized at their Git root.\n\n\
    Requires Git and Git LFS. Does not stage files or create a commit.";
pub const INIT_EXAMPLES: &str = "Examples:
  rou init
  rou init my-realm
  cd my-realm
  rou add .
  rou commit -m 'Initialize realm'";

pub const CLONE: &str = "Clone Git history and check out Realm metadata with Git LFS payloads \
    kept as pointers. Without selectors, the initial cone includes realm/, runtime/ \
    and docs/ plus Git cone-mode root files; authoring payloads remain cold.\n\n\
    With --chunk, --entity or --all, resolve that selection at the cloned HEAD and \
    download the selected source files, declared dependencies and runtime assets. \
    Cone checkout may include other pointer files without downloading their contents. \
    Requires Git and Git LFS; authentication uses your Git configuration.";
pub const CLONE_EXAMPLES: &str = "Examples:
  rou clone /path/to/realm.git village
  rou clone /path/to/realm.git village --chunk=-1,0,-1..1,0,1
  rou clone /path/to/realm.git village --entity house --entity bench
  rou clone /path/to/realm.git village --all";

pub const SPARSE: &str = "Manage the local authoring selection and the source/runtime LFS files \
    needed by it. Selection is stored in .rou/selection.json, outside versioned world data. \
    The alias sparse-checkout accepts the same subcommands.\n\n\
    set replaces the selection, add takes its union with the current selection, and \
    list prints the saved selectors. Changes are resolved against HEAD and require \
    a commit with valid Realm metadata/build records and no unresolved Git conflicts. \
    Selecting a chunk includes every intersecting entity and its model dependencies.\n\n\
    Shrinking a cone is refused if it would remove modified, staged or untracked \
    non-ignored files. Ignored files outside the new cone are moved to .rou/backups/ \
    and their backup paths are reported.";
pub const SPARSE_EXAMPLES: &str = "Examples:
  rou sparse set --chunk=0,0,0..2,0,0
  rou sparse add --entity house
  rou sparse list
  rou sparse-checkout set --all
  rou sparse set --help";
pub const SPARSE_SET: &str = "Replace the saved local selection and materialize its LFS dependencies \
    at HEAD. Requires at least one selector; --all cannot be combined with individual \
    selectors. Metadata stays available even when no entity intersects a selected chunk.\n\n\
    Shrinking refuses to remove modified, staged or untracked non-ignored files. \
    Ignored files outside the new cone are backed up under .rou/backups/. Returns \
    resolved entities, payloads and backup paths; use --json for all report fields.";
pub const SPARSE_SET_EXAMPLES: &str = "Examples:
  rou sparse set --chunk=-1,0,-1..1,0,1
  rou sparse set --chunk=0,0,0 --entity house
  rou sparse set --all";
pub const SPARSE_ADD: &str = "Add selectors to the current local selection and materialize the \
    resulting LFS dependencies at HEAD. Requires at least one selector. Repeated \
    selectors and overlapping chunks are deduplicated. If the current or new selection \
    is --all, the result remains all; use sparse set to narrow it. Returns the resolved \
    selection and backup paths; use --json for all report fields.";
pub const SPARSE_ADD_EXAMPLES: &str = "Examples:
  rou sparse add --chunk=2,0,0..3,0,0
  rou sparse add --entity house --entity bench
  rou sparse add --all";
pub const SPARSE_LIST: &str = "Print the saved all, chunk and entity selectors on three lines; --json emits their JSON fields. \
    This is the selector configuration, not an expanded list of chunks or a check of \
    downloaded payload integrity. Does not change the worktree or download files.";

pub const SWITCH: &str = "Switch to a branch after validating a changed target commit, then \
    reapply the local authoring selection and obtain its required LFS payloads. Without \
    selectors the saved selection is retained; supplied selectors replace it.\n\n\
    A branch name is required unless -c creates a branch. With -c, REVISION is the \
    starting commit and defaults to HEAD. For detached inspection use checkout REVISION. \
    Git's local-change checks and Realm sparse-shrink protections still apply.";
pub const SWITCH_EXAMPLES: &str = "Examples:
  rou switch main
  rou switch -c roof-work
  rou switch -c review v1 --entity house
  rou switch main --chunk=-1,0,0..1,0,0";

pub const CHECKOUT: &str = "Check out a local branch, inspect a commit with detached HEAD, or \
    create a branch with -b. REVISION defaults to HEAD. A changed target commit is \
    validated before switching; the saved selection is then reapplied. Supplied \
    selectors replace the saved selection.\n\n\
    With only selection options (no REVISION or -b), this is equivalent to sparse set \
    and does not switch commits. This command does not accept Git's path checkout \
    syntax; use rou restore [Git options] -- PATH for file restoration. Git local-change \
    checks and Realm sparse-shrink protections still apply.";
pub const CHECKOUT_EXAMPLES: &str = "Examples:
  rou checkout main
  rou checkout HEAD~1
  rou checkout -b review v1
  rou checkout --chunk=0,0,0..2,0,0
  rou restore -- sources/house/house.blend";

pub const STATUS: &str = "Report Git status and the saved local selection. The default output \
    is text; --json adds repository root, HEAD, porcelain-v2 Git status and LFS \
    worktree information.\n\n\
    Does not run Realm validation or download payloads. In JSON, domain_validation \
    is not_run. An LFS '*' indicates materialized bytes and '-' a pointer or absent \
    payload; presence does not certify integrity. Use validate for consistency checks.";
pub const STATUS_EXAMPLES: &str = "Examples:
  rou status
  rou status --json";

pub const DIFF: &str = "Without --realm, forward arguments to git diff unchanged, preserving \
    Git revision, index/worktree, path and output semantics. Paths are relative to \
    the calling directory. Use -- before paths that could look like revisions/options.\n\n\
    With --realm before any -- separator, show changes grouped by entity and model, including \
    placement, input and world-rule changes. This mode reads metadata and descriptors; \
    it does not download cold payloads, inspect geometry bytes or run Blender.";
pub const DIFF_EXAMPLES: &str = "Realm forms (only these arguments are supported):
  rou diff --realm                 Index -> worktree
  rou diff --realm A               Commit A -> worktree
  rou diff --realm A B             Commit A -> commit B
  rou diff --realm --staged        HEAD -> index (empty base before the first commit)
  rou diff --realm --staged A      Commit A -> index
  --cached is an alias for --staged. No path filters or A..B syntax in --realm mode.

Native examples:
  rou diff
  rou diff --staged
  rou diff HEAD~1 HEAD -- realm/
  rou diff -- --realm              Treat --realm as a path
  git help diff                   Full native Git diff manual

Ranges:
  git diff A..B compares two trees; git log A..B selects commits reachable
  from B but not from A. Neither has the inclusive chunk-box meaning of --chunk.";

pub const BUILD: &str = "Build from a frozen snapshot of working files by default, or from \
    the Git index with --staged. Reuse matching per-model records and assets; export \
    changed model inputs through Blender. Required cold LFS payloads may be fetched. \
    The build covers the logical world, not just the local sparse selection.\n\n\
    Write runtime/<model>.glb, runtime/<model>.lod1.glb and runtime/<model>.json to the WORKTREE in both modes, \
    remove obsolete runtime outputs, and report generated/reused/removed files; --json emits the full report. \
    Never stage files or commit; explicitly rou add the intended inputs and outputs. \
    Resolve author-input conflicts first; stale or conflicted runtime outputs can be rebuilt.\n\n\
    In sparse worktrees, unselected runtime deletions or damaged GLBs report stale_runtime. \
    Select the entity before repairing it (rou sparse add --entity ID). Missing tracked \
    build records and runtime symlinks are checked before any fetch or Blender export. \
    Chunk-based record repairs need fresh captured bounds; use --entity when unavailable. \
    Matching cold pointers are reused without downloading their GLBs. Index changes from \
    snapshot capture through publication report snapshot_changed.\n\n\
    Exporter setup: ROU_BLENDER is an executable path (otherwise search PATH, then \
    /Applications/Blender.app/Contents/MacOS/Blender). ROU_TOOLCHAIN_ROOT is the \
    RealmOfUs checkout containing toolchain/blender/export_git_realm.py; otherwise \
    search ancestors of the executable and current directory.";
pub const BUILD_EXAMPLES: &str = "Examples:
  rou build
  rou build --staged
  rou build --keep-build-runs 3
  rou add realm/ sources/ runtime/
  rou commit -m 'Update the house'";

pub const VALIDATE: &str = "Validate a committed Realm (default: HEAD), or the Git index with \
    --staged. Checks the whole logical world: immutable Realm metadata, references, \
    input/build consistency, spatial budgets, exact runtime file set and LFS descriptors. \
    This command does not default to the worktree and has no --worktree mode.\n\n\
    Checks available GLB bytes; new assets may trigger LFS downloads. Unchanged \
    assets referenced by parent build records may stay cold. A successful metadata \
    check therefore need not mean every asset's bytes were inspected: read \
    byte_validation_complete, validated_assets and missing_assets in the JSON report. \
    Does not build, stage files or create a commit.";
pub const VALIDATE_EXAMPLES: &str = "Examples:
  rou validate
  rou validate v1
  rou validate --staged
  rou validate --cached";

pub const COMMIT: &str = "Validate the entire Git index, then create a native Git commit using \
    the required message. Does not build or stage files; source inputs and runtime \
    outputs in the index must already match. Unstaged changes are left in the worktree, \
    so a consistent staged version can be committed independently.\n\n\
    Uses the user's Git identity and hooks. Rechecks the resulting commit. If Git \
    creates a commit but the post-check fails, the error reports the existing commit; \
    it is not automatically rolled back. Asset validation follows validate's cold-payload \
    rules.";
pub const COMMIT_EXAMPLES: &str = "Examples:
  rou build
  rou add realm/ sources/ runtime/
  rou commit -m 'Update the house'
  rou commit -m 'Import contribution' --author 'Name <name@example.com>'";

pub const MERGE: &str = "Validate REVISION, then merge it into the current branch. Fast-forward \
    when possible; if already merged, keep the existing HEAD. A divergent merge \
    runs Git with --no-commit --no-ff and remains pending for review.\n\n\
    Resolve author-input conflicts, rebuild derived outputs if needed, stage the \
    intended result and run rou commit. Binary source conflicts require an explicit \
    resolution. --ff-only rejects divergence. --abort invokes Git's merge abort and \
    refreshes the saved selection; it cannot be combined with REVISION or --ff-only.";
pub const MERGE_EXAMPLES: &str = "Examples:
  rou merge feature/roof
  rou merge --ff-only origin/main
  rou merge --abort

After resolving a pending merge:
  rou build
  rou add realm/ sources/ runtime/
  rou commit -m 'Merge roof changes'";

pub const PULL: &str = "Fetch a remote branch, validate FETCH_HEAD, and merge it using \
    fast-forward only. This is ALWAYS the behavior, even without --ff-only; this \
    command does not rebase or create a divergent merge commit. On divergence the \
    fetch has completed but the merge fails. Use fetch followed by an explicit merge \
    when a divergent integration is intended.\n\n\
    REMOTE defaults to the current branch's configured remote, falling back to origin. \
    BRANCH defaults to its configured upstream branch, falling back to the current \
    local branch name. The saved local selection is refreshed after a successful merge.";
pub const PULL_EXAMPLES: &str = "Examples:
  rou pull
  rou pull --ff-only origin main

Explicit divergent integration:
  rou fetch origin
  rou merge origin/main";

pub const PUSH: &str = "Validate the tip of a local branch, upload its required reachable LFS \
    payloads, then push that exact commit to the same branch name on REMOTE. If \
    required LFS history is missing locally, try obtaining it from the configured \
    default remote before retrying the upload.\n\n\
    REMOTE defaults to the current branch's configured remote, falling back to origin. \
    BRANCH defaults to the current local branch. BRANCH is a local branch name, \
    not a refspec; force push and arbitrary Git push options are not supported here. \
    -u records the remote and same-name upstream after a successful push.";
pub const PUSH_EXAMPLES: &str = "Examples:
  rou push
  rou push -u origin main
  rou push origin feature/roof";

pub const VIEW: &str = "Export a fixed committed Realm (default: HEAD) as a standalone browse \
    bundle. --worktree explicitly exports an already-built working draft instead; \
    it cannot be combined with REVISION. This command does not build or launch the viewer.\n\n\
    Writes realm.json (the complete catalog), version.json and objects/<sha256>.glb. \
    By default, includes only locally available runtime assets in the saved sparse selection, \
    without any network downloads. An empty selection produces a catalog-only bundle. \
    --chunk and --entity select an explicit subset; --all selects every entity. Explicit \
    selectors may fetch required GLBs via LFS, but never authoring sources. The complete \
    catalog is retained, and missing models are shown as missing in the viewer. The same \
    selection rules apply to --worktree. Local materialized GLBs use hard links with a \
    copy fallback; treat bundle assets as read-only.\n\n\
    Bundles contain no viewing entry. The viewer derives a start from content or \
    lets the player choose; --at=x,y,z is a viewer argument in metres, not a rou view \
    option. A worktree bundle has no commit identity; local-trusted validation is \
    not a public signature. Reports the catalog path and version details; --json emits the full report.";
pub const VIEW_EXAMPLES: &str = "Examples:
  rou view
  rou view v1 --all --output ../village-view
  rou view HEAD --chunk=-1,0,-1..1,0,1 --output .rou/view/nearby
  rou view --entity house
  rou build
  rou view --worktree --output .rou/view/draft";
