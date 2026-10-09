//! Human-readable presentation of host reports; machine reports are serialized unchanged.
use crate::{Command, Release, Sparse};
use rou_git::{Repository, Result};
use serde_json::Value;
use std::fmt::Write;

pub fn kind(command: &Command) -> &'static str {
    match command {
        Command::Init { .. } => "init",
        Command::Clone { .. } => "clone",
        Command::Sparse {
            action: Sparse::List,
        } => "selection",
        Command::Sparse { .. } => "sparse",
        Command::Checkout {
            branch: None,
            revision: None,
            select,
        } if select.chosen().is_some() => "sparse",
        Command::Switch { .. } | Command::Checkout { .. } => "switch",
        Command::Status => "status",
        Command::Build { .. } => "build",
        Command::Validate { .. } => "validate",
        Command::Commit { .. } => "commit",
        Command::Merge { abort: true, .. } => "abort",
        Command::Merge { .. } | Command::Pull { .. } => "merge",
        Command::Push { .. } => "push",
        Command::View { .. } => "view",
        Command::Release {
            action: Release::Create { .. },
        } => "release",
        Command::Release {
            action: Release::List,
        } => "release_list",
        Command::Diff(_) => "diff",
        Command::Native(_) => "native",
    }
}

fn array(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}
fn string(value: &Value) -> &str {
    value.as_str().unwrap_or("")
}
fn list(value: &Value) -> String {
    array(value)
        .iter()
        .map(string)
        .collect::<Vec<_>>()
        .join(", ")
}
fn count(n: usize, singular: &str, plural: &str) -> String {
    format!("{n} {}", if n == 1 { singular } else { plural })
}
fn named(value: &Value, singular: &str, plural: &str) -> String {
    let n = array(value).len();
    let mut text = count(n, singular, plural);
    if n > 0 {
        write!(text, " ({})", list(value)).unwrap();
    }
    text
}

pub fn status(repo: &Repository) -> Result<String> {
    let output = repo.run(["status", "--short", "--branch"])?;
    let selection = repo.selection()?;
    Ok(format!(
        "{}Selection: {}\nDomain validation: not run\n",
        String::from_utf8_lossy(&output),
        if selection.all {
            "all current entities".into()
        } else {
            format!(
                "{} chunk selectors, {} explicit entities",
                selection.chunks.len(),
                selection.entities.len()
            )
        }
    ))
}

