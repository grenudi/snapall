// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Health: is protection working, how much room does it take, will old snapshots be cleaned up.
//!
//! Facts go in, a report comes out, and nothing here reads the machine. The story is `assess`, below:
//! each question is one small rule that says what it found, why it matters and, where snapall can do
//! something about it, what. The words are plain ones (see ADR 16); a figure that cannot be measured is
//! said to be unmeasured and why, never guessed.

use crate::calendar::seconds_from_snapper_date;
use crate::coverage::describe;
use crate::domain::{ConfigName, Retention, SnapshotInfo};
use crate::explain::{cleanup_from, cleanup_text, room_counting_text, Cleanup, Limit};
use crate::port::{BackendError, SnapshotBackend};
use crate::services::{self, Job};
use crate::system::{DiskUsage, QuotaMode, ServiceState, SystemProbe, UnitDetails};

// ---- what is known ------------------------------------------------------------------------------

/// What is known about one protected area.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaFacts {
    pub config: ConfigName,
    /// The directory it protects: `/`, `/home`.
    pub subvolume: String,
    pub snapshots: usize,
    /// When the oldest and the newest were taken, as snapper wrote it.
    pub oldest: Option<String>,
    pub newest: Option<String>,
    /// The bytes the snapshots hold, when every one of them was measured; `None` if any was not.
    pub measured_bytes: Option<u64>,
    pub retention: Option<Retention>,
    pub disk: Option<DiskUsage>,
    /// How the disk holding it counts room.
    pub quotas: QuotaMode,
}

impl AreaFacts {
    /// What a person calls it: `Home`.
    pub fn title(&self) -> String {
        describe(&self.subvolume).title
    }

    /// Where its snapshots are kept.
    pub fn lives_in(&self) -> String {
        format!("{}/.snapshots", self.subvolume.trim_end_matches('/'))
    }
}

/// A background job, as the machine reports it and as its unit file describes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobFacts {
    pub job: Job,
    pub state: ServiceState,
    pub details: Option<UnitDetails>,
}

/// Everything the report is made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// The local time, as snapper writes dates, if it could be read.
    pub now: Option<String>,
    pub areas: Vec<AreaFacts>,
    /// Every job in the catalogue (`services::ALL`).
    pub jobs: Vec<JobFacts>,
    /// The boot menu is kept in step by a snapper plugin instead of a background job.
    pub boot_menu_plugin: bool,
}

impl Facts {
    pub fn job(&self, job: &Job) -> Option<&JobFacts> {
        self.jobs.iter().find(|facts| facts.job.unit == job.unit)
    }

    /// How the job stands; a job that is not here is not installed.
    pub fn state_of(&self, job: &Job) -> ServiceState {
        self.job(job)
            .map(|facts| facts.state.clone())
            .unwrap_or_default()
    }

    /// When the job runs, in words, as its own unit file says.
    pub fn schedule_of(&self, job: &Job) -> Vec<String> {
        self.job(job)
            .and_then(|facts| facts.details.as_ref())
            .map(UnitDetails::schedule)
            .unwrap_or_default()
    }

    /// What each area keeps, and protects from cleanup: its real settings.
    pub fn cleanup_limits(&self) -> Vec<Limit> {
        self.areas
            .iter()
            .filter_map(|area| {
                let retention = area.retention.as_ref()?;
                Some(Limit {
                    area: area.title(),
                    keep: retention.keep,
                    min_age_seconds: retention.min_age_seconds,
                })
            })
            .collect()
    }
}

// ---- what it means ------------------------------------------------------------------------------

/// How much a finding matters, least first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Good,
    /// Worth knowing; nothing is wrong.
    Note,
    /// Something to do.
    Attention,
    /// Something to do now.
    Problem,
}

/// Which part of the Health screen a finding belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    Space,
    Cleanup,
    BootMenu,
    Snapshots,
    /// Changes that are running now: not a finding, but an item of the status bar like the others.
    Jobs,
}

/// Something snapall can do about a finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remedy {
    SwitchOnCleanup,
    /// Switch on counting the room snapshots take.
    CountRoom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub level: Level,
    pub topic: Topic,
    pub headline: String,
    /// What it is, why it matters and what to do, in words that assume nothing.
    pub why: String,
    pub remedy: Option<Remedy>,
}

