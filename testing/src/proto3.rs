//! Proto3 message tests: tacky encode → prost decode, and prost encode → tacky decode.

#[allow(dead_code)]
mod prost_proto3 {
    include!(concat!(env!("OUT_DIR"), "/proto3test.rs"));
}

#[cfg(test)]
mod tests {
    use prost::Message;
    use std::collections::HashMap;

    use super::prost_proto3;
    use crate::tacky_proto::proto3test::*;

    // --- Plain (implicit presence) scalars ---

    #[test]
    fn test_plain_scalars_tacky_to_prost() {
        let mut buf = Vec::new();
        let s = ScalarMessage::default();
        ScalarMessage {
            a_int32: s.a_int32.write(&mut buf, 42),
            a_int64: s.a_int64.write(&mut buf, -100i64),
            a_uint32: s.a_uint32.write(&mut buf, 300u32),
            a_uint64: s.a_uint64.write(&mut buf, u64::MAX),
            a_sint32: s.a_sint32.write(&mut buf, -50),
            a_sint64: s.a_sint64.write(&mut buf, i64::MIN),
            a_bool: s.a_bool.write(&mut buf, true),
            a_fixed32: s.a_fixed32.write(&mut buf, 0xDEAD_BEEFu32),
            a_fixed64: s.a_fixed64.write(&mut buf, 0xCAFE_BABE_DEAD_BEEFu64),
            a_sfixed32: s.a_sfixed32.write(&mut buf, i32::MIN),
            a_sfixed64: s.a_sfixed64.write(&mut buf, i64::MIN),
            a_float: s.a_float.write(&mut buf, 3.14f32),
            a_double: s.a_double.write(&mut buf, 2.71828f64),
            a_string: s.a_string.write(&mut buf, "hello"),
            a_bytes: s.a_bytes.write(&mut buf, [0xFFu8, 0x00].as_slice()),
        };

        let decoded = prost_proto3::ScalarMessage::decode(&*buf).unwrap();
        assert_eq!(decoded.a_int32, 42);
        assert_eq!(decoded.a_int64, -100);
        assert_eq!(decoded.a_uint32, 300);
        assert_eq!(decoded.a_uint64, u64::MAX);
        assert_eq!(decoded.a_sint32, -50);
        assert_eq!(decoded.a_sint64, i64::MIN);
        assert_eq!(decoded.a_bool, true);
        assert_eq!(decoded.a_fixed32, 0xDEAD_BEEF);
        assert_eq!(decoded.a_fixed64, 0xCAFE_BABE_DEAD_BEEF);
        assert_eq!(decoded.a_sfixed32, i32::MIN);
        assert_eq!(decoded.a_sfixed64, i64::MIN);
        assert_eq!(decoded.a_float, 3.14);
        assert_eq!(decoded.a_double, 2.71828);
        assert_eq!(decoded.a_string, "hello");
        assert_eq!(decoded.a_bytes, vec![0xFF, 0x00]);
    }

