// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! `snapall setup`: find the parts of the disk that snapshots do not cover, explain what that means
//! in plain words, ask, and set them up. A person should never need a command outside snapall to
//! get protected, nor know a word snapall has not explained.
//!
//! The flow takes its facts and the way it talks and asks as inputs, so every branch is tested as a
//! conversation with a scripted person.

use std::error::Error;
use std::process::Command;

use snapall_core::coverage::{self, Area, Worth};
use snapall_core::explain::{self, Cleanup};
use snapall_core::{protect_area, ConfigName, SnapshotBackend, SystemProbe};

use crate::output::emit;

/// What the flow needs to know about this machine.
pub struct Facts {
    /// The text of `/proc/self/mountinfo`.
    pub mountinfo: String,
    pub is_root: bool,
    pub cleanup: Cleanup,
    /// When the cleanup job runs, in words, from its unit file; empty if that is not known.
    pub cleanup_schedule: Vec<String>,
}

pub struct Options {
    /// Protect this part of the disk instead of the recommended ones.
    pub area: Option<String>,
    /// Accept what snapall recommends without asking.
    pub yes: bool,
    /// Explain and show what would be done, and do nothing.
    pub dry_run: bool,
}

/// How the flow talks to the person and acts on the system.
pub trait Talk {
    fn say(&mut self, text: &str);
    /// A yes/no question whose recommended answer is yes.
    fn ask(&mut self, question: &str) -> Result<bool, Box<dyn Error>>;
    fn switch_on_cleanup(&mut self) -> Result<(), Box<dyn Error>>;
}

/// An area as the report shows it.
pub struct AreaReport {
    pub mount_point: String,
    pub title: String,
    pub protected_by: Option<ConfigName>,
    pub worth: Worth,
}

/// What happened.
pub struct Report {
    pub areas: Vec<AreaReport>,
    pub protected_now: Vec<(String, ConfigName)>,
    pub declined: Vec<String>,
    /// Things that went wrong, each with the plain-words reason.
    pub failed: Vec<(String, String)>,
    pub cleanup: Cleanup,
    pub cleanup_switched_on: bool,
    pub dry_run: bool,
}

impl Report {
    pub fn succeeded(&self) -> bool {
        self.failed.is_empty()
    }
}

pub fn run(
    backend: &dyn SnapshotBackend,
    facts: &Facts,
    talk: &mut dyn Talk,
    options: &Options,
) -> Result<Report, Box<dyn Error>> {
    require_administrator_rights(facts, options)?;
    let areas = find_the_areas(backend, facts)?;
    let mut report = Report {
        areas: areas.iter().map(area_report).collect(),
        protected_now: Vec::new(),
        declined: Vec::new(),
        failed: Vec::new(),
        cleanup: facts.cleanup,
        cleanup_switched_on: false,
        dry_run: options.dry_run,
    };

    explain_the_basics(talk);
    list_the_areas(talk, &areas);
    let to_protect = choose_what_to_protect(&areas, options)?;
    if to_protect.is_empty() {
        say_nothing_needs_doing(talk, &areas, options);
    }
    for area in to_protect {
        offer_protection(backend, talk, options, &area, &mut report);
    }
    offer_cleanup(talk, options, facts, &limits_of(backend), &mut report);
    say_whats_next(talk, options, &report);
    Ok(report)
}

fn require_administrator_rights(facts: &Facts, options: &Options) -> Result<(), Box<dyn Error>> {
    if facts.is_root || options.dry_run {
        return Ok(());
    }
    Err("Setting up protection changes how your computer keeps snapshots, which needs administrator rights. Run it again as:\n\n    sudo snapall setup\n\n(Add --dry-run to only look at what it would do.)".into())
}

fn find_the_areas(
    backend: &dyn SnapshotBackend,
    facts: &Facts,
) -> Result<Vec<Area>, Box<dyn Error>> {
    let mut configs = Vec::new();
    for name in backend.list_configs()? {
        let subvolume = backend.config_subvolume(&name)?;
        configs.push((name, subvolume));
    }
    let mounts = coverage::mounts_from_mountinfo(&facts.mountinfo);
    Ok(coverage::areas(&mounts, &configs))
}

fn area_report(area: &Area) -> AreaReport {
    let info = coverage::describe(&area.mount_point);
    AreaReport {
        mount_point: area.mount_point.clone(),
        title: info.title,
        protected_by: area.protected_by.clone(),
        worth: info.worth,
    }
}

