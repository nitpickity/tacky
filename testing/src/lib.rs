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

    use tacky::MessageSchema;

    use crate::prost_proto::{
        self, AnotherEnum as PAnotherEnum, MsgWithEnums as PMsgWithEnums,
        MsgWithMaps as PMsgWithMaps, MsgWithNesting as PMsgWithNesting,
        SimpleMessage as PSimpleMessage,
    };
    use crate::tacky_proto::example::{
        AnotherEnum, MsgWithEnums, MsgWithEnumsField, MsgWithMaps, MsgWithNesting,
        MsgWithNestingField, SimpleEnum, SimpleMessage, SimpleMessageField,
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

        let tacky_packed = {
            let mut buf = Vec::new();
            let mut writer = SimpleMessage::new_writer(&mut buf, None);
            tacky_macros::write_proto!(
                writer,
                SimpleMessage {
                    normal_int: anumber,
                    zigzag_int: Some(24),
                    fixed_int: Some(12),
                    packed_enum: [SimpleEnum::First, SimpleEnum::Second],
                    manynumbers,
                    manynumbers_unpacked: manynumbers,
                    astring,
                    manystrings: &manystrings,
                    abytes: Some(&abytes),
                    manybytes: &manybytes,
                    yesno: Some(false),
                }
            );
            drop(writer);
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
            let mut writer = MsgWithMaps::new_writer(&mut buf, None);
            tacky_macros::write_proto!(
                writer,
                MsgWithMaps {
                    map1: &map1,
                    map2: &map2
                }
            );
            drop(writer);
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
            let mut writer = MsgWithEnums::new_writer(&mut buf, None);
            tacky_macros::write_proto!(
                writer,
                MsgWithEnums {
                    enum1: Some(SimpleEnum::First),
                    enum2: [AnotherEnum::A, AnotherEnum::B],
                }
            );
            drop(writer);
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
            let mut writer = MsgWithNesting::new_writer(&mut buf, None);
            tacky_macros::write_proto!(
                writer,
                MsgWithNesting {
                    enums: {
                        writer.enums().write_msg(|mut m| {
                            tacky_macros::write_proto!(
                                m,
                                MsgWithEnums {
                                    enum1: Some(SimpleEnum::First),
                                    enum2: [AnotherEnum::A, AnotherEnum::B],
                                }
                            );
                        })
                    },
                    nested: {
                        let mut m = writer.nested();
                        for i in 0..10 {
                            m.append_msg_with(|mut n| {
                                tacky_macros::write_proto!(
                                    n,
                                    SimpleMessage {
                                        normal_int: Some(i),
                                        zigzag_int: Some(i + 1),
                                        fixed_int: Some(i + 3),
                                        manynumbers: [i as i32],
                                        manynumbers_unpacked: [i as i32],
                                        astring: None::<&str>,
                                        manystrings: ["hello"],
                                        abytes: None::<Vec<_>>,
                                        packed_enum: [SimpleEnum::First, SimpleEnum::Second],
                                        manybytes: <Vec<Vec<u8>>>::new(),
                                        yesno: Some(false),
                                    }
                                );
                            });
                        }
                        m.close()
                    }
                }
            );
            drop(writer);
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
        let mut writer = SimpleMessage::new_writer(&mut buf, None);
        tacky_macros::write_proto!(
            writer,
            SimpleMessage {
                normal_int: Some(42i64),
                zigzag_int: Some(-7i64),
                fixed_int: Some(999i64),
                manynumbers: [10i32, 20, 30],
                manynumbers_unpacked: [100i32, 200],
                packed_enum: [SimpleEnum::First, SimpleEnum::Second],
                astring: Some("hello"),
                manystrings: ["foo", "bar"],
                abytes: Some(&b"raw"[..]),
                manybytes: [&b"a"[..], &b"b"[..]],
                yesno: Some(true),
            }
        );
        drop(writer);

        // Decode with field enum
        let mut remaining: &[u8] = &buf;
        let mut normal_int = None;
        let mut zigzag_int = None;
        let mut fixed_int = None;
        let mut packed_numbers: Vec<i32> = Vec::new();
        let mut unpacked_numbers: Vec<i32> = Vec::new();
        let mut packed_enums: Vec<i32> = Vec::new();
        let mut astring = None;
        let mut manystrings: Vec<&str> = Vec::new();
        let mut abytes: Option<&[u8]> = None;
        let mut manybytes: Vec<&[u8]> = Vec::new();
        let mut yesno = None;

        while !remaining.is_empty() {
            let Some(field) = SimpleMessageField::decode(&mut remaining).unwrap() else {
                continue;
            };
            match field {
                SimpleMessageField::NormalInt(v) => normal_int = Some(v),
                SimpleMessageField::ZigzagInt(v) => zigzag_int = Some(v),
                SimpleMessageField::FixedInt(v) => fixed_int = Some(v),
                SimpleMessageField::Manynumbers(packed) => {
                    let mut p = packed;
                    while !p.is_empty() {
                        packed_numbers.push(tacky::decode_varint(&mut p).unwrap() as i32);
                    }
                }
                SimpleMessageField::ManynumbersUnpacked(v) => unpacked_numbers.push(v),
                SimpleMessageField::PackedEnum(packed) => {
                    let mut p = packed;
                    while !p.is_empty() {
                        packed_enums.push(tacky::decode_varint(&mut p).unwrap() as i32);
                    }
                }
                SimpleMessageField::Astring(s) => astring = Some(s),
                SimpleMessageField::Manystrings(s) => manystrings.push(s),
                SimpleMessageField::Abytes(b) => abytes = Some(b),
                SimpleMessageField::Manybytes(b) => manybytes.push(b),
                SimpleMessageField::Yesno(v) => yesno = Some(v),
            }
        }

        assert_eq!(normal_int, Some(42));
        assert_eq!(zigzag_int, Some(-7));
        assert_eq!(fixed_int, Some(999));
        assert_eq!(packed_numbers, vec![10, 20, 30]);
        assert_eq!(unpacked_numbers, vec![100, 200]);
        assert_eq!(packed_enums, vec![0, 1]); // First=0, Second=1
        assert_eq!(astring, Some("hello"));
        assert_eq!(manystrings, vec!["foo", "bar"]);
        assert_eq!(abytes, Some(b"raw".as_slice()));
        assert_eq!(manybytes, vec![b"a".as_slice(), b"b".as_slice()]);
        assert_eq!(yesno, Some(true));
    }

    #[test]
    fn test_decode_enums() {
        let mut buf = Vec::new();
        let mut writer = MsgWithEnums::new_writer(&mut buf, None);
        tacky_macros::write_proto!(
            writer,
            MsgWithEnums {
                enum1: Some(SimpleEnum::Second),
                enum2: [AnotherEnum::A, AnotherEnum::B],
            }
        );
        drop(writer);

        let mut remaining: &[u8] = &buf;
        let mut enum1 = None;
        let mut enum2: Vec<i32> = Vec::new();

        while !remaining.is_empty() {
            let Some(field) = MsgWithEnumsField::decode(&mut remaining).unwrap() else {
                continue;
            };
            match field {
                MsgWithEnumsField::Enum1(v) => enum1 = Some(v),
                MsgWithEnumsField::Enum2(v) => enum2.push(v),
            }
        }

        assert_eq!(enum1, Some(1)); // Second = 1
        assert_eq!(enum2, vec![0, 1]); // A=0, B=1
    }

    #[test]
    fn test_decode_nested() {
        let mut buf = Vec::new();
        let mut writer = MsgWithNesting::new_writer(&mut buf, None);
        tacky_macros::write_proto!(
            writer,
            MsgWithNesting {
                enums: {
                    writer.enums().write_msg(|mut m| {
                        tacky_macros::write_proto!(
                            m,
                            MsgWithEnums {
                                enum1: Some(SimpleEnum::First),
                                enum2: [AnotherEnum::B],
                            }
                        );
                    })
                },
                nested: {
                    let mut m = writer.nested();
                    m.append_msg_with(|mut n| {
                        tacky_macros::write_proto!(
                            n,
                            SimpleMessage {
                                normal_int: Some(77i64),
                                zigzag_int: None::<i64>,
                                fixed_int: None::<i64>,
                                manynumbers: Vec::<i32>::new(),
                                manynumbers_unpacked: Vec::<i32>::new(),
                                packed_enum: Vec::<SimpleEnum>::new(),
                                astring: Some("nested"),
                                manystrings: Vec::<&str>::new(),
                                abytes: None::<&[u8]>,
                                manybytes: Vec::<&[u8]>::new(),
                                yesno: None::<bool>,
                            }
                        );
                    });
                    m.close()
                }
            }
        );
        drop(writer);

        let mut remaining: &[u8] = &buf;
        let mut enums_bytes: Option<&[u8]> = None;
        let mut nested_msgs: Vec<&[u8]> = Vec::new();

        while !remaining.is_empty() {
            let Some(field) = MsgWithNestingField::decode(&mut remaining).unwrap() else {
                continue;
            };
            match field {
                MsgWithNestingField::Enums(b) => enums_bytes = Some(b),
                MsgWithNestingField::Nested(b) => nested_msgs.push(b),
            }
        }

        // Decode nested MsgWithEnums
        let mut sub = enums_bytes.unwrap();
        let mut inner_enum1 = None;
        let mut inner_enum2 = Vec::new();
        while !sub.is_empty() {
            let Some(field) = MsgWithEnumsField::decode(&mut sub).unwrap() else {
                continue;
            };
            match field {
                MsgWithEnumsField::Enum1(v) => inner_enum1 = Some(v),
                MsgWithEnumsField::Enum2(v) => inner_enum2.push(v),
            }
        }
        assert_eq!(inner_enum1, Some(0)); // First = 0
        assert_eq!(inner_enum2, vec![1]); // B = 1

        // Decode nested SimpleMessage
        assert_eq!(nested_msgs.len(), 1);
        let mut sub = nested_msgs[0];
        let mut normal_int = None;
        let mut astring = None;
        while !sub.is_empty() {
            let Some(field) = SimpleMessageField::decode(&mut sub).unwrap() else {
                continue;
            };
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
        // Manually construct bytes with an unknown field (tag=99, varint value=42)
        // followed by a known field (tag=10, varint yesno=1)
        let mut buf = Vec::new();
        // Unknown: tag=99, wire type VARINT => key = (99 << 3) | 0 = 792
        tacky::write_varint(792, &mut buf);
        tacky::write_varint(42, &mut buf); // some value
        // Known: tag=10, wire type VARINT => key = (10 << 3) | 0 = 80
        tacky::write_varint(80, &mut buf);
        tacky::write_varint(1, &mut buf); // true

        let mut remaining: &[u8] = &buf;
        let mut yesno = None;
        let mut skipped = 0;

        while !remaining.is_empty() {
            match SimpleMessageField::decode(&mut remaining).unwrap() {
                Some(SimpleMessageField::Yesno(v)) => yesno = Some(v),
                Some(_) => panic!("unexpected known field"),
                None => skipped += 1,
            }
        }

        assert_eq!(skipped, 1);
        assert_eq!(yesno, Some(true));
    }

    #[test]
    fn test_decode_wire_type_mismatch() {
        // Construct bytes with tag=1 (normal_int, expects VARINT) but wire type LEN
        let mut buf = Vec::new();
        // tag=1, wire type LEN(2) => key = (1 << 3) | 2 = 10
        tacky::write_varint(10, &mut buf);
        tacky::write_varint(3, &mut buf); // length 3
        buf.extend_from_slice(b"abc"); // some bytes

        let mut remaining: &[u8] = &buf;
        let result = SimpleMessageField::decode(&mut remaining);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("wire type mismatch"),
            "expected wire type mismatch error, got: {err}"
        );
    }
}
