#[allow(dead_code)]
mod prost_proto {
    include!(concat!(env!("OUT_DIR"), "/example.rs"));
}
mod tacky_proto {
    include!(concat!(env!("OUT_DIR"), "/simple.rs"));
}

#[cfg(test)]
mod tests {
    use prost::Message;
    use std::collections::{BTreeMap, HashMap};

    use crate::prost_proto::{
        self, AnotherEnum as PAnotherEnum, MsgWithEnums as PMsgWithEnums,
        MsgWithMaps as PMsgWithMaps, MsgWithNesting as PMsgWithNesting,
        SimpleMessage as PSimpleMessage,
    };
    use crate::tacky_proto::example::{
        AnotherEnum, MapsWithMsg, MapsWithMsgField, MsgWithEnums, MsgWithEnumsField,
        MsgWithEnumsFields, MsgWithMaps, MsgWithMapsField, MsgWithNesting, MsgWithNestingField,
        MsgWithNestingFields, SimpleEnum, SimpleMessage, SimpleMessageField, SimpleMessageFields,
    };

    #[test]
    fn test_simple_message() {
        // data
        let anumber = Some(42);
        let manynumbers = &[1, 2, 3];
        let astring = Some("Hello");
        let manystrings = vec!["many", "strings"];
        let abytes = Vec::new();
        let manybytes: Vec<[u8; 12]> = Vec::new();
        let packed_doubles = [1.5, 2.5, 3.5];
        let packed_floats = [0.5f32, 1.0];
        let packed_fixed32 = [100u32, 200];
        let packed_fixed64 = [1000u64, 2000];
        let packed_sfixed32 = [-1i32, -2];
        let packed_sfixed64 = [-100i64, -200];

        let tacky_packed = {
            let mut buf = Vec::new();
            let schema = SimpleMessage::default();
            SimpleMessage {
                normal_int: schema.normal_int.write(&mut buf, anumber),
                zigzag_int: schema.zigzag_int.write(&mut buf, Some(24)),
                fixed_int: schema.fixed_int.write(&mut buf, Some(12)),
                manynumbers: schema.manynumbers.write(&mut buf, manynumbers),
                manynumbers_unpacked: schema.manynumbers_unpacked.write(&mut buf, manynumbers),
                packed_enum: schema
                    .packed_enum
                    .write(&mut buf, &[SimpleEnum::First, SimpleEnum::Second]),
                astring: schema.astring.write(&mut buf, astring),
                manystrings: schema.manystrings.write(&mut buf, &manystrings),
                manybytes: schema.manybytes.write(&mut buf, &manybytes),
                abytes: schema.abytes.write(&mut buf, Some(abytes.as_slice())),
                yesno: schema.yesno.write(&mut buf, Some(false)),
                packed_doubles: schema.packed_doubles.write(&mut buf, &packed_doubles),
                packed_floats: schema.packed_floats.write(&mut buf, &packed_floats),
                packed_fixed32: schema.packed_fixed32.write(&mut buf, &packed_fixed32),
                packed_fixed64: schema.packed_fixed64.write(&mut buf, &packed_fixed64),
                packed_sfixed32: schema.packed_sfixed32.write(&mut buf, &packed_sfixed32),
                packed_sfixed64: schema.packed_sfixed64.write(&mut buf, &packed_sfixed64),
                repeated_ints: schema.repeated_ints.write(&mut buf, Vec::<i32>::new()),
                repeated_floats: schema.repeated_floats.write(&mut buf, Vec::<f32>::new()),
            };
            buf
        };

        let prost_msg = {
            //note the faffing about with clones and collects because the types dont _exactly_ line up
            PSimpleMessage {
                normal_int: anumber,
                zigzag_int: Some(24),
                fixed_int: Some(12),
                manynumbers: manynumbers.to_vec(),
                manynumbers_unpacked: manynumbers.to_vec(),
                astring: astring.map(|s| s.to_string()),
                manystrings: manystrings.into_iter().map(|s| s.to_string()).collect(),
                manybytes: manybytes.into_iter().map(|arr| arr.to_vec()).collect(),
                abytes: Some(abytes),
                yesno: Some(false),
                packed_enum: vec![
                    prost_proto::SimpleEnum::First.into(),
                    prost_proto::SimpleEnum::Second.into(),
                ],
                packed_doubles: packed_doubles.to_vec(),
                packed_floats: packed_floats.to_vec(),
                packed_fixed32: packed_fixed32.to_vec(),
                packed_fixed64: packed_fixed64.to_vec(),
                packed_sfixed32: packed_sfixed32.to_vec(),
                packed_sfixed64: packed_sfixed64.to_vec(),
                repeated_ints: vec![],
                repeated_floats: vec![],
            }
        };

        let unpacked = PSimpleMessage::decode(&*tacky_packed).unwrap();
        //prost can decode what tacky encodes
        assert_eq!(unpacked, prost_msg);
    }

