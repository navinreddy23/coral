//! Request shapes and response parsing for both providers.
//!
//! The bodies are trimmed copies of what the documented endpoints return, kept to the fields
//! Coral reads plus enough neighbours to prove the parser ignores the rest. No network is
//! involved: what would break in the field is the mapping, not the HTTP.

use coral_hosting::pull_request::{NewPullRequest, PrState};
use coral_hosting::{Host, HostKind, github, gitlab};

fn github_host() -> Host {
    Host::detect("https://github.com/torvalds/linux.git").unwrap()
}

fn enterprise_host() -> Host {
    Host::detect("https://github.acme.corp/team/app.git").unwrap()
}

fn gitlab_host() -> Host {
    Host::detect("git@gitlab.com:group/sub/proj.git").unwrap()
}

#[test]
fn github_com_and_an_enterprise_instance_use_different_api_roots() {
    // github.com serves its API from another origin; an instance serves it from /api/v3 on
    // itself. Getting this wrong is a 404 against the HTML site, which reads as "no pull
    // requests" rather than as a misconfiguration.
    assert_eq!(github::api_base(&github_host()), "https://api.github.com");
    assert_eq!(
        github::api_base(&enterprise_host()),
        "https://github.acme.corp/api/v3"
    );
}

#[test]
fn a_gitlab_project_is_addressed_by_its_encoded_full_path() {
    // The slashes between a group, its subgroups and the project are part of the identifier;
    // left raw they address a different, usually absent, endpoint.
    assert_eq!(gitlab::project_id(&gitlab_host()), "group%2Fsub%2Fproj");
    assert!(
        gitlab::merge_requests_url(&gitlab_host(), 50)
            .starts_with("https://gitlab.com/api/v4/projects/group%2Fsub%2Fproj/merge_requests?")
    );
}

#[test]
fn github_distinguishes_merged_from_closed_by_the_merge_timestamp() {
    // GitHub reports both as "closed". Calling a merged branch closed is the difference
    // between "landed" and "rejected".
    let body = br#"[
      {"number":1,"title":"Landed","state":"closed","merged_at":"2026-01-02T03:04:05Z",
       "user":{"login":"alice"},"head":{"ref":"feature/a"},"base":{"ref":"main"},
       "html_url":"https://github.com/o/r/pull/1","updated_at":"2026-01-02T03:04:05Z"},
      {"number":2,"title":"Rejected","state":"closed","merged_at":null,
       "user":{"login":"bob"},"head":{"ref":"feature/b"},"base":{"ref":"main"},
       "html_url":"https://github.com/o/r/pull/2","updated_at":"2026-01-03T00:00:00Z"}
    ]"#;
    let prs = github::parse_pulls(body).unwrap();
    assert_eq!(prs[0].state, PrState::Merged);
    assert_eq!(prs[1].state, PrState::Closed);
    assert_eq!(prs[0].author, "alice");
    assert_eq!(prs[0].source_branch, "feature/a");
    assert_eq!(prs[0].target_branch, "main");
}

#[test]
fn github_reports_a_draft_separately_from_open() {
    let body = br#"[
      {"number":3,"title":"Not ready","state":"open","draft":true,"merged_at":null,
       "user":{"login":"carol"},"head":{"ref":"wip"},"base":{"ref":"main"},
       "html_url":"https://github.com/o/r/pull/3","updated_at":"2026-01-04T00:00:00Z"},
      {"number":4,"title":"Ready","state":"open","draft":false,"merged_at":null,
       "user":{"login":"dan"},"head":{"ref":"done"},"base":{"ref":"main"},
       "html_url":"https://github.com/o/r/pull/4","updated_at":"2026-01-05T00:00:00Z"}
    ]"#;
    let prs = github::parse_pulls(body).unwrap();
    assert_eq!(prs[0].state, PrState::Draft);
    assert_eq!(prs[1].state, PrState::Open);
}