    #[test]
    fn test_plain_scalars_prost_to_tacky() {
        let prost_msg = prost_proto3::ScalarMessage {
            a_int32: -1,
            a_int64: i64::MAX,
            a_uint32: u32::MAX,
            a_uint64: 0,
            a_sint32: i32::MIN,
            a_sint64: 999,
            a_bool: false,
            a_fixed32: 42,
            a_fixed64: 42,
            a_sfixed32: -42,
            a_sfixed64: -42,
            a_float: -0.0,
            a_double: f64::INFINITY,
            a_string: "world".into(),
            a_bytes: vec![1, 2, 3],
        };

        let wire = prost_msg.encode_to_vec();
        let mut int32 = 0i32;
        let mut int64 = 0i64;
        let mut uint32 = 0u32;
        let mut sint32 = 0i32;
        let mut sint64 = 0i64;
        let mut a_bool = false;
        let mut fixed32 = 0u32;
        let mut fixed64 = 0u64;
        let mut sfixed32 = 0i32;
        let mut sfixed64 = 0i64;
        let mut float = 0.0f32;
        let mut double = 0.0f64;
        let mut string = "";
        let mut bytes: &[u8] = &[];

        for field in ScalarMessage::decode(&wire) {
            match field.unwrap() {
                ScalarMessageField::AInt32(v) => int32 = v,
                ScalarMessageField::AInt64(v) => int64 = v,
                ScalarMessageField::AUint32(v) => uint32 = v,
                ScalarMessageField::AUint64(_) => {} // 0 is default, not emitted
                ScalarMessageField::ASint32(v) => sint32 = v,
                ScalarMessageField::ASint64(v) => sint64 = v,
                ScalarMessageField::ABool(_) => a_bool = true, // false is default, not emitted
                ScalarMessageField::AFixed32(v) => fixed32 = v,
                ScalarMessageField::AFixed64(v) => fixed64 = v,
                ScalarMessageField::ASfixed32(v) => sfixed32 = v,
                ScalarMessageField::ASfixed64(v) => sfixed64 = v,
                ScalarMessageField::AFloat(v) => float = v,
                ScalarMessageField::ADouble(v) => double = v,
                ScalarMessageField::AString(v) => string = v,
                ScalarMessageField::ABytes(v) => bytes = v,
            }
        }

        assert_eq!(int32, -1);
        assert_eq!(int64, i64::MAX);
        assert_eq!(uint32, u32::MAX);
        // uint64 was 0 → not emitted (proto3 default)
        assert_eq!(sint32, i32::MIN);
        assert_eq!(sint64, 999);
        // bool was false → not emitted
        assert_eq!(a_bool, false);
        assert_eq!(fixed32, 42);
        assert_eq!(fixed64, 42);
        assert_eq!(sfixed32, -42);
        assert_eq!(sfixed64, -42);
        assert_eq!(float, -0.0);
        assert_eq!(double, f64::INFINITY);
        assert_eq!(string, "world");
        assert_eq!(bytes, &[1, 2, 3]);
    }

    #[test]
    fn test_plain_defaults_not_written() {
        // Proto3: default values (0, false, "") should not appear on the wire.
        let mut buf = Vec::new();
        let s = ScalarMessage::default();
        ScalarMessage {
            a_int32: s.a_int32.write(&mut buf, 0),
            a_int64: s.a_int64.write(&mut buf, 0i64),
            a_uint32: s.a_uint32.write(&mut buf, 0u32),
            a_uint64: s.a_uint64.write(&mut buf, 0u64),
            a_sint32: s.a_sint32.write(&mut buf, 0),
            a_sint64: s.a_sint64.write(&mut buf, 0i64),
            a_bool: s.a_bool.write(&mut buf, false),
            a_fixed32: s.a_fixed32.write(&mut buf, 0u32),
            a_fixed64: s.a_fixed64.write(&mut buf, 0u64),
            a_sfixed32: s.a_sfixed32.write(&mut buf, 0),
            a_sfixed64: s.a_sfixed64.write(&mut buf, 0i64),
            a_float: s.a_float.write(&mut buf, 0.0f32),
            a_double: s.a_double.write(&mut buf, 0.0f64),
            a_string: s.a_string.write(&mut buf, ""),
            a_bytes: s.a_bytes.write(&mut buf, [].as_slice()),
        };

        assert!(
            buf.is_empty(),
            "default values should not be written in proto3"
        );
    }

    // --- Optional (explicit presence) ---

    #[test]
    fn test_optional_fields_tacky_to_prost() {
        let mut buf = Vec::new();
        let s = OptionalMessage::default();
        OptionalMessage {
            opt_int: s.opt_int.write(&mut buf, Some(0)),
            opt_string: s.opt_string.write(&mut buf, Some("")),
            opt_bool: s.opt_bool.write(&mut buf, Some(false)),
            opt_double: s.opt_double.write(&mut buf, None::<f64>),
            opt_bytes: s.opt_bytes.write(&mut buf, None::<&[u8]>),
        };

        let decoded = prost_proto3::OptionalMessage::decode(&*buf).unwrap();
        // Proto3 optional: even default values written when explicitly set
        assert_eq!(decoded.opt_int, Some(0));
        assert_eq!(decoded.opt_string, Some("".into()));
        assert_eq!(decoded.opt_bool, Some(false));
        assert_eq!(decoded.opt_double, None);
        assert_eq!(decoded.opt_bytes, None);
    }

