//! Real CLI, Git and Git LFS. Every subprocess has isolated Git configuration.
//! A deterministic exporter double isolates Git semantics; the acceptance driver uses Blender.
#[path = "../../tests/common/mod.rs"]
mod common;
use rou_core::{documents::*, formats, json, paths, spatial::Profile};
use serde_json::{Value, json as value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn bundle_entities(directory: &Path) -> Vec<PreviewEntity> {
    let root: Manifest = json::parse(&fs::read(directory.join("realm.json")).unwrap()).unwrap();
    let mut catalog = rou_core::viewer::Catalog::load_root(&root).unwrap();
    root.chunks
        .iter()
        .flat_map(|(key, descriptor)| {
            catalog
                .load_page(key, &fs::read(directory.join(&descriptor.page)).unwrap())
                .unwrap()
                .entities
        })
        .collect()
}

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    blender: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("realm");
        fs::create_dir(&root).unwrap();
        let blender = temp.path().join("exporter.py");
        for (file, metrics, (document, binary)) in [
            ("triangle.glb", "metrics.json", common::triangle()),
            (
                "triangle-lod1.glb",
                "lod1-metrics.json",
                common::triangle_lod1(),
            ),
        ] {
            let bytes = common::glb(&document, &binary);
            fs::write(temp.path().join(file), &bytes).unwrap();
            fs::write(
                temp.path().join(metrics),
                serde_json::to_vec(&rou_core::glb::precheck(&bytes).unwrap()).unwrap(),
            )
            .unwrap();
        }
        fs::write(&blender,r#"#!/usr/bin/env python3
import json,sys,pathlib,hashlib,shutil
request=pathlib.Path(sys.argv[sys.argv.index('--request')+1])
r=json.loads(request.read_text())
outputs=[]
for model in r['models']:
    output=pathlib.Path(r['output_dir'])/model['asset_path']
    output.parent.mkdir(parents=True,exist_ok=True)
    output.write_bytes(pathlib.Path(__file__).with_name('triangle.glb').read_bytes())
    data=output.read_bytes()
    lod1=pathlib.Path(r['output_dir'])/model['lod1_path']
    lod1_data=pathlib.Path(__file__).with_name('triangle-lod1.glb').read_bytes()
    lod1.write_bytes(lod1_data)
    descriptor=dict(sha256=hashlib.sha256(data).hexdigest(),bytes=len(data))
    lod1_descriptor=dict(sha256=hashlib.sha256(lod1_data).hexdigest(),bytes=len(lod1_data))
    metrics=json.loads(pathlib.Path(__file__).with_name('metrics.json').read_text())
    metrics['bytes']=len(data)
    lod1_metrics=json.loads(pathlib.Path(__file__).with_name('lod1-metrics.json').read_text())
    texture_file=pathlib.Path(__file__).with_name('textures.json')
    textures=json.loads(texture_file.read_text()) if texture_file.exists() else []
    for texture in textures:
        shutil.copyfile(pathlib.Path(__file__).with_name(pathlib.Path(texture['path']).name), pathlib.Path(r['output_dir'])/texture['path'])
    outputs.append(dict(id=model['id'],asset=dict(path=model['asset_path'],**descriptor),lod1=dict(path=model['lod1_path'],**lod1_descriptor),metrics=dict(asset=metrics,lod1=lod1_metrics),textures=dict(asset=textures,lod1=textures),bounds_blender_m=[[0,0,0],[1,1,1]],dependencies=['sources/shared/wood.png']))
report=dict(format=r['response_format'],models=outputs,tool_versions=dict(exporter=r['response_format'],blender='test double'))
pathlib.Path(sys.argv[sys.argv.index('--output')+1]).write_text(json.dumps(report))
"#).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&blender, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let fixture = Self {
            _temp: temp,
            root,
            blender,
        };
        fixture.rou(&fixture.root, &["init"]);
        fixture
    }
    fn shared_texture(&self, revision: usize) -> Texture {
        let image = [
            b"\x89PNG\r\n\x1a\n".as_slice(),
            &13_u32.to_be_bytes(),
            b"IHDR",
            &8_u32.to_be_bytes(),
            &8_u32.to_be_bytes(),
        ]
        .concat();
        let texture = Texture {
            path: format!("runtime/{}.png", json::digest(&image)),
            sha256: json::digest(&image),
            bytes: image.len() as u64,
        };
        for (file, metrics, (mut document, mut binary)) in [
            ("triangle.glb", "metrics.json", common::triangle()),
            (
                "triangle-lod1.glb",
                "lod1-metrics.json",
                common::triangle_lod1(),
            ),
        ] {
            document["extras"] = value!({"revision":revision});
            document["bufferViews"]
                .as_array_mut()
                .unwrap()
                .push(value!({"buffer":0,"byteOffset":binary.len(),"byteLength":image.len()}));
            binary.extend(&image);
            document["buffers"][0]["byteLength"] = value!(binary.len());
            document["images"] = value!([{"bufferView":1,"mimeType":"image/png"}]);
            let input = common::glb(&document, &binary);
            let plan = rou_core::glb::externalise(&input).unwrap();
            fs::write(
                self._temp.path().join(file),
                plan.materialize(&input).unwrap(),
            )
            .unwrap();
            fs::write(
                self._temp.path().join(metrics),
                serde_json::to_vec(&plan.metrics).unwrap(),
            )
            .unwrap();
        }
        fs::write(
            self._temp
                .path()
                .join(Path::new(&texture.path).file_name().unwrap()),
            &image,
        )
        .unwrap();
        fs::write(self._temp.path().join("textures.json"), serde_json::to_vec(&value!([{
            "path":texture.path,"sha256":texture.sha256,"bytes":texture.bytes,"metrics":rou_core::image::precheck(&image).unwrap()
        }])).unwrap()).unwrap();
        texture
    }
    fn command_builder(&self, program: &str, root: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(program);
        command.current_dir(root).args(args);
        for (key, _) in std::env::vars_os() {
            if key.as_encoded_bytes().starts_with(b"GIT_") {
                command.env_remove(key);
            }
        }
        command
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Realm integration")
            .env("GIT_AUTHOR_EMAIL", "integration@example.invalid")
            .env("GIT_COMMITTER_NAME", "Realm integration")
            .env("GIT_COMMITTER_EMAIL", "integration@example.invalid")
            .env("GIT_LFS_SKIP_SMUDGE", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("ROU_BLENDER", &self.blender);
        command
    }
    fn command(&self, program: &str, root: &Path, args: &[&str]) -> Output {
        self.command_builder(program, root, args).output().unwrap()
    }
    fn raw(&self, root: &Path, args: &[&str]) -> Output {
        self.command(env!("CARGO_BIN_EXE_rou"), root, args)
    }
    fn rou(&self, root: &Path, args: &[&str]) -> Value {
        let mut machine_args = vec!["--json"];
        machine_args.extend_from_slice(args);
        let result = self.raw(root, &machine_args);
        assert!(
            result.status.success(),
            "rou {args:?}: {}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        serde_json::from_slice(&result.stdout)
            .unwrap_or_else(|_| value!(String::from_utf8_lossy(&result.stdout)))
    }
    fn git(&self, root: &Path, args: &[&str]) -> Vec<u8> {
        let output = self.command("git", root, args);
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    }
    fn write(&self, path: &str, bytes: &[u8]) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    fn seed(&self) {
        for (id, position) in [("house", [0, 0, 0]), ("bench", [32000, 0, 0])] {
            let model = Model {
                format: formats::MODEL.into(),
                id: id.into(),
                source: format!("sources/{id}/{id}.blend"),
                entry: Entry {
                    kind: EntryKind::Object,
                    name: id.into(),
                },
                dependencies: vec!["sources/shared/wood.png".into()],
                settings: Default::default(),
            };
            self.write(&model.source, format!("BLENDER {id}").as_bytes());
            self.write(&paths::model(id), &json::pretty(&model).unwrap());
            self.write(
                &paths::entity(id),
                &json::pretty(&Entity {
                    format: formats::ENTITY.into(),
                    id: id.into(),
                    model: id.into(),
                    position_mm: position,
                    label: None,
                })
                .unwrap(),
            );
        }
        self.write("sources/shared/wood.png", b"shared texture");
        self.write("README.md", b"Initial world\n");
        self.rou(&self.root, &["build"]);
        self.rou(&self.root, &["add", "."]);
        self.rou(&self.root, &["commit", "-m", "Initial world"]);
    }
    fn remote(&self) -> PathBuf {
        let remote = self._temp.path().join("origin.git");
        self.git(
            self._temp.path(),
            &["init", "--bare", "-b", "main", remote.to_str().unwrap()],
        );
        self.rou(
            &self.root,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        self.rou(&self.root, &["push", "-u", "origin", "main"]);
        remote
    }
    fn clone_realm(&self, remote: &Path, name: &str) -> PathBuf {
        let destination = self._temp.path().join(name);
        self.rou(
            self._temp.path(),
            &[
                "clone",
                remote.to_str().unwrap(),
                destination.to_str().unwrap(),
            ],
        );
        destination
    }
}

#[test]
fn init_readme_commit_and_human_status() {
    let f = Fixture::new();
    assert!(f.git(&f.root, &["ls-files"]).is_empty());
    let realm: Realm = json::parse(&fs::read(f.root.join("realm.json")).unwrap()).unwrap();
    assert_eq!(realm.profile, Profile::default());
    assert!(!f.root.join("realm/viewpoints").exists());
    f.write("README.md", b"A first text commit\n");
    f.rou(&f.root, &["add", "."]);
    f.rou(&f.root, &["commit", "-m", "README first"]);
    assert!(String::from_utf8_lossy(&f.raw(&f.root, &["status"]).stdout).contains("Selection:"));
    assert_eq!(
        f.rou(&f.root, &["status", "--json"])["domain_validation"],
        "not_run"
    );
    assert!(!f.raw(&f.root, &["sparse", "set"]).status.success());
    let output = f.rou(&f.root, &["view"]);
    let path = output["manifest"].as_str().unwrap();
    let manifest: Manifest = json::parse(&fs::read(path).unwrap()).unwrap();
    assert_eq!(manifest.entity_count, 0);
    let bundle = Path::new(path).parent().unwrap();
    fs::write(bundle.join(".DS_Store"), b"Finder metadata").unwrap();
    fs::write(bundle.join("objects/.DS_Store"), b"Finder metadata").unwrap();
    f.rou(&f.root, &["view"]);
    let unrelated_object = bundle.join("objects/notes.txt");
    fs::write(&unrelated_object, b"keep this file").unwrap();
    let rejected = f.raw(&f.root, &["view"]);
    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("invalid_layout"));
    assert_eq!(fs::read(&unrelated_object).unwrap(), b"keep this file");
    fs::remove_file(unrelated_object).unwrap();
    fs::write(bundle.join(".unrelated"), b"keep this file").unwrap();
    assert!(!f.raw(&f.root, &["view"]).status.success());
    assert_eq!(
        fs::read(bundle.join(".unrelated")).unwrap(),
        b"keep this file"
    );
}

#[cfg(unix)]
#[test]
fn view_allows_external_ancestor_links_but_rejects_bundle_links() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    f.seed();
    let actual = f._temp.path().join("exports");
    fs::create_dir(&actual).unwrap();
    let alias = f._temp.path().join("exports-alias");
    symlink(&actual, &alias).unwrap();
    let output = alias.join("new-parent/bundle");
    for _ in 0..2 {
        let report = f.rou(
            &f.root,
            &["view", "--all", "--output", output.to_str().unwrap()],
        );
        assert_eq!(
            Path::new(report["manifest"].as_str().unwrap()),
            actual
                .canonicalize()
                .unwrap()
                .join("new-parent/bundle/realm.json")
        );
        assert!(output.join("realm.json").is_file());
    }
    // Exercise the real macOS /tmp alias, including creation and replacement.
    let temporary = tempfile::Builder::new()
        .prefix("rou-view-")
        .tempdir_in("/tmp")
        .unwrap();
    let under_tmp = temporary.path().join("new-parent/bundle");
    for _ in 0..2 {
        f.rou(
            &f.root,
            &["view", "--all", "--output", under_tmp.to_str().unwrap()],
        );
    }
    assert!(under_tmp.join("realm.json").is_file());
    let manifest = fs::read(output.join("realm.json")).unwrap();
    let root_link = alias.join("bundle-link");
    symlink(&output, &root_link).unwrap();
    let dangling = alias.join("dangling-link");
    symlink(actual.join("absent"), &dangling).unwrap();
    let author_link = alias.join("author-link");
    symlink(f.root.parent().unwrap(), &author_link).unwrap();
    for path in [
        root_link.clone(),
        PathBuf::from(format!("{}/", root_link.display())),
        dangling,
        author_link.join("realm"),
    ] {
        let result = f.raw(
            &f.root,
            &["view", "--all", "--output", path.to_str().unwrap()],
        );
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("unsafe_path"),
            "{result:?}"
        );
        assert_eq!(manifest, fs::read(output.join("realm.json")).unwrap());
    }
    let objects = output.join("objects");
    let preserved = actual.join("preserved-objects");
    fs::rename(&objects, &preserved).unwrap();
    symlink(&preserved, &objects).unwrap();
    let result = f.raw(
        &f.root,
        &["view", "--all", "--output", output.to_str().unwrap()],
    );
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("unsafe_path"));
    fs::remove_file(&objects).unwrap();
    fs::rename(&preserved, &objects).unwrap();
    let asset = fs::read_dir(&objects)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let bytes = fs::read(&asset).unwrap();
    let outside = actual.join("preserved.glb");
    fs::rename(&asset, &outside).unwrap();
    symlink(&outside, &asset).unwrap();
    assert!(
        !f.raw(
            &f.root,
            &["view", "--all", "--output", output.to_str().unwrap()]
        )
        .status
        .success()
    );
    assert_eq!(manifest, fs::read(output.join("realm.json")).unwrap());
    assert_eq!(bytes, fs::read(&outside).unwrap());
}

