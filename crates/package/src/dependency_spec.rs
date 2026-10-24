use std::fmt::Display;

use contract::SpecError;
use node_semver::{Range, Version};

const NPM_ALIAS_PREFIX: &str = "npm:";
const GIT_PREFIXES: [&str; 9] = [
    "git+",
    "git:",
    "git://",
    "ssh:",
    "ssh://",
    "github:",
    "gitlab:",
    "bitbucket:",
    "gist:",
];
const FILE_PATH_PREFIXES: [&str; 4] = ["./", "../", "/", "~/"];

/// What a `package.json` dependency (or a `craft add` literal) asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum DependencySpec {
    Registry {
        name: String,
        requirement: VersionRequirement,
    },
    /// `"alias": "npm:name@requirement"` — installed under `alias`, fetched as `name`.
    Alias {
        alias: String,
        name: String,
        requirement: VersionRequirement,
    },
    Git {
        name: String,
        url: String,
    },
    Tarball {
        name: String,
        url: String,
    },
    File {
        name: String,
        path: String,
    },
    Link {
        name: String,
        path: String,
    },
    Workspace {
        name: String,
        spec: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum VersionRequirement {
    Exact(Version),
    Range(Range),
    Tag(String),
}

impl DependencySpec {
    /// A missing spec means the `latest` dist-tag. When `spec` is missing and `name` is itself
    /// a git URL, tarball URL or path (as in `craft add github:user/repo`), `name` is the spec.
    pub fn parse(name: &str, spec: Option<&str>) -> Result<Self, SpecError> {
        let Some(spec) = spec else {
            return match classify_location(name, name) {
                Some(location) => Ok(location),
                None => Ok(Self::Registry {
                    name: name.to_string(),
                    requirement: VersionRequirement::Tag("latest".to_string()),
                }),
            };
        };
        let spec = spec.trim();

        if let Some(target) = spec.strip_prefix(NPM_ALIAS_PREFIX) {
            let (target_name, target_spec) = split_name_and_spec(target);
            let requirement = match target_spec {
                Some(target_spec) => VersionRequirement::parse(target_spec)
                    .ok_or_else(|| invalid(name, Some(spec)))?,
                None => VersionRequirement::Tag("latest".to_string()),
            };
            return Ok(Self::Alias {
                alias: name.to_string(),
                name: target_name.to_string(),
                requirement,
            });
        }

        if let Some(location) = classify_location(name, spec) {
            return Ok(location);
        }

        let requirement =
            VersionRequirement::parse(spec).ok_or_else(|| invalid(name, Some(spec)))?;
        Ok(Self::Registry {
            name: name.to_string(),
            requirement,
        })
    }

    /// The registry package to fetch and the version it must match, for registry and alias specs.
    pub fn registry_target(&self) -> Option<(&str, &VersionRequirement)> {
        match self {
            Self::Registry { name, requirement }
            | Self::Alias {
                name, requirement, ..
            } => Some((name, requirement)),
            _ => None,
        }
    }

    /// How a location dependency is written to package.json: paths as `file:<path>` or
    /// `link:<path>` without a leading `./`, URLs as given. `None` for registry specs.
    pub fn location_spec(&self) -> Option<String> {
        match self {
            Self::File { path, .. } => Some(format!("file:{}", path.trim_start_matches("./"))),
            Self::Link { path, .. } => Some(format!("link:{path}")),
            Self::Tarball { url, .. } | Self::Git { url, .. } => Some(url.clone()),
            Self::Registry { .. } | Self::Alias { .. } | Self::Workspace { .. } => None,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Registry { .. } => "registry",
            Self::Alias { .. } => "npm alias",
            Self::Git { .. } => "git",
            Self::Tarball { .. } => "tarball URL",
            Self::File { .. } => "file:",
            Self::Link { .. } => "link:",
            Self::Workspace { .. } => "workspace:",
        }
    }
}

impl VersionRequirement {
    fn parse(spec: &str) -> Option<Self> {
        let is_any_version = spec.is_empty() || spec == "*";
        if is_any_version {
            return Some(Self::Range(Range::any()));
        }

        if let Ok(version) = Version::parse(spec) {
            return Some(Self::Exact(version));
        }

        if let Ok(range) = Range::parse(spec) {
            return Some(Self::Range(range));
        }

        is_valid_tag(spec).then(|| Self::Tag(spec.to_string()))
    }
}

impl Display for VersionRequirement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exact(version) => write!(f, "{version}"),
            Self::Range(range) => write!(f, "{range}"),
            Self::Tag(tag) => write!(f, "{tag}"),
        }
    }
}

