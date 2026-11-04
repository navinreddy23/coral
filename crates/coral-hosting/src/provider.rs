use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum HostKind {
    // Named rather than derived: snake_case would spell these `git_hub` and `git_lab`.
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "gitlab")]
    GitLab,
    /// Gitea and Forgejo, which share one API: Codeberg, and a great many company instances.
    #[serde(rename = "gitea")]
    Gitea,
    /// Bitbucket Cloud. Bitbucket Data Center is a different API altogether.
    #[serde(rename = "bitbucket")]
    Bitbucket,
    /// Azure DevOps Services, and the older `visualstudio.com` addresses of it.
    #[serde(rename = "azure")]
    Azure,
}

impl HostKind {
    /// What a request number is written after: GitLab keeps `#` for issues and quotes a
    /// merge request as `!42`, and Azure DevOps writes a pull request the same way.
    #[must_use]
    pub fn sigil(self) -> char {
        match self {
            Self::GitHub | Self::Gitea | Self::Bitbucket => '#',
            Self::GitLab | Self::Azure => '!',
        }
    }

    /// Whether a host of this kind can be told from its name. Only Gitea cannot: it is
    /// self-hosted under any name, so an instance has to be declared once.
    #[must_use]
    pub fn by_name(hostname: &str) -> Option<Self> {
        let starts = |prefix: &str| hostname.starts_with(prefix);
        Some(match hostname {
            "github.com" => Self::GitHub,
            "gitlab.com" => Self::GitLab,
            "bitbucket.org" => Self::Bitbucket,
            "dev.azure.com" | "ssh.dev.azure.com" => Self::Azure,
            "codeberg.org" | "gitea.com" => Self::Gitea,
            h if h.ends_with(".visualstudio.com") => Self::Azure,
            _ if starts("github.") => Self::GitHub,
            _ if starts("gitlab.") => Self::GitLab,
            _ if starts("gitea.") || starts("forgejo.") => Self::Gitea,
            _ => return None,
        })
    }
}

/// A host resolved from a remote URL. Self-hosted instances keep their own origin, which is
/// why this carries a `Url` rather than just a kind.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Host {
    pub kind: HostKind,
    pub origin: String,
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, thiserror::Error)]
pub enum HostingError {
    #[error("remote URL is not a recognised hosting provider: {0}")]
    Unrecognised(String),
    #[error("remote URL could not be parsed: {0}")]
    Malformed(String),
    #[error("could not reach the host: {0}")]
    Transport(String),
    #[error("the host refused the request ({status}): {detail}")]
    Api { status: u16, detail: String },
    #[error("no token is stored for this host")]
    NoToken,
    #[error("{0}")]
    Unsupported(String),
}

impl HostingError {
    /// Whether the host itself refused, rather than not answering at all.
    ///
    /// What a sign-in needs to know before it keeps a token: a refusal is the host saying the
    /// secret is no good, and a network that is down says nothing about it.
    #[must_use]
    pub const fn is_refusal(&self) -> bool {
        matches!(self, Self::Api { .. })
    }
}

impl Host {
    /// Identifies the provider behind a remote URL, accepting both `https://` and
    /// `git@host:owner/repo.git` (scp-like) forms.
    ///
    /// # Errors
    /// [`HostingError::Malformed`] if the URL cannot be parsed, [`HostingError::Unrecognised`]
    /// if the host is neither GitHub nor GitLab.
    pub fn detect(remote: &str) -> Result<Self, HostingError> {
        Self::detect_with(remote, crate::declared::kind_of)
    }