#[test]
fn metadata_clone_and_selective_lfs() {
    let f = Fixture::new();
    f.seed();
    let remote = f.remote();
    let clone = f.clone_realm(&remote, "alice");
    assert!(!clone.join("sources").exists());
    assert!(
        rou_core::lfs::pointer(&fs::read(clone.join("runtime/house.glb")).unwrap())
            .unwrap()
            .is_some()
    );
    f.rou(&clone, &["sparse", "set", "--entity", "house"]);
    let stats = String::from_utf8(f.git(
        &clone,
        &[
            "ls-files",
            "--debug",
            "sources/house/house.blend",
            "runtime/house.glb",
        ],
    ))
    .unwrap();
    assert!(
        !stats.contains("ctime: 0:"),
        "hydrated stat entries were not refreshed: {stats}"
    );
    // Exclude Git's intentional same-timestamp (racy-clean) fallback so this
    // check specifically detects a missing payload stat refresh.
    fs::File::options()
        .write(true)
        .open(clone.join(".git/index"))
        .unwrap()
        .set_times(
            fs::FileTimes::new()
                .set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(2)),
        )
        .unwrap();
    let trace = f._temp.path().join("first-status-trace.json");
    let first_status = f
        .command_builder("git", &clone, &["status", "--porcelain"])
        .env("GIT_TRACE2_EVENT", &trace)
        .env("GIT_TRACE_PACKET", "1")
        .output()
        .unwrap();
    assert!(first_status.status.success());
    assert!(
        first_status.stdout.is_empty(),
        "{}{}",
        String::from_utf8_lossy(&first_status.stdout),
        String::from_utf8_lossy(&first_status.stderr)
    );
    assert!(
        !String::from_utf8_lossy(&first_status.stderr).contains("command=clean"),
        "stats: {stats}\ntrace: {}",
        String::from_utf8_lossy(&first_status.stderr)
    );
    for path in [
        "sources/house/house.blend",
        "sources/shared/wood.png",
        "runtime/house.glb",
    ] {
        assert_eq!(
            fs::read(clone.join(path)).unwrap(),
            fs::read(f.root.join(path)).unwrap()
        );
    }
    assert!(!clone.join("sources/bench/bench.blend").exists());
    let status = f.git(&clone, &["status", "--porcelain"]);
    assert!(status.is_empty(), "{}", String::from_utf8_lossy(&status));
    let cold = fs::read(clone.join("runtime/bench.glb")).unwrap();
    let index = fs::read(clone.join(".git/index")).unwrap();
    // No remote is available: a reuse-only build must not fetch cold assets.
    fs::rename(&remote, remote.with_extension("offline")).unwrap();
    let report = f.rou(&clone, &["build", "--keep-build-runs", "0"]);
    assert_eq!(report["run"], Value::Null);
    assert_eq!(cold, fs::read(clone.join("runtime/bench.glb")).unwrap());
    assert_eq!(index, fs::read(clone.join(".git/index")).unwrap());
    assert!(f.git(&clone, &["status", "--porcelain"]).is_empty());
    fs::rename(remote.with_extension("offline"), &remote).unwrap();
    // Runtime remains in the cone. A real deletion or rewrite cannot fall back
    // to the unchanged index, even when this model has not been selected.
    let output = clone.join("runtime/bench.glb");
    for corrupt in [None, Some(b"junk".as_slice())] {
        if let Some(bytes) = corrupt {
            fs::write(&output, bytes).unwrap();
        } else {
            fs::remove_file(&output).unwrap();
        }
        for args in [vec!["build"], vec!["build", "--staged"]] {
            let before_index = fs::read(clone.join(".git/index")).unwrap();
            let rejected = f.raw(&clone, &args);
            assert!(!rejected.status.success());
            assert!(
                String::from_utf8_lossy(&rejected.stderr)
                    .contains("select the entity before repairing its runtime output")
            );
            assert!(String::from_utf8_lossy(&rejected.stderr).contains("stale_runtime"));
            assert_eq!(fs::read(&output).ok().as_deref(), corrupt);
            assert_eq!(before_index, fs::read(clone.join(".git/index")).unwrap());
        }
        f.rou(&clone, &["validate", "--staged"]);
        fs::write(&output, &cold).unwrap();
    }
    fs::write(
        clone.join("sources/house/house.blend"),
        b"local author edit",
    )
    .unwrap();
    f.rou(&clone, &["sparse", "add", "--entity", "bench"]);
    assert!(clone.join("sources/bench/bench.blend").is_file());
    assert_eq!(
        fs::read(clone.join("sources/house/house.blend")).unwrap(),
        b"local author edit"
    );
}

