#!/usr/bin/env python3
"""Project tracker: adoption and change-request metrics for Seedling.

    python3 tools/tracker.py --dry-run                     # fixture data, print to stdout
    python3 tools/tracker.py --out governance/TRACKER.md   # live (needs GITHUB_TOKEN)
    python3 tools/tracker.py --out governance/TRACKER.md --update-issue

Collects crates.io downloads, GitHub stars/forks/watchers, open/closed issues,
open PRs, change requests (issues labelled `change-request`), recent comments
and release download counts; renders a Markdown status page; optionally
updates (and pins) a single issue titled "Project status & tracker".

Stdlib only. crates.io requires a User-Agent that identifies the application
and at most 1 request/second (RFC 3463); both are honoured. A crate that is
not published yet is reported as such, not as an error.
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import pathlib
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

import seedling_config as sc

MARKER = "<!-- seedling-tracker -->"


class Http:
    """Live fetcher. Returns (status, json-or-None)."""

    def __init__(self, token: str | None, user_agent: str):
        self.token, self.ua = token, user_agent
        self._last_crates = 0.0

    def get(self, url: str):
        return self.request("GET", url)

    def request(self, method, url, body=None):
        headers = {"User-Agent": self.ua, "Accept": "application/json"}
        if url.startswith(sc.cfg("tracker", "github_api")):
            headers["Accept"] = "application/vnd.github+json"
            headers["X-GitHub-Api-Version"] = "2022-11-28"
            if self.token:
                headers["Authorization"] = f"Bearer {self.token}"
        else:  # crates.io: max 1 request per second
            wait = 1.0 - (time.monotonic() - self._last_crates)
            if wait > 0:
                time.sleep(wait)
            self._last_crates = time.monotonic()
        data = json.dumps(body).encode() if body is not None else None
        if data:
            headers["Content-Type"] = "application/json"
        req = urllib.request.Request(url, data=data, headers=headers, method=method)
        try:
            with urllib.request.urlopen(req, timeout=30) as r:
                raw = r.read()
                return r.status, (json.loads(raw) if raw else None)
        except urllib.error.HTTPError as e:
            try:
                return e.code, json.loads(e.read() or b"null")
            except ValueError:
                return e.code, None
        except urllib.error.URLError as e:
            return 0, {"message": str(e.reason)}


class Fixture:
    """Dry-run fetcher: answers from tools/fixtures/tracker_fixture.json by URL path."""

    def __init__(self, path: pathlib.Path):
        self.data = json.loads(path.read_text(encoding="utf-8"))
        self.writes = []

    def get(self, url):
        p = urllib.parse.urlsplit(url)
        qs = urllib.parse.parse_qs(p.query)
        # Search calls are keyed "path|decoded query"; everything else by path.
        key = p.path + ("|" + qs["q"][0] if "q" in qs else "")
        hit = self.data["responses"].get(key) or self.data["responses"].get(p.path)
        if hit:
            return hit["status"], hit["body"]
        return 404, {"message": "Not Found (no fixture)"}

    def request(self, method, url, body=None):
        self.writes.append((method, url))
        return 200, {"number": 1, "node_id": "FIXTURE", "html_url": "https://github.com/OWNER/seedling/issues/1"}


def q(s: str) -> str:
    return urllib.parse.quote(s, safe="")


def search_count(http, api, query):
    st, body = http.get(f"{api}/search/issues?q={q(query)}&per_page=1")
    return body.get("total_count") if st == 200 and body else None


def collect(http, slug: str, crate: str) -> dict:
    api = sc.cfg("tracker", "github_api")
    out: dict = {"repo": slug, "crate": crate, "errors": []}

    st, body = http.get(sc.cfg("tracker", "crates_io_api") + q(crate))
    if st == 200 and body and "crate" in body:
        c = body["crate"]
        out["crates_io"] = {"published": True, "downloads": c.get("downloads"),
                            "recent_downloads": c.get("recent_downloads"),
                            "max_version": c.get("max_stable_version") or c.get("max_version"),
                            "updated_at": c.get("updated_at")}
    elif st == 404:
        out["crates_io"] = {"published": False}
    else:
        out["crates_io"] = {"published": None}
        out["errors"].append(f"crates.io: HTTP {st}")

    st, repo = http.get(f"{api}/repos/{slug}")
    if st == 200 and repo:
        out["github"] = {k: repo.get(k) for k in ("stargazers_count", "forks_count", "subscribers_count",
                                                  "open_issues_count", "html_url", "pushed_at")}
    else:
        out["github"] = None
        out["errors"].append(f"GitHub repo {slug}: HTTP {st} ({(repo or {}).get('message', '')})")
        return out

    r = f"repo:{slug}"
    label = sc.cfg("tracker", "change_request_label")
    out["issues"] = {"open": search_count(http, api, f"{r} is:issue is:open"),
                     "closed": search_count(http, api, f"{r} is:issue is:closed"),
                     "open_prs": search_count(http, api, f"{r} is:pr is:open"),
                     "merged_prs": search_count(http, api, f"{r} is:pr is:merged")}
    st, body = http.get(f"{api}/search/issues?q={q(f'{r} is:issue is:open label:{label}')}&per_page=20&sort=created")
    out["change_requests"] = [{"number": i["number"], "title": i["title"], "url": i["html_url"],
                               "created_at": i["created_at"], "comments": i.get("comments", 0)}
                              for i in (body or {}).get("items", [])] if st == 200 else []
    n = int(sc.cfg("tracker", "recent_comments"))
    st, body = http.get(f"{api}/repos/{slug}/issues/comments?sort=created&direction=desc&per_page={n}")
    out["recent_comments"] = [{"user": (c.get("user") or {}).get("login", "?"), "url": c["html_url"],
                               "created_at": c["created_at"], "body": c.get("body", "")}
                              for c in (body or [])] if st == 200 else []
    st, body = http.get(f"{api}/repos/{slug}/releases?per_page=100")
    rels = []
    for rel in (body or []) if st == 200 else []:
        rels.append({"tag": rel["tag_name"], "published_at": rel.get("published_at"), "url": rel["html_url"],
                     "downloads": sum(a.get("download_count", 0) for a in rel.get("assets", []))})
    out["releases"] = rels
    return out


def safe(text: str, limit=120) -> str:
    """One-line, table-safe excerpt that cannot @-mention anyone or inject HTML."""
    t = " ".join(str(text).split())
    t = t.replace("|", "\\|").replace("<", "&lt;").replace(">", "&gt;").replace("@", "&#64;")
    return t if len(t) <= limit else t[:limit - 1] + "..."


def render(d: dict, now: str) -> str:
    def v(x):
        return "n/a" if x is None else f"{x:,}" if isinstance(x, int) else str(x)
    L = [MARKER, f"# {sc.cfg('project', 'name')}: project status & tracker", "",
         f"_Updated {now} by `tools/tracker.py` (scheduled workflow `tracker.yml`). Do not edit by hand._", "",
         "## Adoption", "", "| Metric | Value |", "|---|---:|"]
    ci = d.get("crates_io", {})
    if ci.get("published"):
        L += [f"| crates.io downloads (all time) | {v(ci['downloads'])} |",
              f"| crates.io downloads (last 90 days) | {v(ci['recent_downloads'])} |",
              f"| Latest version on crates.io | {v(ci['max_version'])} |"]
    elif ci.get("published") is False:
        L.append(f"| crates.io | not published yet (`{d['crate']}`) |")
    else:
        L.append("| crates.io | unavailable this run |")
    g = d.get("github")
    if g:
        L += [f"| GitHub stars | {v(g['stargazers_count'])} |", f"| Forks | {v(g['forks_count'])} |",
              f"| Watchers | {v(g['subscribers_count'])} |"]
        rel_dl = sum(r["downloads"] for r in d.get("releases", []))
        L.append(f"| Release asset downloads (all releases) | {v(rel_dl)} |")
        i = d.get("issues", {})
        L += ["", "## Issues and changes", "", "| Metric | Value |", "|---|---:|",
              f"| Open issues | {v(i.get('open'))} |", f"| Closed issues | {v(i.get('closed'))} |",
              f"| Open pull requests | {v(i.get('open_prs'))} |", f"| Merged pull requests | {v(i.get('merged_prs'))} |",
              f"| Open change requests (`{sc.cfg('tracker', 'change_request_label')}`) | {len(d.get('change_requests', []))} |"]
        L += ["", "## Open change requests", ""]
        L += [f"- [#{c['number']}]({c['url']}) {safe(c['title'])} ({c['comments']} comment{'' if c['comments'] == 1 else 's'}, opened {c['created_at'][:10]})"
              for c in d.get("change_requests", [])] or ["- None."]
        L += ["", "## Recent comments", ""]
        L += [f"- {c['created_at'][:10]} {safe(c['user'], 40)}: [{safe(c['body'], 100)}]({c['url']})"
              for c in d.get("recent_comments", [])] or ["- None."]
        L += ["", "## Releases", ""]
        L += [f"- [{safe(r['tag'], 40)}]({r['url']}) ({(r['published_at'] or 'draft')[:10]}): {v(r['downloads'])} asset downloads"
              for r in d.get("releases", [])] or ["- No releases yet."]
    if d["errors"]:
        L += ["", "## Collection notes", ""] + [f"- {safe(e, 200)}" for e in d["errors"]]
    return "\n".join(L) + "\n"


def update_issue(http, slug, body_md) -> str:
    api = sc.cfg("tracker", "github_api")
    title = sc.cfg("tracker", "issue_title")
    st, res = http.get(f"{api}/search/issues?q={q(f'repo:{slug} is:issue in:title {title}')}&per_page=20")
    issue = next((i for i in (res or {}).get("items", []) if i["title"] == title), None) if st == 200 else None
    if issue:
        st, res = http.request("PATCH", f"{api}/repos/{slug}/issues/{issue['number']}", {"body": body_md, "state": "open"})
    else:
        st, res = http.request("POST", f"{api}/repos/{slug}/issues", {"title": title, "body": body_md})
        issue = res
    if st not in (200, 201) or not issue:
        return f"issue update failed: HTTP {st}"
    # Pinning has no REST endpoint; GraphQL pinIssue is best effort (needs a token
    # that may pin; otherwise pin it once by hand, it stays pinned).
    st2, _ = http.request("POST", f"{api}/graphql",
                          {"query": "mutation($id:ID!){pinIssue(input:{issueId:$id}){issue{number}}}",
                           "variables": {"id": issue.get("node_id")}})
    return f"issue #{issue.get('number')} updated" + ("" if st2 == 200 else f" (pin attempt HTTP {st2}; pin by hand)")


def _without_stamp(md: str) -> str:
    return "\n".join(l for l in md.splitlines() if not l.startswith("_Updated "))


def commit_via_api(http, slug, path, md, branch) -> str:
    """Commit the tracker file through the GitHub contents API, as the token's
    own identity (no git identity or address needed). Skips no-op updates."""
    import base64
    api = sc.cfg("tracker", "github_api")
    url = f"{api}/repos/{slug}/contents/{path}"
    st, cur = http.get(f"{url}?ref={q(branch)}")
    sha = None
    if st == 200 and cur:
        sha = cur.get("sha")
        old = base64.b64decode(cur.get("content", "")).decode("utf-8", "replace")
        if _without_stamp(old) == _without_stamp(md):
            return "metrics unchanged; no commit"
    body = {"message": "chore(tracker): update project status [skip ci]",
            "content": base64.b64encode(md.encode()).decode(), "branch": branch}
    if sha:
        body["sha"] = sha
    st, _ = http.request("PUT", url, body)
    return f"committed {path} to {branch}" if st in (200, 201) else f"commit failed: HTTP {st}"


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--dry-run", action="store_true", help="use fixture data; never call the network")
    ap.add_argument("--repo", default=None, help="owner/repo (default: $GITHUB_REPOSITORY or config)")
    ap.add_argument("--crate", default=None, help="crates.io name (default: discovered via cargo metadata)")
    ap.add_argument("--out", help="write Markdown here (default: stdout)")
    ap.add_argument("--json", help="also write the raw metrics JSON here")
    ap.add_argument("--update-issue", action="store_true", help="create/update the pinned status issue")
    ap.add_argument("--commit", action="store_true",
                    help="commit [tracker].output via the GitHub contents API when metrics changed")
    ap.add_argument("--branch", default=os.environ.get("GITHUB_REF_NAME", "main"))
    a = ap.parse_args()

    slug = a.repo or sc.repo_slug()
    crate = a.crate
    if not crate:
        try:
            crate = sc.publish_package()["name"]
        except Exception:  # cargo missing on the runner: fall back to the repo name
            crate = slug.split("/")[-1]
    if a.dry_run:
        http = Fixture(sc.ROOT / sc.cfg("tracker", "fixture"))
        now = http.data["now"]
    else:
        http = Http(os.environ.get("GITHUB_TOKEN"), sc.cfg("tracker", "user_agent").format(repo=slug))
        now = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%d %H:%M UTC")
    data = collect(http, slug, crate)
    md = render(data, now)
    if a.out:
        pathlib.Path(a.out).write_text(md, encoding="utf-8")
        print(f"tracker: wrote {a.out}", file=sys.stderr)
    else:
        print(md)
    if a.json:
        pathlib.Path(a.json).write_text(json.dumps(data, indent=2), encoding="utf-8")
    if a.commit:
        print("tracker: " + commit_via_api(http, slug, sc.cfg("tracker", "output"), md, a.branch), file=sys.stderr)
    if a.update_issue:
        print("tracker: " + update_issue(http, slug, md), file=sys.stderr)
        if a.dry_run:
            print(f"tracker: dry run, would have sent {len(http.writes)} write request(s): "
                  + ", ".join(m for m, _ in http.writes), file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