    /// As [`Host::detect`], asking `declared` about a host whose name gives nothing away.
    ///
    /// # Errors
    /// As [`Host::detect`].
    pub fn detect_with(
        remote: &str,
        declared: impl Fn(&str) -> Option<HostKind>,
    ) -> Result<Self, HostingError> {
        let normalised = normalise_scp(remote);
        let url =
            Url::parse(&normalised).map_err(|_| HostingError::Malformed(remote.to_owned()))?;
        let host = url
            .host_str()
            .ok_or_else(|| HostingError::Malformed(remote.to_owned()))?;

        let kind = HostKind::by_name(host)
            .or_else(|| declared(host))
            .ok_or_else(|| HostingError::Unrecognised(remote.to_owned()))?;
        if kind == HostKind::Azure {
            return azure_from(remote, &url, host);
        }

        let mut segments: Vec<&str> = url.path().trim_matches('/').split('/').collect();
        let repo = segments
            .pop()
            .unwrap_or_default()
            .trim_end_matches(".git")
            .to_owned();
        let owner = segments.join("/");
        if owner.is_empty() || repo.is_empty() {
            return Err(HostingError::Malformed(remote.to_owned()));
        }

        // The web and API origin, which is not the remote's scheme: a repository cloned over
        // ssh has no API at `ssh://`, and `git://` has none either. Only an explicit http or
        // https remote says anything about how the instance is served.
        let scheme = match url.scheme() {
            "http" => "http",
            _ => "https",
        };
        // The port belongs to the origin for the same reason the scheme does, and only then: a
        // remote cloned over ssh on 2222 says nothing about where the instance serves its API,
        // while an http remote on 8929 says exactly that. `Url::port` is already None for a
        // scheme's own port, so 443 never appears.
        let port = match (url.scheme(), url.port()) {
            ("http" | "https", Some(port)) => format!(":{port}"),
            _ => String::new(),
        };

        Ok(Self {
            kind,
            origin: format!("{scheme}://{host}{port}"),
            owner,
            repo,
        })
    }

    /// Where this commit is served on the web.
    ///
    /// GitLab puts a `-` before the noun to keep project paths and page routes apart, since a
    /// subgroup could otherwise be called `commit`; GitHub does not.
    #[must_use]
    pub fn commit_url(&self, oid: &str) -> String {
        let Self {
            origin,
            owner,
            repo,
            ..
        } = self;
        match self.kind {
            HostKind::GitHub | HostKind::Gitea => format!("{origin}/{owner}/{repo}/commit/{oid}"),
            HostKind::GitLab => format!("{origin}/{owner}/{repo}/-/commit/{oid}"),
            HostKind::Bitbucket => format!("{origin}/{owner}/{repo}/commits/{oid}"),
            HostKind::Azure => format!("{origin}/{owner}/_git/{repo}/commit/{oid}"),
        }
    }
}

/// An Azure DevOps remote, whose path is not `owner/repo`.
///
/// Three spellings: `https://dev.azure.com/org/project/_git/repo`, the ssh form
/// `ssh.dev.azure.com:v3/org/project/repo`, and the older `https://org.visualstudio.com/
/// project/_git/repo`. The owner is everything the API path needs before the repository —
/// `org/project`, or `project` where the organisation is the host — so the API and the web
/// page are both `{origin}/{owner}/…`.
fn azure_from(remote: &str, url: &Url, host: &str) -> Result<Host, HostingError> {
    let malformed = || HostingError::Malformed(remote.to_owned());
    let segments: Vec<&str> = url
        .path()
        .trim_matches('/')
        .split('/')
        .filter(|s| !s.is_empty())
        .collect();
    let (origin, owner, repo) = match (host, segments.as_slice()) {
        ("ssh.dev.azure.com", ["v3", org, project, repo])
        | ("dev.azure.com", [org, project, "_git", repo]) => (
            "https://dev.azure.com".to_owned(),
            format!("{org}/{project}"),
            *repo,
        ),
        (h, [project, "_git", repo]) if h.ends_with(".visualstudio.com") => {
            (format!("https://{h}"), (*project).to_owned(), *repo)
        }
        // A server declared as Azure DevOps — its own installation, or one standing in for it —
        // addressed the way dev.azure.com is, from the remote's own scheme and port.
        (h, [org, project, "_git", repo]) => {
            let scheme = if url.scheme() == "http" {
                "http"
            } else {
                "https"
            };
            let port = match (url.scheme(), url.port()) {
                ("http" | "https", Some(port)) => format!(":{port}"),
                _ => String::new(),
            };
            (
                format!("{scheme}://{h}{port}"),
                format!("{org}/{project}"),
                *repo,
            )
        }
        _ => return Err(malformed()),
    };
    Ok(Host {
        kind: HostKind::Azure,
        origin,
        owner,
        repo: repo.trim_end_matches(".git").to_owned(),
    })
}