#[test]
fn a_deleted_github_account_leaves_a_pull_request_readable() {
    // `user` is null for a ghosted account; a pull request that fails to parse would take the
    // whole list with it.
    let body = br#"[{"number":5,"title":"Orphan","state":"open","merged_at":null,"user":null,
      "head":{"ref":"x"},"base":{"ref":"main"},
      "html_url":"https://github.com/o/r/pull/5","updated_at":"2026-01-06T00:00:00Z"}]"#;
    let prs = github::parse_pulls(body).unwrap();
    assert_eq!(prs[0].author, "(unknown)");
}

#[test]
fn gitlab_numbers_are_the_per_project_ones() {
    // `id` is instance-wide and means nothing to anyone reading a branch name; `iid` is the
    // number people quote.
    let body = br#"[{"id":90210,"iid":7,"title":"Add a thing","state":"opened",
      "author":{"username":"erin"},"source_branch":"feature/x","target_branch":"main",
      "web_url":"https://gitlab.com/g/p/-/merge_requests/7",
      "updated_at":"2026-01-07T00:00:00Z"}]"#;
    let prs = gitlab::parse_merge_requests(body).unwrap();
    assert_eq!(prs[0].number, 7);
    assert_eq!(prs[0].state, PrState::Open);
}

#[test]
fn gitlab_drafts_are_recognised_however_they_are_marked() {
    // The flag has been `work_in_progress`, then `draft`, and the title prefix has been both
    // `WIP:` and `Draft:`; an instance can be old enough to send any of them.
    let body = br#"[
      {"iid":1,"title":"a","state":"opened","draft":true,
       "source_branch":"a","target_branch":"main","web_url":"u","updated_at":"t"},
      {"iid":2,"title":"b","state":"opened","work_in_progress":true,
       "source_branch":"b","target_branch":"main","web_url":"u","updated_at":"t"},
      {"iid":3,"title":"Draft: c","state":"opened",
       "source_branch":"c","target_branch":"main","web_url":"u","updated_at":"t"},
      {"iid":4,"title":"WIP: d","state":"opened",
       "source_branch":"d","target_branch":"main","web_url":"u","updated_at":"t"}
    ]"#;
    let prs = gitlab::parse_merge_requests(body).unwrap();
    for pr in &prs {
        assert_eq!(pr.state, PrState::Draft, "iid {}", pr.number);
    }
}

#[test]
fn a_locked_gitlab_request_reads_as_closed() {
    let body = br#"[{"iid":8,"title":"t","state":"locked","source_branch":"s",
      "target_branch":"main","web_url":"u","updated_at":"t"}]"#;
    assert_eq!(
        gitlab::parse_merge_requests(body).unwrap()[0].state,
        PrState::Closed
    );
}

#[test]
fn each_provider_asks_for_a_new_request_in_its_own_vocabulary() {
    let new = NewPullRequest {
        title: "Add a thing".to_owned(),
        body: "why".to_owned(),
        source_branch: "feature/x".to_owned(),
        target_branch: "main".to_owned(),
        draft: true,
    };

    let gh = github::create_body(&new);
    assert_eq!(gh["head"], "feature/x");
    assert_eq!(gh["base"], "main");
    assert_eq!(gh["draft"], true);
    assert_eq!(gh["title"], "Add a thing");

    // GitLab has no draft field; it is a title prefix.
    let gl = gitlab::create_body(&new);
    assert_eq!(gl["source_branch"], "feature/x");
    assert_eq!(gl["title"], "Draft: Add a thing");
    assert_eq!(gl["description"], "why");
    assert!(gl.get("draft").is_none());
}