#[test]
fn unselected_record_deletion_is_rejected_before_fetch_or_export() {
    let f = Fixture::new();
    f.seed();
    let remote = f.remote();
    let clone = f.clone_realm(&remote, "record-deletion");
    let selected = f.rou(&clone, &["sparse", "set", "--chunk", "0,0,-1"]);
    assert_eq!(
        selected["resolution"]["assets"],
        value!(["runtime/house.glb", "runtime/house.lod1.glb"])
    );
    let record = clone.join("runtime/bench.json");
    let bytes = fs::read(&record).unwrap();
    let cold = fs::read(clone.join("runtime/bench.glb")).unwrap();
    for symlink in [false, true] {
        fs::remove_file(&record).unwrap();
        if symlink {
            #[cfg(unix)]
            std::os::unix::fs::symlink("house.json", &record).unwrap();
            #[cfg(not(unix))]
            break;
        }
        for staged in [false, true] {
            let trace = f
                ._temp
                .path()
                .join(format!("record-trace-{symlink}-{staged}.json"));
            let index = fs::read(clone.join(".git/index")).unwrap();
            let args = if staged {
                vec!["build", "--staged"]
            } else {
                vec!["build"]
            };
            let output = f
                .command_builder(env!("CARGO_BIN_EXE_rou"), &clone, &args)
                .env("GIT_TRACE2_EVENT", &trace)
                .env("ROU_BLENDER", f._temp.path().join("must-not-export"))
                .output()
                .unwrap();
            assert!(!output.status.success());
            let code = if symlink {
                "unsafe_path"
            } else {
                "stale_runtime"
            };
            assert!(
                String::from_utf8_lossy(&output.stderr).contains(code),
                "{output:?}"
            );
            assert_eq!(index, fs::read(clone.join(".git/index")).unwrap());
            assert_eq!(cold, fs::read(clone.join("runtime/bench.glb")).unwrap());
            assert!(!clone.join("sources/bench/bench.blend").exists());
            assert_eq!(record.symlink_metadata().is_ok(), symlink);
            let events = fs::read_to_string(trace).unwrap();
            assert!(
                !events.contains("smudge"),
                "unexpected LFS download: {events}"
            );
        }
        if symlink {
            fs::remove_file(&record).unwrap();
        }
        fs::write(&record, &bytes).unwrap();
    }
    // The same repair is allowed for a model selected by chunk, without
    // rebuilding or hydrating the unselected model beside it.
    fs::remove_file(clone.join("runtime/house.json")).unwrap();
    assert_eq!(f.rou(&clone, &["build"])["generated"], value!(["house"]));
    assert_eq!(cold, fs::read(clone.join("runtime/bench.glb")).unwrap());
}

#[test]
fn build_rejects_index_change_during_snapshot_capture_before_export() {
    let f = Fixture::new();
    f.seed();
    f.write(
        "sources/house/house.blend",
        b"draft source requiring export",
    );
    let output_before = fs::read(f.root.join("runtime/house.json")).unwrap();
    let real_git = String::from_utf8(
        f.command("/bin/sh", &f.root, &["-c", "command -v git"])
            .stdout,
    )
    .unwrap();
    let wrapper = f._temp.path().join("snapshot-wrappers");
    executable(
        &wrapper.join("git"),
        &format!(
            r#"#!/bin/bash
if [ "$1" = ls-files ] && [ "$2" = --stage ] && [ ! -e .rou/snapshot-index ]; then
  '{git}' "$@" > .rou/snapshot-index || exit $?
  printf 'concurrent index edit\n' > README.md
  '{git}' add README.md || exit $?
  cat .rou/snapshot-index
  exit 0
fi
exec '{git}' "$@"
"#,
            git = real_git.trim()
        ),
    );
    let result = f
        .command_builder(env!("CARGO_BIN_EXE_rou"), &f.root, &["build"])
        .env(
            "PATH",
            format!("{}:{}", wrapper.display(), std::env::var("PATH").unwrap()),
        )
        .env("ROU_BLENDER", f._temp.path().join("must-not-export"))
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("snapshot_changed"),
        "{result:?}"
    );
    assert_eq!(
        output_before,
        fs::read(f.root.join("runtime/house.json")).unwrap()
    );
    assert_eq!(
        f.git(&f.root, &["diff", "--cached", "--name-only"]),
        b"README.md\n"
    );
}

#[test]
fn worktree_payload_hashes_persist_and_cache_hits_do_not_rewrite_disk() {
    use rou_git::{
        io::Stamp,
        snapshot::{Mode, Snapshot},
    };
    let f = Fixture::new();
    let path = "sources/cache.blend";
    let payload = b"BLENDER persistent source payload";
    f.write(path, payload);
    let repo = rou_git::Repository::discover(&f.root).unwrap();
    let wanted = Descriptor::of(payload);
    let first = Snapshot::new(&repo, Mode::Worktree).unwrap();
    assert_eq!(
        first.payload_path(path, &wanted, false).unwrap(),
        repo.root.join(path)
    );
    first.flush().unwrap();
    let file = repo.root.join(".rou/cache/source-hashes.json");
    let bytes = fs::read(&file).unwrap();
    let cache: Value = serde_json::from_slice(&bytes).unwrap();
    let absolute = repo.root.join(path);
    let key = json::digest(absolute.as_os_str().as_encoded_bytes());
    assert_eq!(cache[&key]["descriptor"]["sha256"], wanted.sha256);
    let stamp = Stamp::of(&file.metadata().unwrap());
    for _ in 0..3 {
        let snapshot = Snapshot::new(&repo, Mode::Worktree).unwrap();
        assert_eq!(
            snapshot.payload_path(path, &wanted, false).unwrap(),
            absolute
        );
        snapshot.flush().unwrap();
        assert_eq!(Stamp::of(&file.metadata().unwrap()), stamp);
        assert_eq!(fs::read(&file).unwrap(), bytes);
    }
}

