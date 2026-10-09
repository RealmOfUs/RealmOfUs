//! Online source addresses and release annotation rules.
//!
//! These functions only parse and validate text. They do not resolve hosts, inspect
//! Git repositories, or perform network requests.

use crate::{Error, Result, error::require};
use serde::Serialize;

/// Prefix used in annotated Git tags to bind a release to its browse view root.
pub const VIEW_ROOT_PREFIX: &str = "view-root: ";

/// A validated online browse source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Source {
    /// A public release hosted by a Git forge.
    Repository {
        /// Original canonical address supplied by the user.
        address: String,
        /// Host and optional port, without credentials.
        host: String,
        /// Repository owner path component.
        owner: String,
        /// Repository name without a `.git` suffix.
        repository: String,
        /// Explicit release tag, or `None` to discover the latest browse release.
        tag: Option<String>,
        /// HTTP origin inferred from the host.
        origin: String,
    },
    /// A browse bundle rooted at a public HTTP URL.
    Static {
        /// Original canonical URL supplied by the user.
        address: String,
        /// URL of the bundle's `realm.json`.
        manifest_url: String,
        /// Directory URL used to resolve pages and objects.
        base_url: String,
    },
}

/// Parse a `rou://host/owner/repository[@tag]` address or a static bundle URL.
///
/// Repository addresses never carry credentials. A missing tag requests forge-side
/// latest-release discovery; an explicit tag is checked as a safe Git ref name and
/// must not be a bare commit identifier. Static URLs must use HTTP(S), contain no
/// credentials, query or fragment, and end in `/realm.json`.
pub fn parse_source(input: &str) -> Result<Source> {
    require(
        !input.is_empty() && input.is_ascii() && !input.bytes().any(|byte| byte.is_ascii_control()),
        "invalid_url",
        "online source must be non-empty ASCII without control characters",
    )?;
    require(
        input.trim() == input,
        "invalid_url",
        "online source must not contain surrounding whitespace",
    )?;
    if let Some(rest) = input.strip_prefix("rou://") {
        return parse_repository(input, rest);
    }
    if input.starts_with("https://") || input.starts_with("http://") {
        return parse_static(input);
    }
    Err(Error::new(
        "invalid_url",
        "online source must use rou://, https:// or http://",
    ))
}

/// Create the canonical line stored in an annotated release tag.
pub fn release_annotation(view_root_sha256: &str) -> Result<String> {
    validate_sha256(view_root_sha256)?;
    Ok(format!("{VIEW_ROOT_PREFIX}{view_root_sha256}"))
}

/// Extract a view-root digest from an annotated tag message.
///
/// Messages without the annotation return `None`. Duplicate annotation lines are
/// rejected, even when they contain the same digest.
pub fn parse_release_annotation(message: &str) -> Result<Option<String>> {
    let mut found = None;
    for line in message.lines() {
        if let Some(value) = line.strip_prefix(VIEW_ROOT_PREFIX) {
            require(
                found.is_none(),
                "invalid_release_annotation",
                "release annotation contains more than one view-root line",
            )?;
            validate_sha256(value).map_err(|_| {
                Error::new(
                    "invalid_release_annotation",
                    "release annotation has an invalid view-root SHA-256",
                )
            })?;
            found = Some(value.to_owned());
        }
    }
    Ok(found)
}

fn parse_repository(address: &str, rest: &str) -> Result<Source> {
    reject_url_suffixes(rest)?;
    let (authority, path) = rest.split_once('/').ok_or_else(|| {
        Error::new(
            "invalid_url",
            "repository address must include owner and name",
        )
    })?;
    validate_authority(authority)?;
    require(
        !path.contains('%'),
        "invalid_url",
        "repository address must not use percent encoding",
    )?;
    let (owner, repository_and_tag) = path.split_once('/').ok_or_else(|| {
        Error::new(
            "invalid_url",
            "repository address must include owner and name",
        )
    })?;
    require(
        !owner.is_empty() && !repository_and_tag.is_empty(),
        "invalid_url",
        "repository owner and name must not be empty",
    )?;
    let (repository, tag) = match repository_and_tag.split_once('@') {
        Some((repository, tag)) => {
            require(
                !repository.contains('@') && !tag.contains('@'),
                "invalid_url",
                "repository address contains an unexpected @ character",
            )?;
            validate_release_tag(tag)?;
            (repository, Some(tag.to_owned()))
        }
        None => {
            require(
                !repository_and_tag.contains('/'),
                "invalid_url",
                "repository address must have exactly two path components",
            )?;
            (repository_and_tag, None)
        }
    };
    require(
        !repository.contains('/'),
        "invalid_url",
        "repository address must have exactly two path components before the tag",
    )?;
    validate_repository_component(owner, "owner")?;
    validate_repository_component(repository, "repository")?;
    require(
        !repository.ends_with(".git"),
        "invalid_url",
        "rou repository names must omit the .git suffix",
    )?;
    let scheme = if is_loopback(authority) {
        "http"
    } else {
        "https"
    };
    Ok(Source::Repository {
        address: address.to_owned(),
        host: authority.to_owned(),
        owner: owner.to_owned(),
        repository: repository.to_owned(),
        tag,
        origin: format!("{scheme}://{authority}"),
    })
}

