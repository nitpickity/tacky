pub struct MySimpleMessageWriter<'buf> {
    tack: ::tacky::tack::Tack<'buf>,
        }
impl<'buf> MySimpleMessageWriter<'buf> {
fn new(buf: &'buf mut Vec<u8>, tag: Option<u32>) -> Self {
        Self {tack: ::tacky::tack::Tack::new(buf, tag)}    
    }
pub fn anumber(&mut self, anumber: impl Into<Option<i32>>) -> &mut Self {
                    if let Some(value) = anumber.into() {
                        ::tacky::scalars::write_varint(8, &mut self.tack.buffer);
                        ::tacky::scalars::write_int32(value, &mut self.tack.buffer);
                    }
                self
            }
pub fn astring<'opt>(&mut self, astring: impl Into<Option<&'opt str>>) -> &mut Self {
                        if let Some(value) = astring.into() {
                            ::tacky::scalars::write_varint(18, &mut self.tack.buffer);
                            ::tacky::scalars::write_string(value, &mut self.tack.buffer);
                        }
                    self
                }
pub fn manystrings<T: AsRef<str>>(&mut self, manystrings: impl IntoIterator<Item = T>) -> &mut Self {    
                    for value in manystrings {    
                        let value = value.as_ref();
                        ::tacky::scalars::write_varint(26, &mut self.tack.buffer);
                        ::tacky::scalars::write_string(value, &mut self.tack.buffer);
                    }
                self
            }
pub fn manybytes<T: AsRef<[u8]>>(&mut self, manybytes: impl IntoIterator<Item = T>) -> &mut Self {    
                    for value in manybytes {    
                        let value = value.as_ref();
                        ::tacky::scalars::write_varint(34, &mut self.tack.buffer);
                        ::tacky::scalars::write_bytes(value, &mut self.tack.buffer);
                    }
                self
            }
pub fn abytes<'opt>(&mut self, abytes: impl Into<Option<&'opt [u8]>>) -> &mut Self {
                        if let Some(value) = abytes.into() {
                            ::tacky::scalars::write_varint(42, &mut self.tack.buffer);
                            ::tacky::scalars::write_bytes(value, &mut self.tack.buffer);
                        }
                    self
                }
pub fn amap(&mut self, (key,value): (i32,&str)) -> &mut Self {
            todo!()
    }
}
