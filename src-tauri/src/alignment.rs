use crate::domain::*;
use serde::Serialize;
#[derive(Serialize, Debug)]
pub struct Difference {
    pub expected_start: usize,
    pub expected_end: usize,
    pub observed_start: usize,
    pub observed_end: usize,
    pub expected: String,
    pub observed: String,
    pub ambiguous: bool,
}
pub fn differences(expected: &str, observed: &str) -> Result<Vec<Difference>> {
    let a: Vec<char> = normalize(expected).chars().collect();
    let b: Vec<char> = normalize(observed).chars().collect();
    if a.len() > 2000 || b.len() > 2000 {
        return Err("照合は2000文字以内です".into());
    }
    let n = b.len() + 1;
    let mut d = vec![0usize; (a.len() + 1) * n];
    for i in 0..=a.len() {
        d[i * n] = i;
    }
    for j in 0..=b.len() {
        d[j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            d[i * n + j] = (d[(i - 1) * n + j] + 1)
                .min(d[i * n + j - 1] + 1)
                .min(d[(i - 1) * n + j - 1] + usize::from(a[i - 1] != b[j - 1]));
        }
    }
    let (mut i, mut j) = (a.len(), b.len());
    let mut steps = Vec::new();
    let mut ambiguous = false;
    while i > 0 || j > 0 {
        let best = d[i * n + j];
        let diagonal =
            i > 0 && j > 0 && d[(i - 1) * n + j - 1] + usize::from(a[i - 1] != b[j - 1]) == best;
        let delete = i > 0 && d[(i - 1) * n + j] + 1 == best;
        let insert = j > 0 && d[i * n + j - 1] + 1 == best;
        ambiguous |= (diagonal as u8 + delete as u8 + insert as u8) > 1;
        let (pi, pj) = if diagonal {
            (i - 1, j - 1)
        } else if delete {
            (i - 1, j)
        } else {
            (i, j - 1)
        };
        let same = diagonal && a[i - 1] == b[j - 1];
        steps.push((pi, i, pj, j, same));
        i = pi;
        j = pj;
    }
    steps.reverse();
    let mut out: Vec<Difference> = vec![];
    let mut active = false;
    for (ai, ae, bi, be, same) in steps {
        if same {
            active = false;
            continue;
        }
        if active {
            let last = out.last_mut().unwrap();
            last.expected_end = ae;
            last.observed_end = be;
        } else {
            out.push(Difference {
                expected_start: ai,
                expected_end: ae,
                observed_start: bi,
                observed_end: be,
                expected: String::new(),
                observed: String::new(),
                ambiguous,
            });
            active = true;
        }
    }
    for row in &mut out {
        row.expected = a[row.expected_start..row.expected_end].iter().collect();
        row.observed = b[row.observed_start..row.observed_end].iter().collect();
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locates_reading_difference() {
        let d = differences("いちばへいきます", "しじょうへいきます").unwrap();
        assert!(!d.is_empty());
        assert!(d.iter().all(|r| r.expected_end <= 3 && r.observed_end <= 4));
        assert!(differences("イチバ。", "いちば").unwrap().is_empty());
    }
    #[test]
    fn repeated_alignment_is_ambiguous() {
        assert!(
            differences("ああ", "あ")
                .unwrap()
                .iter()
                .any(|r| r.ambiguous)
        );
    }
}