#[test]
fn an_unexpected_body_is_an_error_rather_than_a_panic() {
    assert!(github::parse_pulls(b"not json").is_err());
    assert!(gitlab::parse_merge_requests(br#"{"message":"401 Unauthorized"}"#).is_err());
}

#[test]
fn unknown_fields_are_ignored_so_a_provider_can_add_them() {
    let body = br#"[{"number":1,"title":"t","state":"open","merged_at":null,
      "user":{"login":"a","id":9,"site_admin":false},"head":{"ref":"h","sha":"deadbeef"},
      "base":{"ref":"main"},"html_url":"u","updated_at":"t","new_field_added_later":42}]"#;
    assert_eq!(github::parse_pulls(body).unwrap()[0].number, 1);
    assert_eq!(github_host().kind, HostKind::GitHub);
}

#[test]
fn each_provider_is_given_its_own_auth_header() {
    use coral_hosting::token::{auth_header, keyring_service};
    use secrecy::SecretString;

    let token = SecretString::from("ghp_secret".to_owned());
    // GitLab refuses a bearer personal access token, and the request is well formed when it
    // does, so the failure arrives as a bare 401 with nothing pointing at the header.
    assert_eq!(
        auth_header(HostKind::GitHub, &token),
        ("Authorization", "Bearer ghp_secret".to_owned())
    );
    assert_eq!(
        auth_header(HostKind::GitLab, &token),
        ("PRIVATE-TOKEN", "ghp_secret".to_owned())
    );

    // Keyed by origin: someone can be signed in to github.com and to a company instance at
    // once, and the two tokens are not interchangeable.
    assert_ne!(
        keyring_service(&github_host()),
        keyring_service(&enterprise_host())
    );
}

#[test]
fn two_profiles_on_one_host_keep_separate_accounts() {
    use coral_hosting::token::Account;

    // Two logins on one origin share a service name, so the account is the only thing that
    // can tell a work token from a personal one on github.com.
    assert_ne!(
        Account::of_profile("work").name(),
        Account::of_profile("personal").name()
    );

    // What everybody's token is filed under today. Changing it would sign the whole install
    // out on upgrade, silently, with the token still in the keyring under the old name.
    assert_eq!(Account::shared().name(), "api-token");
    assert_ne!(Account::of_profile("work").name(), Account::shared().name());

    // A profile whose id happens to spell the shared account's name is still its own account.
    assert_ne!(
        Account::of_profile("api-token").name(),
        Account::shared().name()
    );
}

#[test]
fn a_refusal_says_what_to_do_about_it() {
    use coral_hosting::client::describe_failure;

    assert!(describe_failure(401, b"{}").contains("expired"));
    assert!(describe_failure(404, b"{}").contains("cannot see it"));
    assert!(describe_failure(403, b"{}").contains("does not have access"));
    // A rate limit is the one 403 worth quoting, because waiting is the answer.
    let limited = br#"{"message":"API rate limit exceeded for user"}"#;
    assert!(describe_failure(403, limited).contains("rate limit"));
    // Anything else falls back to whatever sentence the provider sent.
    assert_eq!(
        describe_failure(422, br#"{"message":"A pull request already exists"}"#),
        "A pull request already exists"
    );
    assert!(describe_failure(500, b"<html>").contains("500"));
}

#[test]
fn only_the_host_refusing_counts_against_a_token() {
    // What sign-in keeps a token on. It used to keep every token it was handed and ask the
    // host afterwards, so the window reported "Signed in" about a string the host had never
    // seen. Asking first is only right if an unreachable host does not read as a rejection:
    // nobody could sign in offline, and a token that works would be thrown away.
    let refusal = coral_hosting::HostingError::Api {
        status: 401,
        detail: "the token was rejected".to_owned(),
    };
    assert!(refusal.is_refusal());

    for quiet in [
        coral_hosting::HostingError::Transport("dns failure".to_owned()),
        coral_hosting::HostingError::Malformed("not json".to_owned()),
        coral_hosting::HostingError::NoToken,
    ] {
        assert!(!quiet.is_refusal(), "{quiet}");
    }
}

#[test]
fn a_proposal_carries_the_commit_its_checks_run_on() {
    let sha = "a".repeat(40);
    let github = format!(
        r#"[{{"number":5,"title":"T","state":"open","merged_at":null,"user":{{"login":"e"}},
            "head":{{"ref":"topic","sha":"{sha}"}},"base":{{"ref":"main","sha":"{other}"}},
            "html_url":"https://github.com/o/r/pull/5","updated_at":"2026-01-01T00:00:00Z"}}]"#,
        other = "b".repeat(40)
    );
    assert_eq!(
        github::parse_pulls(github.as_bytes()).unwrap()[0].head_sha,
        sha
    );

    let gitlab = format!(
        r#"[{{"iid":7,"title":"T","state":"opened","source_branch":"topic","target_branch":"main",
            "web_url":"https://gitlab.com/g/p/-/merge_requests/7","updated_at":"2026-01-01T00:00:00Z",
            "sha":"{sha}"}}]"#
    );
    assert_eq!(
        gitlab::parse_merge_requests(gitlab.as_bytes()).unwrap()[0].head_sha,
        sha
    );
}

