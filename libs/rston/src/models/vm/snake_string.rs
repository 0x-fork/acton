//! This module contains functionality for working with snake strings.
//!
//! Since TVM doesn't have a separate string format and data is stored in cells
//! of up to 1023 bits (~127 bytes) and up to 4 references to other cells, we have to split strings
//! into chunks and store them as a linked list of cells.
//!
//! To allow for potential prefixes (e.g. 8-bit prefix), we strictly use up to 126 bytes (1008 bits) per cell,
//! leaving at least 15 bits free in each cell.
//!
//! For example, a string of 300 characters will be stored as:
//! ```text
//! cell("first 126 bytes")
//!     cell("second 126 bytes")
//!         cell("remaining 48 bytes")
//! ```
use crate::cell::{Cell, CellBuilder, CellSlice};

pub(super) fn build_snake_bytes_cell(bytes: &[u8]) -> Cell {
    let total_bits = bytes.len() * 8;

    // Keep some free space in each cell for potential prefixes.
    if total_bits <= 1015 {
        let mut builder = CellBuilder::new();
        builder.store_raw(bytes, total_bits as u16).ok();
        return builder.build().unwrap();
    }

    let mut remaining_bytes = bytes;
    let mut cell_data = Vec::new();

    while !remaining_bytes.is_empty() {
        let chunk_size = std::cmp::min(remaining_bytes.len(), 126);
        let chunk = &remaining_bytes[..chunk_size];
        cell_data.push((chunk, chunk.len() * 8));
        remaining_bytes = &remaining_bytes[chunk_size..];
    }

    let mut next_cell: Option<Cell> = None;
    for (chunk, bits) in cell_data.into_iter().rev() {
        let mut builder = CellBuilder::new();
        builder.store_raw(chunk, bits as u16).ok();

        if let Some(next) = next_cell {
            builder.store_reference(next).ok();
        }

        next_cell = Some(builder.build().unwrap());
    }

    next_cell.unwrap()
}

pub(super) fn parse_snake_bytes_cell(cell: &Cell) -> Option<Vec<u8>> {
    let mut parser = cell.as_slice_allow_exotic();
    parse_snake_bytes_from_slice(&mut parser)
}

pub(super) fn parse_snake_bytes_from_slice(parser: &mut CellSlice<'_>) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    let bits_to_load = parser.size_bits();
    if !bits_to_load.is_multiple_of(8) {
        return None;
    }

    let mut chunk = vec![0u8; bits_to_load.div_ceil(8) as usize];
    parser.load_raw(&mut chunk, bits_to_load).ok()?;
    bytes.extend_from_slice(&chunk);

    if parser.size_refs() == 0 {
        return Some(bytes);
    }

    let mut next_data_ref = parser.load_reference_cloned().ok()?;
    loop {
        let mut parser = next_data_ref.as_slice_allow_exotic();
        let bits_to_load = parser.size_bits();
        if !bits_to_load.is_multiple_of(8) {
            return None;
        }

        let mut chunk = vec![0u8; bits_to_load.div_ceil(8) as usize];
        parser.load_raw(&mut chunk, bits_to_load).ok()?;
        bytes.extend_from_slice(&chunk);

        if parser.size_refs() == 0 {
            break;
        }

        next_data_ref = match parser.load_reference_cloned() {
            Ok(cell) => cell,
            Err(_) => break,
        };
    }

    Some(bytes)
}

pub(super) fn parse_snake_string_from_slice(parser: &mut CellSlice<'_>) -> Option<String> {
    String::from_utf8(parse_snake_bytes_from_slice(parser)?).ok()
}

#[cfg(test)]
mod tests {
    use super::super::stack::{Tuple, TupleItem};
    use super::*;

