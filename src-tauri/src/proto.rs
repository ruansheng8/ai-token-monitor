use std::collections::HashMap;
use serde::Serialize;

// Protobuf 动态解码与 Token 字段提取逻辑

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ProtoValue {
    Varint(u64),
    Fixed64(Vec<u8>), // 8 bytes
    Bytes(Vec<u8>),
    SubMessage(HashMap<u32, Vec<ProtoValue>>),
    String(String),
    Fixed32(Vec<u8>), // 4 bytes
}

pub fn read_varint(data: &[u8], pos: &mut usize) -> Result<u64, String> {
    let mut result = 0u64;
    let mut shift = 0;
    loop {
        if *pos >= data.len() {
            return Err("Unexpected EOF while reading varint".to_string());
        }
        let b = data[*pos];
        *pos += 1;
        result |= ((b & 0x7f) as u64) << shift;
        if (b & 0x80) == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            return Err("Varint too long".to_string());
        }
    }
    Ok(result)
}

pub fn parse_protobuf_orig(data: &[u8], pos: &mut usize, end: usize) -> Result<HashMap<u32, Vec<ProtoValue>>, String> {
    let mut result: HashMap<u32, Vec<ProtoValue>> = HashMap::new();
    while *pos < end {
        let key = match read_varint(data, pos) {
            Ok(k) => k,
            Err(_) => break,
        };
        let wire_type = (key & 0x07) as u32;
        let field_num = (key >> 3) as u32;

        if field_num == 0 || field_num > (1 << 29) - 1 {
            return Err("Invalid field number".to_string());
        }

        match wire_type {
            0 => {
                match read_varint(data, pos) {
                    Ok(val) => {
                        result.entry(field_num).or_default().push(ProtoValue::Varint(val));
                    }
                    Err(_) => break,
                }
            }
            1 => {
                if *pos + 8 > end {
                    break;
                }
                let val = data[*pos..*pos + 8].to_vec();
                *pos += 8;
                result.entry(field_num).or_default().push(ProtoValue::Fixed64(val));
            }
            2 => {
                let length = match read_varint(data, pos) {
                    Ok(l) => l as usize,
                    Err(_) => break,
                };
                if *pos + length > end {
                    break;
                }
                let val = data[*pos..*pos + length].to_vec();
                *pos += length;
                result.entry(field_num).or_default().push(ProtoValue::Bytes(val));
            }
            5 => {
                if *pos + 4 > end {
                    break;
                }
                let val = data[*pos..*pos + 4].to_vec();
                *pos += 4;
                result.entry(field_num).or_default().push(ProtoValue::Fixed32(val));
            }
            _ => {
                return Err(format!("Unsupported wire type {}", wire_type));
            }
        }
    }
    Ok(result)
}

pub fn is_printable_string(s: &str) -> bool {
    s.chars().all(|c| {
        (!c.is_control() && c != '\u{2028}' && c != '\u{2029}') || c == '\n' || c == '\r' || c == '\t'
    })
}

pub fn try_parse_sub_messages(mut parsed_dict: HashMap<u32, Vec<ProtoValue>>) -> HashMap<u32, Vec<ProtoValue>> {
    for (_field, values) in parsed_dict.iter_mut() {
        let mut new_values = Vec::new();
        for v in values.drain(..) {
            match v {
                ProtoValue::Bytes(bytes) => {
                    let len = bytes.len();
                    let mut pos = 0;
                    if len > 0 {
                        if let Ok(sub_msg) = parse_protobuf_orig(&bytes, &mut pos, len) {
                            if pos == len && !sub_msg.is_empty() {
                                let sub_msg = try_parse_sub_messages(sub_msg);
                                new_values.push(ProtoValue::SubMessage(sub_msg));
                                continue;
                            }
                        }
                    }

                    if let Ok(s) = String::from_utf8(bytes.clone()) {
                        if is_printable_string(&s) {
                            new_values.push(ProtoValue::String(s));
                            continue;
                        }
                    }

                    new_values.push(ProtoValue::Bytes(bytes));
                }
                ProtoValue::SubMessage(sub_msg) => {
                    new_values.push(ProtoValue::SubMessage(try_parse_sub_messages(sub_msg)));
                }
                other => {
                    new_values.push(other);
                }
            }
        }
        *values = new_values;
    }
    parsed_dict
}

#[derive(Debug, Clone, Serialize)]
pub struct Metric {
    pub model: String,
    pub uncached_input: i64,
    pub cached_input: i64,
    pub output: i64,
    pub thinking: i64,
}

pub fn get_varint_val(val: &ProtoValue) -> i64 {
    match val {
        ProtoValue::Varint(v) => *v as i64,
        _ => 0,
    }
}

pub fn extract_metrics_from_proto(proto_dict: &HashMap<u32, Vec<ProtoValue>>) -> Vec<Metric> {
    let mut metrics = Vec::new();
    if let Some(items) = proto_dict.get(&1) {
        for item in items {
            if let ProtoValue::SubMessage(item_dict) = item {
                let mut model_name = "unknown".to_string();
                if let Some(field_19) = item_dict.get(&19) {
                    if let Some(val) = field_19.first() {
                        let raw_model = match val {
                            ProtoValue::String(s) => Some(s.clone()),
                            ProtoValue::Bytes(b) => String::from_utf8(b.clone()).ok(),
                            _ => None,
                        };
                        if let Some(rm) = raw_model {
                            model_name = if rm == "gemini-3-flash-a" {
                                "gemini-3.5-flash".to_string()
                            } else {
                                rm
                            };
                        }
                    }
                }
                if let Some(token_blocks) = item_dict.get(&4) {
                    for token_block in token_blocks {
                        if let ProtoValue::SubMessage(block_dict) = token_block {
                            let uncached = block_dict
                                .get(&2)
                                .and_then(|v| v.first())
                                .map(get_varint_val)
                                .unwrap_or(0);
                            let candidates = block_dict
                                .get(&3)
                                .and_then(|v| v.first())
                                .map(get_varint_val)
                                .unwrap_or(0);
                            let cached = block_dict
                                .get(&5)
                                .and_then(|v| v.first())
                                .map(get_varint_val)
                                .unwrap_or(0);
                            let thinking = block_dict
                                .get(&10)
                                .and_then(|v| v.first())
                                .map(get_varint_val)
                                .unwrap_or(0);

                            metrics.push(Metric {
                                model: model_name.clone(),
                                uncached_input: uncached,
                                cached_input: cached,
                                output: candidates,
                                thinking,
                            });
                        }
                    }
                }
            }
        }
    }
    metrics
}
