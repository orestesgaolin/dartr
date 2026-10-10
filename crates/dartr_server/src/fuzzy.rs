// Dart source: pkg/analyzer/lib/src/utilities/fuzzy_matcher.dart

//! Dart `FuzzyMatcher` (the `MatchStyle.TEXT` style): whether a candidate
//! matches a pattern (`score(candidate) >= 0`), as `workspace/symbol` uses
//! it.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CharRole {
    None,
    Separator,
    Tail,
    UcTail,
    Head,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum CharType {
    None,
    Punct,
    Lower,
    Upper,
}

const MAX_INPUT_SIZE: usize = 127;
const MAX_PATTERN_SIZE: usize = 63;
const MIN_SCORE: i64 = -10000;
const TYPES: &[u8] = b"00000000000000000000000000000000100000000000001122222222221000000333333333333333333333333330000002222222222222222222222222200000";

fn lower(units: &[u16]) -> Vec<u16> {
    String::from_utf16_lossy(units).to_lowercase().encode_utf16().collect()
}

/// Dart `FuzzyMatcher` with `MatchStyle.TEXT`.
pub struct FuzzyMatcher {
    pattern: Vec<u16>,
    pattern_lower: Vec<u16>,
    pattern_short: Vec<u16>,
    table: Vec<Vec<i64>>,
    matches_layer_offset: usize,
    candidate_roles: Vec<CharRole>,
    pattern_roles: Vec<CharRole>,
    case_sensitive: bool,
    last_candidate_len: usize,
}

fn fuzzy_map(s: &[u16], roles: &mut [CharRole]) {
    let mut prev = CharType::None;
    for i in 0..s.len() {
        let ch = s[i];
        let ty = if ch < 128 {
            match TYPES[ch as usize] {
                b'0' => CharType::None,
                b'1' => CharType::Punct,
                b'2' => CharType::Lower,
                _ => CharType::Upper,
            }
        } else {
            CharType::Lower
        };
        let mut role = CharRole::None;
        if ty == CharType::Lower {
            role = if prev <= CharType::Punct {
                CharRole::Head
            } else {
                CharRole::Tail
            };
        } else if ty == CharType::Upper {
            role = CharRole::Head;
            let next_lower = i + 1 < s.len() && s[i + 1] >= b'a' as u16 && s[i + 1] <= b'z' as u16;
            if prev == CharType::Upper && !next_lower {
                role = CharRole::UcTail;
            }
        }
        roles[i] = role;
        prev = ty;
    }
    let mut i = s.len();
    while i > 0 && roles[i - 1] == CharRole::Separator {
        roles[i - 1] = CharRole::None;
        i -= 1;
    }
}

impl FuzzyMatcher {
    pub fn new(pattern: &str) -> Self {
        let mut pattern: Vec<u16> = pattern.encode_utf16().collect();
        pattern.truncate(MAX_PATTERN_SIZE);
        let pattern_lower = lower(&pattern);
        let case_sensitive = pattern != pattern_lower;
        let pattern_short = pattern_lower.iter().copied().take(3).collect();
        let matches_layer_offset = pattern.len() + 1;
        let table = vec![vec![0i64; 2 * matches_layer_offset]; MAX_INPUT_SIZE + 1];
        let mut pattern_roles = vec![CharRole::None; pattern.len()];
        fuzzy_map(&pattern, &mut pattern_roles);
        FuzzyMatcher {
            pattern,
            pattern_lower,
            pattern_short,
            table,
            matches_layer_offset,
            candidate_roles: vec![CharRole::None; MAX_INPUT_SIZE],
            pattern_roles,
            case_sensitive,
            last_candidate_len: 0,
        }
    }

    fn score_at(&self, i: usize, j: usize, k: usize) -> i64 {
        self.table[i][j + k * self.matches_layer_offset] >> 1
    }

    fn best_layer_index_at(&self, i: usize, j: usize) -> usize {
        if self.score_at(i, j, 0) < self.score_at(i, j, 1) { 1 } else { 0 }
    }

    fn prev_k(&self, i: usize, j: usize, k: usize) -> usize {
        (self.table[i][j + k * self.matches_layer_offset] & 1) as usize
    }