    #[test]
    fn test_string_roundtrip() {
        let small_string = "Hello World";
        let mut tuple = Tuple::empty();
        tuple.push_string_slice(small_string);
        let serialized = CellBuilder::build_from(&tuple).unwrap();
        let deserialized = Tuple::deserialize(&serialized).unwrap();
        assert_eq!(tuple, deserialized);

        let large_string = "A".repeat(200); // 200 bytes = 1600 bits > 1023
        let mut tuple = Tuple::empty();
        tuple.push_string_slice(&large_string);
        let serialized = CellBuilder::build_from(&tuple).unwrap();
        let deserialized = Tuple::deserialize(&serialized).unwrap();
        assert_eq!(tuple, deserialized);
    }

    #[test]
    fn test_empty_string() {
        let empty_string = "";
        let mut tuple = Tuple::empty();
        tuple.push_string_slice(empty_string);
        let serialized = CellBuilder::build_from(&tuple).unwrap();
        let deserialized = Tuple::deserialize(&serialized).unwrap();
        assert_eq!(tuple, deserialized);

        if let Some(TupleItem::Slice(slice)) = tuple.0.first() {
            let parsed = Tuple::parse_snake_string(slice);
            assert_eq!(parsed, Some(empty_string.to_string()));
        } else {
            panic!("Expected slice item");
        }
    }

    #[test]
    fn test_push_tolk_string_uses_cell_stack_item() {
        let mut tuple = Tuple::empty();
        tuple.push_string("Hello World");

        if let Some(TupleItem::Cell(cell)) = tuple.0.first() {
            let parsed = Tuple::parse_snake_string(cell);
            assert_eq!(parsed, Some("Hello World".to_string()));
        } else {
            panic!("Expected cell item");
        }
    }

    #[test]
    fn test_boundary_sizes() {
        let test_cases = vec![
            ("a".to_string(), 1),   // 1 byte
            ("a".repeat(126), 126), // 126 bytes (fits in one cell)
            ("a".repeat(127), 127), // 127 bytes (requires two cells)
            ("a".repeat(128), 128), // 128 bytes (requires two cells)
            ("a".repeat(252), 252), // 252 bytes (two full cells: 126 * 2)
            ("a".repeat(253), 253), // 253 bytes (requires three cells)
            ("a".repeat(378), 378), // 378 bytes (three full cells: 126 * 3)
        ];

        for (test_string, expected_len) in test_cases {
            assert_eq!(test_string.len(), expected_len);

            let mut tuple = Tuple::empty();
            tuple.push_string_slice(&test_string);
            let serialized = CellBuilder::build_from(&tuple).unwrap();
            let deserialized = Tuple::deserialize(&serialized).unwrap();
            assert_eq!(tuple, deserialized);

            if let Some(TupleItem::Slice(slice)) = tuple.0.first() {
                let parsed = Tuple::parse_snake_string(slice);
                assert_eq!(parsed, Some(test_string.clone()));
            } else {
                panic!("Expected slice item for string of length {expected_len}");
            }
        }
    }

    #[test]
    fn test_utf8_strings() {
        let test_cases = vec![
            "Hello 世界".to_string(),                  // Mixed ASCII and Chinese
            "🚀 Rocket".to_string(),                   // Emoji
            "αβγδε".to_string(),                       // Greek letters
            "café".to_string(),                        // Accented characters
            "русский текст".to_string(),               // Cyrillic
            ("a".repeat(50) + "🚀" + &"b".repeat(50)), // Emoji in middle
        ];

        for test_string in test_cases {
            let mut tuple = Tuple::empty();
            tuple.push_string_slice(&test_string);
            let serialized = CellBuilder::build_from(&tuple).unwrap();
            let deserialized = Tuple::deserialize(&serialized).unwrap();
            assert_eq!(tuple, deserialized);

            if let Some(TupleItem::Slice(slice)) = tuple.0.first() {
                let parsed = Tuple::parse_snake_string(slice);
                assert_eq!(parsed, Some(test_string));
            } else {
                panic!("Expected slice item");
            }
        }
    }

    #[test]
    fn test_parse_snake_bytes() {
        let test_bytes = vec![0x00, 0x01, 0xFF, 0x42, 0x80, 0x7F];

        let mut tuple = Tuple::empty();
        tuple.push_bytes(&test_bytes);

        if let Some(TupleItem::Slice(slice)) = tuple.0.first() {
            let parsed = Tuple::parse_snake_bytes(slice);
            assert_eq!(parsed, Some(test_bytes));
        } else {
            panic!("Expected slice item");
        }
    }

