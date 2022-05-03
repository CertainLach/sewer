use std::{result, str::FromStr};

use peg::str::LineCol;
use regex::bytes::Regex;

use sewer_replacement::{self, Replacement};

struct PRule {
	name: String,
	from: String,
	to: String,
}

pub struct Rule {
	pub name: String,
	pub from: Regex,
	pub to: Replacement,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("parse: {0}")]
	Parse(#[from] peg::error::ParseError<LineCol>),
	#[error("regex: {0}")]
	Regex(#[from] regex::Error),
	#[error("replacement: {0}")]
	Replacement(#[from] sewer_replacement::Error),
}
type Result<T, E = Error> = result::Result<T, E>;

pub fn parse(input: &str) -> Result<Vec<Rule>> {
	let prules = patchfile::root(input)?;
	let mut out = Vec::new();
	for prule in prules {
		out.push(Rule {
			name: prule.name,
			from: Regex::new(&prule.from)?,
			to: Replacement::from_str(&prule.to)?,
		});
	}
	Ok(out)
}

peg::parser! {
grammar patchfile() for str {
rule rest_of_line() -> &'input str
= v:$((!['\n'][_])*) {v}
rule prefixed(prefix: rule<()>) -> String
= v:(prefix() v:rest_of_line() {v})++_ {v.iter().map(|s| s.to_owned()).collect::<Vec<_>>().join("\n")}

pub(super) rule root() -> Vec<PRule>
= _ rules:prule()**_ _ {rules}

rule prule() -> PRule
= "#" name:rest_of_line() "\n"
_
from:prefixed(<"-">)
_
to:prefixed(<"+">)
{PRule {name: name.to_owned(), from, to}}

rule _
= [' ' | '\n' | '\t']*
}
}
