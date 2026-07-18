//! Workspace membership guard for publishable crate inventory.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const REPO_NAME: &str = "sim-audio-daw";

pub fn run(args: Vec<String>) -> Result<(), String> {
    let program = args.first().map(String::as_str).unwrap_or("xtask");
    if args.get(1).map(String::as_str) != Some("workspace-coverage") {
        return Err(format!("usage: {program} workspace-coverage [--check]"));
    }
    for arg in args.iter().skip(2) {
        if arg != "--check" {
            return Err(format!("usage: {program} workspace-coverage [--check]"));
        }
    }

    let root = env::current_dir().map_err(|err| format!("current dir: {err}"))?;
    let workspace = read_workspace(&root)?;
    let local = read_local_crates(&root)?;
    let mut errors = Vec::new();

    for path in &local.paths {
        if !workspace.members.contains(path) && !workspace.excludes.contains(path) {
            errors.push(format!(
                "{path} is neither a workspace member nor a workspace.exclude entry"
            ));
        }
    }

    for member in &workspace.members {
        if member == "xtask" {
            continue;
        }
        if !local.paths.contains(member) {
            errors.push(format!(
                "{member} is a workspace member but has no local crate"
            ));
        }
    }

    for excluded in &workspace.excludes {
        if !local.paths.contains(excluded) {
            errors.push(format!(
                "{excluded} is excluded from the workspace but has no local crate"
            ));
        }
    }

    match locate_sim_private_manifest(&root) {
        Some(path) => compare_sim_private_inventory(&path, &local, &mut errors)?,
        None => {
            println!(
                "workspace-coverage: sim-private manifest not found; checked local crate classification only"
            );
        }
    }

    if errors.is_empty() {
        println!(
            "workspace-coverage: OK ({} local crates, {} members, {} excluded)",
            local.paths.len(),
            workspace.members.len(),
            workspace.excludes.len()
        );
        Ok(())
    } else {
        Err(format!(
            "workspace coverage failed:\n- {}",
            errors.join("\n- ")
        ))
    }
}

struct WorkspaceInventory {
    members: BTreeSet<String>,
    excludes: BTreeSet<String>,
}

struct LocalCrates {
    names: BTreeSet<String>,
    paths: BTreeSet<String>,
}

fn read_workspace(root: &Path) -> Result<WorkspaceInventory, String> {
    let manifest = root.join("Cargo.toml");
    let contents = fs::read_to_string(&manifest)
        .map_err(|err| format!("read {}: {err}", manifest.display()))?;
    Ok(WorkspaceInventory {
        members: parse_section_array(&contents, "workspace", "members")?,
        excludes: parse_section_array(&contents, "workspace", "exclude")?,
    })
}

fn read_local_crates(root: &Path) -> Result<LocalCrates, String> {
    let crates_dir = root.join("crates");
    let mut names = BTreeSet::new();
    let mut paths = BTreeSet::new();
    for entry in
        fs::read_dir(&crates_dir).map_err(|err| format!("read {}: {err}", crates_dir.display()))?
    {
        let entry = entry.map_err(|err| format!("read crate entry: {err}"))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest = path.join("Cargo.toml");
        if !manifest.is_file() {
            continue;
        }
        let name = parse_package_name(&manifest)?;
        names.insert(name);
        let dir_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("non-utf8 crate path {}", path.display()))?;
        paths.insert(format!("crates/{dir_name}"));
    }
    Ok(LocalCrates { names, paths })
}

fn locate_sim_private_manifest(root: &Path) -> Option<PathBuf> {
    if let Ok(path) = env::var("SIM_PRIVATE_REPOS_TOML") {
        return Some(PathBuf::from(path));
    }
    let sibling = root.parent()?.join("sim-private").join("repos.toml");
    sibling.is_file().then_some(sibling)
}

