#![doc = include_str!("../README.md")]

use std::{borrow::Cow, fmt::Display, result, str::FromStr};

use peg::str::LineCol;

#[derive(thiserror::Error, Debug)]
pub enum Error {
	#[error("parse: {0}")]
	Parse(#[from] peg::error::ParseError<LineCol>),
	#[error("repetition not found: ${0}")]
	RepetitionNotFoundId(usize),
	#[error("repetition not found: ${{{0}}}")]
	RepetitionNotFoundString(String),

	#[error("two groups matched:\n\t{0}\n\t{1}")]
	GroupConflict(Replacement, Replacement),
	#[error("no group matched:\n\t{}", .0.iter().map(|g| g.to_string()).collect::<Vec<_>>().join("\n\t"))]
	NoGroupMatched(Vec<Error>),
}
impl Error {
	fn tolerate_group_fail(&self) -> bool {
		matches!(
			self,
			Self::RepetitionNotFoundId(_)
				| Self::RepetitionNotFoundString(_)
				| Self::NoGroupMatched(_)
		)
	}
}
type Result<T, E = Error> = result::Result<T, E>;

#[derive(Debug, Clone)]
pub enum Part {
	Byte(u8),
	RepId(usize),
	RepName(String),
	Group(Vec<Replacement>),
}
impl Display for Part {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Part::Byte(v) => write!(f, "\\x{:02x?}", *v),
			Part::RepId(id) => write!(f, "${id}"),
			Part::RepName(name) => write!(f, "${{{name}}}"),
			Part::Group(g) => {
				write!(f, "(")?;
				for (i, g) in g.iter().enumerate() {
					if i != 0 {
						write!(f, "|")?;
					}
					write!(f, "{g}")?;
				}
				write!(f, ")")
			}
		}
	}
}

#[derive(Debug, Clone)]
pub struct Replacement(Vec<Part>);
impl Display for Replacement {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		for p in &self.0 {
			write!(f, "{p}")?;
		}
		Ok(())
	}
}

pub trait Capture {
	fn get(&self, idx: usize) -> Option<Cow<[u8]>>;
	fn name(&self, name: &str) -> Option<Cow<[u8]>>;
}

impl Replacement {
	pub fn build(&self, cap: &impl Capture) -> Result<Vec<u8>> {
		let mut out = Vec::new();
		for part in &self.0 {
			match part {
				Part::Byte(b) => out.push(*b),
				Part::RepId(i) => {
					let mat = cap.get(*i).ok_or(Error::RepetitionNotFoundId(*i))?;
					out.extend(mat.as_ref());
				}
				Part::RepName(name) => {
					let mat = cap
						.name(name)
						.ok_or_else(|| Error::RepetitionNotFoundString(name.clone()))?;
					out.extend(mat.as_ref());
				}
				Part::Group(g) => {
					let mut errors = Vec::new();
					let mut matched = None;
					for group in g {
						match group.build(cap) {
							Ok(v) => {
								if let Some(old) = matched.replace((v, group.clone())) {
									return Err(Error::GroupConflict(old.1, group.clone()));
								}
							}
							Err(e) if e.tolerate_group_fail() => errors.push(e),
							Err(e) => return Err(e),
						};
					}
					if let Some((matched, _)) = matched {
						out.extend(&matched);
					} else {
						return Err(Error::NoGroupMatched(errors));
					}
				}
			}
		}
		Ok(out)
	}
}

impl FromStr for Replacement {
	type Err = Error;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(replacement::replacement_root(s)?)
	}
}

peg::parser! {
pub grammar replacement() for str {
	rule hex_char() -> u8
	= v:['0'..='9'] {v as u8 - b'0'}
	/ v:['a'..='f'] {v as u8 - b'a' + 10}
	/ v:['A'..='F'] {v as u8 - b'A' + 10}
	pub rule replacement_root() -> Replacement
	= wse:quiet!{"(?x)"?} ws(wse.is_some()) v:replacement(wse.is_some()) ws(wse.is_some()) {v}
	rule replacement(wse:bool) -> Replacement
	=  v:replacement_part(wse)++ws(wse) {Replacement(v)}
	rule replacement_part(wse:bool) -> Part
	= quiet!{"\\\\" {Part::Byte(b'\\')}
	/ "\\ " {Part::Byte(b' ')}
	/ "\\x" a:hex_char() b:hex_char() {Part::Byte((a << 4) | b)}
	/ "\\" {? Err("<special character>")}} / expected!("<special character>")

	/ quiet!{"$$" {Part::Byte(b'$')}
	/ "$" v:$(['0'..='9']+) {? Ok(Part::RepId(usize::from_str(v).map_err(|_| "bad id")?))}
	/ "$<" v:$((!['}'][_])+) ">" {Part::RepName(v.to_owned())}
	/ "$" {? Err("<selector>")}} / expected!("<selector>")

	/ quiet!{"((" {Part::Byte(b'(')}
	/ "))" {Part::Byte(b')')}
	/ "(" ws(wse) groups:replacement(wse)**(ws(wse) "|" ws(wse)) ws(wse) ")" {Part::Group(groups)}
	/ "||" {Part::Byte(b'|')}
	/ "|" {? Err("<group>")}} / expected!("<group>")

	/ !['\\' | '$' | '(' | ')' | '|' | '#'] c:['\0'..='\x7f'] {Part::Byte(c as u8)}

	rule ws(wse: bool)
	= ws_(wse)*
	rule ws_(wse: bool)
	= "#" (!['\n'] [_])* ("\n" / ![_]) {? if wse {Ok(())} else {Err("<unexpected whitespace>")}}
	/ c:$(['\n' | ' ' | '\t']) {? if wse || c.is_empty() {Ok(())} else {Err("<unexpected whitespace>")}}
}
}
