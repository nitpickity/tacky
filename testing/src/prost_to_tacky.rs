//! Reverse interop tests: encode with prost, decode with tacky.

#[cfg(test)]
mod tests {
    use prost::Message;
    use std::collections::HashMap;

    use crate::prost_proto::{
        self, AnotherEnum as PAnotherEnum, MsgWithEnums as PMsgWithEnums,
        MsgWithMaps as PMsgWithMaps, MsgWithNesting as PMsgWithNesting,
        SimpleMessage as PSimpleMessage,
    };
    use crate::tacky_proto::example::{
        MsgWithEnumsField, MsgWithMapsField, MsgWithNestingField, SimpleEnum, SimpleMessage,
        SimpleMessageField,
    };

    #[test]
    fn test_all_scalar_types() {
        let prost_msg = PSimpleMessage {
            normal_int: Some(42),
            zigzag_int: Some(-999),
            fixed_int: Some(0x7FFF_FFFF_FFFF_FFFF),
            manynumbers: vec![1, 2, 3],
            manynumbers_unpacked: vec![10, 20],
            astring: Some("hello world".into()),
            manystrings: vec!["foo".into(), "bar".into()],
            manybytes: vec![vec![1, 2, 3], vec![4, 5]],
            abytes: Some(vec![0xFF, 0x00, 0xAB]),
            yesno: Some(true),
            packed_enum: vec![
                prost_proto::SimpleEnum::First.into(),
                prost_proto::SimpleEnum::Second.into(),
            ],
            packed_doubles: vec![1.5, -2.5, 3.14159],
            packed_floats: vec![0.5, -1.0],
            packed_fixed32: vec![100, 200, u32::MAX],
            packed_fixed64: vec![1000, u64::MAX],
            packed_sfixed32: vec![-1, 0, i32::MAX],
            packed_sfixed64: vec![i64::MIN, 0, i64::MAX],
            repeated_ints: vec![7, 8, 9],
            repeated_floats: vec![1.1, 2.2],
        };

        let wire = prost_msg.encode_to_vec();

        let mut normal_int = None;
        let mut zigzag_int = None;
        let mut fixed_int = None;
        let mut manynumbers = Vec::new();
        let mut manynumbers_unpacked = Vec::new();
        let mut astring = None;
        let mut manystrings = Vec::new();
        let mut manybytes: Vec<&[u8]> = Vec::new();
        let mut abytes = None;
        let mut yesno = None;
        let mut packed_enums = Vec::new();
        let mut doubles = Vec::new();
        let mut floats = Vec::new();
        let mut fixed32s = Vec::new();
        let mut fixed64s = Vec::new();
        let mut sfixed32s = Vec::new();
        let mut sfixed64s = Vec::new();
        let mut repeated_ints = Vec::new();
        let mut repeated_floats = Vec::new();

        for field in SimpleMessage::decode(&wire) {
            match field.unwrap() {
                SimpleMessageField::NormalInt(v) => normal_int = Some(v),
                SimpleMessageField::ZigzagInt(v) => zigzag_int = Some(v),
                SimpleMessageField::FixedInt(v) => fixed_int = Some(v),
                SimpleMessageField::Manynumbers(iter) => {
                    manynumbers.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::ManynumbersUnpacked(v) => manynumbers_unpacked.push(v),
                SimpleMessageField::PackedEnum(iter) => {
                    packed_enums.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::Astring(s) => astring = Some(s),
                SimpleMessageField::Manystrings(s) => manystrings.push(s),
                SimpleMessageField::Manybytes(b) => manybytes.push(b),
                SimpleMessageField::Abytes(b) => abytes = Some(b),
                SimpleMessageField::Yesno(v) => yesno = Some(v),
                SimpleMessageField::PackedDoubles(iter) => {
                    doubles.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::PackedFloats(iter) => {
                    floats.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::PackedFixed32(iter) => {
                    fixed32s.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::PackedFixed64(iter) => {
                    fixed64s.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::PackedSfixed32(iter) => {
                    sfixed32s.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::PackedSfixed64(iter) => {
                    sfixed64s.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::RepeatedInts(v) => repeated_ints.push(v),
                SimpleMessageField::RepeatedFloats(v) => repeated_floats.push(v),
            }
        }

        assert_eq!(normal_int, Some(42));
        assert_eq!(zigzag_int, Some(-999));
        assert_eq!(fixed_int, Some(0x7FFF_FFFF_FFFF_FFFF));
        assert_eq!(manynumbers, vec![1, 2, 3]);
        assert_eq!(manynumbers_unpacked, vec![10, 20]);
        assert_eq!(astring, Some("hello world"));
        assert_eq!(manystrings, vec!["foo", "bar"]);
        assert_eq!(manybytes, vec![[1u8, 2, 3].as_slice(), [4, 5].as_slice()]);
        assert_eq!(abytes, Some([0xFF, 0x00, 0xAB].as_slice()));
        assert_eq!(yesno, Some(true));
        assert_eq!(
            packed_enums.into_iter().map(SimpleEnum::from).collect::<Vec<_>>(),
            vec![SimpleEnum::First, SimpleEnum::Second]
        );
        assert_eq!(doubles, vec![1.5, -2.5, 3.14159]);
        assert_eq!(floats, vec![0.5, -1.0]);
        assert_eq!(fixed32s, vec![100, 200, u32::MAX]);
        assert_eq!(fixed64s, vec![1000, u64::MAX]);
        assert_eq!(sfixed32s, vec![-1, 0, i32::MAX]);
        assert_eq!(sfixed64s, vec![i64::MIN, 0, i64::MAX]);
        assert_eq!(repeated_ints, vec![7, 8, 9]);
        assert_eq!(repeated_floats, vec![1.1f32, 2.2]);
    }

    #[test]
    fn test_negative_varints() {
        let prost_msg = PSimpleMessage {
            normal_int: Some(-1),
            zigzag_int: Some(i64::MIN),
            ..Default::default()
        };

        let wire = prost_msg.encode_to_vec();

        let mut normal_int = None;
        let mut zigzag_int = None;

        for field in SimpleMessage::decode(&wire) {
            match field.unwrap() {
                SimpleMessageField::NormalInt(v) => normal_int = Some(v),
                SimpleMessageField::ZigzagInt(v) => zigzag_int = Some(v),
                _ => {}
            }
        }

        assert_eq!(normal_int, Some(-1));
        assert_eq!(zigzag_int, Some(i64::MIN));
    }

    #[test]
    fn test_empty_message() {
        let prost_msg = PSimpleMessage::default();
        let wire = prost_msg.encode_to_vec();

        let fields: Vec<_> = SimpleMessage::decode(&wire)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(fields.is_empty());
    }

    #[test]
    fn test_maps() {
        let prost_msg = PMsgWithMaps {
            map1: HashMap::from([("alpha".into(), 1), ("beta".into(), 2)]),
            map2: HashMap::from([(10, 1.5), (20, 2.5)]),
        };

        let wire = prost_msg.encode_to_vec();

        let mut map1 = HashMap::new();
        let mut map2 = HashMap::new();

        for field in crate::tacky_proto::example::MsgWithMaps::decode(&wire) {
            match field.unwrap() {
                MsgWithMapsField::Map1((k, v)) => {
                    if let Some(v) = v {
                        map1.insert(k.to_string(), v);
                    }
                }
                MsgWithMapsField::Map2((k, v)) => {
                    if let Some(v) = v {
                        map2.insert(k, v);
                    }
                }
            }
        }

        assert_eq!(map1, HashMap::from([("alpha".into(), 1), ("beta".into(), 2)]));
        assert_eq!(map2, HashMap::from([(10, 1.5), (20, 2.5)]));
    }

    #[test]
    fn test_enums() {
        let prost_msg = PMsgWithEnums {
            enum1: Some(prost_proto::SimpleEnum::Second.into()),
            enum2: vec![PAnotherEnum::A.into(), PAnotherEnum::B.into()],
        };

        let wire = prost_msg.encode_to_vec();

        let mut enum1 = None;
        let mut enum2 = Vec::new();

        for field in crate::tacky_proto::example::MsgWithEnums::decode(&wire) {
            match field.unwrap() {
                MsgWithEnumsField::Enum1(v) => enum1 = Some(v),
                MsgWithEnumsField::Enum2(v) => enum2.push(v),
            }
        }

        assert_eq!(enum1, Some(SimpleEnum::Second));
        assert_eq!(
            enum2,
            vec![
                crate::tacky_proto::example::AnotherEnum::A,
                crate::tacky_proto::example::AnotherEnum::B,
            ]
        );
    }

    #[test]
    fn test_nested() {
        let prost_msg = PMsgWithNesting {
            enums: Some(PMsgWithEnums {
                enum1: Some(prost_proto::SimpleEnum::First.into()),
                enum2: vec![],
            }),
            nested: vec![
                PSimpleMessage {
                    normal_int: Some(10),
                    astring: Some("first".into()),
                    ..Default::default()
                },
                PSimpleMessage {
                    normal_int: Some(20),
                    astring: Some("second".into()),
                    ..Default::default()
                },
            ],
        };

        let wire = prost_msg.encode_to_vec();

        let mut enum1 = None;
        let mut nested_ints = Vec::new();
        let mut nested_strings = Vec::new();

        for field in crate::tacky_proto::example::MsgWithNesting::decode(&wire) {
            match field.unwrap() {
                MsgWithNestingField::Enums(iter) => {
                    for f in iter {
                        if let MsgWithEnumsField::Enum1(v) = f.unwrap() {
                            enum1 = Some(v);
                        }
                    }
                }
                MsgWithNestingField::Nested(iter) => {
                    for f in iter {
                        match f.unwrap() {
                            SimpleMessageField::NormalInt(v) => nested_ints.push(v),
                            SimpleMessageField::Astring(s) => {
                                nested_strings.push(s.to_string())
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        assert_eq!(enum1, Some(SimpleEnum::First));
        assert_eq!(nested_ints, vec![10, 20]);
        assert_eq!(nested_strings, vec!["first", "second"]);
    }
}