/// Splits `name@spec`, keeping the leading `@` of a scoped name: `@types/node@^20` is
/// `("@types/node", Some("^20"))`.
pub(crate) fn split_name_and_spec(literal: &str) -> (&str, Option<&str>) {
    let separator = literal
        .char_indices()
        .skip(1)
        .find(|(_, character)| *character == '@')
        .map(|(index, _)| index);

    match separator {
        Some(index) => (&literal[..index], Some(&literal[index + 1..])),
        None => (literal, None),
    }
}

/// Whether `literal` is a git URL, tarball URL or path rather than `name[@spec]`.
pub(crate) fn is_location(literal: &str) -> bool {
    classify_location(literal, literal).is_some()
}

fn classify_location(name: &str, spec: &str) -> Option<DependencySpec> {
    let name = name.to_string();

    if let Some(workspace_spec) = spec.strip_prefix("workspace:") {
        return Some(DependencySpec::Workspace {
            name,
            spec: workspace_spec.to_string(),
        });
    }

    if let Some(path) = spec.strip_prefix("link:") {
        return Some(DependencySpec::Link {
            name,
            path: path.to_string(),
        });
    }

    if let Some(path) = spec.strip_prefix("file:") {
        return Some(DependencySpec::File {
            name,
            path: path.to_string(),
        });
    }

    if FILE_PATH_PREFIXES
        .iter()
        .any(|prefix| spec.starts_with(prefix))
    {
        return Some(DependencySpec::File {
            name,
            path: spec.to_string(),
        });
    }

    if is_git_url(spec) {
        return Some(DependencySpec::Git {
            name,
            url: spec.to_string(),
        });
    }

    let is_http_url = spec.starts_with("http://") || spec.starts_with("https://");
    is_http_url.then(|| DependencySpec::Tarball {
        name,
        url: spec.to_string(),
    })
}

fn is_git_url(spec: &str) -> bool {
    let has_git_prefix = GIT_PREFIXES.iter().any(|prefix| spec.starts_with(prefix));
    let is_http_url = spec.starts_with("http://") || spec.starts_with("https://");
    let points_at_git_repository = spec.ends_with(".git") || spec.contains(".git#");
    let is_git_over_http = is_http_url && points_at_git_repository;

    has_git_prefix || is_git_over_http || is_github_shorthand(spec)
}

/// `owner/repo` or `owner/repo#ref`, npm's shorthand for a GitHub repository.
fn is_github_shorthand(spec: &str) -> bool {
    let repository = spec.split('#').next().unwrap_or(spec);
    let Some((owner, repo)) = repository.split_once('/') else {
        return false;
    };

    let has_valid_segments = !owner.is_empty() && !repo.is_empty() && !repo.contains('/');
    let has_url_characters =
        repository.contains(':') || repository.chars().any(char::is_whitespace);

    has_valid_segments && !has_url_characters && !owner.starts_with('@')
}

/// npm accepts a dist-tag only if it survives `encodeURIComponent` unchanged.
fn is_valid_tag(spec: &str) -> bool {
    spec.chars()
        .all(|character| character.is_ascii_alphanumeric() || "-_.!~*'()".contains(character))
}

