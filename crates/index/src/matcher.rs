//! Query matching and ranking, shared by indexed and live search so both give the same
//! answers.

use memchr::memmem::Finder;

use crate::{Kind, KindFilter, Query, fold};

pub(crate) struct Matcher {
    /// Every needle a name must contain: the terms, then `.ext` if set.
    needles: Vec<Finder<'static>>,
    /// Index into `needles` of the longest one, which drives the arena scan.
    scan: usize,
    ext: Option<Vec<u8>>,
    kind: KindFilter,
    phrase: Vec<u8>,
    lead: Vec<u8>,
}

/// Whitespace-separated terms, `"quoted phrases"` kept whole, each folded.
fn terms(text: &str) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    for (i, part) in text.split('"').enumerate() {
        if i % 2 == 1 {
            out.push(fold::fold(part));
        } else {
            out.extend(part.split_whitespace().map(fold::fold));
        }
    }
    out.retain(|t| !t.is_empty());
    out
}

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b >= 0x80
}

impl Matcher {
    /// `None` if the query can't match anything (no terms and no extension, or a NUL).
    pub fn new(q: &Query) -> Option<Matcher> {
        let terms = if q.text.trim() == "*" { vec![Vec::new()] } else { terms(q.text) };
        let ext = q.ext.map(|e| [b".".as_slice(), &fold::fold(e.trim_start_matches('.'))].concat());
        let all: Vec<&Vec<u8>> = terms.iter().chain(ext.as_ref()).collect();
        if all.is_empty() || all.iter().any(|t| t.contains(&0)) {
            return None;
        }
        let scan = (0..all.len()).max_by_key(|&i| all[i].len()).unwrap();
        Some(Matcher {
            needles: all.iter().map(|t| Finder::new(t.as_slice()).into_owned()).collect(),
            scan,
            ext: ext.clone(),
            kind: q.kind,
            phrase: terms.join(&b' '),
            lead: terms.first().cloned().unwrap_or_default(),
        })
    }

    pub fn scan_finder(&self) -> &Finder<'static> {
        &self.needles[self.scan]
    }

    /// Kind, extension and every term, on a folded name. (Hidden-ness is the caller's job:
    /// it depends on where the search starts.)
    pub fn accepts(&self, name: &[u8], kind: Kind) -> bool {
        let ok_kind = match self.kind {
            KindFilter::Any => true,
            KindFilter::Files => kind != Kind::Dir,
            KindFilter::Dirs => kind == Kind::Dir,
        };
        if !ok_kind {
            return false;
        }
        if let Some(ext) = &self.ext {
            // A name that is only the extension (".pdf") is a dotfile, not a PDF.
            if name.len() <= ext.len() || !name.ends_with(ext) {
                return false;
            }
        }
        self.needles.iter().all(|f| f.find(name).is_some())
    }

    /// Rank bits for a folded name: match class in the top byte (exact name or stem, prefix,
    /// word start, anywhere), then name length. Callers put a tiebreak in the low 32 bits.
    pub fn rank(&self, name: &[u8]) -> u64 {
        let lead = self.lead.as_slice();
        let stem = memchr::memrchr(b'.', name).map_or(name, |p| &name[..p]);
        let class: u64 = if lead.is_empty() {
            3
        } else if name == self.phrase.as_slice() || stem == self.phrase.as_slice() {
            0
        } else if name.starts_with(lead) {
            1
        } else if memchr::memmem::find_iter(name, lead).any(|p| !is_word_byte(name[p - 1])) {
            2
        } else {
            3
        };
        class << 56 | (name.len().min(0xFF_FFFF) as u64) << 32
    }
}

/// Keep the `limit` smallest keys (unordered). Sorting is left to the caller.
pub(crate) fn keep_best<T: Ord>(v: &mut Vec<T>, limit: Option<usize>) {
    if let Some(k) = limit.filter(|&k| k < v.len()) {
        if k == 0 {
            v.clear();
        } else {
            v.select_nth_unstable(k - 1);
            v.truncate(k);
        }
    }
}
