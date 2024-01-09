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

    use crate::prost_proto::{MySimpleMessage, NestedMore, NestedMsg, StatData};
    use crate::tacky_proto::example::{MySimpleMessageSchema, MySimpleMessageWriter};
    use crate::tacky_proto::useme::StatDataWriter;

    #[test]
    fn zero_len_msg() {
        let mut m = MySimpleMessage::default();
        m.nested = None;
        m.anumber = 0;
        println!("{}", m.encoded_len());
        let v = m.encode_to_vec();
        println!("{v:?}");
        let d = MySimpleMessage::decode(&*v).unwrap();
        println!("{d:?}")
    }
    #[test]
    fn it_works() {
        // data
        let anumber = 42;
        let manynumbers = vec![1, 2, 3];
        let astring = Some("Hello".into());
        let manystrings = vec!["many".into(), "strings".into()];
        let manybytes = Vec::new();
        let abytes = Vec::new();
        let amap = HashMap::new();

        // needs to clone everything
        let m = MySimpleMessage {
            anumber: anumber.clone(),
            manynumbers: manynumbers.clone(),
            manynumbers_unpacked: manynumbers.clone(),
            astring: astring.clone(),
            manystrings: manystrings.clone(),
            manybytes: manybytes.clone(),
            abytes: Some(abytes.clone()),
            amap: amap.clone(),
            nested: Some(NestedMsg {
                num: Some(42),
                astring: Some("hello nested".into()),
                deeper: vec![
                    NestedMore {
                        levels: vec!["some".into(), "strings".into()],
                    },
                    NestedMore {
                        levels: vec!["rep".into(), "str".into()],
                    },
                ],
            }),
        };

        let mut buf = Vec::new();
        {
            //can borrow and iterate over everything
            let mut writer = MySimpleMessageWriter::new(&mut buf, None);
            let s = MySimpleMessageSchema {
                anumber: writer.anumber(Some(anumber)),
                manynumbers: writer.manynumbers(&manynumbers),
                manynumbers_unpacked: writer.manynumbers_unpacked(manynumbers.iter().copied()),
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
            };
        }

        let unpacked = MySimpleMessage::decode(&*buf).unwrap();
        //prost can decode what tacky encodes
        assert_eq!(unpacked, m);
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

    #[test]
    fn basic_bench() {
        // run with cargo test --release --package testing --lib -- tests::basic_bench --exact --nocapture
        // to show this beats prost by a lot for packed varints :)

        let data = (0..1_000_000).collect::<Vec<i32>>();
        {
            let prost_message = MySimpleMessage {
                manynumbers: data.clone(),
                ..Default::default()
            };
            let mut buf = Vec::with_capacity(4_000_000);
            let t0 = std::time::Instant::now();
            let _encoded = prost_message.encode(&mut buf);
            let t1 = t0.elapsed().as_micros();
            println!("prost took: {t1}, len: {}", buf.len())
        }

        {
            let mut buf = Vec::with_capacity(4_000_000);
            let mut w = MySimpleMessageWriter::new(&mut buf, None);
            let t0 = std::time::Instant::now();
            w.manynumbers(&data);
            let t1 = t0.elapsed().as_micros();
            drop(w);
            println!("tacky took: {t1}, len: {}", buf.len())
        }
    }
}
