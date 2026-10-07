//! Freeze one snapshot, export each .blend once, verify, then replace only runtime outputs.
use crate::{
    Context, Result, ensure, io,
    repository::Repository,
    snapshot::{Mode, Snapshot},
};
use bstr::BString;
use rou_core::{
    blender,
    documents::{BlenderExport, Build, Descriptor, ExportMetrics},
    formats, glb, json, limits, paths,
    selection::Selection,
    world::World,
};
use serde::Serialize;
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[derive(Debug, Serialize)]
/// Outcome of a successful runtime build; failure is returned as an error instead.
pub struct BuildReport {
    /// True after the operation has completed successfully.
    pub ok: bool,
    /// Input snapshot label, or Git file mode for an index entry.
    pub mode: String,
    /// Model IDs exported from frozen Blender source files.
    pub generated: Vec<String>,
    /// Model IDs whose current build records were reused.
    pub reused: Vec<String>,
    /// Verified full and middle-detail statistics for generated models only.
    pub metrics: BTreeMap<String, ExportMetrics>,
    /// Advisory reference exceedances per generated model; never validation failures.
    pub warnings: BTreeMap<String, Vec<String>>,
    /// Retained build-run directory, or `None` when successful runs are not retained.
    pub run: Option<PathBuf>,
    /// All expected runtime asset and build-record paths after the build.
    pub outputs: Vec<String>,
    /// Obsolete runtime paths removed by the build.
    pub removed: Vec<String>,
    /// Canonical hash of the complete derived view.
    pub view_root_sha256: String,
}
#[derive(Debug, Clone)]
/// Snapshot selection and retention policy for runtime builds.
pub struct BuildOptions {
    /// Read author inputs from the index instead of the live worktree.
    pub staged: bool,
    /// Number of successful frozen runs to retain; zero removes the current successful run too.
    pub keep_build_runs: usize,
}
impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            staged: false,
            keep_build_runs: 1,
        }
    }
}

/// Find the Blender helper checkout, honoring `ROU_TOOLCHAIN_ROOT` first.
pub fn toolchain_root() -> Result<PathBuf> {
    if let Some(root) = std::env::var_os("ROU_TOOLCHAIN_ROOT") {
        return Ok(root.into());
    }
    for start in [
        std::env::current_exe()?
            .parent()
            .context("executable has no parent")?
            .to_path_buf(),
        std::env::current_dir()?,
    ] {
        for path in start.ancestors() {
            if path.join("toolchain/blender/export_git_realm.py").is_file() {
                return Ok(path.to_path_buf());
            }
        }
    }
    crate::bail!(
        "missing_tool",
        "Blender helper unavailable; set ROU_TOOLCHAIN_ROOT"
    )
}

fn blender_executable() -> PathBuf {
    if let Some(path) = std::env::var_os("ROU_BLENDER") {
        return path.into();
    }
    for directory in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        let candidate = directory.join("blender");
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from("/Applications/Blender.app/Contents/MacOS/Blender")
}

