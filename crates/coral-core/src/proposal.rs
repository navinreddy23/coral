//! Checking out a pull or merge request's tip as a local branch.
//!
//! Both hosts publish every proposal's tip on the repository it was proposed to, fork or not:
//! GitHub as `refs/pull/<n>/head`, GitLab as `refs/merge-requests/<n>/head`. Neither is fetched
//! by a plain `git fetch`, so it is fetched by name into a ref of its own under the remote,
//! and a local branch is made or moved from there.
//! <https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/checking-out-pull-requests-locally>
//! <https://docs.gitlab.com/user/project/merge_requests/merge_request_troubleshooting/#check-out-merge-requests-locally-through-the-head-ref>

use crate::error::CoralError;
use crate::process::{GitCommand, GitRunner, Sink};
use crate::remote::{Progress, parse_progress};
use crate::repo::{Head, RepoLocation};

/// A proposal, by the number people quote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Proposal {
    /// A GitHub pull request.
    Pull(u64),
    /// A GitLab merge request.
    MergeRequest(u64),
}

impl Proposal {
    /// Where the host keeps the proposal's tip.
    #[must_use]
    pub fn head_ref(self) -> String {
        match self {
            Self::Pull(n) => format!("refs/pull/{n}/head"),
            Self::MergeRequest(n) => format!("refs/merge-requests/{n}/head"),
        }
    }

    /// The local branch it is checked out as: `pr/42`, or `mr/42` on GitLab, as each host's
    /// own instructions name it.
    #[must_use]
    pub fn branch(self) -> String {
        match self {
            Self::Pull(n) => format!("pr/{n}"),
            Self::MergeRequest(n) => format!("mr/{n}"),
        }
    }

    /// The remote-tracking ref the tip is fetched into, beside the remote's own branches.
    #[must_use]
    pub fn tracking_ref(self, remote: &str) -> String {
        format!("refs/remotes/{remote}/{}", self.branch())
    }
}

/// What checking a proposal out did to its local branch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckedOut {
    /// The branch did not exist and was made at the proposal's tip.
    Created,
    /// The branch was behind and was fast-forwarded to the tip.
    Updated,
    /// The branch was already at the tip.
    Unchanged,
}

impl RepoLocation {
    /// Fetches a proposal's tip from `remote` and checks it out as its own local branch.
    ///
    /// A local branch that already exists is moved only when that loses nothing: when it is an
    /// ancestor of the tip. Otherwise it holds commits the proposal no longer has — it was
    /// force-pushed, or someone committed to it here — and is left alone.
    ///
    /// # Errors
    /// Git failures, including a proposal the host does not have, and
    /// [`CoralError::Refused`] for a local branch that would lose commits.
    pub async fn checkout_proposal<F>(
        &self,
        runner: &GitRunner,
        remote: &str,
        proposal: Proposal,
        mut on_progress: F,
    ) -> Result<CheckedOut, CoralError>
    where
        F: FnMut(Progress),
    {
        let tracking = proposal.tracking_ref(remote);
        // `+`: the tip a proposal had last time is no reason to refuse the one it has now.
        let fetch = GitCommand::network("fetch", self.display_path())
            .args(["fetch", "--progress", "--no-tags"])
            .arg(remote)
            .arg(format!("+{}:{tracking}", proposal.head_ref()));
        runner
            .stream_err(fetch, |line| {
                if let Some(p) = parse_progress(line) {
                    on_progress(p);
                }
                Ok(Sink::Continue)
            })
            .await?;
        let tip = self.rev_parse(runner, &tracking).await?;
        let branch = proposal.branch();
        let local = format!("refs/heads/{branch}");

        let Ok(have) = self.rev_parse(runner, &local).await else {
            runner
                .output(
                    GitCommand::write("switch", self.display_path())
                        .args(["switch", "--quiet", "--no-track", "-c"])
                        .arg(&branch)
                        .arg(&tip),
                )
                .await?;
            return Ok(CheckedOut::Created);
        };
        let on_it = matches!(self.head(runner).await?, Head::Branch { name } if name == branch);
        if have == tip {
            if !on_it {
                self.checkout(runner, &branch).await?;
            }
            return Ok(CheckedOut::Unchanged);
        }
        if !self.is_ancestor(runner, &have, &tip).await? {
            return Err(CoralError::Refused {
                label: "check out the request",
                detail: format!(
                    "{branch} has commits the request no longer has: it was force-pushed, or \
                     something was committed to {branch} here. Rename or delete {branch} to \
                     check the request out afresh."
                ),
            });
        }
        if on_it {
            runner
                .output(
                    GitCommand::write("merge", self.display_path())
                        .args(["merge", "--quiet", "--ff-only"])
                        .arg(&tip),
                )
                .await?;
        } else {
            runner
                .output(
                    GitCommand::write("branch", self.display_path())
                        .args(["branch", "--force"])
                        .arg(&branch)
                        .arg(&tip),
                )
                .await?;
            self.checkout(runner, &branch).await?;
        }
        Ok(CheckedOut::Updated)
    }
}

#[cfg(test)]
mod tests {
    use super::Proposal;

    #[test]
    fn names_each_host_s_refs_its_own_way() {
        assert_eq!(Proposal::Pull(42).head_ref(), "refs/pull/42/head");
        assert_eq!(Proposal::Pull(42).branch(), "pr/42");
        assert_eq!(
            Proposal::Pull(42).tracking_ref("origin"),
            "refs/remotes/origin/pr/42"
        );
        assert_eq!(
            Proposal::MergeRequest(7).head_ref(),
            "refs/merge-requests/7/head"
        );
        assert_eq!(Proposal::MergeRequest(7).branch(), "mr/7");
    }
}
