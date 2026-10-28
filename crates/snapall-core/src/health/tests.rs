// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! How protection stands: collecting the facts and judging them, against in-memory stand-ins.

#![cfg(test)]

use crate::domain::{AreaName, Retention};
use crate::services::{self, Job};
use crate::system::{DiskUsage, QuotaMode, ServiceState};

use super::*;
use crate::testing::{FakeSystem, InMemoryBackend};

fn name(s: &str) -> AreaName {
    AreaName::parse(s).unwrap()
}

fn retention() -> Retention {
    Retention {
        keep: Some(50),
        keep_important: Some(10),
        min_age_seconds: Some(1800),
        cleanup: true,
        on_a_schedule: false,
        space_limit_permille: Some(500),
        quotas: false,
    }
}

fn area(
    config: &str,
    location: &str,
    snapshots: usize,
    newest: &str,
    used_percent: u64,
) -> AreaFacts {
    AreaFacts {
        config: name(config),
        location: location.to_owned(),
        snapshots,
        oldest: Some("2026-09-01 10:00:00".to_owned()),
        newest: Some(newest.to_owned()),
        measured_bytes: None,
        retention: Some(retention()),
        disk: Some(DiskUsage {
            size: 100,
            used: used_percent,
        }),
        quotas: QuotaMode::Off,
    }
}

fn running(enabled: &str, active: &str) -> ServiceState {
    ServiceState {
        enabled: Some(enabled.to_owned()),
        active: Some(active.to_owned()),
    }
}

/// A machine where everything is as it should be: system and home, cleanup running.
fn healthy() -> Facts {
    Facts {
        now: Some("2026-10-25 12:00:00".to_owned()),
        areas: vec![
            area("root", "/", 3, "2026-10-25 09:00:00", 40),
            area("home", "/home", 2, "2026-10-24 09:00:00", 40),
        ],
        jobs: vec![job_facts(&services::CLEANUP, "enabled", "active")],
        boot_menu_plugin: false,
    }
}

fn job_facts(job: &Job, enabled: &str, active: &str) -> JobFacts {
    JobFacts {
        job: *job,
        state: running(enabled, active),
        details: None,
    }
}

/// Say how one job stands on this machine, adding it if it was not there.
fn set_job(facts: &mut Facts, job: &Job, enabled: &str, active: &str) {
    facts.jobs.retain(|existing| existing.job.unit != job.unit);
    facts.jobs.push(job_facts(job, enabled, active));
}

fn headlines(health: &Health) -> Vec<&str> {
    health
        .findings
        .iter()
        .map(|f| f.headline.as_str())
        .collect()
}

#[test]
fn a_machine_where_everything_is_as_it_should_be_is_all_good() {
    let health = assess(&healthy());
    assert_eq!(
        health.verdict,
        Verdict {
            level: Level::Good,
            text: "All good".into()
        }
    );
    assert!(
        health.findings.iter().all(|f| f.level < Level::Attention),
        "{:?}",
        headlines(&health)
    );
    let labels: Vec<&str> = health.chips.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "5 snapshots",
            "size not measured",
            "disk 40% full",
            "cleanup on"
        ]
    );
}

#[test]
fn a_disk_nearly_full_is_something_to_do_and_a_full_one_is_something_to_do_now() {
    let mut facts = healthy();
    facts.areas[1].disk = Some(DiskUsage {
        size: 100,
        used: 91,
    });
    let health = assess(&facts);
    assert!(
        headlines(&health).contains(&"The disk holding Home is 91% full"),
        "{:?}",
        headlines(&health)
    );
    assert_eq!(health.verdict.text, "1 thing needs attention");
    assert!(health
        .chips
        .iter()
        .any(|c| c.label == "disk 91% full" && c.level == Level::Attention));

    facts.areas[1].disk = Some(DiskUsage {
        size: 100,
        used: 97,
    });
    let health = assess(&facts);
    assert_eq!(
        health.verdict,
        Verdict {
            level: Level::Problem,
            text: "Needs attention now".into()
        }
    );
    assert_eq!(
        health.findings[0].level,
        Level::Problem,
        "the most urgent comes first"
    );
}

