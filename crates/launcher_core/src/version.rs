//! Versioni `MAGGIORE.MINORE.PATCH[-etichetta]` confrontabili.

use std::cmp::Ordering;

/// Versione di un rilascio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
  pub numbers: [u64; 3],
  /// Etichetta di pre-rilascio (`-dev`, `-rc1`): viene prima della versione senza etichetta.
  pub maybe_pre: Option<String>,
  pub text: String,
}

impl Version {
  /// Legge `1.2.3`, `v1.2.3`, `1.2`, `1.2.3-rc1`.
  pub fn parse(text: &str) -> Option<Self> {
    let t = text.trim().trim_start_matches('v');
    let (core, maybe_pre) = match t.split_once('-') {
      Some((c, p)) => (c, Some(p.to_owned())),
      None => (t, None),
    };
    let mut numbers = [0u64; 3];
    let parts: Vec<&str> = core.split('.').collect();
    if parts.is_empty() || parts.len() > 3 {
      return None;
    }
    for (slot, p) in numbers.iter_mut().zip(&parts) {
      *slot = p.parse().ok()?;
    }
    Some(Self { numbers, maybe_pre, text: t.to_owned() })
  }
}

impl PartialOrd for Version {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for Version {
  fn cmp(&self, other: &Self) -> Ordering {
    self.numbers.cmp(&other.numbers).then_with(|| match (&self.maybe_pre, &other.maybe_pre) {
      (None, None) => Ordering::Equal,
      (None, Some(_)) => Ordering::Greater,
      (Some(_), None) => Ordering::Less,
      (Some(a), Some(b)) => a.cmp(b),
    })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn ordering() {
    let v = |s| Version::parse(s).unwrap();
    assert!(v("0.2.0") > v("0.1.9"));
    assert!(v("1.0.0") > v("1.0.0-rc1"));
    assert!(v("v1.10.0") > v("1.9.3"));
    assert_eq!(v("1.2").numbers, [1, 2, 0]);
    assert!(Version::parse("a.b").is_none());
  }
}
