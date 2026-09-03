use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HostKind {
    GitHub,
    GitLab,
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
}

impl Host {
    /// Identifies the provider behind a remote URL, accepting both `https://` and
    /// `git@host:owner/repo.git` (scp-like) forms.
    ///
    /// # Errors
    /// [`HostingError::Malformed`] if the URL cannot be parsed, [`HostingError::Unrecognised`]
    /// if the host is neither GitHub nor GitLab.
    pub fn detect(remote: &str) -> Result<Self, HostingError> {
        let normalised = normalise_scp(remote);
        let url =
            Url::parse(&normalised).map_err(|_| HostingError::Malformed(remote.to_owned()))?;
        let host = url
            .host_str()
            .ok_or_else(|| HostingError::Malformed(remote.to_owned()))?;

        let kind = if host == "github.com" || host.starts_with("github.") {
            HostKind::GitHub
        } else if host == "gitlab.com" || host.starts_with("gitlab.") {
            HostKind::GitLab
        } else {
            return Err(HostingError::Unrecognised(remote.to_owned()));
        };

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

        Ok(Self {
            kind,
            origin: format!("{}://{host}", url.scheme()),
            owner,
            repo,
        })
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
    fn rejects_hosts_we_do_not_support() {
        assert!(matches!(
            Host::detect("https://bitbucket.org/o/r.git"),
            Err(HostingError::Unrecognised(_))
        ));
    }
}
