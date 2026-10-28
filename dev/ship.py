#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

"""Ship the current branch: push, open the pull request, wait for CI, merge, wait for main, release, verify the release.

    GITHUB_TOKEN=... dev/ship.py "refactor: what changed" [--body-file FILE] [--no-release]
    GITHUB_TOKEN=... dev/ship.py --check            only verify the token and the repository

It keeps no state of its own: every step reads GitHub, so it can be stopped and run again, from any session, and carries on
from where the pull request, main or the release already is. A tool call may not last long, so run it in the background and
read its log:

    nohup env GITHUB_TOKEN=... dev/ship.py "title" --body-file /tmp/body.md > /tmp/ship.log 2>&1 &
    tail -n 25 /tmp/ship.log

Rules it keeps, so nobody has to remember them: squash merge only, with the title as the commit subject; the pull request is merged only when
every check passed (judged by the newest run of each check name); the release pull request is merged only after main's own CI passed;
a job that failed in the runner's own setup, before any of our code ran (the openSUSE container, seen twice), is re-run once, and
anything else that fails is reported with the tail of its log and stops everything.
"""
import json, re, subprocess, sys, time, urllib.error, urllib.request

KNOWN_RUNNER_FLAKES = ("A user's tools", "Set up job", "Initialize containers", "Initialize job")
RELEASE_WORKFLOWS = ("Release", "Verify release", "Package repositories", "Install snippets")
MIN_CHECKS = 20

def remote():
    url = subprocess.run(["git", "remote", "get-url", "origin"], capture_output=True, text=True).stdout.strip()
    m = re.search(r"github\.com[:/]([^/]+/[^/.]+)", url)
    return m.group(1) if m else "grenudi/snapall"

REPO = remote()

def token():
    import os
    t = os.environ.get("GITHUB_TOKEN")
    if not t:
        sys.exit("GITHUB_TOKEN is not set")
    return t

def api(path, data=None, method=None):
    request = urllib.request.Request(f"https://api.github.com/repos/{REPO}" + (f"/{path}" if path else ""), data=json.dumps(data).encode() if data is not None else None,
        method=method, headers={"Authorization": f"Bearer {token()}", "Accept": "application/vnd.github+json", "Content-Type": "application/json"})
    try:
        body = urllib.request.urlopen(request, timeout=60).read()
        return json.loads(body) if body else {}
    except urllib.error.HTTPError as error:
        return {"HTTP": error.code, "message": error.read().decode()[:300]}

def say(text):
    print(time.strftime("%H:%M:%S"), text, flush=True)

def sh(*args):
    return subprocess.run(list(args), capture_output=True, text=True)

def newest_checks(sha):
    latest = {}
    for run in sorted(api(f"commits/{sha}/check-runs?per_page=100").get("check_runs", []), key=lambda r: r["started_at"] or ""):
        latest[run["name"]] = run
    return latest

def log_tail(job_id, lines=25):
    out = sh("curl", "-sL", "-H", f"Authorization: Bearer {token()}", "-H", "Accept: application/vnd.github+json", f"https://api.github.com/repos/{REPO}/actions/jobs/{job_id}/logs").stdout
    rows = [re.sub(r"^\S+Z ", "", row) for row in out.split("\n") if row.strip()]
    interesting = [i for i, row in enumerate(rows) if re.search(r"##\[error\]|error(\[|:)|FAILED|panicked|Error:", row)]
    start = max(0, (interesting[0] if interesting else len(rows) - lines) - 4)
    return "\n".join("    " + row[:170] for row in rows[start:start + lines])

def rerun_failed_runner_flakes(sha, names):
    """Re-run, once, the jobs that failed in the runner's setup. Returns True if something was re-run."""
    reran = False
    runs = {}
    for name in names:
        check = newest_checks(sha)[name]
        job = api(f"actions/jobs/{check['id']}")
        steps = [s["name"] for s in job.get("steps", []) if s.get("conclusion") == "failure"]
        if steps and all(any(flake in s for flake in KNOWN_RUNNER_FLAKES) for s in steps) and job.get("run_attempt", 1) == 1:
            run_id = job["run_id"]
            if run_id not in runs:
                runs[run_id] = api(f"actions/runs/{run_id}/rerun-failed-jobs", {}, "POST")
                say(f"re-running {name}: the runner failed in its own setup ({steps[0]}), before any of our code ran")
            reran = True
    return reran

def wait_for_checks(sha, what):
    stable = 0
    reran = False
    while True:
        checks = newest_checks(sha)
        pending = [n for n, c in checks.items() if c["status"] != "completed"]
        failed = [n for n, c in checks.items() if c["status"] == "completed" and c["conclusion"] not in ("success", "skipped", "neutral")]
        say(f"{what}: {len(checks)} checks, {len(pending)} pending, {len(failed)} failed")
        if failed and not pending:
            if not reran and rerun_failed_runner_flakes(sha, failed):
                reran = True
                time.sleep(30)
                continue
            for name in failed:
                say(f"FAILED {name}: {checks[name]['html_url']}")
                print(log_tail(checks[name]["id"]), flush=True)
            sys.exit(1)
        if not pending and len(checks) >= MIN_CHECKS:
            stable += 1
            if stable >= 2:
                return
        else:
            stable = 0
        time.sleep(25)

def wait_for_workflow_run(sha, name, limit=1500):
    deadline = time.time() + limit
    while time.time() < deadline:
        runs = [r for r in api(f"actions/runs?head_sha={sha}&per_page=50").get("workflow_runs", []) if r["name"] == name]
        if runs and runs[0]["status"] == "completed":
            return runs[0]
        time.sleep(20)
    sys.exit(f"{name} did not finish for {sha[:7]}")