/// One item of the status bar: what it says, how it stands, and where a click goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    pub label: String,
    pub level: Level,
    pub topic: Topic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    pub level: Level,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Health {
    pub verdict: Verdict,
    /// Most urgent first.
    pub findings: Vec<Finding>,
    pub chips: Vec<Chip>,
}

// ---- the story ----------------------------------------------------------------------------------

/// Everything worth saying about how protection stands, most urgent first.
pub fn assess(facts: &Facts) -> Health {
    let mut findings = Vec::new();
    findings.extend(the_disk(facts));
    findings.extend(automatic_cleanup(facts));
    findings.extend(areas_over_their_limit(facts));
    findings.extend(space_not_measured(facts));
    findings.extend(snapshots_made_on_a_schedule(facts));
    findings.extend(areas_gone_quiet(facts));
    findings.extend(the_boot_menu(facts));
    findings.sort_by_key(|finding| std::cmp::Reverse(finding.level)); // stable: ties stay in story order
    Health {
        verdict: verdict(&findings),
        chips: chips(facts, &findings),
        findings,
    }
}

// ---- the rules ----------------------------------------------------------------------------------

/// The fullest disk that holds an area, with the area.
fn fullest_disk(facts: &Facts) -> Option<(&AreaFacts, DiskUsage)> {
    facts
        .areas
        .iter()
        .filter_map(|area| Some((area, area.disk?)))
        .max_by_key(|(_, disk)| disk.percent_used())
}

fn the_disk(facts: &Facts) -> Option<Finding> {
    let (area, disk) = fullest_disk(facts)?;
    let percent = disk.percent_used();
    let level = match percent {
        95.. => Level::Problem,
        85..=94 => Level::Attention,
        _ => return None,
    };
    Some(Finding {
        level,
        topic: Topic::Space,
        headline: format!("The disk holding {} is {percent}% full", area.title()),
        why: "A disk that is nearly full gets slow and can stop accepting changes. Snapshots keep old versions of files, so they hold space that only they use: deleting old snapshots frees it, and automatic cleanup does that for you while it runs.".to_owned(),
        remedy: None,
    })
}

fn cleanup_state(facts: &Facts) -> Cleanup {
    let timer = facts.state_of(&services::CLEANUP);
    cleanup_from(timer.enabled.as_deref(), timer.active.as_deref())
}

fn automatic_cleanup(facts: &Facts) -> Option<Finding> {
    let state = cleanup_state(facts);
    let (level, headline, remedy) = match state {
        Cleanup::On => return None,
        Cleanup::Off => (
            Level::Attention,
            "Old snapshots are not cleaned up automatically",
            Some(Remedy::SwitchOnCleanup),
        ),
        Cleanup::Unavailable => (
            Level::Note,
            "snapall could not find out whether old snapshots are cleaned up",
            None,
        ),
    };
    Some(Finding {
        level,
        topic: Topic::Cleanup,
        headline: headline.to_owned(),
        why: cleanup_text(
            state,
            &facts.schedule_of(&services::CLEANUP),
            &facts.cleanup_limits(),
        ),
        remedy,
    })
}

/// Areas that hold more snapshots than their limit while nothing is cleaning them up.
fn areas_over_their_limit(facts: &Facts) -> Option<Finding> {
    if cleanup_state(facts) == Cleanup::On {
        return None;
    }
    let over: Vec<String> = facts
        .areas
        .iter()
        .filter_map(|area| {
            let keep = area.retention.as_ref()?.keep?;
            (area.snapshots > usize::try_from(keep).ok()?).then(|| {
                format!(
                    "{} has {} (its limit is {keep})",
                    area.title(),
                    area.snapshots
                )
            })
        })
        .collect();
    (!over.is_empty()).then(|| Finding {
        level: Level::Attention,
        topic: Topic::Cleanup,
        headline: format!("More snapshots than the limit: {}", over.join("; ")),
        why: "Each area keeps its newest snapshots up to a limit and removes the older ones, but only when the cleanup job runs. Until it does, they pile up and take more and more room.".to_owned(),
        remedy: Some(Remedy::SwitchOnCleanup),
    })
}