fn parse_static(address: &str) -> Result<Source> {
    reject_url_suffixes(address)?;
    let (_, rest) = address
        .split_once("://")
        .expect("caller checked the HTTP scheme");
    let (authority, path) = rest
        .split_once('/')
        .ok_or_else(|| Error::new("invalid_url", "static source must name realm.json"))?;
    validate_authority(authority)?;
    require(
        !path.contains('%') && !path.contains("//"),
        "invalid_url",
        "static source path must be canonical and unescaped",
    )?;
    require(
        path == "realm.json" || path.ends_with("/realm.json"),
        "invalid_url",
        "static source must end in /realm.json",
    )?;
    require(
        !path
            .split('/')
            .any(|component| component.is_empty() || component == "." || component == ".."),
        "invalid_url",
        "static source path must not contain empty or relative components",
    )?;
    let base_url = address
        .strip_suffix("/realm.json")
        .or_else(|| address.strip_suffix("realm.json"))
        .expect("suffix checked")
        .trim_end_matches('/')
        .to_owned();
    Ok(Source::Static {
        address: address.to_owned(),
        manifest_url: address.to_owned(),
        base_url,
    })
}

fn reject_url_suffixes(value: &str) -> Result<()> {
    require(
        !value.contains(['?', '#']),
        "invalid_url",
        "online source must not contain a query or fragment",
    )
}

fn validate_authority(authority: &str) -> Result<()> {
    require(
        !authority.is_empty() && !authority.contains('@'),
        "invalid_url",
        "online source host must be present and must not contain credentials",
    )?;
    require(
        !authority.contains(['/', '\\', ' ', '\t']),
        "invalid_url",
        "online source host is invalid",
    )?;
    let (host, port) = if authority.starts_with('[') {
        let end = authority
            .find(']')
            .ok_or_else(|| Error::new("invalid_url", "IPv6 host is missing closing bracket"))?;
        let suffix = &authority[end + 1..];
        require(
            suffix.is_empty() || suffix.starts_with(':'),
            "invalid_url",
            "invalid characters after IPv6 host",
        )?;
        (&authority[..=end], suffix.strip_prefix(':'))
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        require(
            !host.contains(':'),
            "invalid_url",
            "IPv6 hosts must use brackets",
        )?;
        (host, Some(port))
    } else {
        (authority, None)
    };
    require(
        !host.is_empty(),
        "invalid_url",
        "online source host is empty",
    )?;
    if !host.starts_with('[') {
        require(
            host.bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
                && !host.starts_with(['.', '-'])
                && !host.ends_with(['.', '-'])
                && !host.contains(".."),
            "invalid_url",
            "online source host is invalid",
        )?;
    }
    if let Some(port) = port {
        let number: u16 = port
            .parse()
            .map_err(|_| Error::new("invalid_url", "online source port is invalid"))?;
        require(number != 0, "invalid_url", "online source port is invalid")?;
    }
    Ok(())
}

fn validate_repository_component(value: &str, label: &str) -> Result<()> {
    require(
        !value.is_empty()
            && value != "."
            && value != ".."
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
            && !value.starts_with('.')
            && !value.ends_with('.')
            && !value.contains(".."),
        "invalid_url",
        format!("repository {label} is invalid"),
    )
}

/// Validate a release tag as a safe Git ref suffix: it must not spell a full ref
/// (`refs/...`) and must not be a bare commit identifier.
pub fn validate_release_tag(tag: &str) -> Result<()> {
    require(
        !tag.is_empty()
            && tag.is_ascii()
            && !tag.starts_with(['.', '/'])
            && !tag.starts_with("refs/")
            && !tag.ends_with(['.', '/'])
            && !tag.ends_with(".lock")
            && !tag.contains("..")
            && !tag.contains("@{")
            && !tag.contains("//")
            && !tag.bytes().any(|byte| {
                byte.is_ascii_control()
                    || byte == b' '
                    || matches!(
                        byte,
                        b'~' | b'^' | b':' | b'?' | b'*' | b'[' | b'\\' | b'@' | b'%' | b'#'
                    )
            }),
        "invalid_url",
        "release tag is not a safe Git ref name",
    )?;
    require(
        !(matches!(tag.len(), 40 | 64) && tag.bytes().all(|byte| byte.is_ascii_hexdigit())),
        "invalid_url",
        "release address must use a tag, not a bare commit identifier",
    )
}

fn validate_sha256(value: &str) -> Result<()> {
    require(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "invalid_sha256",
        "view root must be 64 lowercase hexadecimal characters",
    )
}

fn is_loopback(authority: &str) -> bool {
    let host = if authority.starts_with('[') {
        authority
            .split_once(']')
            .map(|(host, _)| format!("{host}]"))
            .unwrap_or_default()
    } else {
        authority.split(':').next().unwrap_or_default().to_owned()
    };
    host == "localhost" || host == "127.0.0.1" || host == "[::1]"
}
