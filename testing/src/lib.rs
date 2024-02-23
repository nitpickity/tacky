mod prost_proto {
    include!(concat!(env!("OUT_DIR"), "/example.rs"));
    include!(concat!(env!("OUT_DIR"), "/useme.rs"));
}
mod tacky_proto {
    include!(concat!(env!("OUT_DIR"), "/simple.rs"));
    include!(concat!(env!("OUT_DIR"), "/useful.rs"));
}
#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};
    use std::net::IpAddr;
    use std::sync::Arc;
    use std::time::Duration;

    use prost::Message;
    use tacky::ProtoWrite;
    use tacky_macros::write_proto;

    use crate::prost_proto::{MySimpleMessage as PMySimpleMessage, SimpleEnum, StatData};
    use crate::tacky_proto;
    use crate::tacky_proto::example::{MySimpleMessage, MySimpleMessageWriter};

    #[test]
    fn write_trait() {
        struct Foo {
            a: String,
            b: HashMap<Arc<str>, i64>,
            c: Option<f64>,
        }
        impl ProtoWrite<tacky_proto::example::MySimpleMessage> for Foo {
            fn write_msg(
                &self,
                mut writer: <tacky_proto::example::MySimpleMessage as tacky::MessageSchema>::Writer<
                    '_,
                >,
            ) {
                write_proto!(
                    writer,
                    MySimpleMessage {
                        astring: Some(&self.a),
                        amap: self.b.iter().map(|(a, b)| { (*b as i32, a) }),
                        anumber: self.c.map(|f| f as i32)
                    }
                );
            }
        }
    }

    #[test]
    fn zero_len_msg() {
        // let mut m = MySimpleMessage::default();
        // m.nested = None;
        // m.anumber = 0;
        // println!("{}", m.encoded_len());
        // let v = m.encode_to_vec();
        // println!("{v:?}");
        // let d = MySimpleMessage::decode(&*v).unwrap();
        // println!("{d:?}")
    }
    #[test]
    fn it_works() {
        // data
        let anumber = Some(42);
        let manynumbers = vec![1, 2, 3];
        let astring = Some("Hello");
        let manystrings = vec!["many", "strings"];
        let manybytes: Vec<Vec<u8>> = Vec::new();
        let abytes = Vec::new();
        let amap: HashMap<i32, &str> = HashMap::new();

        let mut buf = Vec::new();
        {
            let mut writer = MySimpleMessageWriter::new(&mut buf, None);

            let k = tacky_macros::write_proto!(
                writer,
                exhaustive MySimpleMessage {
                    anumber,
                    manynumbers: &manynumbers,
                    manynumbers_unpacked: manynumbers.iter().copied(),
                    astring: fmt Some("hello"),
                    manystrings: &manystrings,
                    abytes: Some(&*abytes),
                    manybytes: &manybytes,
                    amap: amap.iter().map(|(k, v)| (*k, v)),
                    nested: with writer.nested_writer().write_msg_with(|m| {}),
                    numnum: Some(SimpleEnum::One as i32),
                }
            );
        }

        {
            let mut writer = MySimpleMessageWriter::new(&mut buf, None);

            //can borrow and iterate over everything
            let s = MySimpleMessage {
                anumber: writer.anumber(anumber),
                manynumbers: writer.manynumbers(&manynumbers),
                manynumbers_unpacked: writer.manynumbers_unpacked(manynumbers),
                astring: writer.astring(astring.as_deref()),
                manystrings: writer.manystrings(&manystrings),
                manybytes: writer.manybytes(&manybytes),
                abytes: writer.abytes(Some(&*abytes)),
                amap: writer.amap(amap.iter().map(|(k, v)| (*k, v))),
                nested: writer.nested(|mut n| {
                    n.astring(Some("hello nested"));
                    n.num(Some(42));
                    n.deeper(|mut d| {
                        d.levels(["some", "strings"]);
                    });
                    n.deeper(|mut d| {
                        d.levels(["rep", "str"]);
                    });
                }),
                numnum: writer.numnum(Some(SimpleEnum::One as i32)),
            };
        }

        let unpacked = PMySimpleMessage::decode(&*buf).unwrap();
        //prost can decode what tacky encodes
        // assert_eq!(unpacked, m);
    }
    struct Stats {
        ips: BTreeSet<IpAddr>,
        paths: BTreeSet<Arc<str>>,
        timings: Vec<Duration>,
    }

    #[inline(never)]
    fn test_example_tacky(data: &Stats) -> Vec<u8> {
        let mut buf = Vec::new();
        // //can borrow and iterate over everything
        // let mut writer = StatDataWriter::new(&mut buf, None);
        // let t0 = std::time::Instant::now();
        // writer
        //     .durations(data.timings.iter().map(|d| d.as_secs_f64())) // works from iterator
        //     .paths(&data.paths); // works from into-iterator

        // // for ip we can drop to the lower/higher level writer interface, which for protobuf string types implements a write_display fn.
        // //
        // // writer.ips(data.ips.iter().map(|s| s.to_string()));
        // let mut ip_writer = writer.ips_writer();
        // for ip in &data.ips {
        //     ip_writer.write_display(ip)
        // }
        // //and we're done, no copying or collecting data.
        // let t1 = t0.elapsed().as_micros();
        // println!("{t1}");
        // drop(writer);
        buf
    }
    #[inline(never)]
    fn test_example_prost(data: &Stats) -> Vec<u8> {
        let t0 = std::time::Instant::now();
        let prost_msg = StatData {
            ips: data.ips.iter().map(|ip| ip.to_string()).collect(),
            paths: data.paths.iter().map(|s| s.to_string()).collect(),
            durations: data.timings.iter().map(|d| d.as_secs_f64()).collect(),
        }
        .encode_to_vec();
        let t1 = t0.elapsed().as_micros();
        println!("{t1}");
        prost_msg
    }
    #[test]
    fn example_works() {
        // make some data
        let ips = BTreeSet::from_iter(
            [
                "10.0.0.2",
                "2001:0db8:85a3:0000:0000:8a2e:0370:7334",
                "10.0.0.3",
                "10.0.0.4",
                "10.0.0.5",
                "10.0.0.6",
                "10.0.0.2",
                "2001:0db8:85a3:0000:0000:8a2e:0370:1337",
                "2001:0db8:85a3:0000:0000:8a2e:0370:1331",
                "2001:0db8:85a3:0000:0000:8a2e:0370:1332",
                "2001:0db8:85a3:0000:0000:8a2e:0370:1333",
                "2001:0db8:85a3:0000:0000:8a2e:0370:1334",
                "2001:0db8:85a3:0000:0000:8a2e:0370:1335",
                "2001:0db8:85a3:0000:0000:8a2e:0370:1336",
                "2001:0db8:85a3:0000:0000:8a2e:0370:1338",
                "2001:0db8:85a3:0000:0000:8a2e:0370:1339",
                "2001:0db8:85a3:0000:0000:8a2e:0370:1340",
                "2001:0db8:85a3:0000:0000:8a2e:0370:1341",
            ]
            .iter()
            .map(|i| i.parse::<IpAddr>().unwrap()),
        );
        let paths: BTreeSet<Arc<str>> = BTreeSet::from_iter(
            [
                "/one", "/two", "/three", "/four", "/five", "/six", "/seven", "/eight", "/nine",
                "/ten",
            ]
            .iter()
            .map(|&s| s.into()),
        );
        let timings = [1, 2, 10]
            .repeat(10)
            .iter()
            .map(|secs| Duration::from_secs(*secs))
            .collect::<Vec<_>>();

        let data = Stats {
            ips,
            timings,
            paths,
        };

        let a = test_example_tacky(&data);
        let b = test_example_prost(&data);

        let unpacked1 = StatData::decode(&*a).unwrap();
        let unpacked2 = StatData::decode(&*b).unwrap();
        //prost can decode what tacky encodes
        assert_eq!(unpacked1, unpacked2);
    }
}