fn space_not_measured(facts: &Facts) -> Option<Finding> {
    let unmeasured = facts
        .areas
        .iter()
        .find(|area| area.measured_bytes.is_none())?;
    let mode = unmeasured.quotas;
    Some(Finding {
        level: Level::Note,
        topic: Topic::Space,
        headline: "The room each snapshot takes is not measured".to_owned(),
        why: room_counting_text(mode),
        // only the full kind can answer the question, so only that is offered
        remedy: (mode == QuotaMode::Off).then_some(Remedy::CountRoom),
    })
}

fn snapshots_made_on_a_schedule(facts: &Facts) -> Option<Finding> {
    let names: Vec<String> = facts
        .areas
        .iter()
        .filter(|area| area.retention.as_ref().is_some_and(|r| r.on_a_schedule))
        .map(AreaFacts::title)
        .collect();
    (!names.is_empty()).then(|| Finding {
        level: Level::Note,
        topic: Topic::Snapshots,
        headline: format!("Snapshots are also made on a schedule in {}", names.join(", ")),
        why: "On top of the ones you ask for, the snapshot tool's own timer makes more, and cleanup removes the old ones. When snapall sets an area up it turns this off, so only your snapshots exist.".to_owned(),
        remedy: None,
    })
}

/// Areas with no snapshot at all, or none for a month.
fn areas_gone_quiet(facts: &Facts) -> Vec<Finding> {
    let now = facts.now.as_deref().and_then(seconds_from_snapper_date);
    facts
        .areas
        .iter()
        .filter_map(|area| {
            let Some(newest) = area.newest.as_deref().and_then(seconds_from_snapper_date) else {
                return (area.snapshots == 0).then(|| Finding {
                    level: Level::Note,
                    topic: Topic::Snapshots,
                    headline: format!("{} has no snapshots yet", area.title()),
                    why: "Nothing can be put back until one is taken. Press a, or run sudo snapall, to take the first.".to_owned(),
                    remedy: None,
                });
            };
            let days = (now? - newest) / 86_400;
            (days >= 30).then(|| Finding {
                level: Level::Note,
                topic: Topic::Snapshots,
                headline: format!("The newest snapshot of {} is {days} days old", area.title()),
                why: "A snapshot only protects what was there when it was taken: changes since then cannot be undone. Take one before your next change.".to_owned(),
                remedy: None,
            })
        })
        .collect()
}

/// The tool that puts snapshots into the boot menu is installed but not working.
fn the_boot_menu(facts: &Facts) -> Option<Finding> {
    let sync = &facts.state_of(&services::BOOT_MENU);
    let installed_but_idle = sync.is_installed() && !sync.is_running() && !facts.boot_menu_plugin;
    installed_but_idle.then(|| Finding {
        level: Level::Attention,
        topic: Topic::BootMenu,
        headline: "New snapshots are not reaching the boot menu".to_owned(),
        why: "The tool that adds your snapshots to the boot menu (limine-snapper-sync) is installed but not running, so you could not pick a recent snapshot when starting the computer. Start it with: systemctl enable --now limine-snapper-sync.service".to_owned(),
        remedy: None,
    })
}

// ---- the verdict and the status bar -------------------------------------------------------------

fn verdict(findings: &[Finding]) -> Verdict {
    let worst = findings
        .iter()
        .map(|f| f.level)
        .max()
        .unwrap_or(Level::Good);
    let to_do = findings
        .iter()
        .filter(|f| f.level >= Level::Attention)
        .count();
    let text = match (worst, to_do) {
        (Level::Problem, _) => "Needs attention now".to_owned(),
        (_, 0) => "All good".to_owned(),
        (_, 1) => "1 thing needs attention".to_owned(),
        (_, n) => format!("{n} things need attention"),
    };
    Verdict {
        level: if to_do == 0 { Level::Good } else { worst },
        text,
    }
}

/// The items of the status bar, in the order they are read.
fn chips(facts: &Facts, findings: &[Finding]) -> Vec<Chip> {
    let mut chips = vec![snapshot_count(facts), snapshot_space(facts)];
    chips.extend(disk_chip(facts));
    chips.push(cleanup_chip(facts));
    chips.extend(attention_chip(findings));
    chips
}

fn snapshot_count(facts: &Facts) -> Chip {
    let total: usize = facts.areas.iter().map(|a| a.snapshots).sum();
    Chip {
        label: format!("{total} snapshot{}", if total == 1 { "" } else { "s" }),
        level: Level::Good,
        topic: Topic::Snapshots,
    }
}

