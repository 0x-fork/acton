#![cfg(feature = "models")]

use num_bigint::BigInt;
use rston::boc::Boc;
use rston::cell::CellBuilder;
use rston::models::vm::{ContData, Tuple, TupleItem};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    name: String,
    boc: String,
}

fn fixture_bytes(name: &str) -> Vec<u8> {
    use base64::Engine;

    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/tvm/stack.json")).unwrap();
    let fixture = fixtures
        .into_iter()
        .find(|fixture| fixture.name == name)
        .unwrap();
    base64::engine::general_purpose::STANDARD
        .decode(fixture.boc)
        .unwrap()
}

#[test]
fn empty_stack_keeps_its_existing_encoding() {
    let bytes = fixture_bytes("empty");

    assert_eq!(
        Boc::decode(&bytes).unwrap().parse::<Tuple>().unwrap(),
        Tuple::empty()
    );
    assert_eq!(
        Boc::encode(CellBuilder::build_from(Tuple::empty()).unwrap()),
        bytes
    );
}

#[test]
fn every_value_variant_keeps_its_existing_encoding_and_order() {
    let leaf = CellBuilder::build_from(0xab_u8).unwrap();
    let code = CellBuilder::build_from(0x76_u8).unwrap();
    let expected = Tuple(vec![
        TupleItem::Null,
        TupleItem::Int(42.into()),
        TupleItem::Int((-123).into()),
        TupleItem::Nan,
        TupleItem::Cell(leaf.clone()),
        TupleItem::Slice(leaf.clone()),
        TupleItem::Builder(leaf),
        TupleItem::Tuple(Tuple(vec![TupleItem::Null, TupleItem::Int(7.into())])),
        TupleItem::Cont(ContData {
            code,
            stack: Some(Tuple(vec![TupleItem::Int(9.into())])),
            savelist: None,
            nargs: Some(2),
        }),
    ]);
    let bytes = fixture_bytes("values");

    assert_eq!(
        Boc::decode(&bytes).unwrap().parse::<Tuple>().unwrap(),
        expected
    );
    assert_eq!(
        Boc::encode(CellBuilder::build_from(&expected).unwrap()),
        bytes
    );
}

#[test]
fn int257_limits_keep_their_existing_encoding() {
    let cases = [
        ("positive_int257", (BigInt::from(1_u8) << 256_usize) - 1),
        ("negative_int257", -(BigInt::from(1_u8) << 256_usize)),
    ];

    for (name, integer) in cases {
        let expected = Tuple(vec![TupleItem::Int(integer)]);
        let bytes = fixture_bytes(name);

        assert_eq!(
            Boc::decode(&bytes).unwrap().parse::<Tuple>().unwrap(),
            expected
        );
        assert_eq!(
            Boc::encode(CellBuilder::build_from(&expected).unwrap()),
            bytes
        );
    }
}

#[test]
fn snake_string_keeps_its_existing_cell_boundaries() {
    let mut expected = Tuple::empty();
    expected.push_string_slice(&"a".repeat(127));
    let bytes = fixture_bytes("snake_string");

    let decoded = Boc::decode(&bytes).unwrap().parse::<Tuple>().unwrap();

    assert_eq!(decoded, expected);
    assert_eq!(
        Boc::encode(CellBuilder::build_from(&expected).unwrap()),
        bytes
    );
}

#[test]
fn stack_traits_work_inside_another_cell_model() {
    #[derive(Debug, PartialEq, rston::cell::Store, rston::cell::Load)]
    struct FramedStack {
        prefix: u8,
        stack: Tuple,
        suffix: u8,
    }

    let expected = FramedStack {
        prefix: 7,
        stack: Tuple(vec![TupleItem::Int(10.into()), TupleItem::Int(20.into())]),
        suffix: 9,
    };

    let cell = CellBuilder::build_from(&expected).unwrap();
    let decoded = cell.parse::<FramedStack>().unwrap();

    assert_eq!(decoded, expected);
}