/// What each area keeps and protects from cleanup: its real settings, read from snapper.
fn limits_of(backend: &dyn SnapshotBackend) -> Vec<explain::Limit> {
    let Ok(configs) = backend.list_configs() else {
        return Vec::new();
    };
    configs
        .iter()
        .filter_map(|config| {
            let subvolume = backend.config_subvolume(config).ok()?;
            let retention = backend.retention(config).ok()?;
            Some(explain::Limit {
                area: coverage::describe(&subvolume).title,
                keep: retention.keep,
                min_age_seconds: retention.min_age_seconds,
            })
        })
        .collect()
}

fn explain_the_basics(talk: &mut dyn Talk) {
    talk.say(&format!(
        "{}\n\n{}\n\n{}\n",
        explain::WHAT_IS_A_SNAPSHOT,
        explain::WHAT_IS_AN_AREA,
        explain::NOT_A_BACKUP
    ));
}

fn list_the_areas(talk: &mut dyn Talk, areas: &[Area]) {
    if areas.is_empty() {
        talk.say("snapall did not find a part of this computer's disk that it knows how to protect. It works with disks formatted as Btrfs, and none seems to be in use here.\n");
        return;
    }
    let mut text =
        String::from("The parts of your disk, and whether snapshots are kept for them:\n");
    for area in areas {
        text.push_str(&format!(
            "\n  {}\n      {}\n",
            explain::area_line(area),
            explain::area_detail(&coverage::describe(&area.mount_point))
        ));
    }
    talk.say(&text);
}

/// The areas to offer protection to: the one the person named, or those worth protecting that are not.
fn choose_what_to_protect(areas: &[Area], options: &Options) -> Result<Vec<Area>, Box<dyn Error>> {
    let Some(named) = &options.area else {
        return Ok(coverage::gaps(areas).into_iter().cloned().collect());
    };
    let wanted = named.trim_end_matches('/');
    let wanted = if wanted.is_empty() { "/" } else { wanted };
    match areas.iter().find(|a| {
        a.mount_point.trim_end_matches('/') == wanted.trim_end_matches('/')
            || a.mount_point == wanted
    }) {
        Some(area) if area.is_protected() => Ok(Vec::new()),
        Some(area) => Ok(vec![area.clone()]),
        None => {
            let known: Vec<&str> = areas.iter().map(|a| a.mount_point.as_str()).collect();
            Err(format!(
                "{named} is not a part of the disk that snapall can protect. The parts it found are: {}.",
                known.join(", ")
            )
            .into())
        }
    }
}

/// There is nothing to set up. Saying so is part of the answer: silence would leave the person
/// wondering whether anything happened.
fn say_nothing_needs_doing(talk: &mut dyn Talk, areas: &[Area], options: &Options) {
    let message = match &options.area {
        Some(named) => {
            let title = coverage::describe(named).title;
            format!("{title} ({named}) is already protected, so there is nothing to set up.\n")
        }
        None if areas.is_empty() => return,
        None => "Everything snapall recommends protecting is protected already, so there is nothing to set up.\n".to_owned(),
    };
    talk.say(&message);
}

fn offer_protection(
    backend: &dyn SnapshotBackend,
    talk: &mut dyn Talk,
    options: &Options,
    area: &Area,
    report: &mut Report,
) {
    let title = coverage::describe(&area.mount_point).title;
    talk.say(&format!("{}\n", explain::protect_offer(&area.mount_point)));
    if options.dry_run {
        talk.say("(Dry run: nothing was changed.)\n");
        return;
    }
    let go = options.yes || matches!(talk.ask(&format!("Protect {title} now?")), Ok(true));
    if !go {
        talk.say(&format!(
            "Skipped. {title} stays unprotected; run `{}` whenever you want to.\n",
            setup_command(options)
        ));
        report.declined.push(area.mount_point.clone());
        return;
    }
    match protect_area(backend, &area.mount_point) {
        Ok(name) => {
            talk.say(&format!("{}\n", explain::protected_text(&area.mount_point)));
            report.protected_now.push((area.mount_point.clone(), name));
        }
        Err(error) => {
            let why = explain::failure_text(&title, &error);
            talk.say(&format!("{why}\n"));
            report.failed.push((area.mount_point.clone(), why));
        }
    }
}