impl Host {
    /// The server a remote names, lower-cased, whatever its kind: what a declaration is filed
    /// under for one that could not be placed.
    #[must_use]
    pub fn hostname_of(remote: &str) -> Option<String> {
        let url = Url::parse(&normalise_scp(remote)).ok()?;
        url.host_str().map(str::to_ascii_lowercase)
    }
}

/// Rewrites `git@host:owner/repo.git` into a URL the parser accepts. Leaves anything with an
/// explicit scheme untouched.
fn normalise_scp(remote: &str) -> String {
    if remote.contains("://") {
        return remote.to_owned();
    }
    match remote.split_once(':') {
        Some((head, path)) if !path.starts_with("//") => {
            let host = head.rsplit('@').next().unwrap_or(head);
            format!("ssh://{host}/{path}")
        }
        _ => remote.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_github_and_gitlab_over_https_and_ssh() {
        let h = Host::detect("https://github.com/torvalds/linux.git").unwrap();
        assert_eq!(h.kind, HostKind::GitHub);
        assert_eq!(h.owner, "torvalds");
        assert_eq!(h.repo, "linux");

        let h = Host::detect("git@gitlab.com:group/sub/proj.git").unwrap();
        assert_eq!(h.kind, HostKind::GitLab);
        assert_eq!(
            h.owner, "group/sub",
            "GitLab subgroups are part of the owner path"
        );
        assert_eq!(h.repo, "proj");
    }

    #[test]
    fn the_origin_is_where_the_api_is_served_not_how_the_remote_was_cloned() {
        // Cloning over ssh says nothing about the API, which is served over https either way;
        // an `ssh://` origin would build request URLs nothing answers.
        for remote in [
            "git@gitlab.com:group/proj.git",
            "ssh://git@gitlab.com/group/proj.git",
            "https://gitlab.com/group/proj.git",
        ] {
            assert_eq!(
                Host::detect(remote).unwrap().origin,
                "https://gitlab.com",
                "{remote}"
            );
        }
        // An instance explicitly served over plain http keeps it.
        assert_eq!(
            Host::detect("http://gitlab.internal/t/a.git")
                .unwrap()
                .origin,
            "http://gitlab.internal"
        );
    }

    #[test]
    fn an_instance_on_its_own_port_keeps_it() {
        // Dropped, every request went to 443 on a host that serves nothing there, and the
        // panel read it as a repository with no proposals rather than as a wrong address.
        assert_eq!(
            Host::detect("https://gitlab.internal:8929/t/a.git")
                .unwrap()
                .origin,
            "https://gitlab.internal:8929"
        );
        assert_eq!(
            Host::detect("http://github.internal:8080/t/a.git")
                .unwrap()
                .commit_url("abc"),
            "http://github.internal:8080/t/a/commit/abc"
        );
        // The scheme's own port is not part of an origin anybody writes down.
        assert_eq!(
            Host::detect("https://gitlab.internal:443/t/a.git")
                .unwrap()
                .origin,
            "https://gitlab.internal"
        );
        // An ssh port is about ssh. The API is served where it is served.
        assert_eq!(
            Host::detect("ssh://git@gitlab.internal:2222/t/a.git")
                .unwrap()
                .origin,
            "https://gitlab.internal"
        );
    }

    #[test]
    fn detects_self_hosted_instances_by_hostname_prefix() {
        for url in [
            "https://gitlab.example.com/team/app.git",
            "https://gitlab.internal/t/a.git",
        ] {
            assert_eq!(Host::detect(url).unwrap().kind, HostKind::GitLab, "{url}");
        }
        assert_eq!(
            Host::detect("https://github.acme.corp/team/app.git")
                .unwrap()
                .kind,
            HostKind::GitHub
        );
    }

    /// An instance on an unrelated domain cannot be identified from the URL alone; M8 adds an
    /// explicit per-remote host setting for these.
    #[test]
    fn does_not_guess_at_instances_on_unrelated_domains() {
        assert!(matches!(
            Host::detect("https://git.example.com/team/app.git"),
            Err(HostingError::Unrecognised(_))
        ));
    }

    #[test]
    fn builds_the_web_address_of_a_commit_for_each_host() {
        let oid = "0123456789abcdef0123456789abcdef01234567";
        assert_eq!(
            Host::detect("git@github.com:torvalds/linux.git")
                .unwrap()
                .commit_url(oid),
            format!("https://github.com/torvalds/linux/commit/{oid}")
        );
        // The `-` segment, and a subgroup that stays part of the project path.
        assert_eq!(
            Host::detect("https://gitlab.com/group/sub/proj.git")
                .unwrap()
                .commit_url(oid),
            format!("https://gitlab.com/group/sub/proj/-/commit/{oid}")
        );
    }

    #[test]
    fn rejects_hosts_it_cannot_place() {
        assert!(matches!(
            Host::detect_with("https://git.example.invalid/o/r.git", |_| None),
            Err(HostingError::Unrecognised(_))
        ));
    }

    #[test]
    fn detects_gitea_bitbucket_and_azure() {
        let h = Host::detect_with("https://codeberg.org/forgejo/forgejo.git", |_| None).unwrap();
        assert_eq!(
            (h.kind, h.owner.as_str(), h.repo.as_str()),
            (HostKind::Gitea, "forgejo", "forgejo")
        );

        let h = Host::detect_with("git@bitbucket.org:team/thing.git", |_| None).unwrap();
        assert_eq!(
            (h.kind, h.owner.as_str(), h.repo.as_str()),
            (HostKind::Bitbucket, "team", "thing")
        );
        assert_eq!(h.origin, "https://bitbucket.org");

        for remote in [
            "https://dev.azure.com/acme/Platform/_git/coral",
            "https://acme@dev.azure.com/acme/Platform/_git/coral",
            "git@ssh.dev.azure.com:v3/acme/Platform/coral",
        ] {
            let h = Host::detect_with(remote, |_| None).unwrap();
            assert_eq!(h.kind, HostKind::Azure, "{remote}");
            assert_eq!(h.origin, "https://dev.azure.com", "{remote}");
            assert_eq!(h.owner, "acme/Platform", "{remote}");
            assert_eq!(h.repo, "coral", "{remote}");
        }
        let h = Host::detect_with("https://acme.visualstudio.com/Platform/_git/coral", |_| {
            None
        })
        .unwrap();
        assert_eq!(
            (h.origin.as_str(), h.owner.as_str()),
            ("https://acme.visualstudio.com", "Platform")
        );
        assert!(Host::detect_with("https://dev.azure.com/acme/coral.git", |_| None).is_err());
    }

    #[test]
    fn a_declared_host_is_placed_by_its_declaration() {
        let declared = |h: &str| (h == "git.example.invalid").then_some(HostKind::Gitea);
        let h = Host::detect_with("git@git.example.invalid:team/thing.git", declared).unwrap();
        assert_eq!(h.kind, HostKind::Gitea);
        assert_eq!(h.origin, "https://git.example.invalid");
        // Any kind can be declared, which is how a server standing in for one is reached.
        let declared = |_: &str| Some(HostKind::Azure);
        let h = Host::detect_with("http://azure.localhost:8080/acme/P/_git/r", declared).unwrap();
        assert_eq!(
            (h.origin.as_str(), h.owner.as_str()),
            ("http://azure.localhost:8080", "acme/P")
        );
        // A name that says what it is wins over a declaration.
        let h = Host::detect_with("https://github.com/o/r", |_| Some(HostKind::Gitea)).unwrap();
        assert_eq!(h.kind, HostKind::GitHub);
    }

    #[test]
    fn each_host_has_its_own_commit_page() {
        let at = |remote: &str| {
            Host::detect_with(remote, |_| None)
                .unwrap()
                .commit_url("abc")
        };
        assert_eq!(
            at("https://codeberg.org/o/r"),
            "https://codeberg.org/o/r/commit/abc"
        );
        assert_eq!(
            at("https://bitbucket.org/o/r"),
            "https://bitbucket.org/o/r/commits/abc"
        );
        assert_eq!(
            at("https://dev.azure.com/acme/P/_git/r"),
            "https://dev.azure.com/acme/P/_git/r/commit/abc"
        );
    }

    #[test]
    fn a_merge_request_is_quoted_with_a_bang() {
        assert_eq!(HostKind::GitHub.sigil(), '#');
        assert_eq!(HostKind::GitLab.sigil(), '!');
        assert_eq!(HostKind::Azure.sigil(), '!');
        assert_eq!(HostKind::Gitea.sigil(), '#');
    }
}