    #[test]
    fn test_optional_fields_prost_to_tacky() {
        let prost_msg = prost_proto3::OptionalMessage {
            opt_int: Some(42),
            opt_string: Some("test".into()),
            opt_bool: None,
            opt_double: Some(9.99),
            opt_bytes: Some(vec![0xAB]),
        };

        let wire = prost_msg.encode_to_vec();
        let mut opt_int = None;
        let mut opt_string = None;
        let mut opt_bool = None;
        let mut opt_double = None;
        let mut opt_bytes = None;

        for field in OptionalMessage::decode(&wire) {
            match field.unwrap() {
                OptionalMessageField::OptInt(v) => opt_int = Some(v),
                OptionalMessageField::OptString(v) => opt_string = Some(v),
                OptionalMessageField::OptBool(v) => opt_bool = Some(v),
                OptionalMessageField::OptDouble(v) => opt_double = Some(v),
                OptionalMessageField::OptBytes(v) => opt_bytes = Some(v),
            }
        }

        assert_eq!(opt_int, Some(42));
        assert_eq!(opt_string, Some("test"));
        assert_eq!(opt_bool, None);
        assert_eq!(opt_double, Some(9.99));
        assert_eq!(opt_bytes, Some([0xAB].as_slice()));
    }

    // --- Repeated (packed by default in proto3) ---

    #[test]
    fn test_repeated_tacky_to_prost() {
        let mut buf = Vec::new();
        let s = RepeatedMessage::default();
        RepeatedMessage {
            nums: s.nums.write(&mut buf, &[1, 2, 3]),
            strings: s.strings.write(&mut buf, &["a", "b"]),
            floats: s.floats.write(&mut buf, &[1.0f32, 2.0]),
            doubles: s.doubles.write(&mut buf, &[3.14, 2.71]),
            fix32s: s.fix32s.write(&mut buf, &[100u32, 200]),
            fix64s: s.fix64s.write(&mut buf, &[1000u64]),
            sfix32s: s.sfix32s.write(&mut buf, &[-1i32, -2]),
            sfix64s: s.sfix64s.write(&mut buf, &[-100i64]),
            bools: s.bools.write(&mut buf, &[true, false, true]),
            snums: s.snums.write(&mut buf, &[-10, 10]),
            unums: s.unums.write(&mut buf, &[u64::MAX, 0]),
            byte_arrays: s
                .byte_arrays
                .write(&mut buf, &[b"x".as_slice(), b"y".as_slice()]),
        };

        let decoded = prost_proto3::RepeatedMessage::decode(&*buf).unwrap();
        assert_eq!(decoded.nums, vec![1, 2, 3]);
        assert_eq!(decoded.strings, vec!["a", "b"]);
        assert_eq!(decoded.floats, vec![1.0, 2.0]);
        assert_eq!(decoded.doubles, vec![3.14, 2.71]);
        assert_eq!(decoded.fix32s, vec![100, 200]);
        assert_eq!(decoded.fix64s, vec![1000]);
        assert_eq!(decoded.sfix32s, vec![-1, -2]);
        assert_eq!(decoded.sfix64s, vec![-100]);
        assert_eq!(decoded.bools, vec![true, false, true]);
        assert_eq!(decoded.snums, vec![-10, 10]);
        assert_eq!(decoded.unums, vec![u64::MAX, 0]);
        assert_eq!(decoded.byte_arrays, vec![b"x".to_vec(), b"y".to_vec()]);
    }