    #[test]
    fn test_with_maps() {
        //data
        let map1: BTreeMap<&str, i32> = BTreeMap::from_iter([("one", 1), ("two", 2)]);
        let map2: HashMap<i32, f64> = HashMap::from_iter([(1, 1.0), (2, 2.0)]);
        let tacky_packed = {
            let mut buf = Vec::new();
            let schema = MsgWithMaps::default();
            MsgWithMaps {
                map1: schema.map1.write(&mut buf, &map1),
                map2: schema.map2.write(&mut buf, &map2),
            };
            buf
        };

        let prost_msg = {
            //note the faffing about with clones and collects because the types dont _exactly_ line up
            PMsgWithMaps {
                map1: HashMap::from_iter(map1.into_iter().map(|(k, v)| (k.to_string(), v))),
                map2,
            }
        };

        let unpacked = PMsgWithMaps::decode(&*tacky_packed).unwrap();
        //prost can decode what tacky encodes
        assert_eq!(unpacked, prost_msg);
    }
    #[test]
    fn test_with_enums() {
        //data

        let tacky_packed = {
            let mut buf = Vec::new();
            let schema = MsgWithEnums::default();

            MsgWithEnums {
                enum1: schema.enum1.write(&mut buf, Some(SimpleEnum::First)),
                enum2: schema
                    .enum2
                    .write(&mut buf, &[AnotherEnum::A, AnotherEnum::B]),
            };
            buf
        };

        let prost_msg = {
            PMsgWithEnums {
                enum1: Some(prost_proto::SimpleEnum::First.into()),
                enum2: vec![PAnotherEnum::A.into(), PAnotherEnum::B.into()],
            }
        };

        let unpacked = PMsgWithEnums::decode(&*tacky_packed).unwrap();
        //prost can decode what tacky encodes
        assert_eq!(unpacked, prost_msg);
    }