    fn compute_score(&mut self, candidate: &[u16], candidate_lower: &[u16]) -> i64 {
        let plen = self.pattern.len();
        let mlo = self.matches_layer_offset;
        for j in 0..=plen {
            self.table[0][j] = if j == 0 { 0 } else { MIN_SCORE << 1 };
            self.table[0][mlo + j] = MIN_SCORE << 1;
        }
        let mut segments_left = 1;
        let mut last_segment_start = 0usize;
        for i in 0..candidate.len() {
            if self.candidate_roles[i] == CharRole::Separator {
                segments_left += 1;
                last_segment_start = i + 1;
            }
        }
        for i in 1..=candidate.len() {
            let is_head = self.candidate_roles[i - 1] == CharRole::Head;
            if self.candidate_roles[i - 1] == CharRole::Separator && segments_left > 1 {
                segments_left -= 1;
            }
            let segment_score: i64 = if segments_left > 1 { 0 } else { 1 };
            let mut skip_penalty = 0;
            if i - 1 == last_segment_start {
                skip_penalty += 3;
            }
            for j in 0..=plen {
                let mj = mlo + j;
                self.table[i][mj] = MIN_SCORE << 1;
                if segments_left > 1 && j == plen {
                    self.table[i][j] = MIN_SCORE << 1;
                    continue;
                }
                let k = self.best_layer_index_at(i - 1, j);
                let mut skip_score = self.score_at(i - 1, j, k);
                if j != plen {
                    skip_score -= skip_penalty;
                }
                self.table[i][j] = (skip_score << 1) + k as i64;
                if j == 0 || candidate_lower.get(i - 1) != self.pattern_lower.get(j - 1) {
                    continue;
                }
                let mut char_score = segment_score;
                if self.candidate_roles[i - 1] == CharRole::Tail && self.pattern_roles[j - 1] == CharRole::Head {
                    if j > 1 {
                        continue;
                    }
                    let end = candidate_lower.len().min(i - 1 + self.pattern_short.len());
                    if self.pattern_short[..] != candidate_lower[i - 1..end] {
                        continue;
                    }
                    char_score -= 4;
                }
                if candidate[i - 1] == self.pattern[j - 1]
                    || (is_head && (!self.case_sensitive || self.pattern_roles[j - 1] == CharRole::Head))
                {
                    char_score += 1;
                }
                for k in 0..2 {
                    let mut score = self.score_at(i - 1, j - 1, k) + char_score;
                    let prev_matches = k == 1;
                    let consecutive = prev_matches || i - 1 == 0 || i - 1 == last_segment_start;
                    if consecutive || j - 1 == 0 {
                        score += 4;
                    }
                    if !prev_matches
                        && (self.candidate_roles[i - 1] == CharRole::Tail
                            || self.candidate_roles[i - 1] == CharRole::UcTail)
                    {
                        score -= 3;
                    }
                    if score > (self.table[i][mj] >> 1) {
                        self.table[i][mj] = (score << 1) + k as i64;
                    }
                }
            }
        }
        let best = self.best_layer_index_at(candidate.len(), plen);
        self.score_at(candidate.len(), plen, best)
    }

    fn is_poor_match(&self) -> bool {
        if self.pattern.len() < 2 {
            return false;
        }
        let mut i = self.last_candidate_len;
        let mut j = self.pattern.len();
        let mut k = self.best_layer_index_at(i, j);
        let mut counter = 0;
        let mut len = 0;
        while i > 0 {
            let take = k == 1;
            k = self.prev_k(i, j, k);
            if take {
                len += 1;
                if k == 0 && len < 3 && self.candidate_roles[i - 1] == CharRole::Tail {
                    counter += 1;
                    if counter > 1 {
                        return true;
                    }
                }
                j -= 1;
            } else {
                len = 0;
            }
            i -= 1;
        }
        false
    }

    fn matches(&mut self, candidate: &[u16], candidate_lower: &[u16]) -> bool {
        let mut i = 0;
        let mut j = 0;
        while i < candidate_lower.len() && j < self.pattern_lower.len() {
            if candidate_lower[i] == self.pattern_lower[j] {
                j += 1;
            }
            i += 1;
        }
        if j != self.pattern_lower.len() {
            return false;
        }
        let mut roles = std::mem::take(&mut self.candidate_roles);
        fuzzy_map(candidate, &mut roles);
        self.candidate_roles = roles;
        true
    }

    /// Dart `score(candidate)`: `-1.0` when the candidate does not match.
    pub fn score(&mut self, candidate: &str) -> f64 {
        let mut candidate: Vec<u16> = candidate.encode_utf16().collect();
        candidate.truncate(MAX_INPUT_SIZE);
        if self.pattern.is_empty() {
            return 1.0;
        }
        self.last_candidate_len = candidate.len();
        let candidate_lower = lower(&candidate);
        if candidate_lower.len() != candidate.len() {
            return -1.0;
        }
        if self.matches(&candidate, &candidate_lower) {
            let score = self.compute_score(&candidate, &candidate_lower);
            if (score as f64) > (MIN_SCORE as f64) / 2.0 && !self.is_poor_match() {
                if self.pattern.len() == candidate.len() {
                    return 1.0;
                }
                let score = score.max(0) as f64 / (6.0 * self.pattern.len() as f64);
                return score.min(1.0);
            }
        }
        -1.0
    }
}