    #[test]
    fn test_repeated_prost_to_tacky() {
        let prost_msg = prost_proto3::RepeatedMessage {
            nums: vec![10, 20, 30],
            strings: vec!["hello".into(), "world".into()],
            floats: vec![0.5],
            doubles: vec![1.0, 2.0, 3.0],
            fix32s: vec![u32::MAX],
            fix64s: vec![0, 1],
            sfix32s: vec![i32::MIN, i32::MAX],
            sfix64s: vec![0],
            bools: vec![false, true],
            snums: vec![i32::MIN, 0, i32::MAX],
            unums: vec![1, 2, 3],
            byte_arrays: vec![vec![], vec![1, 2]],
        };

        let wire = prost_msg.encode_to_vec();
        let mut nums = Vec::new();
        let mut strings = Vec::new();
        let mut floats = Vec::new();
        let mut doubles = Vec::new();
        let mut fix32s = Vec::new();
        let mut fix64s = Vec::new();
        let mut sfix32s = Vec::new();
        let mut sfix64s = Vec::new();
        let mut bools = Vec::new();
        let mut snums = Vec::new();
        let mut unums = Vec::new();
        let mut byte_arrays: Vec<&[u8]> = Vec::new();

        for field in RepeatedMessage::decode(&wire) {
            match field.unwrap() {
                RepeatedMessageField::Nums(iter) => nums.extend(iter.map(|r| r.unwrap())),
                RepeatedMessageField::Strings(s) => strings.push(s),
                RepeatedMessageField::Floats(iter) => floats.extend(iter.map(|r| r.unwrap())),
                RepeatedMessageField::Doubles(iter) => doubles.extend(iter.map(|r| r.unwrap())),
                RepeatedMessageField::Fix32s(iter) => fix32s.extend(iter.map(|r| r.unwrap())),
                RepeatedMessageField::Fix64s(iter) => fix64s.extend(iter.map(|r| r.unwrap())),
                RepeatedMessageField::Sfix32s(iter) => sfix32s.extend(iter.map(|r| r.unwrap())),
                RepeatedMessageField::Sfix64s(iter) => sfix64s.extend(iter.map(|r| r.unwrap())),
                RepeatedMessageField::Bools(iter) => bools.extend(iter.map(|r| r.unwrap())),
                RepeatedMessageField::Snums(iter) => snums.extend(iter.map(|r| r.unwrap())),
                RepeatedMessageField::Unums(iter) => unums.extend(iter.map(|r| r.unwrap())),
                RepeatedMessageField::ByteArrays(b) => byte_arrays.push(b),
            }
        }

        assert_eq!(nums, vec![10, 20, 30]);
        assert_eq!(strings, vec!["hello", "world"]);
        assert_eq!(floats, vec![0.5]);
        assert_eq!(doubles, vec![1.0, 2.0, 3.0]);
        assert_eq!(fix32s, vec![u32::MAX]);
        assert_eq!(fix64s, vec![0, 1]);
        assert_eq!(sfix32s, vec![i32::MIN, i32::MAX]);
        assert_eq!(sfix64s, vec![0]);
        assert_eq!(bools, vec![false, true]);
        assert_eq!(snums, vec![i32::MIN, 0, i32::MAX]);
        assert_eq!(unums, vec![1, 2, 3]);
        assert_eq!(byte_arrays, vec![[].as_slice(), [1u8, 2].as_slice()]);
    }

    #[test]
    fn test_repeated_empty() {
        let mut buf = Vec::new();
        let s = RepeatedMessage::default();
        RepeatedMessage {
            nums: s.nums.write(&mut buf, &[]),
            strings: s.strings.write(&mut buf, Vec::<&str>::new()),
            floats: s.floats.write(&mut buf, &[]),
            doubles: s.doubles.write(&mut buf, &[]),
            fix32s: s.fix32s.write(&mut buf, &[]),
            fix64s: s.fix64s.write(&mut buf, &[]),
            sfix32s: s.sfix32s.write(&mut buf, &[]),
            sfix64s: s.sfix64s.write(&mut buf, &[]),
            bools: s.bools.write(&mut buf, &[]),
            snums: s.snums.write(&mut buf, &[]),
            unums: s.unums.write(&mut buf, &[]),
            byte_arrays: s.byte_arrays.write(&mut buf, Vec::<&[u8]>::new()),
        };

        assert!(
            buf.is_empty(),
            "empty repeated fields should not be written"
        );
        let decoded = prost_proto3::RepeatedMessage::decode(&*buf).unwrap();
        assert_eq!(decoded, prost_proto3::RepeatedMessage::default());
    }

    // --- Enums ---

    #[test]
    fn test_enums_tacky_to_prost() {
        let mut buf = Vec::new();
        let s = WithEnum::default();
        WithEnum {
            status: s.status.write(&mut buf, Status::Active),
            history: s.history.write(
                &mut buf,
                &[Status::Active, Status::Inactive, Status::Unknown],
            ),
        };

        let decoded = prost_proto3::WithEnum::decode(&*buf).unwrap();
        assert_eq!(decoded.status, prost_proto3::Status::Active as i32);
        assert_eq!(
            decoded.history,
            vec![
                prost_proto3::Status::Active as i32,
                prost_proto3::Status::Inactive as i32,
                prost_proto3::Status::Unknown as i32,
            ]
        );
    }

