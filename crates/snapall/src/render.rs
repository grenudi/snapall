//! Presentation: pure functions from domain values to text or JSON.

use serde_json::{json, Value};
use snapall_core::doctor::{Check, OsRelease, Status};
use snapall_core::{ConfigName, Description, Plan, Report, SnapshotInfo, SnapshotKind};

pub fn report(report: &Report) -> String {
    let width = report
        .outcomes
        .iter()
        .map(|o| o.config.as_str().len())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for outcome in &report.outcomes {
        match &outcome.result {
            Ok(id) => out.push_str(&format!("ok    {:<width$}  #{id}\n", outcome.config)),
            Err(err) => out.push_str(&format!("FAIL  {:<width$}  {err}\n", outcome.config)),
        }
    }
    out
}

pub fn report_json(report: &Report) -> Value {
    let outcomes: Vec<Value> = report
        .outcomes
        .iter()
        .map(|o| match &o.result {
            Ok(id) => json!({"config": o.config.as_str(), "snapshot": id.get()}),
            Err(err) => json!({"config": o.config.as_str(), "error": err.to_string()}),
        })
        .collect();
    json!({"ok": report.is_success(), "outcomes": outcomes})
}

fn status_label(status: Status) -> &'static str {
    match status {
        Status::Ok => "ok",
        Status::Warn => "warn",
        Status::Fail => "FAIL",
    }
}

pub fn doctor(os: &OsRelease, checks: &[Check]) -> String {
    let mut out = format!("system: {}\n", os.display_name());
    for check in checks {
        out.push_str(&format!(
            "{:<5} {:<11} {}\n",
            status_label(check.status),
            check.name,
            check.detail
        ));
        if let Some(fix) = &check.fix {
            out.push_str(&format!("      {:<11} fix: {fix}\n", ""));
        }
    }
    out
}

pub fn doctor_json(os: &OsRelease, checks: &[Check]) -> Value {
    let worst = snapall_core::doctor::worst(checks);
    json!({
        "system": os.display_name(),
        "ok": worst != Status::Fail,
        "checks": checks.iter().map(|c| json!({
            "name": c.name,
            "status": status_label(c.status).to_lowercase(),
            "detail": c.detail,
            "fix": c.fix,
        })).collect::<Vec<_>>(),
    })
}

pub fn dry_run(plan: &Plan, description: &Description) -> String {
    let mut out = format!(
        "dry run: would snapshot {} config(s) with description \"{description}\"\n",
        plan.targets().len()
    );
    for config in plan.targets() {
        out.push_str(&format!("  {config}\n"));
    }
    out
}

pub fn dry_run_json(plan: &Plan, description: &Description) -> Value {
    json!({
        "dry_run": true,
        "description": description.as_str(),
        "would_snapshot": plan.targets().iter().map(ConfigName::as_str).collect::<Vec<_>>(),
    })
}

pub fn configs(configs: &[ConfigName]) -> String {
    let mut sorted: Vec<&ConfigName> = configs.iter().collect();
    sorted.sort();
    sorted.iter().map(|c| format!("{c}\n")).collect()
}

pub fn configs_json(configs: &[ConfigName]) -> Value {
    let mut sorted: Vec<&str> = configs.iter().map(ConfigName::as_str).collect();
    sorted.sort_unstable();
    json!({ "configs": sorted })
}

fn kind_label(info: &SnapshotInfo) -> String {
    match info.kind {
        SnapshotKind::Single => "single".to_owned(),
        SnapshotKind::Pre => "pre".to_owned(),
        SnapshotKind::Post { pre } => format!("post<-{pre}"),
    }
}

/// One block per config: a header, then an aligned table, newest snapshot last.
pub fn snapshots(all: &[(ConfigName, Vec<SnapshotInfo>)]) -> String {
    let mut blocks = Vec::new();
    for (config, list) in all {
        let mut block = format!("{config}\n");
        if list.is_empty() {
            block.push_str("  (no snapshots)\n");
        } else {
            let rows: Vec<[String; 5]> = list
                .iter()
                .map(|s| {
                    [
                        s.id.to_string(),
                        kind_label(s),
                        s.date.clone().unwrap_or_default(),
                        s.cleanup.clone().unwrap_or_default(),
                        s.description.clone(),
                    ]
                })
                .collect();
            let header = ["#", "type", "date", "cleanup", "description"].map(str::to_owned);
            let mut widths = [0usize; 4];
            for row in std::iter::once(&header).chain(rows.iter()) {
                for (width, cell) in widths.iter_mut().zip(row.iter()) {
                    *width = (*width).max(cell.chars().count());
                }
            }
            for row in std::iter::once(&header).chain(rows.iter()) {
                block.push_str(&format!(
                    "  {:>w0$}  {:<w1$}  {:<w2$}  {:<w3$}  {}\n",
                    row[0],
                    row[1],
                    row[2],
                    row[3],
                    row[4],
                    w0 = widths[0],
                    w1 = widths[1],
                    w2 = widths[2],
                    w3 = widths[3],
                ));
            }
        }
        blocks.push(block);
    }
    blocks.join("\n")
}

