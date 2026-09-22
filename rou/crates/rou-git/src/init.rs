//! Initialize Git/LFS templates or create a metadata-only sparse clone. No implicit commit.
use crate::{
    Result, ensure, io,
    repository::{self, Repository},
};
use rou_core::{
    documents::Realm, formats, json, limits, paths, selection::Selection, spatial::Profile,
};
use std::{fs, path::Path};

impl Repository {
    /// Create or extend a repository with Realm/LFS templates, preserving an existing root document.
    /// Does not stage files or create a commit; rejects symlinks and invalid existing roots.
    pub fn init(directory: &Path) -> Result<Self> {
        io::reject_symlinks(directory)?;
        fs::create_dir_all(directory)?;
        let output = if !directory.join(".git").exists() {
            let output = repository::execute(directory, ["init", "-b", "main"], None)?;
            repository::checked(output.clone())?;
            Some(output)
        } else {
            None
        };
        let repo = Self::discover(directory)?;
        repo.output.borrow_mut().extend(output);
        ensure!(
            "invalid_repository",
            repo.root == directory.canonicalize()?,
            "init must target the Git root"
        );
        repo.run_visible(["lfs", "install", "--local", "--skip-smudge"])?;
        append_template(
            &io::destination(&repo.root, b".gitignore")?,
            "/.rou/\n/.godot/\n*.blend1\n*.blend2\n",
        )?;
        let attributes = [
            "blend", "glb", "png", "jpg", "jpeg", "exr", "hdr", "tif", "tiff",
        ]
        .map(|ext| format!("*.{ext} filter=lfs diff=lfs merge=lfs -text\n"))
        .concat();
        append_template(
            &io::destination(&repo.root, b".gitattributes")?,
            &attributes,
        )?;
        let realm_path = io::destination(&repo.root, paths::REALM.as_bytes())?;
        if realm_path.exists() {
            json::parse::<Realm>(&io::read(&realm_path, limits::JSON_BYTES)?)?.validate()?;
        } else {
            io::write_json(
                &realm_path,
                &Realm {
                    format: formats::REALM.into(),
                    id: format!("realm_{}", uuid::Uuid::new_v4().simple()),
                    profile: Profile::default(),
                },
            )?;
        }
        for relative in [
            "realm/models",
            "realm/entities",
            "runtime",
            "sources",
            "docs",
        ] {
            fs::create_dir_all(io::destination(&repo.root, relative.as_bytes())?)?;
        }
        let selection = io::local(&repo.root, "selection.json")?;
        if !selection.exists() {
            io::write_json(&selection, &Selection::default())?;
        }
        Ok(repo)
    }
    /// Clone native Git metadata with smudge disabled and only the base sparse cone.
    /// Initialize repository-local LFS configuration and an empty local selection.
    pub fn clone(source: &str, destination: &Path) -> Result<Self> {
        ensure!(
            "invalid_argument",
            !source.is_empty() && !source.starts_with('-'),
            "clone requires a source URL"
        );
        let current = std::env::current_dir()?;
        let output = repository::execute(
            &current,
            [
                std::ffi::OsStr::new("clone"),
                "--no-checkout".as_ref(),
                "--".as_ref(),
                source.as_ref(),
                destination.as_os_str(),
            ],
            None,
        )?;
        repository::checked(output.clone())?;
        let repo = Self::discover(destination)?;
        repo.output.borrow_mut().push(output);
        repo.run_visible(["lfs", "install", "--local", "--skip-smudge"])?;
        repo.run([
            "sparse-checkout",
            "set",
            "--cone",
            "--",
            "realm",
            "runtime",
            "docs",
        ])?;
        repo.run_visible(["checkout"])?;
        io::write_json(
            &io::local(&repo.root, "selection.json")?,
            &Selection::default(),
        )?;
        Ok(repo)
    }
}

fn append_template(path: &Path, template: &str) -> Result<()> {
    let mut current = if path.exists() {
        String::from_utf8(io::read(path, limits::JSON_BYTES)?)?
    } else {
        String::new()
    };
    if !current.is_empty() && !current.ends_with('\n') {
        current.push('\n');
    }
    for line in template.lines() {
        if !current.lines().any(|existing| existing == line) {
            current.push_str(line);
            current.push('\n');
        }
    }
    io::atomic_write(path, current.as_bytes())
}
