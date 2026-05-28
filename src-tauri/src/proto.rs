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

fn skip_field(data: &[u8], pos: &mut usize, end: usize, wire_type: u32) -> bool {
    match wire_type {
        0 => {
            // Varint
            read_varint(data, pos).is_ok()
        }
        1 => {
            // 64-bit
            if *pos + 8 <= end {
                *pos += 8;
                true
            } else {
                false
            }
        }
        2 => {
            // Length-delimited
            if let Ok(len) = read_varint(data, pos) {
                let len = len as usize;
                if *pos + len <= end {
                    *pos += len;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        }
        5 => {
            // 32-bit
            if *pos + 4 <= end {
                *pos += 4;
                true
            } else {
                false
            }
        }
        _ => {
            // 3 (Start group), 4 (End group) 暂不支持，或者未知 wire_type 无法跳过
            false
        }
    }
}

pub fn parse_protobuf_orig(data: &[u8], pos: &mut usize, end: usize, verbose: bool) -> Result<HashMap<u32, Vec<ProtoValue>>, String> {
    let mut result: HashMap<u32, Vec<ProtoValue>> = HashMap::new();
    while *pos < end {
        let start_pos = *pos;
        let key = match read_varint(data, pos) {
            Ok(k) => k,
            Err(e) => {
                #[cfg(debug_assertions)]
                if verbose {
                    println!("Protobuf decode: read key varint failed at pos {}: {}", start_pos, e);
                }
                break;
            }
        };
        let wire_type = (key & 0x07) as u32;
        let field_num = (key >> 3) as u32;

        if field_num == 0 || field_num > (1 << 29) - 1 {
            #[cfg(debug_assertions)]
            if verbose {
                let err_len = (end - start_pos).min(16);
                println!(
                    "Protobuf decode: invalid field_num {} with wire_type {} at pos {}. Hex: {:02X?}",
                    field_num, wire_type, start_pos, &data[start_pos..start_pos+err_len]
                );
            }
            if !skip_field(data, pos, end, wire_type) {
                break;
            }
            continue;
        }

        match wire_type {
            0 => {
                match read_varint(data, pos) {
                    Ok(val) => {
                        result.entry(field_num).or_default().push(ProtoValue::Varint(val));
                    }
                    Err(e) => {
                        #[cfg(debug_assertions)]
                        if verbose {
                            let err_len = (end - start_pos).min(16);
                            println!(
                                "Protobuf decode: read varint value failed for field {} at pos {}: {}. Hex: {:02X?}",
                                field_num, *pos, e, &data[start_pos..start_pos+err_len]
                            );
                        }
                        break;
                    }
                }
            }
            1 => {
                if *pos + 8 > end {
                    #[cfg(debug_assertions)]
                    if verbose {
                        let err_len = (end - start_pos).min(16);
                        println!(
                            "Protobuf decode: Fixed64 out of bounds for field {} at pos {}. Hex: {:02X?}",
                            field_num, *pos, &data[start_pos..start_pos+err_len]
                        );
                    }
                    break;
                }
                let val = data[*pos..*pos + 8].to_vec();
                *pos += 8;
                result.entry(field_num).or_default().push(ProtoValue::Fixed64(val));
            }
            2 => {
                let length = match read_varint(data, pos) {
                    Ok(l) => l as usize,
                    Err(e) => {
                        #[cfg(debug_assertions)]
                        if verbose {
                            let err_len = (end - start_pos).min(16);
                            println!(
                                "Protobuf decode: read length for Bytes failed for field {} at pos {}: {}. Hex: {:02X?}",
                                field_num, *pos, e, &data[start_pos..start_pos+err_len]
                            );
                        }
                        break;
                    }
                };
                if *pos + length > end {
                    #[cfg(debug_assertions)]
                    if verbose {
                        let err_len = (end - start_pos).min(16);
                        println!(
                            "Protobuf decode: Bytes length {} out of bounds for field {} at pos {}. Hex: {:02X?}",
                            length, field_num, *pos, &data[start_pos..start_pos+err_len]
                        );
                    }
                    break;
                }
                let val = data[*pos..*pos + length].to_vec();
                *pos += length;
                result.entry(field_num).or_default().push(ProtoValue::Bytes(val));
            }
            5 => {
                if *pos + 4 > end {
                    #[cfg(debug_assertions)]
                    if verbose {
                        let err_len = (end - start_pos).min(16);
                        println!(
                            "Protobuf decode: Fixed32 out of bounds for field {} at pos {}. Hex: {:02X?}",
                            field_num, *pos, &data[start_pos..start_pos+err_len]
                        );
                    }
                    break;
                }
                let val = data[*pos..*pos + 4].to_vec();
                *pos += 4;
                result.entry(field_num).or_default().push(ProtoValue::Fixed32(val));
            }
            _ => {
                #[cfg(debug_assertions)]
                if verbose {
                    let err_len = (end - start_pos).min(16);
                    println!(
                        "Protobuf decode: unsupported wire type {} for field {} at pos {}. Hex: {:02X?}",
                        wire_type, field_num, start_pos, &data[start_pos..start_pos+err_len]
                    );
                }
                if !skip_field(data, pos, end, wire_type) {
                    break;
                }
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
                        if let Ok(sub_msg) = parse_protobuf_orig(&bytes, &mut pos, len, false) {
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
                            model_name = if rm == "gemini-3-flash-a" || rm == "gemini-3-flash-b" {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protobuf_fault_tolerance() {
        // 构建一个格式完好与损坏字节混合的 protobuf 流
        // Field 1: Varint = 150 (key: 1 << 3 | 0 = 8, val: 150 -> 0x96, 0x01)
        // Field 2: Invalid wire type 7 (key: 2 << 3 | 7 = 23)
        // Field 3: Length-delimited "hello" (key: 3 << 3 | 2 = 26, len: 5, bytes: b"hello")
        let data = vec![
            8, 0x96, 0x01, // Field 1: Varint(150)
            23, 0xAB, 0xCD, // Field 2: Invalid wire type 7 (应该被 skip_field 优雅跳过，0xAB, 0xCD 作为不可知字节跳过失败或直接终止，但前面成功的字段已保留)
            26, 5, b'h', b'e', b'l', b'l', b'o' // Field 3: Bytes("hello")
        ];

        let mut pos = 0;
        let res = parse_protobuf_orig(&data, &mut pos, data.len(), false);
        assert!(res.is_ok());
        
        let map = res.unwrap();
        // 验证即使中间存在不支持的 wire type 字段，之前成功解码的字段依然成功返回！
        assert!(map.contains_key(&1));
    }
}
