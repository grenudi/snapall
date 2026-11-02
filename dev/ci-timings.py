#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 grenudi
#
# SPDX-License-Identifier: GPL-3.0-or-later

"""What CI costs, from the runs GitHub kept: wall time, job minutes, and where the time goes.

    GITHUB_TOKEN=... dev/ci-timings.py [--runs N] [--workflow NAME] [--steps]

Median wall time of a run (from its creation to its last job), the sum of its jobs' minutes (what a runner is billed for, and
what a busy day queues behind), the slowest jobs, and with --steps the slowest steps. Read before changing a workflow and
again after: an optimisation that was not measured is a guess. The first thing it found was a job that compiled a tool from
source on every run and was 181 of a 193 second run.

--workflow takes the workflow's name (CI, Release, Verify release, ...). Only successful runs are counted, so a failure does not
pass for a fast run. Needs GITHUB_TOKEN (a token with read access to Actions).
"""
import datetime, os, statistics, sys, urllib.request, json

REPO = os.environ.get("GITHUB_REPOSITORY", "grenudi/snapall")


def api(path):
    request = urllib.request.Request(f"https://api.github.com/repos/{REPO}/{path}", headers={"Authorization": f"Bearer {os.environ['GITHUB_TOKEN']}", "Accept": "application/vnd.github+json"})
    with urllib.request.urlopen(request) as response:
        return json.load(response)


def when(text):
    return datetime.datetime.fromisoformat(text.replace("Z", "+00:00"))


def main(argv):
    option = lambda name, default: next((argv[i + 1] for i, a in enumerate(argv) if a == f"--{name}" and i + 1 < len(argv)), default)
    count, workflow, steps = int(option("runs", "8")), option("workflow", "CI"), "--steps" in argv
    runs = [r for r in api("actions/runs?per_page=100&status=completed")["workflow_runs"] if r["name"] == workflow and r["conclusion"] == "success"][:count]
    if not runs:
        sys.exit(f"no successful runs of {workflow!r} among the recent ones")
    walls, minutes, jobs_seen, steps_seen = [], [], {}, {}
    for run in runs:
        jobs = api(f"actions/runs/{run['id']}/jobs?per_page=100")["jobs"]
        finished = [j for j in jobs if j["completed_at"]]
        walls.append((max(when(j["completed_at"]) for j in finished) - when(run["created_at"])).total_seconds())
        minutes.append(sum((when(j["completed_at"]) - when(j["started_at"])).total_seconds() for j in finished) / 60)
        for job in finished:
            jobs_seen.setdefault(job["name"], []).append((when(job["completed_at"]) - when(job["started_at"])).total_seconds())
            for step in job["steps"]:
                if step["completed_at"] and step["started_at"]:
                    steps_seen.setdefault((job["name"], step["name"]), []).append((when(step["completed_at"]) - when(step["started_at"])).total_seconds())
    print(f"{workflow}: {len(runs)} successful runs, events {sorted({r['event'] for r in runs})}")
    print(f"  wall time    median {statistics.median(walls):5.0f} s   runs {[int(w) for w in walls]}")
    print(f"  job minutes  median {statistics.median(minutes):5.1f}     runs {[round(m, 1) for m in minutes]}")
    print("  slowest jobs (median seconds):")
    for name, seconds in sorted(jobs_seen.items(), key=lambda item: -statistics.median(item[1]))[:10]:
        print(f"    {statistics.median(seconds):5.0f}  {name[:70]}")
    if steps:
        print("  slowest steps (median seconds):")
        for (job, step), seconds in sorted(steps_seen.items(), key=lambda item: -statistics.median(item[1]))[:14]:
            print(f"    {statistics.median(seconds):5.0f}  {job[:30]:30} {step[:50]}")


if __name__ == "__main__":
    main(sys.argv[1:])
