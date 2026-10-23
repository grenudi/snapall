//! Presentation: pure functions from domain values to text.

use snapall_core::{ConfigName, Description, Plan, Report};

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

pub fn configs(configs: &[ConfigName]) -> String {
    let mut sorted: Vec<&ConfigName> = configs.iter().collect();
    sorted.sort();
    sorted.iter().map(|c| format!("{c}\n")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapall_core::testing::InMemoryBackend;
    use snapall_core::{plan_snapshots, snapshot_all};

    #[test]
    fn report_lists_successes_and_failures_aligned() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]).failing_on("home");
        let r = snapshot_all(&backend, None, &Description::default()).unwrap();
        let text = report(&r);
        assert_eq!(
            text,
            "FAIL  home  `in-memory` failed (injected failure): snapshot of home refused\nok    root  #1\n"
        );
    }

    #[test]
    fn dry_run_names_every_target() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]);
        let plan = plan_snapshots(&backend, None).unwrap();
        let text = dry_run(&plan, &Description::parse("x").unwrap());
        assert!(text.contains("would snapshot 2 config(s)"));
        assert!(text.contains("  home\n") && text.contains("  root\n"));
    }
}
