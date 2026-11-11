//! Lookup derived exclusively from observed fst-print text and CLI behavior.
use std::collections::HashSet;
use std::ops::Range;

type Error = Box<dyn std::error::Error>;

pub struct Transducer<'a> {
    symbols: Vec<&'a str>,
    finals: &'a [u8],
    offsets: &'a [u8],
    arcs: &'a [u8],
    characters: Vec<(char, usize)>,
    multichar: Vec<(usize, &'a str)>,
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

fn take<'a>(bytes: &mut &'a [u8], n: usize) -> Result<&'a [u8], Error> {
    if n > bytes.len() {
        return Err("truncated transducer".into());
    }
    let (head, tail) = bytes.split_at(n);
    *bytes = tail;
    Ok(head)
}

impl<'a> Transducer<'a> {
    pub fn from_bytes(mut bytes: &'a [u8]) -> Result<Self, Error> {
        if take(&mut bytes, 8)? != b"MLMSPK01" {
            return Err("invalid header".into());
        }
        let counts = take(&mut bytes, 12)?;
        let states = u32_at(counts, 0) as usize;
        let arc_count = u32_at(counts, 4) as usize;
        let symbol_count = u32_at(counts, 8) as usize;
        if states == 0
            || symbol_count == 0
            || symbol_count > 65536
            || symbol_count > bytes.len() / 4
        {
            return Err("invalid counts".into());
        }
        let mut symbols = Vec::with_capacity(symbol_count);
        for _ in 0..symbol_count {
            let len = u32_at(take(&mut bytes, 4)?, 0) as usize;
            symbols.push(std::str::from_utf8(take(&mut bytes, len)?)?);
        }
        if !symbols[0].is_empty() || symbols[1..].contains(&"") {
            return Err("invalid epsilon".into());
        }
        let finals = take(&mut bytes, states.div_ceil(8))?;
        let offsets = take(
            &mut bytes,
            states
                .checked_add(1)
                .and_then(|v| v.checked_mul(4))
                .ok_or("offset overflow")?,
        )?;
        let arcs = take(&mut bytes, arc_count.checked_mul(8).ok_or("arc overflow")?)?;
        if !bytes.is_empty()
            || u32_at(offsets, 0) != 0
            || u32_at(offsets, states * 4) as usize != arc_count
        {
            return Err("invalid lengths".into());
        }
        let mut characters = Vec::with_capacity(symbol_count);
        let mut multichar = Vec::with_capacity(symbol_count);
        for (id, &printed) in symbols.iter().enumerate().skip(1) {
            // fst-print escapes colon; fst-infl matches its literal character.
            let input = if printed == r"\:" { ":" } else { printed };
            let mut chars = input.chars();
            let first = chars.next().ok_or("empty symbol")?;
            if chars.next().is_none() {
                characters.push((first, id));
            } else {
                multichar.push((id, input));
            }
        }
        characters.sort_unstable();
        if characters.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err("duplicate character symbol".into());
        }
        multichar.sort_unstable_by_key(|(_, symbol)| {
            (symbol.as_bytes()[0], std::cmp::Reverse(symbol.len()))
        });
        let result = Self {
            symbols,
            finals,
            offsets,
            arcs,
            characters,
            multichar,
        };
        for state in 0..states {
            let start = u32_at(offsets, state * 4) as usize;
            let end = u32_at(offsets, (state + 1) * 4) as usize;
            if start > end || end > arc_count {
                return Err("invalid offsets".into());
            }
            let mut previous = 0;
            for i in start..end {
                let (target, analysis, surface) = result.arc(i);
                if target >= states
                    || analysis >= symbol_count
                    || surface >= symbol_count
                    || surface < previous
                {
                    return Err("invalid arc".into());
                }
                previous = surface;
            }
        }
        Ok(result)
    }

    fn arc(&self, index: usize) -> (usize, usize, usize) {
        let bytes = &self.arcs[index * 8..index * 8 + 8];
        (
            u32_at(bytes, 0) as usize,
            u16::from_le_bytes([bytes[4], bytes[5]]) as usize,
            u16::from_le_bytes([bytes[6], bytes[7]]) as usize,
        )
    }

    pub fn analyse(&self, input: &str) -> Vec<String> {
        self.lookup(input, false)
    }
    pub fn generate(&self, input: &str) -> Vec<String> {
        self.lookup(input, true)
    }

    /// Embedded data plus retained heap buffers; excludes per-lookup scratch space.
    #[allow(dead_code)] // Used by the standalone benchmark, not the public wrapper.
    pub fn memory_bytes(&self) -> usize {
        20 + self.symbols.iter().map(|s| 4 + s.len()).sum::<usize>()
            + self.finals.len()
            + self.offsets.len()
            + self.arcs.len()
            + self.symbols.capacity() * std::mem::size_of::<&str>()
            + self.characters.capacity() * std::mem::size_of::<(char, usize)>()
            + self.multichar.capacity() * std::mem::size_of::<(usize, &str)>()
            + std::mem::size_of::<Self>()
    }

    fn lookup(&self, input: &str, generate: bool) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut remaining = input;
        while !remaining.is_empty() {
            let first = remaining.as_bytes()[0];
            let start = self
                .multichar
                .partition_point(|(_, s)| s.as_bytes()[0] < first);
            if let Some(&(id, symbol)) = self.multichar[start..]
                .iter()
                .take_while(|(_, s)| s.as_bytes()[0] == first)
                .find(|(_, s)| remaining.starts_with(s))
            {
                tokens.push(id);
                remaining = &remaining[symbol.len()..];
            } else {
                let ch = remaining.chars().next().unwrap();
                let Ok(index) = self.characters.binary_search_by_key(&ch, |&(ch, _)| ch) else {
                    return Vec::new();
                };
                tokens.push(self.characters[index].1);
                remaining = &remaining[ch.len_utf8()..];
            }
        }
        let mut results = HashSet::new();
        let mut output = String::new();
        let mut stack = vec![self.frame(0, 0, 0, &tokens, generate)];
        if tokens.is_empty() && self.is_final(0) {
            results.insert(String::new());
        }
        while let Some(frame) = stack.last_mut() {
            let next = frame.ranges[0].next().or_else(|| frame.ranges[1].next());
            let Some(index) = next else {
                output.truncate(frame.restore_len);
                stack.pop();
                continue;
            };
            let pos = frame.pos;
            let (target, analysis, surface) = self.arc(index);
            let (matched, emitted) = if generate {
                (analysis, surface)
            } else {
                (surface, analysis)
            };
            if matched != 0 && tokens.get(pos) != Some(&matched) {
                continue;
            }
            // Only epsilon arcs can revisit a state at the same input position.
            // Keep this check path-local so reconverging paths retain every output.
            if matched == 0
                && stack
                    .iter()
                    .rev()
                    .take_while(|f| f.pos == pos)
                    .any(|f| f.state == target)
            {
                continue;
            }
            let restore_len = output.len();
            output.push_str(self.symbols[emitted]);
            let pos = pos + usize::from(matched != 0);
            if pos == tokens.len() && self.is_final(target) {
                results.insert(output.clone());
            }
            stack.push(self.frame(target, pos, restore_len, &tokens, generate));
        }
        let mut results: Vec<_> = results.into_iter().collect();
        results.sort_unstable();
        results
    }

    fn is_final(&self, state: usize) -> bool {
        self.finals[state / 8] & (1 << (state % 8)) != 0
    }

    fn frame(
        &self,
        state: usize,
        pos: usize,
        restore_len: usize,
        tokens: &[usize],
        generate: bool,
    ) -> Frame {
        let start = u32_at(self.offsets, state * 4) as usize;
        let end = u32_at(self.offsets, (state + 1) * 4) as usize;
        let ranges = if generate {
            [start..end, end..end]
        } else {
            let epsilon_end = self.bound(start, end, 1);
            let matched = if let Some(&symbol) = tokens.get(pos) {
                self.bound(epsilon_end, end, symbol)..self.bound(epsilon_end, end, symbol + 1)
            } else {
                end..end
            };
            [start..epsilon_end, matched]
        };
        Frame {
            state,
            pos,
            ranges,
            restore_len,
        }
    }

    fn bound(&self, mut start: usize, mut end: usize, symbol: usize) -> usize {
        while start < end {
            let mid = start + (end - start) / 2;
            if self.arc(mid).2 < symbol {
                start = mid + 1;
            } else {
                end = mid;
            }
        }
        start
    }
}

