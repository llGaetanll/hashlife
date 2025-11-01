use hashlife::parse_rle;

#[test]
fn test_patterns() -> anyhow::Result<()> {
    let pattern_dir = std::fs::read_dir("tests/rle_pats")?;
    let mut tested = 0;
    let mut failed = Vec::new();

    for entry in pattern_dir {
        let path = entry?.path();
        let bytes = std::fs::read(&path)?;

        match parse_rle::read_rle(&bytes, |_x, _y| {}) {
            Ok(_) => tested += 1,
            Err(e) => failed.push((path.clone(), e)),
        }
    }

    if !failed.is_empty() {
        for (path, err) in &failed {
            eprintln!("Failed to parse {:?}: {:#}", path, err);
        }

        panic!(
            "{}/{} patterns failed to parse",
            failed.len(),
            tested + failed.len()
        );
    }

    println!("Successfully parsed {} RLE patterns", tested);

    Ok(())
}
