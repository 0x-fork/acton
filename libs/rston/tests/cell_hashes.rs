use expect_test::expect;
use rston::boc::Boc;
use rston::cell::CellBuilder;

#[test]
fn sparse_level_mask_matches_node_hashes() -> Result<(), rston::error::Error> {
    let cell = Boc::decode(include_bytes!("fixtures/cell_hashes/sparse-level-mask.boc")).unwrap();
    assert_eq!(cell.level_mask().to_byte(), 0b010);

    // Rebuilding also exercises the builder's hash computation, not only BoC decoding.
    let rebuilt = CellBuilder::build_from(cell.as_slice()?)?;
    let hashes = (0..4)
        .map(|level| {
            assert_eq!(rebuilt.hash(level), cell.hash(level));
            format!("level {level}: {}", cell.hash(level))
        })
        .collect::<Vec<_>>()
        .join("\n");

    expect![[r#"
        level 0: 8d74e4b0817f68b2775e1d084a13b7e1c7fd30ab251f4a1394352227a622a2d0
        level 1: 8d74e4b0817f68b2775e1d084a13b7e1c7fd30ab251f4a1394352227a622a2d0
        level 2: 4e099ef87b52e04fee9febc47329511d0a9084db2bbe3441b2ee81f4f8077642
        level 3: 4e099ef87b52e04fee9febc47329511d0a9084db2bbe3441b2ee81f4f8077642"#]]
    .assert_eq(&hashes);

    Ok(())
}

#[test]
fn nested_merkle_block_matches_the_nodes_root_hash() {
    let boc = include_bytes!("fixtures/cell_hashes/nested-merkle-block.boc");
    let block = Boc::decode(boc).unwrap();
    assert!(block.level_mask().is_empty());

    // A sparse descendant changes higher-level hashes that enclosing Merkle
    // cells use, so the error propagates even to an ordinary level-zero root.
    let hashes = format!("file: {}\nroot: {}", Boc::file_hash(boc), block.repr_hash());
    expect![[r#"
        file: f4a4329fe5bfb4cdb097a09469656b1e35a316c3107ae73a6d2b6cd45c4f1ae6
        root: ebd9bcf0a55dacc9e73ab586cca32e2f2b8a82b20644187a1405020bf8fe1266"#]]
    .assert_eq(&hashes);
}
