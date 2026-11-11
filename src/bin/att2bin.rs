//! Build tool based exclusively on observed fst-print text output.
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};

fn symbol_id(
    symbols: &mut Vec<String>,
    ids: &mut HashMap<String, u16>,
    s: &str,
) -> Result<u16, Box<dyn std::error::Error>> {
    if let Some(&id) = ids.get(s) {
        return Ok(id);
    }
    let id = u16::try_from(symbols.len())?;
    symbols.push(s.to_owned());
    ids.insert(s.to_owned(), id);
    Ok(id)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: att2bin input.att output.bin".into());
    }
    let mut symbols = vec![String::new()];
    let mut ids = HashMap::from([("<>".to_owned(), 0)]);
    let mut arcs = Vec::new();
    let mut finals = Vec::new();
    let mut states = 0u32;
    for (line_number, line) in BufReader::new(File::open(&args[1])?).lines().enumerate() {
        let line = line?;
        let fields: Vec<_> = line.split('\t').collect();
        let invalid = || {
            format!(
                "unsupported AT&T record on line {}: {line:?}",
                line_number + 1
            )
        };
        if fields.len() != 1 && fields.len() != 4 {
            return Err(invalid().into());
        }
        let source: u32 = fields[0].parse().map_err(|_| invalid())?;
        states = states.max(source.checked_add(1).ok_or("state overflow")?);
        if fields.len() == 1 {
            finals.push(source);
        } else {
            let target: u32 = fields[1].parse().map_err(|_| invalid())?;
            states = states.max(target.checked_add(1).ok_or("state overflow")?);
            let analysis = symbol_id(&mut symbols, &mut ids, fields[2])?;
            let surface = symbol_id(&mut symbols, &mut ids, fields[3])?;
            arcs.push((source, surface, analysis, target));
        }
    }
    if states == 0 {
        return Err("empty transducer".into());
    }
    arcs.sort_unstable();
    let mut offsets = vec![0u32; states as usize + 1];
    for &(source, _, _, _) in &arcs {
        offsets[source as usize + 1] += 1;
    }
    for i in 1..offsets.len() {
        offsets[i] = offsets[i]
            .checked_add(offsets[i - 1])
            .ok_or("too many arcs")?;
    }
    let mut final_bits = vec![0u8; (states as usize).div_ceil(8)];
    for &state in &finals {
        final_bits[state as usize / 8] |= 1 << (state % 8);
    }
    let mut output = BufWriter::new(File::create(&args[2])?);
    output.write_all(b"MLMSPK01")?;
    for value in [
        states,
        u32::try_from(arcs.len())?,
        u32::try_from(symbols.len())?,
    ] {
        output.write_all(&value.to_le_bytes())?;
    }
    for symbol in &symbols {
        output.write_all(&u32::try_from(symbol.len())?.to_le_bytes())?;
        output.write_all(symbol.as_bytes())?;
    }
    output.write_all(&final_bits)?;
    for offset in offsets {
        output.write_all(&offset.to_le_bytes())?;
    }
    for (_, surface, analysis, target) in &arcs {
        output.write_all(&target.to_le_bytes())?;
        output.write_all(&analysis.to_le_bytes())?;
        output.write_all(&surface.to_le_bytes())?;
    }
    output.flush()?;
    println!(
        "states={states} arcs={} symbols={} (including epsilon) finals={}",
        arcs.len(),
        symbols.len(),
        finals.len()
    );
    Ok(())
}