struct Baseline {
    head: Option<String>,
    branch: Vec<u8>,
    index: Vec<u8>,
    metadata: BTreeMap<BString, Descriptor>,
}
impl Baseline {
    fn capture(snapshot: &Snapshot, head: Option<String>, branch: Vec<u8>) -> Result<Self> {
        let mut metadata = BTreeMap::new();
        for path in &snapshot.paths {
            if path == paths::REALM.as_bytes() || path.starts_with(b"realm/") {
                metadata.insert(
                    path.clone(),
                    Descriptor::of(&snapshot.read(path, limits::JSON_BYTES)?),
                );
            }
        }
        Ok(Self {
            head,
            branch,
            index: snapshot.index.clone(),
            metadata,
        })
    }
    fn verify(&self, snapshot: &Snapshot, world: &World) -> Result<()> {
        ensure!(
            "snapshot_changed",
            snapshot.repo.head()? == self.head
                && snapshot
                    .repo
                    .status(["symbolic-ref", "--quiet", "HEAD"])?
                    .stdout
                    == self.branch
                && snapshot.repo.run(["ls-files", "--stage", "-z"])? == self.index,
            "HEAD or index changed during build"
        );
        if snapshot.mode != Mode::Worktree {
            return Ok(());
        }
        let current: BTreeSet<BString> = snapshot
            .repo
            .run([
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
                "-z",
            ])?
            .split(|c| *c == 0)
            .filter(|p| *p == paths::REALM.as_bytes() || p.starts_with(b"realm/"))
            .filter(|p| {
                snapshot.skipped.contains(*p)
                    || snapshot.conflicts.contains_key(*p)
                    || crate::repository::os_path(p)
                        .is_ok_and(|p| snapshot.repo.root.join(p).symlink_metadata().is_ok())
            })
            .map(BString::from)
            .collect();
        ensure!(
            "snapshot_changed",
            current == self.metadata.keys().cloned().collect(),
            "author metadata paths changed during build"
        );
        for (path, expected) in &self.metadata {
            let file = io::destination(&snapshot.repo.root, path)?;
            if file.exists() {
                expected.verify(&io::read(&file, limits::JSON_BYTES)?)?;
            } else {
                ensure!(
                    "snapshot_changed",
                    snapshot.skipped.contains(path),
                    "author metadata deleted during build"
                );
            }
        }
        for (path, expected) in world
            .inputs
            .values()
            .flat_map(|i| i.iter())
            .collect::<BTreeMap<_, _>>()
        {
            ensure!(
                "snapshot_changed",
                snapshot.descriptor(path)? == *expected,
                "author input changed during build: {path}"
            );
        }
        Ok(())
    }
}