struct Frame {
    state: usize,
    pos: usize,
    ranges: [Range<usize>; 2],
    restore_len: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Vec<u8> {
        let mut bytes = b"MLMSPK01".to_vec();
        for count in [3u32, 4, 4] {
            bytes.extend(count.to_le_bytes());
        }
        for symbol in ["", "a", "b", "<n>"] {
            bytes.extend((symbol.len() as u32).to_le_bytes());
            bytes.extend(symbol.as_bytes());
        }
        bytes.push(4);
        for offset in [0u32, 2, 4, 4] {
            bytes.extend(offset.to_le_bytes());
        }
        // Two outputs reach state 1; an epsilon cycle must terminate.
        for (target, analysis, surface) in [(1u32, 1u16, 1u16), (1, 2, 1), (1, 0, 0), (2, 3, 0)] {
            bytes.extend(target.to_le_bytes());
            bytes.extend(analysis.to_le_bytes());
            bytes.extend(surface.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn lookup_preserves_outputs_and_terminates_cycles() {
        let bytes = fixture();
        let fst = Transducer::from_bytes(&bytes).unwrap();
        assert_eq!(fst.analyse("a"), ["a<n>", "b<n>"]);
        assert_eq!(fst.generate("b<n>"), ["a"]);
        assert!(fst.analyse("💥").is_empty());
        fn send_sync<T: Send + Sync>(_: &T) {}
        send_sync(&fst);
    }

    #[test]
    fn rejects_truncation_and_bad_indices() {
        let bytes = fixture();
        for len in 0..bytes.len() {
            assert!(Transducer::from_bytes(&bytes[..len]).is_err());
        }
        let mut invalid = bytes;
        let last = invalid.len() - 4;
        invalid[last..last + 2].copy_from_slice(&65535u16.to_le_bytes());
        assert!(Transducer::from_bytes(&invalid).is_err());
    }

    #[test]
    fn arbitrary_bytes_and_strings_do_not_panic() {
        // Deterministic generated properties; no unapproved test dependency.
        let mut seed = 0x5eed_u64;
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let valid = fixture();
        let fst = Transducer::from_bytes(&valid).unwrap();
        for _ in 0..10_000 {
            let len = (next() % 256) as usize;
            let bytes: Vec<u8> = (0..len).map(|_| next() as u8).collect();
            let _ = Transducer::from_bytes(&bytes);
            let mut mutated = valid.clone();
            let index = next() as usize % mutated.len();
            mutated[index] = next() as u8;
            if let Ok(parsed) = Transducer::from_bytes(&mutated) {
                let _ = parsed.analyse("a");
                let _ = parsed.generate("a<n>");
            }
            let text: String = (0..len)
                .map(|_| char::from_u32((next() % 0x110000) as u32).unwrap_or('\u{fffd}'))
                .collect();
            let _ = fst.analyse(&text);
            let _ = fst.generate(&text);
        }
    }

    #[test]
    fn long_inputs_use_heap_instead_of_call_stack() {
        let mut bytes = b"MLMSPK01".to_vec();
        for count in [1u32, 1, 2] {
            bytes.extend(count.to_le_bytes());
        }
        for symbol in ["", "a"] {
            bytes.extend((symbol.len() as u32).to_le_bytes());
            bytes.extend(symbol.as_bytes());
        }
        bytes.push(1);
        for value in [0u32, 1, 0] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend(1u16.to_le_bytes());
        bytes.extend(1u16.to_le_bytes());
        let fst = Transducer::from_bytes(&bytes).unwrap();
        let word = "a".repeat(100_000);
        assert_eq!(fst.analyse(&word), [word]);
    }
}