    #[test]
    fn test_enums_prost_to_tacky() {
        let prost_msg = prost_proto3::WithEnum {
            status: prost_proto3::Status::Inactive as i32,
            history: vec![
                prost_proto3::Status::Active as i32,
                prost_proto3::Status::Unknown as i32,
            ],
        };

        let wire = prost_msg.encode_to_vec();
        let mut status = None;
        let mut history = Vec::new();

        for field in WithEnum::decode(&wire) {
            match field.unwrap() {
                WithEnumField::Status(v) => status = Some(v),
                WithEnumField::History(iter) => {
                    history.extend(iter.map(|r| Status::from(r.unwrap())));
                }
            }
        }

        assert_eq!(status, Some(Status::Inactive));
        assert_eq!(history, vec![Status::Active, Status::Unknown]);
    }

    // --- Nested messages ---

    #[test]
    fn test_nesting_tacky_to_prost() {
        let mut buf = Vec::new();
        let s = WithNesting::default();
        WithNesting {
            single: s.single.write_msg(&mut buf, |buf, s| {
                s.label.write(buf, "inner");
                s.value.write(buf, 42);
            }),
            many: {
                for (label, val) in [("a", 1), ("b", 2)] {
                    s.many.write_msg(&mut buf, |buf, s| {
                        s.label.write(buf, label);
                        s.value.write(buf, val);
                    });
                }
                s.many
            },
            name: s.name.write(&mut buf, "outer"),
        };

        let decoded = prost_proto3::WithNesting::decode(&*buf).unwrap();
        assert_eq!(
            decoded.single,
            Some(prost_proto3::Nested {
                label: "inner".into(),
                value: 42,
            })
        );
        assert_eq!(decoded.many.len(), 2);
        assert_eq!(decoded.many[0].label, "a");
        assert_eq!(decoded.many[0].value, 1);
        assert_eq!(decoded.many[1].label, "b");
        assert_eq!(decoded.many[1].value, 2);
        assert_eq!(decoded.name, "outer");
    }

    #[test]
    fn test_nesting_prost_to_tacky() {
        let prost_msg = prost_proto3::WithNesting {
            single: Some(prost_proto3::Nested {
                label: "inner".into(),
                value: 99,
            }),
            many: vec![prost_proto3::Nested {
                label: "x".into(),
                value: 10,
            }],
            name: "outer".into(),
        };

        let wire = prost_msg.encode_to_vec();
        let mut single_label = None;
        let mut single_value = None;
        let mut many_labels = Vec::new();
        let mut name = None;

        for field in WithNesting::decode(&wire) {
            match field.unwrap() {
                WithNestingField::Single(iter) => {
                    for f in iter {
                        match f.unwrap() {
                            NestedField::Label(v) => single_label = Some(v.to_string()),
                            NestedField::Value(v) => single_value = Some(v),
                        }
                    }
                }
                WithNestingField::Many(iter) => {
                    for f in iter {
                        if let NestedField::Label(v) = f.unwrap() {
                            many_labels.push(v.to_string());
                        }
                    }
                }
                WithNestingField::Name(v) => name = Some(v.to_string()),
            }
        }

        assert_eq!(single_label.as_deref(), Some("inner"));
        assert_eq!(single_value, Some(99));
        assert_eq!(many_labels, vec!["x"]);
        assert_eq!(name.as_deref(), Some("outer"));
    }

    // --- Maps ---

    #[test]
    fn test_maps_tacky_to_prost() {
        let mut buf = Vec::new();
        let s = WithMaps::default();
        WithMaps {
            str_int: s.str_int.write(&mut buf, [("a", 1), ("b", 2)]),
            int_str: s.int_str.write(&mut buf, [(10, "ten"), (20, "twenty")]),
            str_msg: {
                s.str_msg.write_msg(&mut buf, "key1", |buf, s| {
                    s.label.write(buf, "nested1");
                    s.value.write(buf, 111);
                });
                s.str_msg
            },
        };

        let decoded = prost_proto3::WithMaps::decode(&*buf).unwrap();
        assert_eq!(decoded.str_int.get("a"), Some(&1));
        assert_eq!(decoded.str_int.get("b"), Some(&2));
        assert_eq!(decoded.int_str.get(&10), Some(&"ten".to_string()));
        assert_eq!(decoded.int_str.get(&20), Some(&"twenty".to_string()));
        let nested = decoded.str_msg.get("key1").unwrap();
        assert_eq!(nested.label, "nested1");
        assert_eq!(nested.value, 111);
    }