#[test]
fn staged_snapshot_is_independent_of_unstaged_source() {
    let f = Fixture::new();
    f.seed();
    let index = f.git(&f.root, &["ls-files", "--stage", "-z"]);
    f.write("sources/house/house.blend", b"saved but unstaged v2");
    f.rou(&f.root, &["build", "--staged"]);
    assert_eq!(
        fs::read(f.root.join("sources/house/house.blend")).unwrap(),
        b"saved but unstaged v2"
    );
    assert_eq!(index, f.git(&f.root, &["ls-files", "--stage", "-z"]));
    f.rou(&f.root, &["validate", "--staged"]);
    f.rou(&f.root, &["add", "sources/house/house.blend"]);
    assert!(!f.raw(&f.root, &["validate", "--staged"]).status.success());
    f.rou(&f.root, &["build"]);
    f.rou(&f.root, &["add", "runtime"]);
    f.rou(&f.root, &["validate", "--staged"]);
}

#[test]
fn independent_entity_moves_merge_without_rebuild() {
    let f = Fixture::new();
    f.seed();
    f.rou(&f.root, &["switch", "-c", "alice"]);
    let mut house: Entity =
        json::parse(&fs::read(f.root.join(paths::entity("house"))).unwrap()).unwrap();
    house.position_mm = [1000, 0, 0];
    f.write(&paths::entity("house"), &json::pretty(&house).unwrap());
    f.rou(&f.root, &["add", "realm"]);
    f.rou(&f.root, &["commit", "-m", "Move house"]);
    f.rou(&f.root, &["switch", "-c", "bob", "main"]);
    // A sparse switch may leave source payloads cold. Placement-only changes need no build.
    let mut bench: Entity =
        json::parse(&fs::read(f.root.join(paths::entity("bench"))).unwrap()).unwrap();
    bench.position_mm = [33000, 0, 0];
    f.write(&paths::entity("bench"), &json::pretty(&bench).unwrap());
    f.rou(&f.root, &["add", "realm"]);
    f.rou(&f.root, &["commit", "-m", "Move bench"]);
    f.rou(&f.root, &["merge", "alice"]);
    f.rou(&f.root, &["commit", "-m", "Merge placements"]);
    assert_eq!(
        String::from_utf8(f.git(&f.root, &["rev-list", "--parents", "-n", "1", "HEAD"]))
            .unwrap()
            .split_whitespace()
            .count(),
        3
    );
    f.rou(&f.root, &["validate"]);
}