fn offer_cleanup(
    talk: &mut dyn Talk,
    options: &Options,
    facts: &Facts,
    limits: &[explain::Limit],
    report: &mut Report,
) {
    let state = facts.cleanup;
    talk.say(&format!(
        "{}\n",
        explain::cleanup_text(state, &facts.cleanup_schedule, limits)
    ));
    if state != Cleanup::Off {
        return;
    }
    talk.say(&format!(
        "{}\n",
        explain::cleanup_offer(&facts.cleanup_schedule, limits)
    ));
    if options.dry_run {
        return;
    }
    if !(options.yes || matches!(talk.ask("Switch on automatic cleanup now?"), Ok(true))) {
        return;
    }
    match talk.switch_on_cleanup() {
        Ok(()) => {
            talk.say("Automatic cleanup is on.\n");
            report.cleanup_switched_on = true;
        }
        Err(error) => {
            let why =
                format!("Could not switch automatic cleanup on: {error}. Nothing was changed.");
            talk.say(&format!("{why}\n"));
            report.failed.push(("cleanup".into(), why));
        }
    }
}

/// The command to run this again, as the person ran it.
fn setup_command(options: &Options) -> String {
    match &options.area {
        Some(area) => format!("sudo snapall setup {area}"),
        None => "sudo snapall setup".to_owned(),
    }
}

fn say_whats_next(talk: &mut dyn Talk, options: &Options, report: &Report) {
    let mut next = String::from("What next:\n");
    if report.dry_run {
        next.push_str(&format!(
            "  - Run `{}` to do this.\n",
            setup_command(options)
        ));
    }
    if !report.protected_now.is_empty() {
        next.push_str("  - Take the first snapshot of everything with `sudo snapall`.\n");
    }
    next.push_str("  - Open the snapall window with `sudo snapall tui`: it lists your snapshots and restores from them.\n");
    next.push_str(
        "  - Or right-click a file in your file manager and choose \"Restore from snapshots\".\n",
    );
    talk.say(&next);
}

/// The parts of the disk the window offers to protect: the recommended ones first, then the ones that
/// are up to the person. What is not worth protecting is not offered.
pub fn offers(backend: &dyn SnapshotBackend, facts: &Facts) -> Vec<snapall_tui::Offer> {
    let Ok(areas) = find_the_areas(backend, facts) else {
        return Vec::new();
    };
    let mut offers: Vec<snapall_tui::Offer> = areas
        .iter()
        .filter(|area| !area.is_protected())
        .filter_map(|area| {
            let info = coverage::describe(&area.mount_point);
            (info.worth != Worth::NotUseful).then(|| snapall_tui::Offer {
                mount_point: area.mount_point.clone(),
                title: info.title,
                summary: explain::protect_summary(&area.mount_point),
                recommended: info.worth == Worth::Recommended,
            })
        })
        .collect();
    offers.sort_by_key(|offer| !offer.recommended); // stable: the recommended first, in disk order
    offers
}

/// The footer line for when something recommended has no snapshots: what it is and what to press.
pub fn hint_for(offers: &[snapall_tui::Offer]) -> Option<String> {
    let titles: Vec<&str> = offers
        .iter()
        .filter(|offer| offer.recommended)
        .map(|offer| offer.title.as_str())
        .collect();
    if titles.is_empty() {
        return None;
    }
    let verb = if titles.len() == 1 { "has" } else { "have" };
    Some(format!(
        "{} {verb} no snapshots yet. Press p to see what that means and protect {}.",
        titles.join(" and "),
        if titles.len() == 1 { "it" } else { "them" }
    ))
}

// ---- the real world ---------------------------------------------------------------------------

