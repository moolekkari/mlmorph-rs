//! Phase 1 lookup, derived from fst-print output, with no SFST runtime code.
use std::collections::HashSet;

type Error = Box<dyn std::error::Error>;

pub struct Transducer<'a> {
    symbols: Vec<&'a str>,
    finals: &'a [u8],
    offsets: &'a [u8],
    arcs: &'a [u8],
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
        let result = Self {
            symbols,
            finals,
            offsets,
            arcs,
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

    fn lookup(&self, input: &str, generate: bool) -> Vec<String> {
        let mut tokens = Vec::new();
        let mut remaining = input;
        while !remaining.is_empty() {
            let matched = self
                .symbols
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(_, s)| remaining.starts_with(**s))
                .max_by_key(|(_, s)| s.len());
            let Some((id, symbol)) = matched else {
                return Vec::new();
            };
            tokens.push(id);
            remaining = &remaining[symbol.len()..];
        }
        let mut results = HashSet::new();
        self.visit(
            0,
            0,
            &tokens,
            generate,
            &mut String::new(),
            &mut HashSet::new(),
            &mut results,
        );
        let mut results: Vec<_> = results.into_iter().collect();
        results.sort_unstable();
        results
    }

    // A path-local set preserves distinct outputs when paths reconverge.
    // shortcut: output-producing epsilon cycles are cut, revisit if the grammar contains such cycles.
    #[allow(clippy::too_many_arguments)]
    fn visit(
        &self,
        state: usize,
        pos: usize,
        tokens: &[usize],
        generate: bool,
        output: &mut String,
        path: &mut HashSet<(usize, usize)>,
        results: &mut HashSet<String>,
    ) {
        if !path.insert((state, pos)) {
            return;
        }
        if pos == tokens.len() && self.finals[state / 8] & (1 << (state % 8)) != 0 {
            results.insert(output.clone());
        }
        let start = u32_at(self.offsets, state * 4) as usize;
        let end = u32_at(self.offsets, (state + 1) * 4) as usize;
        for i in start..end {
            let (target, analysis, surface) = self.arc(i);
            let (matched, emitted) = if generate {
                (analysis, surface)
            } else {
                (surface, analysis)
            };
            if matched != 0 && tokens.get(pos) != Some(&matched) {
                continue;
            }
            let length = output.len();
            output.push_str(self.symbols[emitted]);
            self.visit(
                target,
                pos + usize::from(matched != 0),
                tokens,
                generate,
                output,
                path,
                results,
            );
            output.truncate(length);
        }
        path.remove(&(state, pos));
    }
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
}