#[test]
fn sparse_shrink_moves_ignored_backups_and_preserves_unrelated_dirty_files() {
    let f = Fixture::new();
    f.seed();
    let remote = f.remote();
    let clone = f.clone_realm(&remote, "alice");
    f.rou(&clone, &["sparse", "set", "--entity", "house"]);
    fs::write(clone.join("sources/house/house.blend1"), b"author backup").unwrap();
    fs::write(clone.join("README.md"), b"unrelated text draft").unwrap();
    fs::create_dir_all(clone.join(".godot")).unwrap();
    fs::write(clone.join(".godot/cache"), b"viewer cache").unwrap();
    fs::create_dir_all(clone.join("unrelated")).unwrap();
    fs::write(clone.join("unrelated/draft.txt"), b"local unrelated draft").unwrap();
    let result = f.rou(&clone, &["sparse", "set", "--entity", "bench"]);
    assert_eq!(result["backups"].as_array().unwrap().len(), 1);
    assert_eq!(
        fs::read(clone.join(".godot/cache")).unwrap(),
        b"viewer cache"
    );
    assert_eq!(
        fs::read(clone.join("unrelated/draft.txt")).unwrap(),
        b"local unrelated draft"
    );
    let backup = result["backups"][0].as_str().unwrap();
    assert_eq!(fs::read(backup).unwrap(), b"author backup");
    assert!(
        !clone.join("sources/house").exists(),
        "remaining: {:?}; status: {}",
        fs::read_dir(clone.join("sources/house"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect::<Vec<_>>(),
        String::from_utf8_lossy(&f.git(&clone, &["status", "--porcelain"]))
    );
    assert_eq!(
        fs::read(clone.join("README.md")).unwrap(),
        b"unrelated text draft"
    );
    fs::write(
        clone.join("sources/bench/bench.blend"),
        b"modified author file",
    )
    .unwrap();
    assert!(
        !f.raw(&clone, &["sparse", "set", "--entity", "house"])
            .status
            .success()
    );
}

#[test]
fn grouped_build_prunes_successful_runs_and_retired_outputs() {
    let f = Fixture::new();
    f.seed();
    let mut bench: Model =
        json::parse(&fs::read(f.root.join(paths::model("bench"))).unwrap()).unwrap();
    bench.source = "sources/house/house.blend".into();
    f.write(&paths::model("bench"), &json::pretty(&bench).unwrap());
    f.write("sources/house/house.blend", b"both models changed");
    let result = f.rou(&f.root, &["build"]);
    let run = Path::new(result["run"].as_str().unwrap());
    assert!(run.join("source-0000/blender.log").is_file());
    assert!(!run.join("source-0001").exists());
    assert_eq!(fs::read_dir(f.root.join(".rou/build")).unwrap().count(), 1);
    fs::remove_file(f.root.join(paths::model("bench"))).unwrap();
    fs::remove_file(f.root.join(paths::entity("bench"))).unwrap();
    let result = f.rou(&f.root, &["build"]);
    assert_eq!(
        result["removed"],
        value!([
            "runtime/bench.glb",
            "runtime/bench.json",
            "runtime/bench.lod1.glb"
        ])
    );
}

fn executable(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

#[test]
fn native_fast_forward_and_binary_conflict() {
    let f = Fixture::new();
    f.seed();
    f.rou(&f.root, &["switch", "-c", "feature"]);
    f.write("README.md", b"feature notes\n");
    f.rou(&f.root, &["add", "README.md"]);
    f.rou(&f.root, &["commit", "-m", "Notes"]);
    let feature = f.git(&f.root, &["rev-parse", "HEAD"]);
    f.rou(&f.root, &["switch", "main"]);
    f.rou(&f.root, &["merge", "--ff-only", "feature"]);
    assert_eq!(feature, f.git(&f.root, &["rev-parse", "HEAD"]));
    f.rou(&f.root, &["sparse", "set", "--entity", "house"]);
    for branch in ["left", "right"] {
        f.rou(&f.root, &["switch", "-c", branch, "main"]);
        f.write(
            "sources/house/house.blend",
            format!("{branch}\0geometry").as_bytes(),
        );
        f.rou(&f.root, &["build"]);
        f.rou(&f.root, &["add", "sources/house", "runtime"]);
        f.rou(&f.root, &["commit", "-m", branch]);
    }
    f.rou(&f.root, &["switch", "left"]);
    assert!(!f.raw(&f.root, &["merge", "right"]).status.success());
    assert!(f.root.join(".git/MERGE_HEAD").is_file());
    let stages = f.git(
        &f.root,
        &["ls-files", "-u", "--", "sources/house/house.blend"],
    );
    assert_eq!(stages.iter().filter(|c| **c == b'\n').count(), 3);
    assert!(
        !f.raw(&f.root, &["commit", "-m", "Unresolved"])
            .status
            .success()
    );
    f.rou(&f.root, &["merge", "--abort"]);
    assert!(!f.root.join(".git/MERGE_HEAD").exists());
}

#[test]
fn commit_validates_actual_index_and_retains_post_commit_failure() {
    let f = Fixture::new();
    f.seed();
    f.write("README.md", b"staged v1\n");
    f.rou(&f.root, &["add", "README.md"]);
    f.write("README.md", b"unstaged v2\n");
    f.rou(&f.root, &["commit", "-m", "Only v1"]);
    assert_eq!(f.git(&f.root, &["show", "HEAD:README.md"]), b"staged v1\n");
    assert_eq!(
        fs::read(f.root.join("README.md")).unwrap(),
        b"unstaged v2\n"
    );
    f.rou(&f.root, &["add", "README.md"]);
    let before = f.git(&f.root, &["rev-parse", "HEAD"]);
    // A real native hook changes the actual tree after pre-validation.
    executable(
        &f.root.join(".git/hooks/pre-commit"),
        "#!/bin/sh\nprintf '{}\\n' > realm.json\ngit add realm.json\n",
    );
    let result = f.raw(&f.root, &["commit", "-m", "Hook changed realm"]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("was not rolled back"));
    assert_ne!(before, f.git(&f.root, &["rev-parse", "HEAD"]));
    assert!(!f.root.join(".rou/write.lock").exists());
}

#[test]
fn concurrent_index_change_is_rejected_before_commit() {
    let f = Fixture::new();
    f.seed();
    f.write("README.md", b"draft\n");
    f.rou(&f.root, &["add", "README.md"]);
    let real_git = String::from_utf8(
        f.command("/bin/sh", &f.root, &["-c", "command -v git"])
            .stdout,
    )
    .unwrap();
    let wrapper = f._temp.path().join("wrappers");
    executable(
        &wrapper.join("git"),
        &format!(
            r#"#!/bin/bash
if [ "$1" = write-tree ]; then
  if [ -e .rou/test-first-write-tree ]; then
    printf 'concurrent change\n' > README.md
    '{git}' add README.md
  else
    touch .rou/test-first-write-tree
  fi
fi
exec '{git}' "$@"
"#,
            git = real_git.trim()
        ),
    );
    let before = f.git(&f.root, &["rev-parse", "HEAD"]);
    let output = f
        .command_builder(
            env!("CARGO_BIN_EXE_rou"),
            &f.root,
            &["commit", "-m", "Race"],
        )
        .env(
            "PATH",
            format!("{}:{}", wrapper.display(), std::env::var("PATH").unwrap()),
        )
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("changed during validation"));
    assert_eq!(before, f.git(&f.root, &["rev-parse", "HEAD"]));
}

#[test]
fn push_restores_missing_history_and_pins_validated_commit() {
    let f = Fixture::new();
    f.seed();
    f.write("sources/house/house.blend", b"second binary version\0");
    f.rou(&f.root, &["build"]);
    f.rou(&f.root, &["add", "sources", "runtime"]);
    f.rou(&f.root, &["commit", "-m", "Second version"]);
    let remote = f.remote();
    let clone = f.clone_realm(&remote, "author");
    let fork = f._temp.path().join("fork.git");
    f.git(
        f._temp.path(),
        &["init", "--bare", "-b", "main", fork.to_str().unwrap()],
    );
    f.rou(&clone, &["remote", "add", "fork", fork.to_str().unwrap()]);
    let validated = f.git(&clone, &["rev-parse", "HEAD"]);
    executable(
        &clone.join(".git/hooks/pre-push"),
        "#!/bin/sh\nprintf 'concurrent advancement\\n' > README.md\ngit add README.md\ngit -c core.hooksPath=/dev/null commit -m 'Concurrent editor' >&2\ngit lfs pre-push \"$@\"\n",
    );
    f.rou(&clone, &["push", "fork", "main"]);
    assert_ne!(validated, f.git(&clone, &["rev-parse", "HEAD"]));
    assert_eq!(validated, f.git(&fork, &["rev-parse", "refs/heads/main"]));
    // The new remote independently serves the historical and current payloads.
    fs::rename(&remote, remote.with_extension("unavailable")).unwrap();
    let check = f.clone_realm(&fork, "check");
    f.rou(&check, &["sparse", "set", "--entity", "house"]);
    assert_eq!(
        fs::read(check.join("sources/house/house.blend")).unwrap(),
        b"second binary version\0"
    );
    f.rou(&check, &["checkout", "HEAD~1"]);
    assert_eq!(
        fs::read(check.join("sources/house/house.blend")).unwrap(),
        b"BLENDER house"
    );
    assert!(f.git(&check, &["status", "--porcelain"]).is_empty());
}

#[test]
fn caller_relative_paths_and_checkout_attachment() {
    let f = Fixture::new();
    f.seed();
    f.write("docs/new.txt", b"caller relative\n");
    let caller = f.root.join("docs");
    f.rou(&caller, &["add", "new.txt"]);
    assert_eq!(
        f.git(&f.root, &["diff", "--cached", "--name-only"]),
        b"docs/new.txt\n"
    );
    f.rou(&caller, &["commit", "-m", "Add text"]);
    f.rou(&caller, &["mv", "new.txt", "renamed.txt"]);
    f.rou(&caller, &["commit", "-m", "Rename text"]);
    f.rou(&caller, &["rm", "renamed.txt"]);
    assert_eq!(
        f.git(&f.root, &["diff", "--cached", "--name-status"]),
        b"D\tdocs/renamed.txt\n"
    );
    for reference in ["main", "HEAD"] {
        f.rou(&caller, &["checkout", reference]);
        assert_eq!(
            f.git(&f.root, &["symbolic-ref", "--short", "HEAD"]),
            b"main\n"
        );
    }
    f.rou(&caller, &["tag", "text-milestone"]);
    f.rou(&caller, &["show", "text-milestone:docs/renamed.txt"]);
    f.rou(&caller, &["reset", "HEAD", "--", "renamed.txt"]);
}

#[test]
fn sparse_clone_can_push_to_same_origin_without_expanding_cone() {
    let f = Fixture::new();
    f.seed();
    let remote = f.remote();
    let clone = f.clone_realm(&remote, "author");
    f.rou(&clone, &["switch", "-c", "feature/sparse"]);
    f.rou(&clone, &["sparse", "set", "--entity", "house"]);
    fs::write(clone.join("sources/house/house.blend"), b"improved house\0").unwrap();
    f.rou(&clone, &["build"]);
    f.rou(&clone, &["add", "sources/house", "runtime"]);
    f.rou(&clone, &["commit", "-m", "Improve selected house"]);
    f.rou(&clone, &["push", "-u", "origin", "feature/sparse"]);
    assert!(!clone.join("sources/bench").exists());
    assert_eq!(
        f.git(&remote, &["rev-parse", "refs/heads/feature/sparse"]),
        f.git(&clone, &["rev-parse", "HEAD"])
    );
}

#[test]
fn repository_routing_environment_cannot_replace_validated_index() {
    let f = Fixture::new();
    f.seed();
    f.write("README.md", b"real staged tree\n");
    f.rou(&f.root, &["add", "README.md"]);
    let alternate = f._temp.path().join("alternate-index");
    fs::copy(f.root.join(".git/index"), &alternate).unwrap();
    f.write("README.md", b"alternate staged tree\n");
    assert!(
        f.command_builder("git", &f.root, &["add", "README.md"])
            .env("GIT_INDEX_FILE", &alternate)
            .status()
            .unwrap()
            .success()
    );
    let result = f
        .command_builder(
            env!("CARGO_BIN_EXE_rou"),
            &f.root,
            &["commit", "-m", "Actual index"],
        )
        .env("GIT_INDEX_FILE", &alternate)
        .env("GIT_DIR", f._temp.path().join("not-a-repository"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        f.git(&f.root, &["show", "HEAD:README.md"]),
        b"real staged tree\n"
    );
}

#[test]
fn all_selection_replaces_explicit_selectors() {
    let f = Fixture::new();
    f.seed();
    let remote = f.remote();
    let clone = f.clone_realm(&remote, "author");
    f.rou(&clone, &["sparse", "set", "--entity", "house"]);
    f.rou(&clone, &["sparse", "add", "--all"]);
    let selection = f.rou(&clone, &["sparse", "list"]);
    assert_eq!(
        selection,
        value!({"all": true, "chunks": [], "entities": []})
    );
    assert!(clone.join("sources/bench/bench.blend").is_file());
    let result = f.rou(&clone, &["sparse", "set", "--chunk", "1,0,1..-1,0,-1"]);
    assert_eq!(result["resolution"]["chunks"].as_array().unwrap().len(), 9);
}

#[cfg(unix)]
#[test]
fn signals_release_writer_lock_and_dead_owners_are_recovered() {
    use std::time::{Duration, Instant};
    let f = Fixture::new();
    f.seed();
    let original = fs::read_to_string(&f.blender).unwrap();
    let blocking = original.replace("outputs=[]", "import os,time\npathlib.Path('.rou/test-export-started').touch()\nparent=os.getppid()\nwhile os.getppid()==parent: time.sleep(0.02)\nraise SystemExit(1)\noutputs=[]");
    executable(&f.blender, &blocking);
    f.write("sources/house/house.blend", b"change requiring export");
    for signal in ["-INT", "-TERM"] {
        let marker = f.root.join(".rou/test-export-started");
        let mut child = f
            .command_builder(env!("CARGO_BIN_EXE_rou"), &f.root, &["build"])
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !marker.exists() && Instant::now() < deadline {
            assert!(child.try_wait().unwrap().is_none());
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(marker.exists());
        assert!(!f.raw(&f.root, &["add", "README.md"]).status.success());
        f.rou(&f.root, &["validate"]);
        f.rou(&f.root, &["lfs", "env"]);
        f.rou(&f.root, &["view"]);
        assert!(
            Command::new("/bin/kill")
                .args([signal, &child.id().to_string()])
                .status()
                .unwrap()
                .success()
        );
        assert_eq!(child.wait().unwrap().code(), Some(130));
        assert!(!f.root.join(".rou/write.lock").exists());
        fs::remove_file(marker).unwrap();
    }
    f.write(
        ".rou/write.lock",
        br#"{"pid":2147483647,"token":"dead-test-owner"}"#,
    );
    f.rou(&f.root, &["add", "README.md"]);
    assert!(!f.root.join(".rou/write.lock").exists());
}

#[cfg(unix)]
#[test]
fn git_passthrough_preserves_non_utf8_and_unicode_path_bytes() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new();
    f.seed();
    for name in [
        "\u{6587}\u{5b57}-\u{e9}.txt".as_bytes().to_vec(),
        "\u{6587}\u{5b57}-e\u{301}.txt".as_bytes().to_vec(),
    ] {
        let name = std::ffi::OsString::from_vec(name);
        fs::write(f.root.join(&name), b"ordinary author document").unwrap();
        let output = f
            .command_builder(env!("CARGO_BIN_EXE_rou"), &f.root, &["add", "--"])
            .arg(&name)
            .output()
            .unwrap();
        assert!(output.status.success());
        let staged = f.git(&f.root, &["diff", "--cached", "--name-only", "-z"]);
        // Git's own precomposeunicode may select its canonical spelling; rou must
        // pass the caller bytes through unchanged and preserve binary names.
        assert!(
            staged
                .split(|c| *c == 0)
                .any(|p| p.starts_with("\u{6587}\u{5b57}-".as_bytes()))
        );
    }
    // APFS refuses invalid UTF-8 filesystem names, but Git trees/indexes can
    // contain them. Exercise the byte path through a real index and CLI show.
    let oid = f.git(&f.root, &["hash-object", "-w", "README.md"]);
    let mut index_entry = format!("100644 {}\t", String::from_utf8_lossy(&oid).trim()).into_bytes();
    index_entry.extend_from_slice(b"raw-\xff.txt\0");
    use std::io::Write;
    let mut child = f
        .command_builder("git", &f.root, &["update-index", "-z", "--index-info"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&index_entry).unwrap();
    assert!(child.wait().unwrap().success());
    let result = f
        .command_builder(env!("CARGO_BIN_EXE_rou"), &f.root, &["show"])
        .arg(std::ffi::OsString::from_vec(b":raw-\xff.txt".to_vec()))
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout, b"Initial world\n");
    f.rou(&f.root, &["commit", "-m", "Byte paths"]);
    f.rou(&f.root, &["validate"]);
}

#[test]
fn lfs_directory_is_cached_per_snapshot_and_configuration_refreshes_between_operations() {
    let f = Fixture::new();
    f.seed();
    let trace = f._temp.path().join("trace.json");
    let output = f
        .command_builder(env!("CARGO_BIN_EXE_rou"), &f.root, &["validate"])
        .env("GIT_TRACE2_EVENT", &trace)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let events: Vec<Value> = fs::read_to_string(trace)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        events
            .iter()
            .filter(|e| e["event"] == "child_start" && e["argv"] == value!(["git-lfs", "env"]))
            .count(),
        1
    );
    let repo = rou_git::Repository::discover(&f.root).unwrap();
    let first = rou_git::snapshot::Snapshot::new(&repo, rou_git::snapshot::Mode::Index).unwrap();
    let old = first.media_dir().unwrap();
    f.git(&f.root, &["config", "lfs.storage", "custom-lfs"]);
    assert_eq!(first.media_dir().unwrap(), old);
    let second = rou_git::snapshot::Snapshot::new(&repo, rou_git::snapshot::Mode::Index).unwrap();
    assert_ne!(second.media_dir().unwrap(), old);
}

#[test]
fn lineage_cache_does_not_hide_new_ancestors_or_changed_realm_rules() {
    let f = Fixture::new();
    f.seed();
    f.rou(&f.root, &["validate"]);
    let mut realm: Realm = json::parse(&fs::read(f.root.join("realm.json")).unwrap()).unwrap();
    realm.id = "different_realm".into();
    f.write("realm.json", &json::pretty(&realm).unwrap());
    f.git(&f.root, &["add", "realm.json"]);
    f.git(&f.root, &["commit", "-m", "Native invalid ancestry"]);
    let output = f.raw(&f.root, &["validate"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("immutable realm rules"));
    f.git(&f.root, &["checkout", "HEAD~1", "--", "realm.json"]);
    f.git(&f.root, &["commit", "-m", "Restore current rules only"]);
    let output = f.raw(&f.root, &["validate"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("immutable realm rules"));
}

#[test]
fn oversized_non_lfs_source_reports_the_tracking_requirement() {
    let f = Fixture::new();
    f.seed();
    let data = vec![b'x'; rou_core::limits::POINTER_BYTES + 1];
    f.write("sources/house/house.blend", &data);
    let oid = String::from_utf8(f.git(
        &f.root,
        &[
            "hash-object",
            "--no-filters",
            "-w",
            "sources/house/house.blend",
        ],
    ))
    .unwrap();
    f.git(
        &f.root,
        &[
            "update-index",
            "--cacheinfo",
            &format!("100644,{},sources/house/house.blend", oid.trim()),
        ],
    );
    let output = f.raw(&f.root, &["validate", "--staged"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("source must be tracked by Git LFS"));
}

#[cfg(unix)]
#[test]
fn interrupted_hydration_releases_owned_index_lock_but_keeps_foreign_lock() {
    use std::time::{Duration, Instant};
    let f = Fixture::new();
    f.seed();
    let remote = f.remote();
    let clone = f.clone_realm(&remote, "signal-index");
    let real_git = String::from_utf8(
        f.command("/bin/sh", &f.root, &["-c", "command -v git"])
            .stdout,
    )
    .unwrap();
    let wrapper = f._temp.path().join("wrappers");
    executable(
        &wrapper.join("git"),
        &format!(
            r#"#!/usr/bin/env python3
import os,pathlib,sys,time
if 'add' in sys.argv and '--refresh' in sys.argv:
    pathlib.Path('.rou/index-test-started').touch()
    parent=os.getppid()
    while os.getppid()==parent: time.sleep(.02)
    sys.exit(1)
os.execv({git:?}, [{git:?}, *sys.argv[1:]])
"#,
            git = real_git.trim()
        ),
    );
    let mut child = f
        .command_builder(
            env!("CARGO_BIN_EXE_rou"),
            &clone,
            &["sparse", "set", "--entity", "house"],
        )
        .env(
            "PATH",
            format!("{}:{}", wrapper.display(), std::env::var("PATH").unwrap()),
        )
        .spawn()
        .unwrap();
    // Allow real Git/LFS startup on loaded machines before testing signal cleanup.
    let deadline = Instant::now() + Duration::from_secs(60);
    let marker = clone.join(".rou/index-test-started");
    while !marker.exists() && Instant::now() < deadline {
        assert!(child.try_wait().unwrap().is_none());
        std::thread::sleep(Duration::from_millis(20));
    }
    if !marker.exists() {
        let _ = child.kill();
        let _ = child.wait();
        panic!("hydration did not reach index refresh within 60 seconds");
    }
    let index_lock = clone.join(".git/index.lock");
    assert!(index_lock.exists());
    assert!(
        Command::new("/bin/kill")
            .args(["-INT", &child.id().to_string()])
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(child.wait().unwrap().code(), Some(130));
    assert!(!index_lock.exists());
    assert!(!clone.join(".rou/write.lock").exists());
    assert!(!fs::read_dir(clone.join(".git")).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("rou-index-")
    }));
    fs::write(&index_lock, b"foreign writer").unwrap();
    assert!(
        !f.raw(&clone, &["sparse", "set", "--entity", "bench"])
            .status
            .success()
    );
    assert_eq!(fs::read(index_lock).unwrap(), b"foreign writer");
}

#[test]
fn arbitrary_git_commands_and_native_help_are_forwarded() {
    let f = Fixture::new();
    f.seed();
    for args in [
        vec!["tag", "-l"],
        vec!["stash", "list"],
        vec!["show", "HEAD"],
        vec!["rev-parse", "--show-prefix"],
        vec!["ls-tree", "HEAD"],
    ] {
        let output = f.raw(&f.root, &args);
        assert!(output.status.success());
        assert_eq!(output.stdout, f.git(&f.root, &args));
    }
    for args in [vec!["help", "rev-parse"], vec!["rev-parse", "--help"]] {
        let output = f
            .command_builder(env!("CARGO_BIN_EXE_rou"), &f.root, &args)
            .env("GIT_MAN_VIEWER", "cat")
            .output()
            .unwrap();
        assert!(!String::from_utf8_lossy(&output.stderr).contains("unrecognized subcommand"));
    }
    let unknown = f.raw(&f.root, &["not-a-git-command"]);
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("is not a git command"));
    let reserved = f.raw(&f.root, &["commit", "--amend"]);
    assert_eq!(reserved.status.code(), Some(2));
    let help = f.raw(&f.root, &["--help"]);
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("Any other command is forwarded to Git unchanged"));
    assert!(!help.contains("Native Git commands ("));
    assert!(
        !help
            .lines()
            .any(|line| line.trim_start().starts_with("tag "))
    );
}

#[test]
fn human_reports_preserve_git_output_and_global_json_reports() {
    let f = Fixture::new();
    f.seed();
    f.write("README.md", b"Human-readable output\n");
    f.rou(&f.root, &["add", "README.md"]);
    let commit = f.raw(&f.root, &["commit", "-m", "Readable commit"]);
    assert!(commit.status.success());
    let text = String::from_utf8_lossy(&commit.stdout);
    assert!(text.lines().next().unwrap().starts_with("[main "));
    assert!(text.contains("Realm: 2 entities, 2 models validated"));
    assert!(!text.contains("git_output"));
    let switched = f.raw(&f.root, &["switch", "-c", "feature/output"]);
    assert!(switched.status.success());
    assert!(String::from_utf8_lossy(&switched.stderr).contains("Switched to a new branch"));
    for args in [
        vec!["--json", "validate"],
        vec!["validate", "--json"],
        vec!["sparse", "list", "--json"],
        vec!["diff", "--realm", "HEAD", "--json"],
    ] {
        let output = f.raw(&f.root, &args);
        assert!(output.status.success(), "{args:?}: {:?}", output.stderr);
        serde_json::from_slice::<Value>(&output.stdout).unwrap();
    }
    let selection = f.raw(&f.root, &["sparse", "list"]);
    assert_eq!(
        String::from_utf8_lossy(&selection.stdout),
        "All: false\nChunks: none\nEntities: none\n"
    );
    let build = f.raw(&f.root, &["build"]);
    assert!(String::from_utf8_lossy(&build.stdout).contains("Built 0 models; reused 2"));
    let view = f.raw(&f.root, &["view", "--entity", "house"]);
    assert!(String::from_utf8_lossy(&view.stdout).contains("2 models included, 0 missing"));
    let diff = f.raw(&f.root, &["diff", "--realm", "HEAD"]);
    assert!(
        String::from_utf8_lossy(&diff.stdout)
            .contains("Entities:\n  unchanged\nModels:\n  unchanged")
    );
    let machine = f.rou(&f.root, &["validate"]);
    assert_eq!(machine.as_object().unwrap().len(), 8);
    assert_eq!(machine["entity_count"], 2);
    assert!(machine["byte_validation_complete"].as_bool().unwrap());
}

#[test]
fn browse_selection_is_offline_by_default_and_explicit_selectors_fetch_only_assets() {
    let f = Fixture::new();
    let exporter = fs::read_to_string(&f.blender).unwrap().replace(
        "with_name('triangle.glb')",
        "with_name(model['id'] + '.glb')",
    );
    fs::write(&f.blender, exporter).unwrap();
    for id in ["house", "bench"] {
        let (mut document, binary) = common::triangle();
        document["extras"] = value!({"model": id});
        fs::write(
            f._temp.path().join(format!("{id}.glb")),
            common::glb(&document, &binary),
        )
        .unwrap();
    }
    f.seed();
    let remote = f.remote();
    let clone = f.clone_realm(&remote, "browser");
    let empty = f.rou(&clone, &["view"]);
    assert_eq!(empty["entity_count"], 2);
    assert_eq!(empty["downloaded_entities"], value!([]));
    let bundle = Path::new(empty["manifest"].as_str().unwrap())
        .parent()
        .unwrap();
    assert_eq!(fs::read_dir(bundle.join("objects")).unwrap().count(), 0);
    f.rou(&clone, &["sparse", "set", "--entity", "house"]);
    let trace = f._temp.path().join("browse-trace.log");
    let output = f
        .command_builder(
            env!("CARGO_BIN_EXE_rou"),
            &clone,
            &["view", "HEAD", "--json"],
        )
        .env("GIT_TRACE", &trace)
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output.stderr);
    let selected: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(selected["downloaded_entities"], value!(["house"]));
    let trace = fs::read_to_string(trace).unwrap();
    assert!(!trace.contains("git-lfs smudge") && !trace.contains("git-lfs fetch"));
    assert_eq!(fs::read_dir(bundle.join("objects")).unwrap().count(), 2);
    let catalog: Manifest = json::parse(&fs::read(bundle.join("realm.json")).unwrap()).unwrap();
    assert_eq!(catalog.entity_count, 2);
    let entities = bundle_entities(bundle);
    let house = entities.iter().find(|entity| entity.id == "house").unwrap();
    assert_ne!(house.asset.sha256, house.lod1.sha256);
    for asset in [&house.asset, &house.lod1] {
        let bytes = fs::read(bundle.join(&asset.path)).unwrap();
        asset.descriptor().verify(&bytes).unwrap();
        rou_core::glb::precheck(&bytes).unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            fs::metadata(bundle.join(&house.asset.path)).unwrap().ino(),
            fs::metadata(clone.join("runtime/house.glb")).unwrap().ino()
        );
        assert_eq!(
            fs::metadata(bundle.join(&house.lod1.path)).unwrap().ino(),
            fs::metadata(clone.join("runtime/house.lod1.glb"))
                .unwrap()
                .ino()
        );
    }
    assert!(
        rou_core::lfs::pointer(&fs::read(clone.join("runtime/bench.glb")).unwrap())
            .unwrap()
            .is_some()
    );
    let chunk = f.rou(&clone, &["view", "--chunk", "1,0,-1..1,0,0"]);
    assert_eq!(chunk["downloaded_entities"], value!(["bench"]));
    assert!(!clone.join("sources/bench").exists());
    assert_eq!(fs::read_dir(bundle.join("objects")).unwrap().count(), 2);
    let all = f.rou(&clone, &["view", "--all"]);
    assert_eq!(all["downloaded_entities"], value!(["bench", "house"]));
    // The two models have different full assets and share a third, distinct lod1 asset.
    assert_eq!(fs::read_dir(bundle.join("objects")).unwrap().count(), 3);
    let draft = f.rou(&clone, &["view", "--worktree"]);
    assert_eq!(draft["downloaded_entities"], value!(["house"]));
    assert_eq!(draft["version"]["commit_oid"], "");
    let explicit = f.rou(&clone, &["view", "--worktree", "--entity", "bench"]);
    assert_eq!(explicit["downloaded_entities"], value!(["bench"]));
    assert!(
        !f.raw(&clone, &["view", "--all", "--entity", "house"])
            .status
            .success()
    );
    assert!(
        !f.raw(&clone, &["view", "--all", "--chunk", "0,0,0"])
            .status
            .success()
    );
    assert!(
        !f.raw(&clone, &["view", "--entity", "unknown"])
            .status
            .success()
    );
    assert!(
        !f.raw(&clone, &["view", "--chunk", "999,0,0"])
            .status
            .success()
    );
    // Editing the worktree's runtime bytes must not leak into a fixed revision.
    fs::write(clone.join("runtime/house.glb"), b"changed worktree asset").unwrap();
    fs::write(
        clone.join("runtime/house.lod1.glb"),
        b"changed worktree lod1",
    )
    .unwrap();
    let fixed = f.rou(&clone, &["view", "--entity", "house"]);
    let entities = bundle_entities(
        Path::new(fixed["manifest"].as_str().unwrap())
            .parent()
            .unwrap(),
    );
    let house = entities.iter().find(|entity| entity.id == "house").unwrap();
    for asset in [&house.asset, &house.lod1] {
        asset
            .descriptor()
            .verify(&fs::read(bundle.join(&asset.path)).unwrap())
            .unwrap();
    }
}