/// Read the facts of this machine.
pub fn gather() -> Facts {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let ask_systemd = |question: &str| {
        Command::new("systemctl")
            .args([question, "snapper-cleanup.timer"])
            .output()
            .ok()
            .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    let (enabled, active) = (ask_systemd("is-enabled"), ask_systemd("is-active"));
    Facts {
        mountinfo: std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default(),
        is_root: crate::doctor::effective_uid(&status) == Some(0),
        cleanup: explain::cleanup_from(enabled.as_deref(), active.as_deref()),
        cleanup_schedule: snapall_system::LinuxSystem::new()
            .unit_details(snapall_core::services::CLEANUP.unit)
            .map(|details| details.schedule())
            .unwrap_or_default(),
    }
}

/// A terminal: it says things on stdout and asks on stderr.
pub struct Terminal;

impl Talk for Terminal {
    fn say(&mut self, text: &str) {
        let _ = emit(&format!("{text}\n"));
    }

    fn ask(&mut self, question: &str) -> Result<bool, Box<dyn Error>> {
        if !crate::ask::can_ask() {
            return Err(
                "snapall needs a terminal to ask you; add --yes to accept what it recommends"
                    .into(),
            );
        }
        crate::ask::confirm(question, true)
    }

    fn switch_on_cleanup(&mut self) -> Result<(), Box<dyn Error>> {
        let out = Command::new("systemctl")
            .args(["enable", "--now", "snapper-cleanup.timer"])
            .output()?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr)
                .trim()
                .to_owned()
                .into())
        }
    }
}

/// For `--json`: nothing is said, and nothing is asked.
pub struct Quiet;

impl Talk for Quiet {
    fn say(&mut self, _text: &str) {}

    fn ask(&mut self, _question: &str) -> Result<bool, Box<dyn Error>> {
        Ok(false)
    }

