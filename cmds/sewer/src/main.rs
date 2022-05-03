#![doc = include_str!("../README.md")]

use std::{
	borrow::Cow,
	fs,
	io::{self, Write},
	path::PathBuf,
	process, result,
};

use clap::Parser;
use regex::bytes::{Captures, Regex};
use sewer_replacement::Replacement;
use tempfile::{NamedTempFile, PersistError};
mod patchfile;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("regex: {0}")]
	Regex(#[from] regex::Error),
	#[error("replacement: {0}")]
	Replace(#[from] sewer_replacement::Error),
	#[error("patchfile: {0}")]
	Patchfile(#[from] patchfile::Error),
	#[error("io: {0}")]
	Io(#[from] io::Error),
	#[error("persist: {0}")]
	Persist(#[from] PersistError),

	#[error("one or more rules failed")]
	OneOrMoreRulesReturnedErrors,

	#[error("source pattern not found, file is already patched?")]
	SourcePatternNotFound,
	#[error(
		"source pattern found multiple times, is this patch correct for this version of file?"
	)]
	MultipleSourcesFound,

	#[error("source match was {0} bytes, but result is {1}")]
	MismatchedLen(usize, usize),
}

type Result<T, E = Error> = result::Result<T, E>;

#[derive(Parser)]
struct Opts {
	/// File to patch
	file: PathBuf,

	/// Copy original file to specified path
	#[clap(long)]
	backup: Option<PathBuf>,

	#[clap(subcommand)]
	command: Command,

	/// Do not edit, implies verbose
	#[clap(long, short = 'n')]
	dry_run: bool,
	/// Print matches
	#[clap(long, short = 'v')]
	verbose: bool,
}
#[derive(Parser)]
pub enum Command {
	/// Replace single occurence
	Single {
		/// Search regex, see syntax description here:
		/// https://docs.rs/regex/latest/regex/#syntax
		find: Regex,
		/// Replacement pattern, see syntax description here:
		/// https://docs.rs/sewer-replacement/latest/sewer-replacement
		replace: Replacement,
	},

	/// Use patchfile
	PatchFile {
		/// Patch file
		file: PathBuf,
		/// Keep going if one of patches fails
		#[clap(long)]
		partial: bool,
	},
}

fn main() {
	match main_wrapped() {
		Ok(()) => {}
		Err(e) => {
			eprintln!("{e}");
			process::exit(1);
		}
	}
}

fn main_wrapped() -> Result<()> {
	let mut opts = Opts::parse();
	if opts.dry_run {
		opts.verbose = true;
	}

	let mut data = fs::read(&opts.file)?;

	let mut has_failed = false;
	let mut has_succeded = false;

	match opts.command {
		Command::Single { find, replace } => {
			replace_single(&mut data, find, replace, opts.verbose)?;
		}
		Command::PatchFile { file, partial } => {
			let input = fs::read_to_string(file)?;
			let rules = patchfile::parse(&input)?;

			for rule in rules {
				if opts.verbose {
					eprintln!("#{}", rule.name);
				}
				match replace_single(&mut data, rule.from, rule.to, opts.verbose) {
					Ok(()) => {
						has_succeded = true;
					}
					Err(e) if partial => {
						eprintln!("{e}");
						has_failed = true;
					}
					Err(e) => return Err(e),
				}
			}
		}
	}

	if !opts.dry_run && has_succeded {
		if let Some(backup) = opts.backup {
			fs::rename(&opts.file, backup)?;
		}

		let mut temp = NamedTempFile::new_in(
			opts.file
				.parent()
				.as_ref()
				.expect("we already read this file"),
		)?;
		temp.write_all(&data)?;
		temp.persist(opts.file)?;
	}

	if has_failed {
		return Err(Error::OneOrMoreRulesReturnedErrors);
	}

	Ok(())
}

struct RegexCapture<'t>(&'t Captures<'t>);
impl<'t> sewer_replacement::Capture for RegexCapture<'t> {
	fn get(&self, idx: usize) -> Option<Cow<[u8]>> {
		self.0.get(idx).map(|m| Cow::Borrowed(m.as_bytes()))
	}

	fn name(&self, name: &str) -> Option<Cow<[u8]>> {
		self.0.name(name).map(|m| Cow::Borrowed(m.as_bytes()))
	}
}

fn replace_single(data: &mut [u8], from: Regex, to: Replacement, verbose: bool) -> Result<()> {
	let (range, out) = {
		let cap = if let Some(m) = from.captures(data) {
			m
		} else {
			return Err(Error::SourcePatternNotFound);
		};

		let mat = cap.get(0).expect("full match always present");

		if from.find_at(data, mat.end()).is_some() {
			return Err(Error::MultipleSourcesFound);
		}

		let out = to.build(&RegexCapture(&cap))?;

		if verbose {
			eprintln!("@{}..{}", mat.start(), mat.end());
			eprint!("-");
			for i in mat.as_bytes() {
				eprint!("\\x{i:02x?}");
			}
			eprintln!();
			eprint!("+");
			for i in &out {
				eprint!("\\x{i:02x?}");
			}
			eprintln!();
		}

		if mat.as_bytes().len() != out.len() {
			return Err(Error::MismatchedLen(mat.as_bytes().len(), out.len()));
		}
		(mat.range(), out)
	};

	data[range].copy_from_slice(&out);

	Ok(())
}