    #[test]
    fn test_with_nested() {
        let tacky_packed = {
            let mut buf = Vec::new();
            let msg_schema = MsgWithNesting::default();

            MsgWithNesting {
                enums: msg_schema.enums.write_msg(&mut buf, |buf, scm| {
                    scm.enum1.write(buf, Some(SimpleEnum::First));
                    scm.enum2.write(buf, &[AnotherEnum::A, AnotherEnum::B]);
                }),
                nested: {
                    for i in 0..10 {
                        msg_schema.nested.write_msg(&mut buf, |buf, scm| {
                            SimpleMessage {
                                normal_int: scm.normal_int.write(buf, Some(i)),
                                zigzag_int: scm.zigzag_int.write(buf, Some(i + 1)),
                                fixed_int: scm.fixed_int.write(buf, Some(i + 3)),
                                manynumbers: scm.manynumbers.write(buf, &[i as i32]),
                                manynumbers_unpacked: scm
                                    .manynumbers_unpacked
                                    .write(buf, &[i as i32]),
                                astring: scm.astring.write(buf, None::<&str>),
                                manystrings: scm.manystrings.write(buf, &["hello"]),
                                abytes: scm.abytes.write(buf, None::<&[u8]>),
                                manybytes: scm.manybytes.write(buf, Vec::<&[u8]>::new()),
                                packed_enum: scm
                                    .packed_enum
                                    .write(buf, &[SimpleEnum::First, SimpleEnum::Second]),
                                yesno: scm.yesno.write(buf, Some(false)),
                                packed_doubles: scm.packed_doubles.write(buf, Vec::<f64>::new()),
                                packed_floats: scm.packed_floats.write(buf, Vec::<f32>::new()),
                                packed_fixed32: scm.packed_fixed32.write(buf, Vec::<u32>::new()),
                                packed_fixed64: scm.packed_fixed64.write(buf, Vec::<u64>::new()),
                                packed_sfixed32: scm.packed_sfixed32.write(buf, Vec::<i32>::new()),
                                packed_sfixed64: scm.packed_sfixed64.write(buf, Vec::<i64>::new()),
                                repeated_ints: scm.repeated_ints.write(buf, Vec::<i32>::new()),
                                repeated_floats: scm.repeated_floats.write(buf, Vec::<f32>::new()),
                            };
                        });
                    }
                    msg_schema.nested
                },
            };
            buf
        };

        let prost_msg = {
            PMsgWithNesting {
                enums: Some(PMsgWithEnums {
                    enum1: Some(prost_proto::SimpleEnum::First.into()),
                    enum2: vec![PAnotherEnum::A.into(), PAnotherEnum::B.into()],
                }),
                nested: {
                    let mut v = Vec::new();
                    for i in 0..10 {
                        v.push(PSimpleMessage {
                            normal_int: Some(i),
                            zigzag_int: Some(i + 1),
                            fixed_int: Some(i + 3),
                            manynumbers: vec![i as i32],
                            manynumbers_unpacked: vec![i as i32],
                            astring: None,
                            manystrings: vec!["hello".into()],
                            abytes: None,
                            packed_enum: vec![
                                prost_proto::SimpleEnum::First.into(),
                                prost_proto::SimpleEnum::Second.into(),
                            ],
                            manybytes: vec![],
                            yesno: Some(false),
                            packed_doubles: vec![],
                            packed_floats: vec![],
                            packed_fixed32: vec![],
                            packed_fixed64: vec![],
                            packed_sfixed32: vec![],
                            packed_sfixed64: vec![],
                            repeated_ints: vec![],
                            repeated_floats: vec![],
                        })
                    }
                    v
                },
            }
        };

        let unpacked = PMsgWithNesting::decode(&*tacky_packed).unwrap();
        //prost can decode what tacky encodes
        assert_eq!(unpacked, prost_msg);
    }

    // --- Decode tests using generated field enums ---