def verify_release(sha):
    for attempt in (1, 2):
        say(f"release {sha[:7]}: waiting for {', '.join(RELEASE_WORKFLOWS)}")
        runs = {name: wait_for_workflow_run(sha, name) for name in RELEASE_WORKFLOWS}
        bad = {n: r for n, r in runs.items() if r["conclusion"] != "success"}
        if not bad:
            break
        if attempt == 2:
            break
        for name, run in bad.items():
            jobs = api(f"actions/runs/{run['id']}/jobs?per_page=50").get("jobs", [])
            flaky = [j for j in jobs if j.get("conclusion") == "failure" and all(any(f in s["name"] for f in KNOWN_RUNNER_FLAKES) for s in j["steps"] if s.get("conclusion") == "failure")]
            if flaky and len(flaky) == len([j for j in jobs if j.get("conclusion") == "failure"]):
                say(f"re-running {name}: the runner failed in its own setup")
                api(f"actions/runs/{run['id']}/rerun-failed-jobs", {}, "POST")
                time.sleep(40)
            else:
                say(f"{name} failed for a reason of ours: {run['html_url']}")
                sys.exit(1)
    for name in RELEASE_WORKFLOWS:
        run = api(f"actions/runs/{runs[name]['id']}")
        jobs = api(f"actions/runs/{run['id']}/jobs?per_page=50&filter=latest").get("jobs", [])
        counts = {}
        for job in jobs:
            counts[job["conclusion"] or job["status"]] = counts.get(job["conclusion"] or job["status"], 0) + 1
        say(f"   {name:21} {run['conclusion']:8} attempt {run['run_attempt']} {counts}")
    if any(r["conclusion"] != "success" for r in (api(f"actions/runs/{r['id']}") for r in runs.values())):
        sys.exit(1)
    live = sh("bash", "-c", "curl -s -m 20 https://grenudi.github.io/snapall/apt/Packages | awk '/^Version:/{print $2}' | sort -uV | tail -1").stdout.strip()
    say(f"released and verified; the apt repository serves {live}")

def main(argv):
    if "--check" in argv:
        info = api("")
        say(f"repository {REPO}: {info.get('full_name', info)}")
        return
    flags = {a for a in argv if a.startswith("--")}
    positional = [a for a in argv if not a.startswith("--")]
    body_file = argv[argv.index("--body-file") + 1] if "--body-file" in argv else None
    if body_file:
        positional = [a for a in positional if a != body_file]
    title = positional[0] if positional else sh("git", "log", "-1", "--format=%s").stdout.strip()
    branch = sh("git", "rev-parse", "--abbrev-ref", "HEAD").stdout.strip()
    if branch == "main":
        sys.exit("on main: ship a branch")
    owner = REPO.split("/")[0]
    prs = api(f"pulls?head={owner}:{branch}&state=all").get("HTTP") is None and api(f"pulls?head={owner}:{branch}&state=all") or []
    pr = prs[0] if isinstance(prs, list) and prs else None
    if pr is None:
        sh("git", "push", "-q", f"https://x-access-token:{token()}@github.com/{REPO}.git", branch)
        body = open(body_file).read() if body_file else sh("git", "log", "-1", "--format=%b").stdout
        pr = api("pulls", {"title": title, "head": branch, "base": "main", "body": body}, "POST")
        if "number" not in pr:
            sys.exit(f"could not open the pull request: {pr}")
        say(f"opened pull request #{pr['number']}")
    else:
        say(f"pull request #{pr['number']} is {pr['state']}{' (merged)' if pr.get('merged_at') else ''}")
        if pr["state"] == "open":
            sh("git", "push", "-q", f"https://x-access-token:{token()}@github.com/{REPO}.git", branch)
    number = pr["number"]
    if not pr.get("merged_at"):
        time.sleep(15)
        sha = api(f"pulls/{number}")["head"]["sha"]
        wait_for_checks(sha, f"#{number}")
        say("all checks passed: merging (squash)")
        merged = api(f"pulls/{number}/merge", {"merge_method": "squash", "commit_title": f"{title} (#{number})"}, "PUT")
        if not merged.get("merged"):
            sys.exit(f"merge refused: {merged}")
        merge_sha = merged["sha"]
    else:
        merge_sha = pr["merge_commit_sha"]
    say(f"merged as {merge_sha[:7]}; waiting for main's own CI")
    deadline = time.time() + 1500
    ci = None
    while time.time() < deadline:
        runs = [r for r in api(f"actions/runs?head_sha={merge_sha}&per_page=30").get("workflow_runs", []) if r["name"] == "CI"]
        if runs and runs[0]["status"] == "completed":
            ci = runs[0]
            break
        time.sleep(25)
    if not ci or ci["conclusion"] != "success":
        sys.exit(f"main's CI did not pass: {ci and ci['html_url']}")
    say("main is green")
    if "--no-release" in flags:
        return
    release = None
    for _ in range(12):
        open_prs = api("pulls?state=open&per_page=20")
        release = next((p for p in open_prs if isinstance(open_prs, list) and p["title"].startswith("chore: release")), None) if isinstance(open_prs, list) else None
        if release:
            break
        time.sleep(15)
    if not release:
        say("no release pull request was opened: this change does not release")
        return
    merged = api(f"pulls/{release['number']}/merge", {"merge_method": "squash", "commit_title": f"chore: release (#{release['number']})"}, "PUT")
    if not merged.get("merged"):
        sys.exit(f"release merge refused: {merged}")
    verify_release(merged["sha"])

if __name__ == "__main__":
    main(sys.argv[1:])
