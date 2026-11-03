// SPDX-FileCopyrightText: 2026 grenudi
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Asking before a change: a yes for what can be undone, the word typed for what cannot be, and the end of a command that was not told to go ahead.

use std::error::Error;
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;

use snapall_core::{Act, Weight};

/// Whether there is a terminal to ask on.
pub fn can_ask() -> bool {
    io::stdin().is_terminal()
}

/// A command that ends early because it was not told to go ahead: the story stops at the `?` after the question.
#[derive(Debug)]
pub struct Stopped(ExitCode);

impl std::fmt::Display for Stopped {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(out, "stopped before anything was changed")
    }
}

impl Error for Stopped {}

/// The exit code of a command that was stopped, or nothing if it ended some other way.
pub fn how_it_ended(error: &(dyn Error + 'static)) -> Option<ExitCode> {
    error.downcast_ref::<Stopped>().map(|stopped| stopped.0)
}

/// What came of asking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Go,
    Declined,
    /// It needed an answer and there is nobody to type one.
    Nobody,
}

/// What the person already agreed to on the command line. `--yes` answers a question about a change, and only
/// `--destroy` answers one about what is lost for good: which of the two counts is decided by the weight of the act
/// being asked about, here, and not by each command.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Agreed {
    yes: bool,
    destroy: bool,
}

impl Agreed {
    /// A command that offers both `--yes` and `--destroy`.
    pub fn to(yes: bool, destroy: bool) -> Agreed {
        Agreed { yes, destroy }
    }

    /// A command that offers only `--yes`: it changes things, and never destroys what cannot be put back.
    pub fn by_yes(yes: bool) -> Agreed {
        Agreed::to(yes, false)
    }

    /// A command that offers only `--destroy`: what it does cannot be put back, so `--yes` would mean nothing.
    pub fn by_destroy(destroy: bool) -> Agreed {
        Agreed::to(false, destroy)
    }

    /// Whether this already answers a question about something of this weight.
    pub fn covers(self, weight: Weight) -> bool {
        match weight {
            Weight::Light => true,
            Weight::Changes => self.yes,
            Weight::Destroys => self.destroy,
        }
    }
}

/// Read one line from stdin.
fn read_line() -> Result<String, Box<dyn Error>> {
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().lock().read_line(&mut answer)?;
    Ok(answer)
}

/// Ask what the weight of the thing requires, and nothing less: `y` or `yes` for a change, `destroy` (and
/// the name, where there is one) for what loses something for good, never a bare Enter. Agreed already on the
/// command line, it is not asked again. The caller reports [`Decision::Nobody`] in its own words.
pub fn decide(
    weight: Weight,
    agreed: Agreed,
    question: &str,
    target: Option<&str>,
) -> Result<Decision, Box<dyn Error>> {
    if !weight.asks() || agreed.covers(weight) {
        return Ok(Decision::Go);
    }
    if !can_ask() {
        return Ok(Decision::Nobody);
    }
    eprint!("{question}\n{}: ", weight.instruction(target));
    Ok(if weight.accepts(&read_line()?, target) {
        Decision::Go
    } else {
        Decision::Declined
    })
}

/// Leave to make a change, and proof that the person was asked. Asking is the only way to get it (an answer, a flag
/// that already said it, or an act that asks nothing), and a change cannot begin without it, so a change cannot be
/// made unasked. It is used up by the change it begins: one leave, one change.
#[derive(Debug)]
#[must_use = "a change cannot begin without the leave that asking gives"]
pub struct Approved(());

/// For an act that weighs nothing and so asks nothing: taking a snapshot is a safety net, not a risk.
pub fn nothing_to_ask(act: Act) -> Result<Approved, Box<dyn Error>> {
    if act.weight().asks() {
        return Err(format!("{} needs an answer first, and was not asked", act.title()).into());
    }
    Ok(Approved(()))
}

/// What is not there cannot be lost: the lookup that found nothing is the proof, so there is nothing to ask. The
/// attempt still goes ahead, so that its failure is recorded.
pub fn nothing_to_lose<T>(act: Act, found: &Option<T>) -> Result<Approved, Box<dyn Error>> {
    if found.is_some() {
        return Err(format!("{} would lose something, and was not asked", act.title()).into());
    }
    Ok(Approved(()))
}

