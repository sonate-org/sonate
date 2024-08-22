use stablegui_common::ElId;

pub struct BufferTooSmall;

pub struct Element<'a> {
    pub value: ElId,
    pub parent: ElId,
    pub attributes: Vec<(&'a str, &'a str)>,
    pub string_value: &'a str,
}

struct Reader<'a> {
    data: &'a [u8],
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Reader { data }
    }

    fn read_u32(&mut self) -> Result<u32, ()> {
        if self.data.len() < 4 {
            return Err(());
        }

        let (bytes, rest) = self.data.split_at(4);
        self.data = rest;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn read_string(&mut self) -> Result<&'a str, ()> {
        let len = self.read_u32()? as usize;
        if self.data.len() < len {
            return Err(());
        }

        let (bytes, rest) = self.data.split_at(len);
        self.data = rest;
        Ok(std::str::from_utf8(bytes).map_err(|_| ())?)
    }
}

struct Writer<'a> {
    data: &'a mut Vec<u8>,
}

impl<'a> Writer<'a> {
    pub fn new(data: &'a mut Vec<u8>) -> Self {
        Writer { data }
    }

    pub fn write_u32(&mut self, value: u32) {
        self.data.extend_from_slice(&value.to_le_bytes());
    }

    pub fn write_string(&mut self, value: &str) {
        self.write_u32(value.len() as u32);
        self.data.extend_from_slice(value.as_bytes());
    }
}

pub fn decode_element(data: &[u8]) -> Result<Element, BufferTooSmall> {
    let mut reader = Reader::new(data);
    let value = reader.read_u32().unwrap();
    let parent = reader.read_u32().unwrap();
    let attribute_count = reader.read_u32().unwrap();
    let mut attributes = vec![];

    for _ in 0..attribute_count {
        let key = reader.read_string().unwrap();
        let value = reader.read_string().unwrap();
        attributes.push((key, value));
    }

    let string_value = reader.read_string().unwrap();

    Ok(Element {
        value,
        parent,
        attributes,
        string_value,
    })
}

pub fn encode_element(vec: &mut Vec<u8>, el: &Element) {
    let mut writer = Writer::new(vec);
    writer.write_u32(el.value);
    writer.write_u32(el.parent);
    writer.write_u32(el.attributes.len() as u32);

    for (key, value) in &el.attributes {
        writer.write_string(key);
        writer.write_string(value);
    }

    writer.write_string(el.string_value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(4, 4);
    }
}
