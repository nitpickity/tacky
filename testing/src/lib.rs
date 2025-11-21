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
        AnotherEnum, MsgWithEnums, MsgWithMaps, MsgWithNesting, SimpleEnum, SimpleMessage,
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
}
