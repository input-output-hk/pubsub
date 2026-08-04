#!/usr/bin/env python3
"""Generate web/assets/commit-activity.json — the data behind the activity section.

Writes per-day commit counts for the heatmap plus the headline figures the
rotating stat badge cycles through. Run from the repository root; the deploy
workflow runs this same script so the published numbers match a local preview.

Git history must be complete (the workflow checks out with fetch-depth: 0) or
the windowed counts will be short. GitHub-derived figures need a token in
GITHUB_TOKEN; without one they are simply omitted and the badge skips them.
"""

import json
import os
import subprocess
import urllib.error
import urllib.request
from collections import Counter
from datetime import date, timedelta

REPO = os.environ.get("ACTIVITY_REPO", "input-output-hk/pubsub")
OUT = "web/assets/commit-activity.json"


def git(*args):
    return subprocess.run(
        ["git", *args], capture_output=True, text=True, check=True
    ).stdout


def commit_days():
    """Per-day commit counts over all history, plus the total."""
    days = git("log", "--format=%as").split()
    return dict(sorted(Counter(days).items())), len(days)


def commits_since(days):
    since = (date.today() - timedelta(days=days)).isoformat()
    out = git("log", "--since", since, "--format=%H").split()
    return len(out)


def lines_since(days):
    """Lines added across the window. Merge commits are skipped so a merge does
    not double-count the diff its branch already contributed."""
    since = (date.today() - timedelta(days=days)).isoformat()
    out = git("log", "--since", since, "--no-merges", "--numstat", "--format=")
    added = 0
    for line in out.splitlines():
        parts = line.split("\t")
        if len(parts) == 3 and parts[0].isdigit():
            added += int(parts[0])
    return added


def api(path):
    token = os.environ.get("GITHUB_TOKEN")
    if not token:
        return None
    req = urllib.request.Request(
        "https://api.github.com/" + path,
        headers={
            "Authorization": "Bearer " + token,
            "Accept": "application/vnd.github+json",
            "User-Agent": "pubsub-activity-build",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=20) as resp:
            return json.load(resp)
    except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, ValueError):
        return None


def issues_closed():
    data = api("search/issues?q=repo:%s+is:issue+is:closed&per_page=1" % REPO)
    return data.get("total_count") if isinstance(data, dict) else None


def milestone():
    """The milestone with the most work in it — the one worth reporting on."""
    data = api("repos/%s/milestones?state=all&per_page=100" % REPO)
    if not isinstance(data, list) or not data:
        return None
    best = max(data, key=lambda m: m.get("open_issues", 0) + m.get("closed_issues", 0))
    closed = best.get("closed_issues", 0)
    total = closed + best.get("open_issues", 0)
    if not total:
        return None
    return {
        "title": best.get("title", ""),
        "closed": closed,
        "total": total,
        "percent": round(100 * closed / total),
    }


def main():
    counts, total = commit_days()
    stats = {
        "commits_year": commits_since(365),
        "lines_added_90d": lines_since(90),
    }
    closed = issues_closed()
    if closed is not None:
        stats["issues_closed"] = closed
    ms = milestone()
    if ms is not None:
        stats["milestone"] = ms

    with open(OUT, "w") as f:
        json.dump({"counts": counts, "total": total, "stats": stats}, f)
    print("wrote %s — %d days, %d commits, stats: %s" % (OUT, len(counts), total, stats))


if __name__ == "__main__":
    main()