pub fn render(kind: &str, value: &Value) -> String {
    let mut text = match kind {
        "init" | "clone" => format!(
            "{} realm at {}\n{}",
            if kind == "init" {
                "Initialized"
            } else {
                "Cloned"
            },
            string(&value["root"]),
            render("selection", &value["selection"])
        ),
        "selection" => format!(
            "All: {}\nChunks: {}\nEntities: {}\n",
            value["all"].as_bool().unwrap_or(false),
            if array(&value["chunks"]).is_empty() {
                "none".into()
            } else {
                list(&value["chunks"])
            },
            if array(&value["entities"]).is_empty() {
                "none".into()
            } else {
                list(&value["entities"])
            }
        ),
        "sparse" => {
            let resolved = &value["resolution"];
            let directories = array(&resolved["directories"])
                .iter()
                .map(string)
                .filter(|s| s.starts_with("sources/"))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "Selected {}: {}, {}{}\n",
                named(&resolved["entity_ids"], "entity", "entities"),
                count(
                    array(&resolved["paths"]).len(),
                    "source file",
                    "source files"
                ),
                count(
                    array(&resolved["assets"]).len(),
                    "runtime asset",
                    "runtime assets"
                ),
                if directories.is_empty() {
                    String::new()
                } else {
                    format!(" in {directories}")
                }
            )
        }
        "build" => {
            let mut text = format!(
                "Built {}; reused {}; outputs written to runtime/, nothing staged\n",
                named(&value["generated"], "model", "models"),
                array(&value["reused"]).len()
            );
            for id in array(&value["generated"]) {
                let id = string(id);
                for tier in ["asset", "lod1"] {
                    let metrics = &value["metrics"][&id][tier];
                    if metrics.is_null() {
                        writeln!(text, "  {id} {tier}: statistics unavailable").unwrap();
                        continue;
                    }
                    writeln!(text, "  {id} {tier}: {} bytes, {} vertices, {} indices, {} nodes, max image {} px, decoded {} bytes",
                        metrics["bytes"], metrics["vertices"], metrics["indices"], metrics["nodes"], metrics["max_image_dimension"], metrics["image_decoded_bytes"]).unwrap();
                }
                for warning in array(&value["warnings"][&id]) {
                    writeln!(text, "warning: {id}: {}", string(warning)).unwrap();
                }
            }
            if !array(&value["removed"]).is_empty() {
                writeln!(text, "Removed: {}", list(&value["removed"])).unwrap();
            }
            text
        }
        "validate" | "commit" => {
            let report = if kind == "commit" {
                &value["validation"]
            } else {
                value
            };
            let missing = array(&report["missing_assets"]).len();
            let mut text = format!(
                "{}: {} entities, {} models{}",
                if kind == "commit" { "Realm" } else { "OK" },
                report["entity_count"],
                report["model_count"],
                if kind == "commit" { " validated" } else { "" }
            );
            if missing > 0 {
                write!(
                    text,
                    "; byte validation incomplete ({} not downloaded)",
                    count(missing, "asset", "assets")
                )
                .unwrap();
            }
            text.push('\n');
            text
        }
        "switch" => format!("Realm: selection applied at {}\n", string(&value["head"])),
        "merge" => format!(
            "Realm: merge pending at {}; inspect status before committing\n",
            string(&value["head"])
        ),
        "integrated" => format!(
            "Realm: integrated at {}; selection refreshed\n",
            string(&value["head"])
        ),
        "abort" => "Realm: merge aborted; selection restored\n".into(),
        "push" => format!(
            "Realm: published {} with required LFS history\n",
            string(&value["commit"])
        ),
        "release" => {
            let mut text = format!(
                "Created browse tag {} at {} (view root {})\nNothing was pushed. Publish with: rou push --follow-tags\n",
                string(&value["tag"]),
                string(&value["commit_oid"]),
                string(&value["view_root_sha256"])
            );
            if let Some(address) = value["address"].as_str() {
                writeln!(text, "Share after publishing: {address}").unwrap();
            }
            text
        }
        "release_list" => {
            let mut text = String::new();
            for release in array(value) {
                writeln!(
                    text,
                    "{} {} {}",
                    string(&release["tag"]),
                    string(&release["commit_oid"]),
                    string(&release["view_root_sha256"])
                )
                .unwrap();
            }
            if text.is_empty() {
                text.push_str("No browse releases\n");
            }
            text
        }
        "diff" => domain_diff(value),
        _ => unreachable!("missing human report for {kind}"),
    };
    let backups = array(&value["backups"]);
    if let Some(first) = backups.first() {
        let path = std::path::Path::new(string(first));
        let directory = path
            .ancestors()
            .find(|p| {
                p.parent()
                    .is_some_and(|parent| parent.file_name().is_some_and(|name| name == "backups"))
            })
            .unwrap_or(path);
        writeln!(
            text,
            "Moved {} to {}/",
            count(backups.len(), "ignored backup", "ignored backups"),
            directory.display()
        )
        .unwrap();
    }
    text
}

fn domain_diff(value: &Value) -> String {
    let mut text = format!(
        "Realm diff: {} -> {}\n",
        string(&value["from"]),
        string(&value["to"])
    );
    if !array(&value["realm_changes"]).is_empty() {
        writeln!(text, "Realm: {}", list(&value["realm_changes"])).unwrap();
    }
    text.push_str("Entities:\n");
    let mut entities = 0;
    for (key, verb) in [("added_entities", "added"), ("removed_entities", "removed")] {
        for id in array(&value[key]) {
            writeln!(text, "  {}: {verb}", string(id)).unwrap();
            entities += 1;
        }
    }
    for entity in array(&value["changed_entities"]) {
        writeln!(
            text,
            "  {}: {}",
            string(&entity["id"]),
            list(&entity["changes"])
        )
        .unwrap();
        entities += 1;
    }
    if entities == 0 {
        text.push_str("  unchanged\n");
    }
    text.push_str("Models:\n");
    for model in array(&value["model_changes"]) {
        writeln!(
            text,
            "  {}: {}",
            string(&model["id"]),
            string(&model["change"])
        )
        .unwrap();
    }
    if array(&value["model_changes"]).is_empty() {
        text.push_str("  unchanged\n");
    }
    text
}

pub fn sparse(value: &Value, materialized: usize, already_present: usize) -> String {
    let text = render("sparse", value);
    let (first, rest) = text.split_once('\n').unwrap_or((&text, ""));
    format!(
        "{first}; materialized {}, {already_present} already present\n{rest}",
        count(materialized, "payload", "payloads")
    )
}

pub fn view(report: &rou_git::view::ViewReport) -> String {
    format!(
        "Bundle: {}; {} entities in catalog, {} models included, {} missing; {}\n",
        report.manifest.parent().unwrap().display(),
        report.entity_count,
        report.included_models,
        report.model_count - report.included_models,
        if report.version.commit_oid.is_empty() {
            "worktree draft".into()
        } else {
            format!("commit {}", report.version.commit_oid)
        }
    )
}