    #[test]
    fn test_invalid_utf8_parse_snake_string() {
        let invalid_utf8_bytes = vec![0xFF, 0xFE, 0xFD];

        let mut tuple = Tuple::empty();
        tuple.push_bytes(&invalid_utf8_bytes);

        if let Some(TupleItem::Slice(slice)) = tuple.0.first() {
            let parsed = Tuple::parse_snake_string(slice);
            assert_eq!(parsed, None); // Should fail UTF-8 conversion

            let parsed_bytes = Tuple::parse_snake_bytes(slice);
            assert_eq!(parsed_bytes, Some(invalid_utf8_bytes)); // But bytes should work
        } else {
            panic!("Expected slice item");
        }
    }

    #[test]
    fn test_non_byte_aligned_data() {
        // Create a cell with non-byte-aligned bits (e.g., 7 bits)
        let mut builder = CellBuilder::new();
        builder.store_small_uint(0b1010101, 7).unwrap();
        let cell = builder.build().unwrap();

        let parsed = Tuple::parse_snake_string(&cell);
        assert_eq!(parsed, None); // Should fail due to non-byte-aligned data

        let parsed_bytes = Tuple::parse_snake_bytes(&cell);
        assert_eq!(parsed_bytes, None);
    }

    #[test]
    fn test_exact_cell_capacity() {
        // Test exactly 126 bytes (1008 bits) - should fit in one cell
        let test_string = "a".repeat(126);
        let mut tuple = Tuple::empty();
        tuple.push_string_slice(&test_string);

        if let Some(TupleItem::Slice(slice)) = tuple.0.first() {
            assert_eq!(slice.bit_len(), 1008);
            assert_eq!(slice.reference_count(), 0);

            let parsed = Tuple::parse_snake_string(slice);
            assert_eq!(parsed, Some(test_string));
        } else {
            panic!("Expected slice item");
        }
    }

    #[test]
    fn test_over_cell_capacity() {
        // Test 127 bytes (1016 bits) - should require two cells
        let test_string = "a".repeat(127);
        let mut tuple = Tuple::empty();
        tuple.push_string_slice(&test_string);

        if let Some(TupleItem::Slice(slice)) = tuple.0.first() {
            assert_eq!(slice.bit_len(), 1008); // First cell has 126 bytes
            assert_eq!(slice.reference_count(), 1); // Has reference to second cell

            let second_cell = slice.references().next().unwrap();
            assert_eq!(second_cell.bit_len(), 8); // Second cell has 1 byte
            assert_eq!(second_cell.reference_count(), 0);

            let parsed = Tuple::parse_snake_string(slice);
            assert_eq!(parsed, Some(test_string));
        } else {
            panic!("Expected slice item");
        }
    }

    #[test]
    fn test_very_large_string() {
        let large_string = "x".repeat(10000); // ~79 cells needed
        let mut tuple = Tuple::empty();
        tuple.push_string_slice(&large_string);

        let serialized = CellBuilder::build_from(&tuple).unwrap();
        let deserialized = Tuple::deserialize(&serialized).unwrap();
        assert_eq!(tuple, deserialized);

        if let Some(TupleItem::Slice(slice)) = tuple.0.first() {
            let parsed = Tuple::parse_snake_string(slice);
            assert_eq!(parsed, Some(large_string));
        } else {
            panic!("Expected slice item");
        }
    }

    #[test]
    fn test_empty_bytes() {
        let empty_bytes = vec![];
        let mut tuple = Tuple::empty();
        tuple.push_bytes(&empty_bytes);

        if let Some(TupleItem::Slice(slice)) = tuple.0.first() {
            let parsed = Tuple::parse_snake_bytes(slice);
            assert_eq!(parsed, Some(empty_bytes));
        } else {
            panic!("Expected slice item");
        }
    }
}
