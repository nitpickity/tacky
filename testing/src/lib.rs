mod prost_proto {

    include!(concat!(env!("OUT_DIR"), "/example.rs"));
}
mod tacky_proto {
    include!(concat!(env!("OUT_DIR"), "/simple.rs"));
}
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use prost::Message;

    use crate::prost_proto::MySimpleMessage;
    use crate::tacky_proto::MySimpleMessageWriter;

    #[test]
    fn it_works() {
        // data
        let anumber = Some(42);
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
        };

        let mut buf = Vec::new();
        {
            //can borrow and iterate over everything
            let mut writer = MySimpleMessageWriter::new(&mut buf, None);
            writer
                .anumber(anumber)
                .manynumbers(&manynumbers)
                .manynumbers_unpacked(&manynumbers)
                .astring(astring.as_deref())
                .manystrings(&manystrings)
                .manybytes(&manybytes)
                .abytes(Some(&*abytes))
                .amap(amap.iter().map(|(a, b)| (a, b.as_str())));
        }

        let unpacked = MySimpleMessage::decode(&*buf).unwrap();
        //prost can decode what tacky encodes
        assert_eq!(unpacked, m);
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
            let t0 = std::time::Instant::now();
            let encoded = prost_message.encode_to_vec();
            let t1 = t0.elapsed().as_micros();
            println!("prost took: {t1}, len: {}", encoded.len())
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