fn compare_sim_private_inventory(
    manifest: &Path,
    local: &LocalCrates,
    errors: &mut Vec<String>,
) -> Result<(), String> {
    let contents = fs::read_to_string(manifest)
        .map_err(|err| format!("read {}: {err}", manifest.display()))?;
    let Some(row) = find_repo_row(&contents, REPO_NAME) else {
        errors.push(format!("sim-private manifest has no {REPO_NAME} row"));
        return Ok(());
    };
    let crate_names = parse_array_in_text(row, "crate_names")?;
    let source_paths = parse_array_in_text(row, "source_paths")?;
    if crate_names != local.names {
        errors.push(format!(
            "sim-private crate_names differ from local crates: manifest={crate_names:?} local={:?}",
            local.names
        ));
    }
    if source_paths != local.paths {
        errors.push(format!(
            "sim-private source_paths differ from local crates: manifest={source_paths:?} local={:?}",
            local.paths
        ));
    }
    Ok(())
}

fn find_repo_row<'a>(contents: &'a str, repo_name: &str) -> Option<&'a str> {
    contents
        .split("[[repo]]")
        .find(|row| parse_string_value(row, "name").as_deref() == Some(repo_name))
}

fn parse_package_name(manifest: &Path) -> Result<String, String> {
    let contents = fs::read_to_string(manifest)
        .map_err(|err| format!("read {}: {err}", manifest.display()))?;
    let mut in_package = false;
    for line in contents.lines() {
        let line = without_comment(line);
        if line.starts_with('[') {
            in_package = line == "[package]";
            continue;
        }
        if in_package && let Some(name) = parse_string_assignment(line, "name") {
            return Ok(name);
        }
    }
    Err(format!("{} has no [package] name", manifest.display()))
}

fn parse_section_array(
    contents: &str,
    section: &str,
    key: &str,
) -> Result<BTreeSet<String>, String> {
    let mut in_section = false;
    let mut section_text = String::new();
    for line in contents.lines() {
        let trimmed = without_comment(line);
        if trimmed.starts_with('[') {
            in_section = trimmed == format!("[{section}]");
            continue;
        }
        if in_section {
            section_text.push_str(line);
            section_text.push('\n');
        }
    }
    parse_array_in_text(&section_text, key)
}

fn parse_array_in_text(contents: &str, key: &str) -> Result<BTreeSet<String>, String> {
    let mut values = BTreeSet::new();
    let mut collecting = false;
    let mut found = false;
    for line in contents.lines() {
        let line = without_comment(line);
        if collecting {
            collect_quoted_strings(line, &mut values)?;
            if line.contains(']') {
                return Ok(values);
            }
            continue;
        }
        if let Some(rest) = line
            .strip_prefix(key)
            .and_then(|rest| rest.trim_start().strip_prefix('='))
        {
            found = true;
            let rest = rest.trim_start();
            if !rest.starts_with('[') {
                return Err(format!("{key} must be a string array"));
            }
            collect_quoted_strings(rest, &mut values)?;
            if rest.contains(']') {
                return Ok(values);
            }
            collecting = true;
        }
    }
    if found {
        Err(format!("{key} array is not closed"))
    } else {
        Ok(values)
    }
}

fn parse_string_value(contents: &str, key: &str) -> Option<String> {
    contents
        .lines()
        .map(without_comment)
        .find_map(|line| parse_string_assignment(line, key))
}

fn parse_string_assignment(line: &str, key: &str) -> Option<String> {
    line.strip_prefix(key)
        .and_then(|rest| rest.trim_start().strip_prefix('='))
        .and_then(|rest| first_quoted_string(rest.trim_start()))
}

fn collect_quoted_strings(line: &str, values: &mut BTreeSet<String>) -> Result<(), String> {
    let mut rest = line;
    while let Some(start) = rest.find('"') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('"') else {
            return Err(format!("unterminated string in {line:?}"));
        };
        values.insert(rest[..end].to_owned());
        rest = &rest[end + 1..];
    }
    Ok(())
}

fn first_quoted_string(line: &str) -> Option<String> {
    let rest = line.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

fn without_comment(line: &str) -> &str {
    line.split('#').next().unwrap_or("").trim()
}