pub fn snapshots_json(all: &[(ConfigName, Vec<SnapshotInfo>)]) -> Value {
    let configs: serde_json::Map<String, Value> = all
        .iter()
        .map(|(config, list)| {
            let list: Vec<Value> = list
                .iter()
                .map(|s| {
                    let mut value = json!({
                        "id": s.id.get(),
                        "type": match s.kind {
                            SnapshotKind::Single => "single",
                            SnapshotKind::Pre => "pre",
                            SnapshotKind::Post { .. } => "post",
                        },
                        "date": s.date,
                        "user": s.user,
                        "cleanup": s.cleanup,
                        "description": s.description,
                    });
                    if let SnapshotKind::Post { pre } = s.kind {
                        value["pre"] = json!(pre.get());
                    }
                    value
                })
                .collect();
            (config.to_string(), Value::Array(list))
        })
        .collect();
    json!({ "configs": configs })
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapall_core::testing::InMemoryBackend;
    use snapall_core::{all_snapshots, plan_snapshots, snapshot_all, SnapshotBackend};

    #[test]
    fn report_lists_successes_and_failures_aligned() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]).failing_on("home");
        let r = snapshot_all(&backend, None, &Description::default()).unwrap();
        assert_eq!(
            report(&r),
            "FAIL  home  `in-memory` failed (injected failure): snapshot of home refused\nok    root  #1\n"
        );
        let value = report_json(&r);
        assert_eq!(value["ok"], false);
        assert_eq!(value["outcomes"][1]["snapshot"], 1);
        assert!(value["outcomes"][0]["error"]
            .as_str()
            .unwrap()
            .contains("refused"));
    }

    #[test]
    fn dry_run_names_every_target_in_both_formats() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]);
        let plan = plan_snapshots(&backend, None).unwrap();
        let description = Description::parse("x").unwrap();
        let text = dry_run(&plan, &description);
        assert!(text.contains("would snapshot 2 config(s)"));
        assert!(text.contains("  home\n") && text.contains("  root\n"));
        let value = dry_run_json(&plan, &description);
        assert_eq!(value["would_snapshot"], json!(["home", "root"]));
        assert_eq!(value["dry_run"], true);
    }

    #[test]
    fn configs_are_sorted_in_both_formats() {
        let names = [
            ConfigName::parse("root").unwrap(),
            ConfigName::parse("home").unwrap(),
        ];
        assert_eq!(configs(&names), "home\nroot\n");
        assert_eq!(configs_json(&names), json!({"configs": ["home", "root"]}));
    }

    #[test]
    fn snapshot_tables_align_and_empty_configs_say_so() {
        let backend = InMemoryBackend::with_configs(&["home", "root"]);
        let home = ConfigName::parse("home").unwrap();
        for text in ["before theming", "after theming"] {
            backend
                .create_snapshot(&home, &Description::parse(text).unwrap())
                .unwrap();
        }
        let all = all_snapshots(&backend).unwrap();
        let text = snapshots(&all);
        assert_eq!(
            text,
            "home\n  \
             #  type    date                 cleanup  description\n  \
             1  single  2026-01-01 00:00:01  number   before theming\n  \
             2  single  2026-01-01 00:00:02  number   after theming\n\
             \nroot\n  (no snapshots)\n"
        );
        let value = snapshots_json(&all);
        assert_eq!(value["configs"]["home"][1]["description"], "after theming");
        assert_eq!(value["configs"]["home"][0]["type"], "single");
        assert_eq!(value["configs"]["root"], json!([]));
    }

    #[test]
    fn doctor_output_shows_status_detail_and_fix() {
        use snapall_core::doctor::{diagnose, Probe};
        let probe = Probe {
            os: OsRelease::parse("ID=arch\nPRETTY_NAME=\"Arch Linux\"\n"),
            snapper: Err("No such file or directory".into()),
            is_root: Some(false),
            configs: None,
        };
        let checks = diagnose(&probe);
        let text = doctor(&probe.os, &checks);
        assert!(text.starts_with("system: Arch Linux\n"), "{text}");
        assert!(text.contains("FAIL  snapper"), "{text}");
        assert!(
            text.contains("fix: pacman -S --needed --noconfirm snapper"),
            "{text}"
        );
        assert!(text.contains("warn  privileges"), "{text}");
        let value = doctor_json(&probe.os, &checks);
        assert_eq!(value["ok"], false);
        assert_eq!(value["checks"][0]["status"], "fail");
        assert_eq!(value["checks"][1]["fix"], "run with sudo");
    }
}