#[test]
fn dual_assets_are_validated_and_large_builds_warn_without_rejection() {
    let f = Fixture::new();
    let (mut document, _) = common::triangle();
    let vertices = 600_000;
    let binary = vec![0; vertices * 12];
    document["buffers"][0]["byteLength"] = value!(binary.len());
    document["bufferViews"][0]["byteLength"] = value!(binary.len());
    document["accessors"][0]["count"] = value!(vertices);
    let bytes = common::glb(&document, &binary);
    fs::write(f._temp.path().join("triangle.glb"), &bytes).unwrap();
    fs::write(
        f._temp.path().join("metrics.json"),
        serde_json::to_vec(&rou_core::glb::precheck(&bytes).unwrap()).unwrap(),
    )
    .unwrap();
    f.seed();
    f.write("sources/house/house.blend", b"updated full source");
    let built = f.rou(&f.root, &["build"]);
    assert_eq!(built["metrics"]["house"]["asset"]["vertices"], vertices);
    assert!(
        built["warnings"]["house"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str().unwrap().contains("vertices"))
    );
    let record: Build = json::parse(&fs::read(f.root.join("runtime/house.json")).unwrap()).unwrap();
    for asset in [&record.asset, &record.lod1] {
        asset
            .descriptor()
            .verify(&fs::read(f.root.join(&asset.path)).unwrap())
            .unwrap();
    }
    f.write("sources/house/house.blend", b"another full source");
    let human = f.raw(&f.root, &["build"]);
    assert!(human.status.success());
    let output = String::from_utf8(human.stdout).unwrap();
    assert!(output.contains("600000 vertices") && output.contains("warning: house: asset:"));
    f.rou(&f.root, &["add", "."]);
    let validation = f.rou(&f.root, &["validate", "--staged"]);
    assert!(validation.get("warnings").is_none());
    f.write("runtime/house.lod1.glb", b"tampered simplified asset");
    f.rou(&f.root, &["add", "runtime/house.lod1.glb"]);
    let invalid = f.raw(&f.root, &["validate", "--staged"]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("house.lod1.glb"));
}