    fn switch_on_cleanup(&mut self) -> Result<(), Box<dyn Error>> {
        Terminal.switch_on_cleanup()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapall_core::testing::InMemoryBackend;

    /// A person who answers from a script and writes down everything said to them.
    struct Person {
        answers: Vec<bool>,
        asked: Vec<String>,
        said: String,
        cleanup_switched_on: bool,
        cleanup_fails: bool,
    }

    impl Person {
        fn answering(answers: &[bool]) -> Self {
            Self {
                answers: answers.to_vec(),
                asked: Vec::new(),
                said: String::new(),
                cleanup_switched_on: false,
                cleanup_fails: false,
            }
        }
    }

    impl Talk for Person {
        fn say(&mut self, text: &str) {
            self.said.push_str(text);
            self.said.push('\n');
        }
        fn ask(&mut self, question: &str) -> Result<bool, Box<dyn Error>> {
            self.asked.push(question.to_owned());
            Ok(if self.answers.is_empty() {
                false
            } else {
                self.answers.remove(0)
            })
        }
        fn switch_on_cleanup(&mut self) -> Result<(), Box<dyn Error>> {
            if self.cleanup_fails {
                return Err("Failed to enable unit".into());
            }
            self.cleanup_switched_on = true;
            Ok(())
        }
    }

    const MOUNTINFO: &str = "\\
24 1 0:21 /@ / rw - btrfs /dev/nvme0n1p2 rw,subvol=/@\n\\
25 24 0:21 /@home /home rw - btrfs /dev/nvme0n1p2 rw,subvol=/@home\n\\
26 24 0:21 /@log /var/log rw - btrfs /dev/nvme0n1p2 rw,subvol=/@log\n";

    fn facts(is_root: bool, cleanup: Cleanup) -> Facts {
        Facts {
            mountinfo: MOUNTINFO.into(),
            is_root,
            cleanup,
            cleanup_schedule: Vec::new(),
        }
    }
    fn options() -> Options {
        Options {
            area: None,
            yes: false,
            dry_run: false,
        }
    }
    /// A machine whose only protected area is the system.
    fn machine() -> InMemoryBackend {
        InMemoryBackend::with_configs(&["root"]).with_root_config("root")
    }

    #[test]
    fn it_explains_first_lists_the_areas_and_offers_only_what_is_worth_protecting() {
        let backend = machine();
        let mut person = Person::answering(&[false]);
        let report = run(&backend, &facts(true, Cleanup::On), &mut person, &options()).unwrap();
        for must in [
            "A snapshot is a saved moment",
            "System (/): protected",
            "Home (/home): NOT protected",
            "Protect Home (/home)?",
            "Skipped.",
        ] {
            assert!(
                person.said.contains(must),
                "`{must}` not said:\n{}",
                person.said
            );
        }
        assert!(
            !person.said.contains("Protect System"),
            "the system is already protected"
        );
        assert!(
            !person.said.contains("Protect System logs"),
            "logs are not worth offering"
        );
        assert_eq!(person.asked, ["Protect Home now?"]);
        assert_eq!(report.declined, ["/home"]);
        assert_eq!(
            backend.list_configs().unwrap().len(),
            1,
            "a no changes nothing"
        );
    }

    #[test]
    fn a_yes_protects_the_area_turns_off_schedules_and_says_what_comes_next() {
        let backend = machine();
        let mut person = Person::answering(&[true]);
        let report = run(&backend, &facts(true, Cleanup::On), &mut person, &options()).unwrap();
        assert_eq!(report.protected_now.len(), 1);
        assert_eq!(backend.timeline_create("home"), Some(false));
        assert!(
            person.said.contains("Home (/home) is protected now"),
            "{}",
            person.said
        );
        assert!(
            person
                .said
                .contains("Take the first snapshot of everything with `sudo snapall`"),
            "{}",
            person.said
        );
        assert!(report.succeeded());
    }

    #[test]
    fn yes_accepts_the_recommendations_without_a_single_question() {
        let backend = machine();
        let mut person = Person::answering(&[]);
        let report = run(
            &backend,
            &facts(true, Cleanup::Off),
            &mut person,
            &Options {
                yes: true,
                ..options()
            },
        )
        .unwrap();
        assert!(person.asked.is_empty());
        assert_eq!(report.protected_now.len(), 1);
        assert!(
            person.cleanup_switched_on && report.cleanup_switched_on,
            "cleanup was off, so it was switched on"
        );
    }

    #[test]
    fn a_dry_run_explains_everything_and_changes_nothing_not_even_without_administrator_rights() {
        let backend = machine();
        let mut person = Person::answering(&[]);
        let report = run(
            &backend,
            &facts(false, Cleanup::Off),
            &mut person,
            &Options {
                dry_run: true,
                ..options()
            },
        )
        .unwrap();
        assert!(person.asked.is_empty() && !person.cleanup_switched_on);
        assert_eq!(backend.list_configs().unwrap().len(), 1);
        assert!(
            person.said.contains("Protect Home (/home)?")
                && person.said.contains("Dry run: nothing was changed"),
            "{}",
            person.said
        );
        assert!(
            person.said.contains("Run `sudo snapall setup` to do this"),
            "{}",
            person.said
        );
        assert!(report.dry_run && report.protected_now.is_empty());
    }

    #[test]
    fn without_administrator_rights_it_says_so_in_snapalls_own_terms_before_saying_anything_else() {
        let backend = machine();
        let mut person = Person::answering(&[]);
        let error = run(
            &backend,
            &facts(false, Cleanup::On),
            &mut person,
            &options(),
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("sudo snapall setup"), "{error}");
        assert!(person.said.is_empty(), "nothing was said before refusing");
    }

    #[test]
    fn automatic_cleanup_is_explained_and_only_offered_when_it_is_off() {
        let backend = machine();
        for (state, offered) in [
            (Cleanup::On, false),
            (Cleanup::Off, true),
            (Cleanup::Unavailable, false),
        ] {
            let mut person = Person::answering(&[false, false]);
            run(&backend, &facts(true, state), &mut person, &options()).unwrap();
            assert_eq!(person.said.contains("Switch it on?"), offered, "{state:?}");
            assert_eq!(
                person.asked.iter().any(|q| q.contains("cleanup")),
                offered,
                "{state:?}"
            );
        }
        // and when the person declines it, it stays off
        let mut person = Person::answering(&[false, false]);
        let report = run(
            &backend,
            &facts(true, Cleanup::Off),
            &mut person,
            &options(),
        )
        .unwrap();
        assert!(!person.cleanup_switched_on && !report.cleanup_switched_on);
    }

    #[test]
    fn a_failure_is_explained_plainly_nothing_is_left_changed_and_the_report_says_it_failed() {
        let backend = machine();
        backend.refuse_option_changes();
        let mut person = Person::answering(&[true]);
        let report = run(&backend, &facts(true, Cleanup::On), &mut person, &options()).unwrap();
        assert!(!report.succeeded());
        assert!(
            person.said.contains("Could not protect Home")
                && person.said.contains("Nothing was changed"),
            "{}",
            person.said
        );
        assert_eq!(
            backend.list_configs().unwrap().len(),
            1,
            "the half-made area was taken back"
        );
        // a cleanup that cannot be switched on is a failure too, said plainly
        let backend = machine();
        let mut person = Person::answering(&[false, true]);
        person.cleanup_fails = true;
        let report = run(
            &backend,
            &facts(true, Cleanup::Off),
            &mut person,
            &options(),
        )
        .unwrap();
        assert!(
            person
                .said
                .contains("Could not switch automatic cleanup on")
                && !report.succeeded(),
            "{}",
            person.said
        );
    }

    #[test]
    fn a_named_area_is_offered_even_if_optional_and_one_that_does_not_exist_is_refused_listing_the_real_ones(
    ) {
        let backend = machine();
        let mut person = Person::answering(&[true]);
        let report = run(
            &backend,
            &facts(true, Cleanup::On),
            &mut person,
            &Options {
                area: Some("/var/log".into()),
                ..options()
            },
        )
        .unwrap();
        assert!(person.said.contains("Not recommended:"), "{}", person.said);
        assert_eq!(report.protected_now[0].0, "/var/log");
        let error = run(
            &backend,
            &facts(true, Cleanup::On),
            &mut Person::answering(&[]),
            &Options {
                area: Some("/nowhere".into()),
                ..options()
            },
        )
        .err()
        .unwrap();
        assert!(
            error.to_string().contains("not a part of the disk")
                && error.to_string().contains("/, /home, /var/log"),
            "{error}"
        );
    }

    #[test]
    fn when_everything_worth_protecting_is_protected_it_says_so_and_asks_nothing() {
        let backend = InMemoryBackend::with_configs(&["root", "home"]).with_root_config("root");
        // make the in-memory `home` config's subvolume /home, as a real one would be
        let mut person = Person::answering(&[]);
        let facts = Facts {
            mountinfo: MOUNTINFO.into(),
            is_root: true,
            cleanup: Cleanup::On,
            cleanup_schedule: Vec::new(),
        };
        let both = InMemoryBackend::with_configs(&[]);
        both.create_config(&ConfigName::parse("root").unwrap(), "/")
            .unwrap();
        both.create_config(&ConfigName::parse("home").unwrap(), "/home")
            .unwrap();
        let report = run(&both, &facts, &mut person, &options()).unwrap();
        assert!(
            person.asked.is_empty() && report.protected_now.is_empty(),
            "{}",
            person.said
        );
        assert!(!person.said.contains("Protect Home"), "{}", person.said);
        let _ = backend;
    }

    #[test]
    fn when_there_is_nothing_to_set_up_it_says_so_instead_of_staying_silent() {
        let facts = Facts {
            mountinfo: MOUNTINFO.into(),
            is_root: true,
            cleanup: Cleanup::On,
            cleanup_schedule: Vec::new(),
        };
        let both = InMemoryBackend::with_configs(&[]);
        both.create_config(&ConfigName::parse("root").unwrap(), "/")
            .unwrap();
        both.create_config(&ConfigName::parse("home").unwrap(), "/home")
            .unwrap();
        let mut person = Person::answering(&[]);
        run(&both, &facts, &mut person, &options()).unwrap();
        assert!(
            person
                .said
                .contains("protected already, so there is nothing to set up"),
            "{}",
            person.said
        );
        let mut person = Person::answering(&[]);
        run(
            &both,
            &facts,
            &mut person,
            &Options {
                area: Some("/home".into()),
                ..options()
            },
        )
        .unwrap();
        assert!(
            person
                .said
                .contains("Home (/home) is already protected, so there is nothing to set up"),
            "{}",
            person.said
        );
    }

    #[test]
    fn the_window_is_offered_what_is_worth_protecting_best_first_and_a_hint_only_for_the_recommended(
    ) {
        let backend = machine();
        let offered = offers(&backend, &facts(true, Cleanup::On));
        let titles: Vec<&str> = offered.iter().map(|o| o.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Home"],
            "the logs are not worth offering and the system is protected"
        );
        assert!(offered[0].recommended && offered[0].summary.contains("Protect Home (/home)?"));
        let hint = hint_for(&offered).unwrap();
        assert_eq!(
            hint,
            "Home has no snapshots yet. Press p to see what that means and protect it."
        );
        assert_eq!(hint_for(&[]), None);
        let optional = snapall_tui::Offer {
            mount_point: "/srv".into(),
            title: "Served data".into(),
            summary: String::new(),
            recommended: false,
        };
        assert_eq!(
            hint_for(&[optional]),
            None,
            "nothing recommended is missing, so no nagging"
        );
    }
}