    #[test]
    fn test_decode_simple_message() {
        // Encode
        let mut buf = Vec::new();
        let schema = SimpleMessage::default();
        SimpleMessage {
            normal_int: schema.normal_int.write(&mut buf, Some(42)),
            zigzag_int: schema.zigzag_int.write(&mut buf, Some(-7)),
            fixed_int: schema.fixed_int.write(&mut buf, Some(999)),
            manynumbers: schema.manynumbers.write(&mut buf, &[10, 20, 30]),
            manynumbers_unpacked: schema.manynumbers_unpacked.write(&mut buf, &[100, 200]),
            packed_enum: schema
                .packed_enum
                .write(&mut buf, &[SimpleEnum::First, SimpleEnum::Second]),
            astring: schema.astring.write(&mut buf, Some("hello")),
            manystrings: schema.manystrings.write(&mut buf, &["foo", "bar"]),
            manybytes: schema
                .manybytes
                .write(&mut buf, &[b"a".as_slice(), b"b".as_slice()]),
            abytes: schema.abytes.write(&mut buf, Some(b"raw".as_slice())),
            yesno: schema.yesno.write(&mut buf, Some(true)),
            packed_doubles: schema.packed_doubles.write(&mut buf, &[1.5, 2.5]),
            packed_floats: schema.packed_floats.write(&mut buf, &[0.5f32]),
            packed_fixed32: schema.packed_fixed32.write(&mut buf, &[42u32]),
            packed_fixed64: schema.packed_fixed64.write(&mut buf, &[999u64]),
            packed_sfixed32: schema.packed_sfixed32.write(&mut buf, &[-1i32]),
            packed_sfixed64: schema.packed_sfixed64.write(&mut buf, &[-100i64]),
            repeated_ints: schema.repeated_ints.write(&mut buf, Vec::<i32>::new()),
            repeated_floats: schema.repeated_floats.write(&mut buf, Vec::<f32>::new()),
        };

        // Decode with field enum
        let mut normal_int = None;
        let mut zigzag_int = None;
        let mut fixed_int = None;
        let mut packed_numbers: Vec<i32> = Vec::new();
        let mut unpacked_numbers: Vec<i32> = Vec::new();
        let mut packed_enums: Vec<SimpleEnum> = Vec::new();
        let mut astring = None;
        let mut manystrings: Vec<&str> = Vec::new();
        let mut abytes: Option<&[u8]> = None;
        let mut manybytes: Vec<&[u8]> = Vec::new();
        let mut yesno = None;
        let mut doubles: Vec<f64> = Vec::new();
        let mut floats: Vec<f32> = Vec::new();
        let mut fixed32s: Vec<u32> = Vec::new();
        let mut fixed64s: Vec<u64> = Vec::new();
        let mut sfixed32s: Vec<i32> = Vec::new();
        let mut sfixed64s: Vec<i64> = Vec::new();

        for field in SimpleMessage::decode(&buf) {
            let field = field.unwrap();
            match field {
                SimpleMessageField::NormalInt(v) => normal_int = Some(v),
                SimpleMessageField::ZigzagInt(v) => zigzag_int = Some(v),
                SimpleMessageField::FixedInt(v) => fixed_int = Some(v),
                SimpleMessageField::Manynumbers(iter) => {
                    packed_numbers.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::ManynumbersUnpacked(v) => unpacked_numbers.push(v),
                SimpleMessageField::PackedEnum(iter) => {
                    packed_enums.extend(iter.map(|r| SimpleEnum::try_from(r.unwrap()).unwrap()));
                }
                SimpleMessageField::Astring(s) => astring = Some(s),
                SimpleMessageField::Manystrings(s) => manystrings.push(s),
                SimpleMessageField::Abytes(b) => abytes = Some(b),
                SimpleMessageField::Manybytes(b) => manybytes.push(b),
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
                SimpleMessageField::RepeatedInts(_) => {}
                SimpleMessageField::RepeatedFloats(_) => {}
            }
        }

        assert_eq!(normal_int, Some(42));
        assert_eq!(zigzag_int, Some(-7));
        assert_eq!(fixed_int, Some(999));
        assert_eq!(packed_numbers, vec![10, 20, 30]);
        assert_eq!(unpacked_numbers, vec![100, 200]);
        assert_eq!(packed_enums, vec![SimpleEnum::First, SimpleEnum::Second]);
        assert_eq!(astring, Some("hello"));
        assert_eq!(manystrings, vec!["foo", "bar"]);
        assert_eq!(abytes, Some(b"raw".as_slice()));
        assert_eq!(manybytes, vec![b"a".as_slice(), b"b".as_slice()]);
        assert_eq!(yesno, Some(true));
        assert_eq!(doubles, vec![1.5, 2.5]);
        assert_eq!(floats, vec![0.5]);
        assert_eq!(fixed32s, vec![42]);
        assert_eq!(fixed64s, vec![999]);
        assert_eq!(sfixed32s, vec![-1]);
        assert_eq!(sfixed64s, vec![-100]);
    }

    #[test]
    fn test_decode_enums() {
        let mut buf = Vec::new();
        let scm = MsgWithEnums::default();
        MsgWithEnums {
            enum1: scm.enum1.write(&mut buf, Some(SimpleEnum::Second)),
            enum2: scm.enum2.write(&mut buf, &[AnotherEnum::A, AnotherEnum::B]),
        };

        let remaining: &[u8] = &buf;
        let mut enum1 = None;
        let mut enum2: Vec<AnotherEnum> = Vec::new();

        for field in MsgWithEnumsFields::new(remaining) {
            let field = field.unwrap();
            match field {
                MsgWithEnumsField::Enum1(v) => enum1 = Some(v),
                MsgWithEnumsField::Enum2(v) => enum2.push(v),
            }
        }

        assert_eq!(enum1, Some(SimpleEnum::Second));
        assert_eq!(enum2, vec![AnotherEnum::A, AnotherEnum::B]);
    }
    #[test]
    fn test_decode_maps() {
        let mut buf = Vec::new();
        let scm = MsgWithMaps::default();
        MsgWithMaps {
            map1: scm
                .map1
                .write(&mut buf, &BTreeMap::from_iter([("one", 1), ("two", 2)])),
            map2: scm
                .map2
                .write(&mut buf, &HashMap::from([(1, 1.0), (2, 2.0)])),
        };

        let remaining: &[u8] = &buf;
        let mut map1 = BTreeMap::new();
        let mut map2 = HashMap::new();

        for field in MsgWithMaps::decode(remaining) {
            let field = field.unwrap();
            match field {
                MsgWithMapsField::Map1((k, v)) => {
                    if let Some(v) = v {
                        map1.insert(k, v);
                    }
                }
                MsgWithMapsField::Map2((k, v)) => {
                    if let Some(v) = v {
                        map2.insert(k, v);
                    }
                }
            }
        }

        assert_eq!(map1, BTreeMap::from_iter([("one", 1), ("two", 2)]));
        assert_eq!(map2, HashMap::from_iter([(1, 1.0), (2, 2.0)]));
    }

    #[test]
    fn test_maps_with_msg_values() {
        let s = MapsWithMsg::default();
        let mut buf = Vec::new();
        s.map1.write_msg(&mut buf, "key", |buf, s| {
            s.normal_int.write(buf, Some(42));
            s.astring.write(buf, Some("hello"));
        });

        let fld = MapsWithMsg::decode(&buf);
        for f in fld {
            let MapsWithMsgField::Map1((k, v)) = f.unwrap();
            assert_eq!(k, "key");
            let mut normal_int = None;
            let mut astring = None;
            let v = v.unwrap();
            for subfield in v {
                let subfield = subfield.unwrap();
                match subfield {
                    SimpleMessageField::NormalInt(n) => normal_int = Some(n),
                    SimpleMessageField::Astring(s) => astring = Some(s),
                    _ => {}
                }
            }
            assert_eq!(normal_int, Some(42));
            assert_eq!(astring, Some("hello"));
        }
    }
    #[test]
    fn test_decode_nested() {
        let mut buf = Vec::new();
        let schema = MsgWithNesting::default();
        MsgWithNesting {
            enums: schema.enums.write_msg(&mut buf, |buf, scm| {
                scm.enum1.write(buf, Some(SimpleEnum::First));
                scm.enum2.write(buf, &[AnotherEnum::B]);
            }),
            nested: schema.nested.write_msg(&mut buf, |buf, scm| {
                SimpleMessage {
                    normal_int: scm.normal_int.write(buf, Some(77)),
                    zigzag_int: scm.zigzag_int.write(buf, None::<i64>),
                    fixed_int: scm.fixed_int.write(buf, None::<i64>),
                    manynumbers: scm.manynumbers.write(buf, Vec::<i32>::new()),
                    manynumbers_unpacked: scm.manynumbers_unpacked.write(buf, Vec::<i32>::new()),
                    packed_enum: scm.packed_enum.write(buf, Vec::<SimpleEnum>::new()),
                    astring: scm.astring.write(buf, Some("nested")),
                    manystrings: scm.manystrings.write(buf, Vec::<&str>::new()),
                    manybytes: scm.manybytes.write(buf, Vec::<&[u8]>::new()),
                    abytes: scm.abytes.write(buf, Some("hello".as_bytes())),
                    yesno: scm.yesno.write(buf, Some(true)),
                    packed_doubles: scm.packed_doubles.write(buf, Vec::<f64>::new()),
                    packed_floats: scm.packed_floats.write(buf, Vec::<f32>::new()),
                    packed_fixed32: scm.packed_fixed32.write(buf, Vec::<u32>::new()),
                    packed_fixed64: scm.packed_fixed64.write(buf, Vec::<u64>::new()),
                    packed_sfixed32: scm.packed_sfixed32.write(buf, Vec::<i32>::new()),
                    packed_sfixed64: scm.packed_sfixed64.write(buf, Vec::<i64>::new()),
                    repeated_ints: scm.repeated_ints.write(buf, Vec::<i32>::new()),
                    repeated_floats: scm.repeated_floats.write(buf, Vec::<f32>::new()),
                };
            }),
        };

        let mut nested_msgs: Vec<SimpleMessageFields<'_>> = Vec::new();

        for field in MsgWithNestingFields::new(&buf) {
            let field = field.unwrap();
            match field {
                MsgWithNestingField::Enums(b) => {
                    let mut inner_enum1 = None;
                    let mut inner_enum2 = Vec::new();
                    for field in b {
                        let field = field.unwrap();
                        match field {
                            MsgWithEnumsField::Enum1(v) => inner_enum1 = Some(v),
                            MsgWithEnumsField::Enum2(v) => inner_enum2.push(v),
                        }
                    }
                    assert_eq!(inner_enum1, Some(SimpleEnum::First));
                    assert_eq!(inner_enum2, vec![AnotherEnum::B]);
                }
                MsgWithNestingField::Nested(b) => nested_msgs.push(b),
            }
        }

        // Decode nested SimpleMessage
        assert_eq!(nested_msgs.len(), 1);
        let sub = nested_msgs[0];
        let mut normal_int = None;
        let mut astring = None;
        for field in sub {
            let field = field.unwrap();
            match field {
                SimpleMessageField::NormalInt(v) => normal_int = Some(v),
                SimpleMessageField::Astring(s) => astring = Some(s),
                _ => {}
            }
        }

        assert_eq!(normal_int, Some(77));
        assert_eq!(astring, Some("nested"));
    }
    #[test]
    fn test_decode_unknown_field_skipping() {
        use tacky::*;
        // Manually construct bytes with an unknown field (tag=99, varint value=42)
        // followed by a known field (tag=10, varint yesno=1)
        let mut buf = Vec::new();
        Field::<99, Plain<Int32>>::new().write(&mut buf, 42);
        Field::<10, Plain<Bool>>::new().write(&mut buf, true);
        Field::<420, Plain<PbString>>::new().write(&mut buf, "should be skipped");

        let remaining: &[u8] = &buf;

        let it = SimpleMessageFields::new(remaining);
        for f in it {
            let f = f.unwrap();
            assert!(
                matches!(f, SimpleMessageField::Yesno(true)),
                "expected to only find the known yesno field, got: {f:?}"
            );
        }
    }
    #[test]
    fn test_decode_wire_type_mismatch() {
        // Construct bytes with tag=1 (normal_int, expects VARINT) but wire type LEN
        let mut buf = Vec::new();
        // tag=1, wire type LEN(2) => key = (1 << 3) | 2 = 10
        tacky::write_varint(10, &mut buf);
        tacky::write_varint(3, &mut buf); // length 3
        buf.extend_from_slice(b"abc"); // some bytes

        let remaining: &[u8] = &buf;
        let result = SimpleMessageFields::new(remaining).next().unwrap();
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("wire type mismatch"),
            "expected wire type mismatch error, got: {err}"
        );
    }