fn invalid(name: &str, spec: Option<&str>) -> SpecError {
    let dependency = match spec {
        Some(spec) => format!("{name}@{spec}"),
        None => name.to_string(),
    };
    SpecError::Invalid { dependency }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_spec_means_latest() {
        let spec = DependencySpec::parse("react", None).unwrap();
        assert_eq!(
            spec,
            registry("react", VersionRequirement::Tag("latest".to_string()))
        );
    }

    #[test]
    fn test_exact_version() {
        let spec = DependencySpec::parse("react", Some("17.0.2")).unwrap();
        let version = Version::parse("17.0.2").unwrap();
        assert_eq!(spec, registry("react", VersionRequirement::Exact(version)));
    }

    #[test]
    fn test_ranges() {
        for range in ["^18.0.0", "~1.2.3", ">=1 <2", "1.x || >=3", "=1.2.3", "18"] {
            let spec = DependencySpec::parse("react", Some(range)).unwrap();
            let requirement = spec.registry_target().map(|(_, requirement)| requirement);
            assert!(
                matches!(requirement, Some(VersionRequirement::Range(_))),
                "{range} should be a range, got {spec:?}"
            );
        }
    }

    #[test]
    fn test_empty_and_star_mean_any_version() {
        for any in ["", "*", "  "] {
            let spec = DependencySpec::parse("react", Some(any)).unwrap();
            assert_eq!(
                spec,
                registry("react", VersionRequirement::Range(Range::any()))
            );
        }
    }

    #[test]
    fn test_dist_tags() {
        for tag in ["latest", "next", "beta", "canary", "rc-1"] {
            let spec = DependencySpec::parse("next", Some(tag)).unwrap();
            assert_eq!(
                spec,
                registry("next", VersionRequirement::Tag(tag.to_string()))
            );
        }
    }

    #[test]
    fn test_invalid_spec_is_an_error() {
        let error = DependencySpec::parse("react", Some("not a version")).unwrap_err();
        assert_eq!(
            error.to_string(),
            "invalid dependency spec: react@not a version"
        );
    }

    #[test]
    fn test_npm_alias() {
        let spec = DependencySpec::parse("wrap-ansi-cjs", Some("npm:wrap-ansi@^7.0.0")).unwrap();
        let DependencySpec::Alias {
            alias,
            name,
            requirement,
        } = spec
        else {
            panic!("expected an alias");
        };
        assert_eq!(alias, "wrap-ansi-cjs");
        assert_eq!(name, "wrap-ansi");
        assert!(matches!(requirement, VersionRequirement::Range(_)));
    }

    #[test]
    fn test_npm_alias_to_scoped_package_without_version() {
        let spec = DependencySpec::parse("query", Some("npm:@tanstack/query-core")).unwrap();
        assert_eq!(
            spec.registry_target(),
            Some((
                "@tanstack/query-core",
                &VersionRequirement::Tag("latest".to_string())
            ))
        );
    }

    #[test]
    fn test_locations() {
        let cases = [
            ("git+https://github.com/user/repo.git", "git"),
            ("git+ssh://git@github.com:user/repo.git", "git"),
            ("git://github.com/user/repo.git", "git"),
            ("github:user/repo", "git"),
            ("gitlab:group/project", "git"),
            ("user/repo", "git"),
            ("user/repo#v1.0.0", "git"),
            ("https://github.com/user/repo.git", "git"),
            (
                "https://registry.npmjs.org/ms/-/ms-2.1.3.tgz",
                "tarball URL",
            ),
            ("file:../local", "file:"),
            ("./vendor/lib", "file:"),
            ("link:../linked", "link:"),
            ("workspace:*", "workspace:"),
        ];

        for (location, kind) in cases {
            let spec = DependencySpec::parse("pkg", Some(location)).unwrap();
            assert_eq!(spec.kind(), kind, "{location}");
            assert_eq!(spec.registry_target(), None, "{location}");
        }
    }

    #[test]
    fn test_location_as_name_without_spec() {
        let spec = DependencySpec::parse("github:user/repo", None).unwrap();
        assert_eq!(spec.kind(), "git");
    }

    #[test]
    fn test_scoped_name_is_not_a_github_shorthand() {
        assert!(!is_location("@types/node"));
        assert!(!is_github_shorthand("@types/node"));
    }

    #[test]
    fn test_split_name_and_spec() {
        assert_eq!(split_name_and_spec("react"), ("react", None));
        assert_eq!(split_name_and_spec("react@^18"), ("react", Some("^18")));
        assert_eq!(split_name_and_spec("@types/node"), ("@types/node", None));
        assert_eq!(
            split_name_and_spec("@types/node@20"),
            ("@types/node", Some("20"))
        );
        assert_eq!(
            split_name_and_spec("sw@npm:string-width@^4"),
            ("sw", Some("npm:string-width@^4"))
        );
    }

    fn registry(name: &str, requirement: VersionRequirement) -> DependencySpec {
        DependencySpec::Registry {
            name: name.to_string(),
            requirement,
        }
    }

    #[test]
    fn test_location_spec_is_normalized_for_saving() {
        let location = |name: &str, spec: Option<&str>| {
            DependencySpec::parse(name, spec).unwrap().location_spec()
        };
        assert_eq!(location("./libs/x", None), Some("file:libs/x".to_string()));
        assert_eq!(
            location("x", Some("file:../shared")),
            Some("file:../shared".to_string())
        );
        assert_eq!(
            location("x", Some("link:../x")),
            Some("link:../x".to_string())
        );
        assert_eq!(
            location("x", Some("https://example.com/x.tgz")),
            Some("https://example.com/x.tgz".to_string())
        );
        assert_eq!(location("x", Some("^1.0.0")), None);
    }
}