#[test]
fn build_rejects_oversized_output_descriptors_before_publication() {
    let f = Fixture::new();
    f.seed();
    let previous = fs::read(f.root.join("runtime/house.json")).unwrap();
    let exporter = fs::read_to_string(&f.blender)
        .unwrap()
        .replace("bytes=len(data)", "bytes=300*1024*1024");
    fs::write(&f.blender, exporter).unwrap();
    f.write("sources/house/house.blend", b"changed");
    let invalid = f.raw(&f.root, &["build"]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("GLB_BYTES"));
    assert_eq!(
        previous,
        fs::read(f.root.join("runtime/house.json")).unwrap()
    );
}

#[test]
fn shared_textures_deduplicate_survive_deletion_and_repair_from_lfs_history() {
    let f = Fixture::new();
    let texture = f.shared_texture(0);
    f.seed();
    let mut third: Model =
        json::parse(&fs::read(f.root.join(paths::model("bench"))).unwrap()).unwrap();
    third.id = "third".into();
    third.entry.name = "third".into();
    f.write(&paths::model("third"), &json::pretty(&third).unwrap());
    f.write(
        &paths::entity("third"),
        &json::pretty(&Entity {
            format: formats::ENTITY.into(),
            id: "third".into(),
            model: "third".into(),
            position_mm: [64000, 0, 0],
            label: None,
        })
        .unwrap(),
    );
    assert_eq!(f.rou(&f.root, &["build"])["generated"], value!(["third"]));
    f.rou(&f.root, &["add", "."]);
    f.rou(
        &f.root,
        &["commit", "-m", "Third model shares both image tiers"],
    );
    let image = f.root.join(&texture.path);
    let original = fs::read(&image).unwrap();
    let pointer = f.git(&f.root, &["show", &format!("HEAD:{}", texture.path)]);
    assert_eq!(
        rou_core::lfs::pointer(&pointer).unwrap().unwrap(),
        texture.descriptor()
    );
    assert_eq!(
        fs::read_dir(f.root.join("runtime"))
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|e| e == "png"))
            .count(),
        1
    );
    let lfs = f
        .root
        .join(".git/lfs/objects")
        .join(&texture.sha256[..2])
        .join(&texture.sha256[2..4])
        .join(&texture.sha256);
    assert_eq!(fs::read(&lfs).unwrap(), original);
    f.shared_texture(1);
    f.write("sources/house/house.blend", b"geometry revision");
    let built = f.rou(&f.root, &["build"]);
    assert_eq!(built["generated"], value!(["house"]));
    assert_eq!(fs::read(&image).unwrap(), original);
    f.rou(&f.root, &["add", "."]);
    let staged = f.git(&f.root, &["diff", "--cached", "--name-only"]);
    assert!(!String::from_utf8_lossy(&staged).contains(&texture.path));
    f.rou(&f.root, &["commit", "-m", "Geometry revision"]);
    // Simulate a merge removing the shared output while its surviving model still needs it.
    f.git(&f.root, &["rm", &texture.path]);
    assert!(!f.raw(&f.root, &["validate", "--staged"]).status.success());
    let repaired = f.rou(&f.root, &["build"]);
    assert_eq!(repaired["generated"], value!([]));
    assert_eq!(fs::read(&image).unwrap(), original);
    f.rou(&f.root, &["add", &texture.path]);
    f.rou(&f.root, &["validate", "--staged"]);
    fs::remove_file(f.root.join(paths::model("bench"))).unwrap();
    fs::remove_file(f.root.join(paths::entity("bench"))).unwrap();
    f.rou(&f.root, &["build"]);
    assert_eq!(fs::read(&image).unwrap(), original);
    fs::remove_file(f.root.join(paths::model("third"))).unwrap();
    fs::remove_file(f.root.join(paths::entity("third"))).unwrap();
    f.rou(&f.root, &["build"]);
    assert_eq!(fs::read(&image).unwrap(), original);
    fs::remove_file(f.root.join(paths::model("house"))).unwrap();
    fs::remove_file(f.root.join(paths::entity("house"))).unwrap();
    let deleted = f.rou(&f.root, &["build"]);
    assert!(!image.exists());
    assert!(
        deleted["removed"]
            .as_array()
            .unwrap()
            .contains(&value!(texture.path))
    );
}