fn snapshot_space(facts: &Facts) -> Chip {
    let all: Option<u64> = facts.areas.iter().map(|a| a.measured_bytes).sum();
    match all.filter(|_| !facts.areas.is_empty()) {
        Some(bytes) => Chip {
            label: format_bytes(bytes),
            level: Level::Good,
            topic: Topic::Space,
        },
        None => Chip {
            label: "size not measured".to_owned(),
            level: Level::Note,
            topic: Topic::Space,
        },
    }
}

fn disk_chip(facts: &Facts) -> Option<Chip> {
    let (_, disk) = fullest_disk(facts)?;
    let level = match disk.percent_used() {
        95.. => Level::Problem,
        85..=94 => Level::Attention,
        _ => Level::Good,
    };
    Some(Chip {
        label: format!("disk {}% full", disk.percent_used()),
        level,
        topic: Topic::Space,
    })
}

fn cleanup_chip(facts: &Facts) -> Chip {
    let (label, level) = match cleanup_state(facts) {
        Cleanup::On => ("cleanup on", Level::Good),
        Cleanup::Off => ("cleanup off", Level::Attention),
        Cleanup::Unavailable => ("cleanup unknown", Level::Note),
    };
    Chip {
        label: label.to_owned(),
        level,
        topic: Topic::Cleanup,
    }
}

/// How many things need attention, when any do; a click goes to the most urgent.
fn attention_chip(findings: &[Finding]) -> Option<Chip> {
    let urgent: Vec<&Finding> = findings
        .iter()
        .filter(|f| f.level >= Level::Attention)
        .collect();
    let first = urgent.first()?;
    Some(Chip {
        label: format!("{} to look at", urgent.len()),
        level: first.level,
        topic: first.topic,
    })
}

/// `3.2 GiB`: a byte count in the unit a person reads.
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

// ---- gathering ----------------------------------------------------------------------------------

/// Read everything the report is made from. An area whose snapshots cannot be listed is left out
/// rather than failing the whole report: the rest is still worth showing.
pub fn gather(
    backend: &dyn SnapshotBackend,
    system: &dyn SystemProbe,
) -> Result<Facts, BackendError> {
    let areas = backend
        .list_configs()?
        .into_iter()
        .filter_map(|config| area_facts(backend, system, config))
        .collect();
    Ok(Facts {
        now: system.local_time(),
        areas,
        jobs: services::ALL
            .iter()
            .map(|job| JobFacts {
                job: *job,
                state: system.service(job.unit),
                details: system.unit_details(job.unit),
            })
            .collect(),
        boot_menu_plugin: system.file_exists("/usr/lib/snapper/plugins/10-limine-snapper-sync"),
    })
}

fn area_facts(
    backend: &dyn SnapshotBackend,
    system: &dyn SystemProbe,
    config: ConfigName,
) -> Option<AreaFacts> {
    let subvolume = backend.config_subvolume(&config).ok()?;
    let snapshots = backend.list_snapshots(&config).ok()?;
    Some(AreaFacts {
        retention: backend.retention(&config).ok(),
        disk: system.disk_usage(&subvolume),
        quotas: system.quota_mode(&subvolume),
        oldest: snapshots.iter().filter_map(|s| s.date.clone()).min(),
        newest: snapshots.iter().filter_map(|s| s.date.clone()).max(),
        measured_bytes: measured_bytes(&snapshots),
        snapshots: snapshots.len(),
        subvolume,
        config,
    })
}

/// The bytes the snapshots hold, if every one of them was measured.
fn measured_bytes(snapshots: &[SnapshotInfo]) -> Option<u64> {
    snapshots.iter().map(|s| s.used_bytes).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{FakeSystem, InMemoryBackend};

    fn name(s: &str) -> ConfigName {
        ConfigName::parse(s).unwrap()
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
        subvolume: &str,
        snapshots: usize,
        newest: &str,
        used_percent: u64,
    ) -> AreaFacts {
        AreaFacts {
            config: name(config),
            subvolume: subvolume.to_owned(),
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
    fn cleanup_that_is_off_says_so_offers_to_switch_it_on_and_is_never_called_on_when_merely_enabled(
    ) {
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
        assert!(
            headlines(&assess(&facts)).contains(&"The room each snapshot takes is not measured")
        );
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
        let backend = InMemoryBackend::with_configs(&["home", "root"]).with_root_config("root");
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
}
