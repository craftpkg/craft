use std::path::Path;

use anyhow::{Context as _, anyhow, bail};
use contract::Result;
use node_semver::{Range, Version};
use tokio::process::Command;

const SEMVER_PREFIX: &str = "semver:";
const FULL_SHA_LENGTH: usize = 40;
const MIN_SHORT_SHA_LENGTH: usize = 7;
const HOSTS: [(&str, Host); 3] = [
    ("github.com", Host::GitHub),
    ("gitlab.com", Host::GitLab),
    ("bitbucket.org", Host::Bitbucket),
];

/// A git dependency: where to fetch it from and which commit-ish to install.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GitSpec {
    pub clone_url: String,
    pub hosted: Option<HostedRepo>,
    pub committish: Committish,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HostedRepo {
    host: Host,
    owner: String,
    name: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Host {
    GitHub,
    GitLab,
    Bitbucket,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Committish {
    Default,
    Ref(String),
    Semver(Range),
}

impl GitSpec {
    /// Parses npm's git spec forms: `github:`/`gitlab:`/`bitbucket:`/`gist:` shorthands,
    /// `owner/repo`, and `git+<protocol>://` or `git://` URLs, each with an optional
    /// `#<branch|tag|sha>` or `#semver:<range>`.
    pub fn parse(spec: &str) -> Option<Self> {
        let (location, committish) = match spec.split_once('#') {
            Some((location, committish)) => (location, parse_committish(committish)?),
            None => (spec, Committish::Default),
        };

        let shorthand = [
            ("github:", Host::GitHub),
            ("gitlab:", Host::GitLab),
            ("bitbucket:", Host::Bitbucket),
        ]
        .into_iter()
        .find_map(|(prefix, host)| Some((host, location.strip_prefix(prefix)?)));
        if let Some((host, path)) = shorthand {
            let hosted = HostedRepo::from_path(host, path)?;
            return Some(Self {
                clone_url: hosted.https_url(),
                hosted: Some(hosted),
                committish,
            });
        }

        if let Some(gist) = location.strip_prefix("gist:") {
            let id = gist.rsplit('/').next().unwrap_or(gist);
            return Some(Self {
                clone_url: format!("https://gist.github.com/{id}.git"),
                hosted: None,
                committish,
            });
        }

        let is_url = location.contains("://") || location.starts_with("git@");
        if !is_url {
            let hosted = HostedRepo::from_path(Host::GitHub, location)?;
            return Some(Self {
                clone_url: hosted.https_url(),
                hosted: Some(hosted),
                committish,
            });
        }

        let clone_url = location
            .strip_prefix("git+")
            .unwrap_or(location)
            .to_string();
        Some(Self {
            hosted: HostedRepo::from_url(&clone_url),
            clone_url,
            committish,
        })
    }
}

impl HostedRepo {
    fn from_path(host: Host, path: &str) -> Option<Self> {
        let (owner, name) = path.trim_matches('/').split_once('/')?;
        let name = name.strip_suffix(".git").unwrap_or(name);
        let is_valid = !owner.is_empty() && !name.is_empty() && !name.contains('/');
        is_valid.then(|| Self {
            host,
            owner: owner.to_string(),
            name: name.to_string(),
        })
    }

    /// Recognizes `https://github.com/o/r.git`, `ssh://git@github.com/o/r.git` and
    /// `git@github.com:o/r.git`, so public repos can be fetched as tarballs.
    fn from_url(url: &str) -> Option<Self> {
        HOSTS.into_iter().find_map(|(domain, host)| {
            let (_, path) = url.split_once(domain)?;
            let path = path.strip_prefix([':', '/'])?;
            Self::from_path(host, path)
        })
    }

    fn https_url(&self) -> String {
        let domain = match self.host {
            Host::GitHub => "github.com",
            Host::GitLab => "gitlab.com",
            Host::Bitbucket => "bitbucket.org",
        };
        format!("https://{domain}/{}/{}.git", self.owner, self.name)
    }

    pub fn tarball_url(&self, sha: &str) -> String {
        let Self { owner, name, .. } = self;
        match self.host {
            Host::GitHub => format!("https://codeload.github.com/{owner}/{name}/tar.gz/{sha}"),
            Host::GitLab => {
                format!("https://gitlab.com/{owner}/{name}/-/archive/{sha}/{name}-{sha}.tar.gz")
            }
            Host::Bitbucket => format!("https://bitbucket.org/{owner}/{name}/get/{sha}.tar.gz"),
        }
    }
}

/// The commit `committish` names in the remote, found with `git ls-remote` so nothing is
/// cloned just to pin a ref.
pub(crate) async fn resolve_commit(spec: &GitSpec) -> Result<String> {
    if let Committish::Ref(reference) = &spec.committish
        && is_full_sha(reference)
    {
        return Ok(reference.clone());
    }

    let output = git(&["ls-remote", &spec.clone_url], None).await?;
    let refs = parse_ls_remote(&output);
    select_commit(&refs, &spec.committish).ok_or_else(|| {
        anyhow!(
            "no commit in {} matches {}",
            spec.clone_url,
            describe(&spec.committish)
        )
    })
}

/// Fetches exactly `sha` into `dir`, falling back to a full clone for servers that refuse
/// fetching commits by id.
pub(crate) async fn checkout(clone_url: &str, sha: &str, dir: &Path) -> Result<()> {
    git(&["init", "--quiet"], Some(dir)).await?;
    let fetched = git(
        &["fetch", "--quiet", "--depth", "1", clone_url, sha],
        Some(dir),
    )
    .await;
    if fetched.is_err() {
        let all_refs = [
            "fetch",
            "--quiet",
            clone_url,
            "+refs/heads/*:refs/remotes/origin/*",
            "+refs/tags/*:refs/tags/*",
        ];
        git(&all_refs, Some(dir)).await?;
    }
    let target = if fetched.is_ok() { "FETCH_HEAD" } else { sha };
    git(&["checkout", "--quiet", target], Some(dir)).await?;
    Ok(())
}

async fn git(args: &[&str], dir: Option<&Path>) -> Result<String> {
    let mut command = Command::new("git");
    command.args(args).env("GIT_TERMINAL_PROMPT", "0");
    if let Some(dir) = dir {
        command.current_dir(dir);
    }

    let output = command
        .output()
        .await
        .context("git dependencies need git on PATH")?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn parse_committish(committish: &str) -> Option<Committish> {
    if committish.is_empty() {
        return Some(Committish::Default);
    }
    match committish.strip_prefix(SEMVER_PREFIX) {
        Some(range) => Range::parse(range).ok().map(Committish::Semver),
        None => Some(Committish::Ref(committish.to_string())),
    }
}

/// `(sha, ref)` pairs from `git ls-remote` output.
fn parse_ls_remote(output: &str) -> Vec<(String, String)> {
    output
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(sha, reference)| (sha.to_string(), reference.to_string()))
        .collect()
}

/// Annotated tags are listed twice; the peeled `^{}` entry is the commit itself.
fn select_commit(refs: &[(String, String)], committish: &Committish) -> Option<String> {
    let find = |reference: &str| {
        refs.iter()
            .find(|(_, listed)| listed == reference)
            .map(|(sha, _)| sha.clone())
    };

    match committish {
        Committish::Default => find("HEAD"),
        Committish::Ref(reference) => find(&format!("refs/tags/{reference}^{{}}"))
            .or_else(|| find(&format!("refs/tags/{reference}")))
            .or_else(|| find(&format!("refs/heads/{reference}")))
            .or_else(|| find(reference))
            .or_else(|| is_short_sha(reference).then(|| reference.clone())),
        Committish::Semver(range) => refs
            .iter()
            .filter_map(|(sha, reference)| {
                let tag = reference.strip_prefix("refs/tags/")?;
                let tag = tag.strip_suffix("^{}").unwrap_or(tag);
                let version = Version::parse(tag.strip_prefix('v').unwrap_or(tag)).ok()?;
                range.satisfies(&version).then_some((version, tag, sha))
            })
            .max_by(|(a, ..), (b, ..)| a.cmp(b))
            .and_then(|(_, tag, _)| select_commit(refs, &Committish::Ref(tag.to_string()))),
    }
}

fn is_full_sha(reference: &str) -> bool {
    reference.len() == FULL_SHA_LENGTH && reference.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_short_sha(reference: &str) -> bool {
    let has_sha_length = (MIN_SHORT_SHA_LENGTH..=FULL_SHA_LENGTH).contains(&reference.len());
    has_sha_length && reference.chars().all(|c| c.is_ascii_hexdigit())
}

fn describe(committish: &Committish) -> String {
    match committish {
        Committish::Default => "HEAD".to_string(),
        Committish::Ref(reference) => reference.clone(),
        Committish::Semver(range) => format!("semver:{range}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LS_REMOTE: &str = "\
1111111111111111111111111111111111111111\tHEAD
1111111111111111111111111111111111111111\trefs/heads/main
2222222222222222222222222222222222222222\trefs/heads/next
3333333333333333333333333333333333333333\trefs/tags/v1.0.0
4444444444444444444444444444444444444444\trefs/tags/v1.2.0
5555555555555555555555555555555555555555\trefs/tags/v1.2.0^{}
6666666666666666666666666666666666666666\trefs/tags/v2.0.0
";

    fn select(committish: &str) -> Option<String> {
        let committish = parse_committish(committish).unwrap();
        select_commit(&parse_ls_remote(LS_REMOTE), &committish)
    }

    #[test]
    fn test_parses_hosted_shorthands() {
        let spec = GitSpec::parse("github:sindresorhus/is-plain-obj#v4.1.0").unwrap();
        assert_eq!(
            spec.clone_url,
            "https://github.com/sindresorhus/is-plain-obj.git"
        );
        assert_eq!(spec.committish, Committish::Ref("v4.1.0".to_string()));
        assert_eq!(
            spec.hosted.unwrap().tarball_url("abc"),
            "https://codeload.github.com/sindresorhus/is-plain-obj/tar.gz/abc"
        );

        let shorthand = GitSpec::parse("expressjs/express").unwrap();
        assert_eq!(
            shorthand.clone_url,
            "https://github.com/expressjs/express.git"
        );
        assert_eq!(shorthand.committish, Committish::Default);

        let gitlab = GitSpec::parse("gitlab:group/project#semver:^1.0.0").unwrap();
        assert_eq!(
            gitlab.hosted.unwrap().tarball_url("abc"),
            "https://gitlab.com/group/project/-/archive/abc/project-abc.tar.gz"
        );
        assert!(matches!(gitlab.committish, Committish::Semver(_)));
    }

    #[test]
    fn test_parses_git_urls() {
        let ssh = GitSpec::parse("git+ssh://git@github.com/user/repo.git#main").unwrap();
        assert_eq!(ssh.clone_url, "ssh://git@github.com/user/repo.git");
        assert_eq!(
            ssh.hosted.unwrap().tarball_url("abc"),
            "https://codeload.github.com/user/repo/tar.gz/abc"
        );

        let scp = GitSpec::parse("git@bitbucket.org:team/repo.git").unwrap();
        assert_eq!(
            scp.hosted.unwrap().tarball_url("abc"),
            "https://bitbucket.org/team/repo/get/abc.tar.gz"
        );

        let generic = GitSpec::parse("git+https://git.example.com/repo.git").unwrap();
        assert_eq!(generic.clone_url, "https://git.example.com/repo.git");
        assert_eq!(generic.hosted, None);

        let local = GitSpec::parse("git+file:///tmp/repo#v1.0.0").unwrap();
        assert_eq!(local.clone_url, "file:///tmp/repo");
    }

    #[test]
    fn test_selects_commits_like_npm() {
        assert_eq!(select(""), Some("1".repeat(40)));
        assert_eq!(select("next"), Some("2".repeat(40)));
        assert_eq!(select("v1.0.0"), Some("3".repeat(40)));
        assert_eq!(select("v1.2.0"), Some("5".repeat(40)));
        assert_eq!(select("semver:^1.0.0"), Some("5".repeat(40)));
        assert_eq!(select("semver:^3.0.0"), None);
        assert_eq!(select("abc1234"), Some("abc1234".to_string()));
        assert_eq!(select("missing-branch"), None);
    }

    #[test]
    fn test_invalid_semver_committish_is_rejected() {
        assert_eq!(GitSpec::parse("user/repo#semver:not a range"), None);
    }
}