pub fn before(act: Act, agreed: Agreed, question: &str) -> Result<Approved, Box<dyn Error>> {
    gate(act, agreed, question, None)
}

/// [`before`], after showing what is about to happen: only when there is something to answer, and also when there
/// is nobody to answer, so that "read the above" is true.
pub fn before_showing(
    act: Act,
    agreed: Agreed,
    preview: impl FnOnce() -> String,
    question: &str,
) -> Result<Approved, Box<dyn Error>> {
    approval(act, agreed, Some(preview), question, None)
}

/// [`decide`], and if the answer is not a go, what to say, and the story ends there: the caller writes `?`.
pub fn gate(
    act: Act,
    agreed: Agreed,
    question: &str,
    target: Option<&str>,
) -> Result<Approved, Box<dyn Error>> {
    approval(act, agreed, None::<fn() -> String>, question, target)
}

fn approval(
    act: Act,
    agreed: Agreed,
    preview: Option<impl FnOnce() -> String>,
    question: &str,
    target: Option<&str>,
) -> Result<Approved, Box<dyn Error>> {
    let weight = act.weight();
    if let (true, Some(preview)) = (weight.asks() && !agreed.covers(weight), preview) {
        eprintln!("{}", preview());
    }
    let leaving = match decide(weight, agreed, question, target)? {
        Decision::Go => return Ok(Approved(())),
        Decision::Declined => {
            eprintln!("snapall: nothing changed");
            ExitCode::FAILURE
        }
        Decision::Nobody => {
            let flag = weight.script_flag().unwrap_or("--yes");
            eprintln!("snapall: this needs an answer and there is no terminal to ask on: read the above, then pass {flag}");
            ExitCode::from(2)
        }
    };
    Err(Box::new(Stopped(leaving)))
}

/// A yes or no for a change, typed: only `y` or `yes` is a yes, and Enter alone is a no. For callers that
/// have already checked [`can_ask`].
pub fn yes_no(question: &str) -> Result<bool, Box<dyn Error>> {
    eprint!("{question}\n{}: ", Weight::Changes.instruction(None));
    Ok(Weight::Changes.accepts(&read_line()?, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_act_that_weighs_nothing_needs_no_answer_and_every_other_is_refused_without_one() {
        for act in Act::EVERY {
            assert_eq!(nothing_to_ask(act).is_ok(), !act.weight().asks(), "{act:?}");
        }
    }

    #[test]
    fn what_is_not_there_cannot_be_lost_and_what_is_there_can() {
        assert!(nothing_to_lose::<u8>(Act::DeleteSnapshots, &None).is_ok());
        assert!(nothing_to_lose(Act::DeleteSnapshots, &Some(1)).is_err());
    }

    #[test]
    fn yes_answers_a_change_and_only_destroy_answers_a_loss() {
        let nothing = Agreed::default();
        assert!(
            nothing.covers(Weight::Light),
            "what weighs nothing asks nothing"
        );
        assert!(!nothing.covers(Weight::Changes));
        assert!(!nothing.covers(Weight::Destroys));
        let yes = Agreed::by_yes(true);
        assert!(yes.covers(Weight::Changes));
        assert!(
            !yes.covers(Weight::Destroys),
            "--yes cannot answer a question about what is lost for good"
        );
        let destroy = Agreed::by_destroy(true);
        assert!(destroy.covers(Weight::Destroys));
        assert!(
            !destroy.covers(Weight::Changes),
            "--destroy is not a yes to a plain change"
        );
    }

    #[test]
    fn a_flag_that_already_said_it_gives_leave_without_asking_anyone() {
        assert!(before(Act::DeleteSnapshots, Agreed::by_destroy(true), "Delete it?").is_ok());
        assert!(before(Act::Restore, Agreed::by_yes(true), "Restore it?").is_ok());
    }

    #[test]
    fn with_nobody_to_ask_and_nothing_said_the_command_stops_with_its_own_exit_code() {
        if can_ask() {
            return; // a person is at this terminal: asking would wait for them
        }
        let error = before(Act::Restore, Agreed::default(), "Restore it?").unwrap_err();
        assert_eq!(
            format!("{:?}", how_it_ended(error.as_ref())),
            format!("{:?}", Some(ExitCode::from(2)))
        );
        let other: Box<dyn Error> = "something else".into();
        assert!(
            how_it_ended(other.as_ref()).is_none(),
            "any other error is not a stop"
        );
    }
}