    #[test]
    fn test_maps_prost_to_tacky() {
        let prost_msg = prost_proto3::WithMaps {
            str_int: HashMap::from([("x".into(), 10), ("y".into(), 20)]),
            int_str: HashMap::from([(1, "one".into())]),
            str_msg: HashMap::from([(
                "k".into(),
                prost_proto3::Nested {
                    label: "inside".into(),
                    value: 7,
                },
            )]),
        };

        let wire = prost_msg.encode_to_vec();
        let mut str_int = HashMap::new();
        let mut int_str = HashMap::new();
        let mut msg_label = None;

        for field in WithMaps::decode(&wire) {
            match field.unwrap() {
                WithMapsField::StrInt((k, v)) => {
                    if let Some(v) = v {
                        str_int.insert(k.to_string(), v);
                    }
                }
                WithMapsField::IntStr((k, v)) => {
                    if let Some(v) = v {
                        int_str.insert(k, v.to_string());
                    }
                }
                WithMapsField::StrMsg((k, v)) => {
                    if let Some(iter) = v {
                        for f in iter {
                            if let NestedField::Label(l) = f.unwrap() {
                                msg_label = Some((k.to_string(), l.to_string()));
                            }
                        }
                    }
                }
            }
        }

        assert_eq!(str_int, HashMap::from([("x".into(), 10), ("y".into(), 20)]));
        assert_eq!(int_str, HashMap::from([(1, "one".into())]));
        assert_eq!(msg_label, Some(("k".into(), "inside".into())));
    }

    // --- Oneof ---

    #[test]
    fn test_oneof_text_tacky_to_prost() {
        let mut buf = Vec::new();
        let s = WithOneof::default();
        WithOneof {
            id: s.id.write(&mut buf, "req-1"),
            payload: s.payload.write_text(&mut buf, "error msg"),
        };

        let decoded = prost_proto3::WithOneof::decode(&*buf).unwrap();
        assert_eq!(decoded.id, "req-1");
        assert_eq!(
            decoded.payload,
            Some(prost_proto3::with_oneof::Payload::Text("error msg".into()))
        );
    }

    #[test]
    fn test_oneof_number_tacky_to_prost() {
        let mut buf = Vec::new();
        let s = WithOneof::default();
        WithOneof {
            id: s.id.write(&mut buf, "req-2"),
            payload: s.payload.write_number(&mut buf, 404),
        };

        let decoded = prost_proto3::WithOneof::decode(&*buf).unwrap();
        assert_eq!(decoded.id, "req-2");
        assert_eq!(
            decoded.payload,
            Some(prost_proto3::with_oneof::Payload::Number(404))
        );
    }

    #[test]
    fn test_oneof_nested_tacky_to_prost() {
        let mut buf = Vec::new();
        let s = WithOneof::default();
        WithOneof {
            id: s.id.write(&mut buf, "req-3"),
            payload: s.payload.write_nested_msg(&mut buf, |buf, s| {
                s.label.write(buf, "payload");
                s.value.write(buf, 7);
            }),
        };

        let decoded = prost_proto3::WithOneof::decode(&*buf).unwrap();
        assert_eq!(decoded.id, "req-3");
        assert_eq!(
            decoded.payload,
            Some(prost_proto3::with_oneof::Payload::Nested(
                prost_proto3::Nested {
                    label: "payload".into(),
                    value: 7,
                }
            ))
        );
    }

    #[test]
    fn test_oneof_prost_to_tacky() {
        let prost_msg = prost_proto3::WithOneof {
            id: "req-4".into(),
            payload: Some(prost_proto3::with_oneof::Payload::Text("hello".into())),
        };

        let wire = prost_msg.encode_to_vec();
        let mut id = None;
        let mut text = None;

        for field in WithOneof::decode(&wire) {
            match field.unwrap() {
                WithOneofField::Id(v) => id = Some(v),
                WithOneofField::Text(v) => text = Some(v),
                _ => {}
            }
        }

        assert_eq!(id, Some("req-4"));
        assert_eq!(text, Some("hello"));
    }

    #[test]
    fn test_oneof_skipped() {
        let mut buf = Vec::new();
        let s = WithOneof::default();
        WithOneof {
            id: s.id.write(&mut buf, "req-5"),
            payload: s.payload,
        };

        let decoded = prost_proto3::WithOneof::decode(&*buf).unwrap();
        assert_eq!(decoded.id, "req-5");
        assert_eq!(decoded.payload, None);
    }
}