    #[test]
    fn test_decode_packed_fixed_types() {
        let doubles = [1.0, -2.5, 3.14159];
        let floats = [0.5f32, -1.0, 100.0];
        let fixed32 = [0u32, 1, u32::MAX];
        let fixed64 = [0u64, 1, u64::MAX];
        let sfixed32 = [i32::MIN, 0, i32::MAX];
        let sfixed64 = [i64::MIN, 0, i64::MAX];

        // Encode with tacky
        let mut buf = Vec::new();
        let scm = SimpleMessage::default();

        SimpleMessage {
            normal_int: scm.normal_int.write(&mut buf, None::<i64>),
            zigzag_int: scm.zigzag_int.write(&mut buf, None::<i64>),
            fixed_int: scm.fixed_int.write(&mut buf, None::<i64>),
            manynumbers: scm.manynumbers.write(&mut buf, &[]),
            manynumbers_unpacked: scm.manynumbers_unpacked.write(&mut buf, &[]),
            packed_enum: scm.packed_enum.write(&mut buf, &[]),
            astring: scm.astring.write(&mut buf, None::<&str>),
            manystrings: scm.manystrings.write(&mut buf, <Vec<String>>::new()),
            manybytes: scm.manybytes.write(&mut buf, Vec::<&[u8]>::new()),
            abytes: scm.abytes.write(&mut buf, None::<&[u8]>),
            yesno: scm.yesno.write(&mut buf, None::<bool>),
            packed_doubles: scm.packed_doubles.write(&mut buf, &doubles),
            packed_floats: scm.packed_floats.write(&mut buf, &floats),
            packed_fixed32: scm.packed_fixed32.write(&mut buf, &fixed32),
            packed_fixed64: scm.packed_fixed64.write(&mut buf, &fixed64),
            packed_sfixed32: scm.packed_sfixed32.write(&mut buf, &sfixed32),
            packed_sfixed64: scm.packed_sfixed64.write(&mut buf, &sfixed64),
            repeated_ints: scm.repeated_ints.write(&mut buf, Vec::<i32>::new()),
            repeated_floats: scm.repeated_floats.write(&mut buf, Vec::<f32>::new()),
        };

        // Verify prost can decode it
        let prost_msg = PSimpleMessage::decode(&*buf).unwrap();
        assert_eq!(prost_msg.packed_doubles, doubles.to_vec());
        assert_eq!(prost_msg.packed_floats, floats.to_vec());
        assert_eq!(prost_msg.packed_fixed32, fixed32.to_vec());
        assert_eq!(prost_msg.packed_fixed64, fixed64.to_vec());
        assert_eq!(prost_msg.packed_sfixed32, sfixed32.to_vec());
        assert_eq!(prost_msg.packed_sfixed64, sfixed64.to_vec());

        // Decode with field enum
        let remaining: &[u8] = &buf;
        let mut decoded_doubles = Vec::new();
        let mut decoded_floats = Vec::new();
        let mut decoded_fixed32 = Vec::new();
        let mut decoded_fixed64 = Vec::new();
        let mut decoded_sfixed32 = Vec::new();
        let mut decoded_sfixed64 = Vec::new();

        for field in SimpleMessageFields::new(remaining) {
            let field = field.unwrap();
            match field {
                SimpleMessageField::PackedDoubles(iter) => {
                    decoded_doubles.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::PackedFloats(iter) => {
                    decoded_floats.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::PackedFixed32(iter) => {
                    decoded_fixed32.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::PackedFixed64(iter) => {
                    decoded_fixed64.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::PackedSfixed32(iter) => {
                    decoded_sfixed32.extend(iter.map(|r| r.unwrap()));
                }
                SimpleMessageField::PackedSfixed64(iter) => {
                    decoded_sfixed64.extend(iter.map(|r| r.unwrap()));
                }
                _ => {}
            }
        }

        assert_eq!(decoded_doubles, doubles.to_vec());
        assert_eq!(decoded_floats, floats.to_vec());
        assert_eq!(decoded_fixed32, fixed32.to_vec());
        assert_eq!(decoded_fixed64, fixed64.to_vec());
        assert_eq!(decoded_sfixed32, sfixed32.to_vec());
        assert_eq!(decoded_sfixed64, sfixed64.to_vec());
    }

    #[test]
    fn test_packed_iter_edge_cases() {
        use tacky::scalars::*;
        // Empty packed field - iterator yields nothing
        fn iter<T: Packable>(data: &[u8]) -> tacky::packed::PackedIter<'_, T> {
            tacky::packed::PackedIter::<T>::new(data)
        }
        let empty = iter::<Uint64>(&[]);
        assert_eq!(empty.count(), 0);

        let empty_f32 = iter::<Float>(&[]);
        assert_eq!(empty_f32.count(), 0);

        let empty_f64 = iter::<Double>(&[]);
        assert_eq!(empty_f64.count(), 0);

        // Single element packed varint
        let mut single_buf = Vec::new();
        tacky::write_varint(42, &mut single_buf);
        let single = iter::<Uint64>(&single_buf);
        let vals: Vec<u64> = single.map(|r| r.unwrap()).collect();
        assert_eq!(vals, vec![42]);

        // Single element packed fixed32
        let bytes_f32 = 3.14f32.to_le_bytes();
        let single_f32 = iter::<Float>(&bytes_f32);
        let vals: Vec<f32> = single_f32.map(|r| r.unwrap()).collect();
        assert_eq!(vals, vec![3.14f32]);

        // Single element packed fixed64
        let bytes_f64 = 2.718f64.to_le_bytes();
        let single_f64 = iter::<Double>(&bytes_f64);
        let vals: Vec<f64> = single_f64.map(|r| r.unwrap()).collect();
        assert_eq!(vals, vec![2.718f64]);

        // Truncated fixed32 should error
        let truncated = iter::<Float>(&[1, 2, 3]);
        let results: Vec<_> = truncated.collect();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());

        // Truncated fixed64 should error
        let truncated = iter::<Double>(&[1, 2, 3, 4, 5, 6, 7]);
        let results: Vec<_> = truncated.collect();
        assert_eq!(results.len(), 1);
        assert!(results[0].is_err());
    }
}