#[test]
fn checks_are_asked_of_the_commit_on_each_provider() {
    let sha = "c".repeat(40);
    assert_eq!(
        github::check_runs_url(&github_host(), &sha),
        format!(
            "https://api.github.com/repos/torvalds/linux/commits/{sha}/check-runs?per_page=100"
        )
    );
    assert_eq!(
        github::statuses_url(&enterprise_host(), &sha),
        format!("https://github.acme.corp/api/v3/repos/team/app/commits/{sha}/status")
    );
    assert_eq!(
        gitlab::pipelines_url(&gitlab_host(), 7),
        "https://gitlab.com/api/v4/projects/group%2Fsub%2Fproj/merge_requests/7/pipelines?per_page=1"
    );
}

mod review {
    use coral_hosting::review::{Remark, Reply, Verdict, in_order};
    use coral_hosting::{HostingError, github, gitlab};

    use super::{github_host, gitlab_host};

    #[test]
    fn github_line_comments_keep_their_place_even_after_the_line_moved_away() {
        let body = br#"[
          {"user":{"login":"ada"},"body":"Off by one?","created_at":"2026-10-01T10:00:00Z",
           "path":"src/lanes.rs","line":42,"original_line":40},
          {"user":{"login":"bob"},"body":"Outdated now","created_at":"2026-10-01T11:00:00Z",
           "path":"src/lanes.rs","line":null,"original_line":17}
        ]"#;
        let remarks = github::parse_comments(body).unwrap();
        assert_eq!(remarks[0].path.as_deref(), Some("src/lanes.rs"));
        assert_eq!(remarks[0].line, Some(42));
        assert_eq!(remarks[1].line, Some(17));
    }

    #[test]
    fn github_conversation_comments_are_about_no_line() {
        let body = br#"[{"user":{"login":"ada"},"body":"Looks good","created_at":"2026-10-01T10:00:00Z"}]"#;
        let remarks = github::parse_comments(body).unwrap();
        assert_eq!((remarks[0].path.as_deref(), remarks[0].line), (None, None));
    }

    #[test]
    fn github_reviews_keep_verdicts_and_what_was_said_and_drop_the_rest() {
        let body = br#"[
          {"user":{"login":"ada"},"state":"APPROVED","body":"","submitted_at":"2026-10-02T09:00:00Z"},
          {"user":{"login":"bob"},"state":"CHANGES_REQUESTED","body":"Needs a test","submitted_at":"2026-10-02T10:00:00Z"},
          {"user":{"login":"cy"},"state":"COMMENTED","body":"","submitted_at":"2026-10-02T11:00:00Z"},
          {"user":{"login":"cy"},"state":"COMMENTED","body":"One question","submitted_at":"2026-10-02T12:00:00Z"},
          {"user":{"login":"dee"},"state":"PENDING","body":"draft","submitted_at":null},
          {"user":{"login":"eve"},"state":"DISMISSED","body":"stale","submitted_at":"2026-10-02T13:00:00Z"}
        ]"#;
        let remarks = github::parse_reviews(body).unwrap();
        let said: Vec<(&str, Option<Verdict>)> = remarks
            .iter()
            .map(|r| (r.author.as_str(), r.verdict))
            .collect();
        assert_eq!(
            said,
            [
                ("ada", Some(Verdict::Approved)),
                ("bob", Some(Verdict::ChangesRequested)),
                ("cy", None),
            ]
        );
    }

    #[test]
    fn github_replies_go_to_the_conversation_or_become_a_review() {
        let host = github_host();
        let (url, body) = github::reply_request(
            &host,
            7,
            &Reply::Comment {
                body: "Thanks".into(),
            },
        );
        assert_eq!(
            url,
            "https://api.github.com/repos/torvalds/linux/issues/7/comments"
        );
        assert_eq!(body["body"], "Thanks");

        let (url, body) = github::reply_request(
            &host,
            7,
            &Reply::RequestChanges {
                body: "Fix it".into(),
            },
        );
        assert_eq!(
            url,
            "https://api.github.com/repos/torvalds/linux/pulls/7/reviews"
        );
        assert_eq!(body["event"], "REQUEST_CHANGES");
        let (_, body) = github::reply_request(
            &host,
            7,
            &Reply::Approve {
                body: String::new(),
            },
        );
        assert_eq!(body["event"], "APPROVE");
    }

    #[test]
    fn gitlab_keeps_approvals_from_its_system_notes_and_drops_the_other_events() {
        let body = br#"[
          {"author":{"username":"ada"},"body":"Looks right","created_at":"2026-10-01T10:00:00.000Z","system":false},
          {"author":{"username":"ada"},"body":"added 1 commit","created_at":"2026-10-01T10:05:00.000Z","system":true},
          {"author":{"username":"bob"},"body":"approved this merge request","created_at":"2026-10-01T11:00:00.000Z","system":true},
          {"author":{"username":"bob"},"body":"Why removed?","created_at":"2026-10-01T12:00:00.000Z","system":false,
           "position":{"new_path":null,"new_line":null,"old_path":"src/old.rs","old_line":9}}
        ]"#;
        let remarks = gitlab::parse_notes(body).unwrap();
        assert_eq!(remarks.len(), 3);
        assert_eq!(remarks[1].verdict, Some(Verdict::Approved));
        assert_eq!(remarks[1].body, "");
        assert_eq!(remarks[2].path.as_deref(), Some("src/old.rs"));
        assert_eq!(remarks[2].line, Some(9));
    }

    #[test]
    fn gitlab_approves_where_approvals_count_and_cannot_request_changes() {
        let host = gitlab_host();
        let calls = gitlab::reply_requests(
            &host,
            3,
            &Reply::Approve {
                body: "Ship it".into(),
            },
        )
        .unwrap();
        assert_eq!(calls.len(), 2);
        assert!(calls[0].0.ends_with("/merge_requests/3/approve"));
        assert!(calls[1].0.ends_with("/merge_requests/3/notes"));
        assert!(matches!(
            gitlab::reply_requests(&host, 3, &Reply::RequestChanges { body: "x".into() }),
            Err(HostingError::Unsupported(_))
        ));
    }

    #[test]
    fn remarks_from_several_lists_read_in_the_order_they_were_made() {
        let at = |t: &str| Remark {
            author: t.into(),
            body: String::new(),
            at: t.into(),
            verdict: None,
            path: None,
            line: None,
        };
        let ordered = in_order(vec![at("2026-10-03"), at("2026-10-01"), at("2026-10-02")]);
        let times: Vec<&str> = ordered.iter().map(|r| r.at.as_str()).collect();
        assert_eq!(times, ["2026-10-01", "2026-10-02", "2026-10-03"]);
    }
}