#[test]
fn cleanup_that_is_off_says_so_offers_to_switch_it_on_and_is_never_called_on_when_merely_enabled() {
    let mut facts = healthy();
    set_job(&mut facts, &services::CLEANUP, "disabled", "inactive");
    let health = assess(&facts);
    let finding = health
        .findings
        .iter()
        .find(|f| f.topic == Topic::Cleanup)
        .unwrap();
    assert_eq!(
        (finding.level, finding.remedy),
        (Level::Attention, Some(Remedy::SwitchOnCleanup))
    );
    assert!(finding.headline.contains("not cleaned up automatically"));
    assert!(health
        .chips
        .iter()
        .any(|c| c.label == "cleanup off" && c.level == Level::Attention));

    // enabled but not running is not "on": nothing is cleaning anything
    set_job(&mut facts, &services::CLEANUP, "enabled", "inactive");
    let health = assess(&facts);
    let finding = health
        .findings
        .iter()
        .find(|f| f.topic == Topic::Cleanup)
        .unwrap();
    assert_eq!(
        (finding.level, finding.remedy),
        (Level::Note, None),
        "it could not be found out"
    );
    assert!(health.chips.iter().any(|c| c.label == "cleanup unknown"));
}

#[test]
fn an_area_over_its_limit_is_only_a_problem_while_nothing_cleans_it_up() {
    let mut facts = healthy();
    facts.areas[1].snapshots = 61;
    assert!(
        !headlines(&assess(&facts))
            .iter()
            .any(|h| h.contains("More snapshots than the limit")),
        "cleanup is running"
    );
    set_job(&mut facts, &services::CLEANUP, "disabled", "inactive");
    let health = assess(&facts);
    assert!(
        headlines(&health)
            .iter()
            .any(|h| h.contains("Home has 61 (its limit is 50)")),
        "{:?}",
        headlines(&health)
    );
    assert_eq!(
        health.verdict.text, "2 things need attention",
        "cleanup off, and the pile it lets grow"
    );
}

#[test]
fn the_room_snapshots_take_is_shown_only_when_every_one_was_measured() {
    let mut facts = healthy();
    assert!(headlines(&assess(&facts)).contains(&"The room each snapshot takes is not measured"));
    facts.areas[0].measured_bytes = Some(3_018_752);
    facts.areas[1].measured_bytes = Some(1_048_576);
    let health = assess(&facts);
    assert!(!headlines(&health).contains(&"The room each snapshot takes is not measured"));
    assert!(
        health.chips.iter().any(|c| c.label == "3.9 MiB"),
        "{:?}",
        health.chips
    );
    // one area unmeasured and the total is not claimed
    facts.areas[1].measured_bytes = None;
    assert!(assess(&facts)
        .chips
        .iter()
        .any(|c| c.label == "size not measured"));
}

#[test]
fn byte_counts_are_written_in_the_unit_a_person_reads() {
    for (bytes, text) in [
        (0, "0 B"),
        (1023, "1023 B"),
        (1024, "1.0 KiB"),
        (3_018_752, "2.9 MiB"),
        (5 * 1024 * 1024 * 1024, "5.0 GiB"),
        (3 * 1024_u64.pow(4), "3.0 TiB"),
    ] {
        assert_eq!(format_bytes(bytes), text);
    }
}

#[test]
fn snapshots_made_by_a_schedule_and_areas_gone_quiet_are_worth_knowing_not_alarming() {
    let mut facts = healthy();
    facts.areas[1].retention.as_mut().unwrap().on_a_schedule = true;
    facts.areas[0].newest = Some("2026-09-01 09:00:00".to_owned()); // 54 days before "now"
    let health = assess(&facts);
    assert!(
        headlines(&health).contains(&"Snapshots are also made on a schedule in Home"),
        "{:?}",
        headlines(&health)
    );
    assert!(
        headlines(&health).contains(&"The newest snapshot of System is 54 days old"),
        "{:?}",
        headlines(&health)
    );
    assert_eq!(
        health.verdict.text, "All good",
        "neither is something to do"
    );
    // an area with nothing in it says how to change that; one a day old says nothing
    facts.areas[0].snapshots = 0;
    facts.areas[0].newest = None;
    assert!(headlines(&assess(&facts)).contains(&"System has no snapshots yet"));
    facts.now = None;
    facts.areas[0].newest = Some("2026-01-01 00:00:00".to_owned());
    assert!(
        !headlines(&assess(&facts))
            .iter()
            .any(|h| h.contains("days old")),
        "without the time, no age is claimed"
    );
}