impl Repository {
    /// Freeze inputs, export each changed source once and replace verified runtime outputs.
    /// Reuse matching pointers without fetching cold assets. Recheck author/index baselines
    /// before publication; never stage outputs. Requires a writer lock.
    pub fn build(&self, options: &BuildOptions) -> Result<BuildReport> {
        let mode = if options.staged {
            Mode::Index
        } else {
            Mode::Worktree
        };
        let head = self.head()?;
        let branch = self.status(["symbolic-ref", "--quiet", "HEAD"])?.stdout;
        let snapshot = Snapshot::new(self, mode)?;
        for path in snapshot.conflicts.keys() {
            ensure!(
                "unresolved_conflict",
                path.starts_with(paths::RUNTIME.as_bytes()),
                "resolve author input conflict first: {path}"
            );
        }
        let world = snapshot.world(true)?;
        let baseline = Baseline::capture(&snapshot, head, branch)?;
        baseline.verify(&snapshot, &world)?;
        let selection = self.selection()?;
        let sparse = self.status(["config", "--bool", "core.sparseCheckout"])?;
        preflight_records(
            &snapshot,
            &world,
            sparse.stdout.starts_with(b"true"),
            &selection,
        )?;
        let base = io::local(&self.root, "build")?;
        fs::create_dir_all(&base)?;
        let run = base.join(format!(
            "run-{}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis(),
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir(&run)?;
        let mut records = snapshot.records(&world, true)?;
        let mut groups: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (id, model) in &world.models {
            if !records.contains_key(id) {
                groups.entry(&model.source).or_default().push(id);
            }
        }
        let mut generated = Vec::new();
        let mut metrics = BTreeMap::new();
        let mut warnings = BTreeMap::new();
        let mut reused = Vec::new();
        let mut outputs: BTreeMap<String, PathBuf> = BTreeMap::new();
        for (group, (source, ids)) in groups.into_iter().enumerate() {
            let folder = run.join(format!("source-{group:04}"));
            let exported = export_source(&snapshot, &world, source, &ids, &folder)?;
            for (id, (record, sizes)) in exported {
                for asset in [&record.asset, &record.lod1] {
                    outputs.insert(asset.path.clone(), folder.join("output").join(&asset.path));
                }
                let notes: Vec<_> = [("asset", &sizes.asset), ("lod1", &sizes.lod1)]
                    .into_iter()
                    .flat_map(|(tier, report)| {
                        report
                            .warnings()
                            .into_iter()
                            .map(move |note| format!("{tier}: {note}"))
                    })
                    .collect();
                warnings.insert(id.clone(), notes);
                metrics.insert(id.clone(), sizes);
                generated.push(id.clone());
                records.insert(id, record);
            }
        }
        let view = world.projections(&records)?;
        let selected = selection.resolve(&world, &view)?.assets;
        for (id, record) in &records {
            if generated.contains(id) {
                continue;
            }
            for asset in [&record.asset, &record.lod1] {
                let path = &asset.path;
                let wanted = asset.descriptor();
                // A matching pointer is a complete reusable output. Do not fetch or
                // materialize cold assets merely to reuse their build metadata.
                let destination = snapshot.working_path(path)?;
                let intact = destination.is_file()
                    && !snapshot.conflicts.contains_key(path.as_bytes())
                    && snapshot
                        .working_descriptor(path)
                        .is_ok_and(|actual| actual == wanted);
                if intact {
                    continue;
                }
                if sparse.stdout.starts_with(b"true") && !selected.contains(path) {
                    ensure!(
                        "stale_runtime",
                        !destination.exists()
                            && snapshot.skipped.contains(path.as_bytes())
                            && snapshot.entries.get(path.as_bytes()).is_some_and(|entry| {
                                snapshot
                                    .read_entry(entry, limits::POINTER_BYTES)
                                    .ok()
                                    .and_then(|bytes| rou_core::lfs::pointer(&bytes).ok().flatten())
                                    .as_ref()
                                    == Some(&wanted)
                            })
                            && !snapshot.conflicts.contains_key(path.as_bytes()),
                        "select the entity before repairing its runtime output: {path}"
                    );
                    continue;
                }
                // Repair selected missing, conflicted or stale outputs only.
                let output = snapshot.payload_path(path, &wanted, true)?;
                let bytes = io::read(&output, limits::GLB_BYTES)?;
                wanted.verify(&bytes)?;
                glb::precheck(&bytes)?;
                if output != destination {
                    let frozen = run.join("reused").join(path);
                    io::atomic_write(&frozen, &bytes)?;
                    outputs.insert(path.clone(), frozen);
                }
            }
            reused.push(id.clone());
        }
        baseline
            .verify(&snapshot, &world)
            .with_context(|| format!("frozen outputs retained at {}", run.display()))?;
        let mut expected = BTreeSet::new();
        for id in records.keys() {
            expected.insert(paths::asset(id));
            expected.insert(paths::lod1(id));
            expected.insert(paths::build(id));
        }
        let runtime = io::destination(&self.root, b"runtime")?;
        let mut obsolete = Vec::new();
        if runtime.exists() {
            for entry in fs::read_dir(&runtime)? {
                let entry = entry?;
                ensure!(
                    "invalid_layout",
                    entry.file_type()?.is_file(),
                    "unexpected runtime directory or symlink: {}",
                    entry.path().display()
                );
                let relative = format!(
                    "{}{}",
                    paths::RUNTIME,
                    entry
                        .file_name()
                        .to_str()
                        .context("runtime filenames must be UTF-8")?
                );
                ensure!(
                    "invalid_layout",
                    relative.ends_with(".json") || relative.ends_with(".glb"),
                    "unrecognized runtime file: {relative}"
                );
                if !expected.contains(&relative) {
                    obsolete.push(relative);
                }
            }
        }
        // Preflight all destinations before making any replacement or deletion.
        for path in expected.iter().chain(&obsolete) {
            io::destination(&self.root, path.as_bytes())?;
        }
        for (relative, source) in &outputs {
            io::atomic_write(
                &io::destination(&self.root, relative.as_bytes())?,
                &io::read(source, limits::GLB_BYTES)?,
            )?;
        }
        for (id, record) in &records {
            io::write_json(
                &io::destination(&self.root, paths::build(id).as_bytes())?,
                record,
            )?;
        }
        for relative in &obsolete {
            fs::remove_file(io::destination(&self.root, relative.as_bytes())?)?;
        }
        generated.sort();
        reused.sort();
        obsolete.sort();
        ensure!(
            "snapshot_changed",
            self.run(["ls-files", "--stage", "-z"])? == baseline.index,
            "index changed during build publication"
        );
        let report = BuildReport {
            ok: true,
            mode: snapshot.mode.label().to_lowercase(),
            generated,
            reused,
            metrics,
            warnings,
            run: (options.keep_build_runs != 0).then(|| run.clone()),
            outputs: expected.into_iter().collect(),
            removed: obsolete,
            view_root_sha256: view.root_hash()?,
        };
        io::write_json(&run.join("report.json"), &report)?;
        snapshot.flush()?;
        prune_successful_runs(&base, options.keep_build_runs)?;
        Ok(report)
    }
}

/// Reject unsafe destinations and unselected record deletions before fetching or exporting.
/// Index records provide only validated bounds for chunk selection, never repaired outputs.
fn preflight_records(
    snapshot: &Snapshot,
    world: &World,
    sparse: bool,
    selection: &Selection,
) -> Result<()> {
    for (id, model) in &world.models {
        let path = paths::build(id);
        let file = snapshot.working_path(&path)?;
        snapshot.working_path(&paths::asset(id))?;
        snapshot.working_path(&paths::lod1(id))?;
        if !sparse || file.is_file() || snapshot.skipped.contains(path.as_bytes()) {
            continue;
        }
        let Some(entry) = snapshot.entries.get(path.as_bytes()) else {
            // A new model has no previous runtime record to repair.
            continue;
        };
        if selection.all
            || selection.entities.iter().any(|entity| {
                world
                    .entities
                    .get(entity)
                    .is_some_and(|entity| entity.model == *id)
            })
        {
            continue;
        }
        let selected_by_chunk = (|| -> Result<bool> {
            if selection.chunks.is_empty() {
                return Ok(false);
            }
            let record: Build = json::parse(&snapshot.read_entry(entry, limits::JSON_BYTES)?)?;
            record.validate(&world.realm.profile, model, &world.inputs[id])?;
            let subset = World {
                realm: world.realm.clone(),
                models: BTreeMap::from([(id.clone(), model.clone())]),
                entities: world
                    .entities
                    .iter()
                    .filter(|(_, entity)| entity.model == *id)
                    .map(|(id, entity)| (id.clone(), entity.clone()))
                    .collect(),
                inputs: BTreeMap::from([(id.clone(), world.inputs[id].clone())]),
            };
            let view = subset.projections(&BTreeMap::from([(id.clone(), record)]))?;
            let chunks = Selection {
                chunks: selection.chunks.clone(),
                ..Selection::default()
            };
            Ok(!chunks.resolve(&subset, &view)?.assets.is_empty())
        })()
        .unwrap_or(false);
        ensure!(
            "stale_runtime",
            selected_by_chunk,
            "select the entity before repairing its runtime output: {path}"
        );
    }
    Ok(())
}

fn export_source(
    snapshot: &Snapshot,
    world: &World,
    source: &str,
    ids: &[&str],
    folder: &Path,
) -> Result<BTreeMap<String, (Build, ExportMetrics)>> {
    let inputs = folder.join("inputs");
    let output = folder.join("output");
    fs::create_dir_all(&inputs)?;
    fs::create_dir_all(&output)?;
    let mut files = BTreeMap::new();
    for id in ids {
        for (path, descriptor) in &world.inputs[*id] {
            if let Some(previous) = files.insert(path, descriptor) {
                ensure!(
                    "invalid_dependency",
                    previous == descriptor,
                    "inconsistent shared input descriptor"
                );
            }
        }
    }
    for (path, descriptor) in files {
        snapshot.freeze_payload(
            path,
            descriptor,
            &io::destination(&inputs, path.as_bytes())?,
        )?;
    }
    let models: Vec<_> = ids
        .iter()
        .map(|id| {
            let entity_ids: Vec<_> = world
                .entities
                .values()
                .filter(|entity| entity.model == **id)
                .map(|entity| &entity.id)
                .collect();
            json!({
                "id": id,
                "entry": world.models[*id].entry,
                "asset_path": paths::asset(id),
                "lod1_path": paths::lod1(id),
                "entity_ids": entity_ids,
            })
        })
        .collect();
    let request = json!({
        "response_format": formats::BLENDER_EXPORT,
        "source_root": inputs,
        "source": source,
        "output_dir": output,
        "models": models,
    });
    io::write_json(&folder.join("request.json"), &request)?;
    let log = fs::File::create(folder.join("blender.log"))?;
    let executable = blender_executable();
    ensure!(
        "missing_tool",
        executable.is_file(),
        "Blender unavailable; set ROU_BLENDER"
    );
    let status = Command::new(executable)
        .arg("--background")
        .arg("--disable-autoexec")
        .arg(inputs.join(source))
        .args(["--python-exit-code", "1", "--python"])
        .arg(toolchain_root()?.join("toolchain/blender/export_git_realm.py"))
        .arg("--")
        .arg("--request")
        .arg(folder.join("request.json"))
        .arg("--output")
        .arg(folder.join("response.json"))
        .stdout(log.try_clone()?)
        .stderr(Stdio::from(log))
        .status()?;
    ensure!(
        "export_failed",
        status.success(),
        "Blender export failed; inspect {}",
        folder.join("blender.log").display()
    );
    let response: BlenderExport = json::parse(&io::read(
        &folder.join("response.json"),
        limits::JSON_BYTES,
    )?)?;
    ensure!(
        "invalid_export",
        response
            .models
            .iter()
            .map(|m| m.id.as_str())
            .collect::<BTreeSet<_>>()
            == ids.iter().copied().collect(),
        "Blender model IDs differ from request"
    );
    let mut result = BTreeMap::new();
    for exported in response.models {
        let model = &world.models[&exported.id];
        for dependency in &exported.dependencies {
            ensure!(
                "missing_dependency",
                inputs.join(dependency).is_file(),
                "missing Blender dependency: {dependency}"
            );
            ensure!(
                "invalid_dependency",
                model.dependencies.contains(dependency),
                "undeclared Blender dependency: {dependency}"
            );
        }
        for (asset, metrics) in [
            (&exported.asset, &exported.metrics.asset),
            (&exported.lod1, &exported.metrics.lod1),
        ] {
            let data = io::read(
                &io::destination(&output, asset.path.as_bytes())?,
                limits::GLB_BYTES,
            )?;
            asset.descriptor().verify(&data)?;
            ensure!(
                "invalid_export",
                glb::precheck(&data)? == *metrics,
                "export statistics differ from GLB: {}",
                asset.path
            );
        }
        let record = Build {
            format: formats::BUILD.into(),
            model: model.id.clone(),
            inputs: world.inputs[&model.id].clone(),
            input_digest: world.input_digest(&model.id)?,
            asset: exported.asset,
            lod1: exported.lod1,
            local_bounds_mm: blender::bounds_native(exported.bounds_blender_m)?,
            tool_versions: Some(response.tool_versions.clone()),
        };
        record.validate(&world.realm.profile, model, &world.inputs[&model.id])?;
        result.insert(model.id.clone(), (record, exported.metrics));
    }
    Ok(result)
}

fn prune_successful_runs(base: &Path, keep: usize) -> Result<()> {
    let mut runs = Vec::new();
    for entry in fs::read_dir(base)? {
        let entry = entry?;
        if entry.file_type()?.is_dir()
            && entry.file_name().to_string_lossy().starts_with("run-")
            && entry.path().join("report.json").is_file()
        {
            runs.push(entry.path());
        }
    }
    runs.sort();
    let count = runs.len().saturating_sub(keep);
    for path in runs.into_iter().take(count) {
        io::reject_symlinks(&path)?;
        fs::remove_dir_all(path)?;
    }
    Ok(())
}
