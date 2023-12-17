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
        let m = MySimpleMessage {
            anumber: anumber.clone(),
            manynumbers: manynumbers.clone(),
            astring: astring.clone(),
            manystrings: manystrings.clone(),
            manybytes: manybytes.clone(),
            abytes: Some(abytes.clone()),
            amap: amap.clone(),
        };

        let mut buf = Vec::new();
        {
            let mut writer = MySimpleMessageWriter::new(&mut buf, None);
            writer
                .anumber(anumber)
                .manynumbers(&manynumbers)
                .astring(astring.as_deref())
                .manystrings(&manystrings)
                .manybytes(&manybytes)
                .abytes(Some(&*abytes))
                .amap(amap.iter().map(|(a, b)| (a, b.as_str())));
        }

        let unpacked = MySimpleMessage::decode(&*buf).unwrap();
        assert_eq!(unpacked, m);
        // println!("{unpacked:#?}");
    }
}