#[test]
fn the_boot_menu_tool_is_only_a_concern_if_it_is_installed_and_not_doing_its_job() {
    let mut facts = healthy();
    assert!(
        !headlines(&assess(&facts))
            .iter()
            .any(|h| h.contains("boot menu")),
        "not installed: nothing to say"
    );
    set_job(&mut facts, &services::BOOT_MENU, "enabled", "inactive");
    let health = assess(&facts);
    let finding = health
        .findings
        .iter()
        .find(|f| f.topic == Topic::BootMenu)
        .unwrap();
    assert_eq!(finding.level, Level::Attention);
    assert!(finding
        .why
        .contains("systemctl enable --now limine-snapper-sync.service"));
    facts.boot_menu_plugin = true; // the snapper plugin does the job instead
    assert!(!headlines(&assess(&facts))
        .iter()
        .any(|h| h.contains("boot menu")));
    facts.boot_menu_plugin = false;
    set_job(&mut facts, &services::BOOT_MENU, "enabled", "active");
    assert!(
        !headlines(&assess(&facts))
            .iter()
            .any(|h| h.contains("boot menu")),
        "running: all well"
    );
}

#[test]
fn the_status_bar_ends_with_how_many_things_need_attention_and_a_click_goes_to_the_worst() {
    let mut facts = healthy();
    set_job(&mut facts, &services::CLEANUP, "disabled", "inactive");
    facts.areas[1].disk = Some(DiskUsage {
        size: 100,
        used: 90,
    });
    let health = assess(&facts);
    let last = health.chips.last().unwrap();
    assert_eq!(last.label, "2 to look at");
    assert_eq!(
        last.topic, health.findings[0].topic,
        "it points at the most urgent"
    );
    assert!(
        assess(&healthy())
            .chips
            .iter()
            .all(|c| !c.label.contains("to look at")),
        "nothing to say when all is good"
    );
}

#[test]
fn everything_it_says_is_in_words_a_newcomer_was_told() {
    let mut facts = healthy();
    set_job(&mut facts, &services::CLEANUP, "disabled", "inactive");
    facts.areas[1].snapshots = 61;
    facts.areas[1].disk = Some(DiskUsage {
        size: 100,
        used: 97,
    });
    facts.areas[1].retention.as_mut().unwrap().on_a_schedule = true;
    facts.areas[0].snapshots = 0;
    facts.areas[0].newest = None;
    set_job(&mut facts, &services::BOOT_MENU, "enabled", "inactive");
    for finding in assess(&facts).findings {
        // the one tool name a person must type is allowed to contain the word snapper
        let text = format!("{} {}", finding.headline, finding.why)
            .replace("limine-snapper-sync", "the-boot-tool");
        let lower = text.to_lowercase();
        for word in ["subvolume", "btrfs", "snapper", "config", "qgroup"] {
            assert!(!lower.contains(word), "`{word}` in: {text}");
        }
    }
}

#[test]
fn gathering_reads_each_area_through_the_ports_and_skips_nothing_that_can_be_read() {
    let backend = InMemoryBackend::with_areas(&["home", "root"]).with_root_area("root");
    let plan = crate::usecase::plan_snapshots(&backend, None).unwrap();
    crate::usecase::run_plan(
        &backend,
        &plan,
        &crate::domain::Description::parse("x").unwrap(),
        None,
    );
    crate::usecase::run_plan(
        &backend,
        &plan,
        &crate::domain::Description::parse("y").unwrap(),
        None,
    );
    let system = FakeSystem::new()
        .with_disk(1000, 300)
        .at("2026-01-02 00:00:00")
        .with_service("snapper-cleanup.timer", "enabled", "active")
        .with_file("/usr/lib/snapper/plugins/10-limine-snapper-sync");
    let facts = gather(&backend, &system).unwrap();
    assert_eq!(facts.areas.len(), 2);
    let home = facts
        .areas
        .iter()
        .find(|a| a.config.as_str() == "home")
        .unwrap();
    assert_eq!(
        (home.snapshots, home.oldest.is_some(), home.newest.is_some()),
        (2, true, true)
    );
    assert_eq!(
        home.measured_bytes, None,
        "the in-memory backend measures nothing: not zero"
    );
    assert_eq!(home.disk.map(|d| d.percent_used()), Some(30));
    assert_eq!(home.retention.as_ref().and_then(|r| r.keep), Some(50));
    assert!(facts.boot_menu_plugin && facts.state_of(&services::CLEANUP).is_running());
    assert_eq!(assess(&facts).verdict.text, "All good");
}
