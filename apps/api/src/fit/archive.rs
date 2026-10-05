use fitparser::{FitDataField, Value as FitValue, de::{DecodeOption, FitObject, FitStreamProcessor}};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use crate::{error::FitError, model::RawFitRecord};
use super::raw::{json_value, raw_record};
use super::runs::{self, ARCHIVE_SCHEMA_VERSION};

pub(super) enum Frame {
    Definition(Value),
    Message(Value, RawFitRecord),
}

pub(super) fn scan<F>(bytes:&[u8],emit:&mut F)->Result<Value,FitError>
where F:FnMut(Frame)->Result<(),FitError> {
    if bytes.len() < 12 || !matches!(bytes[0], 12 | 14) {
        return Err(FitError::InvalidFit);
    }
    let data_size = u32::from_le_bytes(bytes[4..8].try_into().map_err(|_| FitError::InvalidFit)?) as usize;
    let expected_size = (bytes[0] as usize).checked_add(data_size).and_then(|size| size.checked_add(2)).ok_or(FitError::InvalidFit)?;
    if bytes.len() != expected_size {
        return Err(FitError::InvalidFit);
    }
    let mut processor = FitStreamProcessor::new();
    processor.add_option(DecodeOption::KeepCompositeFields);
    processor.add_option(DecodeOption::PreserveInvalidValues);
    processor.add_option(DecodeOption::PreserveUnknownDeveloperFields);
    let mut input = bytes;
    let mut definition_count = 0usize;
    let mut active = HashMap::new();
    let mut message_count = 0usize;
    let mut applications: HashMap<u8, Value> = HashMap::new();
    let mut descriptions: HashMap<(u8,u8), Value> = HashMap::new();
    let mut headers = 0;
    let mut warnings = vec![json!({"code":"SUBFIELD_PROVENANCE_UNAVAILABLE","reason":"The pinned profile resolves subfield values but does not expose their reference predicates."})];
    while !input.is_empty() {
        let offset = bytes.len() - input.len();
        let wire = input;
        let (next, object) = processor.deserialize_next(input).map_err(|_| FitError::InvalidFit)?;
        let length = input.len() - next.len();
        if length == 0 { return Err(FitError::InvalidFit); }
        input = next;
        match object {
            FitObject::Header(header) => {
                headers += 1;
                if headers != 1 {
                    return Err(FitError::UnsupportedRun { code: "UNSUPPORTED_SESSION_LAYOUT", reason: "Chained FIT files are not supported." });
                }
                warnings.push(json!({"code":"PROFILE_COVERAGE_LIMIT","profileVersion":header.profile_ver_enc(),"decoderProfile":fitparser::profile::VERSION}));
            }
            FitObject::DefinitionMessage(definition) => {
                let id = definition_count;
                definition_count += 1;
                active.insert(definition.local_message_number(), (id, definition.clone()));
                emit(Frame::Definition(json!({
                    "index":id,"globalMessageNumber":definition.global_message_number(),
                    "localMessageNumber":definition.local_message_number(),
                    "byteOrder": if wire[2] == 0 { "little" } else { "big" },
                    "sourceReference":{"byteOffset":offset,"byteLength":length},
                    "fields":definition.field_definitions().iter().map(|f|json!({"fieldNumber":f.field_definition_number(),"size":f.size(),"baseType":f.base_type().as_i64()})).collect::<Vec<_>>(),
                    "developerFields":definition.developer_field_definitions().iter().map(|f|json!({"fieldNumber":f.field_number(),"size":f.size(),"developerDataIndex":f.developer_data_index()})).collect::<Vec<_>>()
                })))?;
            }
            FitObject::DataMessage(message) => {
                let local = if wire[0] & 0x80 != 0 { (wire[0] >> 5) & 3 } else { wire[0] & 15 };
                let (definition_id, definition) = active.get(&local).ok_or(FitError::InvalidFit)?;
                let global = message.global_message_number();
                let index = message_count;
                let record = processor.decode_message(message.clone()).map_err(|_| FitError::InvalidFit)?;
                if global == 207 {
                    if let Some(index) = message.fields().get(&3).and_then(numeric_u8) {
                        applications.insert(index, json!({"messageIndex":message_count,"applicationId":message.fields().get(&1).map(json_value)}));
                    }
                }
                if global == 206 {
                    if let (Some(dev),Some(number)) = (message.fields().get(&0).and_then(numeric_u8),message.fields().get(&1).and_then(numeric_u8)) {
                        descriptions.insert((dev,number), json!({"messageIndex":index,"baseType":message.fields().get(&2).map(json_value)}));
                    }
                }
                let mut fields = Vec::new();
                let mut byte = offset + 1;
                for definition in definition.field_definitions() {
                    let number = definition.field_definition_number();
                    let decoded = record.fields().iter().find(|f|f.number()==number && f.developer_data_index().is_none() && f.component_parent().is_none());
                    let raw = message.fields().get(&number);
                    let mut field = archived_field(index, global, number, None, decoded, raw);
                    field["baseType"] = json!(definition.base_type().as_i64());
                    field["byteSize"] = json!(definition.size());
                    field["sourceReference"]["byteOffset"] = json!(byte);
                    field["sourceReference"]["byteLength"] = json!(definition.size());
                    fields.push(field);
                    byte += definition.size() as usize;
                    for expanded in record.fields().iter().filter(|f|f.component_parent()==Some(number) && f.developer_data_index().is_none()) {
                        let mut field = archived_field(index, global, expanded.number(), None, Some(expanded), None);
                        field["role"] = json!("expanded");
                        field["componentParent"] = json!(number);
                        field["sourceReference"]["componentParent"] = json!(number);
                        field["sourceReference"]["byteOffset"] = json!(byte-definition.size() as usize);
                        field["sourceReference"]["byteLength"] = json!(definition.size());
                        fields.push(field);
                    }
                }
                for definition in definition.developer_field_definitions() {
                    let number = definition.field_number();
                    let dev = definition.developer_data_index();
                    let decoded = record.fields().iter().find(|f| f.number()==number && f.developer_data_index()==Some(dev));
                    let raw = message.developer_fields().get(&(dev,number));
                    let mut identity = json!({"developerDataIndex":dev,"fieldDefinitionNumber":number});
                    if let Some(application) = applications.get(&dev) {
                        identity["applicationId"] = application["applicationId"].clone();
                        identity["metadataReference"] = json!({"messageIndex":application["messageIndex"]});
                    }
                    let mut field = archived_field(index,global,number,Some(identity),decoded,raw);
                    field["byteSize"] = json!(definition.size());
                    field["sourceReference"]["byteOffset"] = json!(byte);
                    field["sourceReference"]["byteLength"] = json!(definition.size());
                    if let Some(description) = descriptions.get(&(dev,number)) {
                        field["descriptionReference"] = json!({"messageIndex":description["messageIndex"]});
                        field["baseType"] = description["baseType"].clone();
                    } else {
                        field["warnings"] = json!(["DEVELOPER_DESCRIPTION_UNAVAILABLE"]);
                    }
                    fields.push(field);
                    byte += definition.size() as usize;
                }
                if message.time_offset().is_some() {
                    if let Some(timestamp) = record.fields().iter().find(|f|f.number()==253) {
                        let mut field = archived_field(index,global,253,None,Some(timestamp),None);
                        field["role"] = json!("reconstructed");
                        field["sourceReference"]["byteOffset"] = json!(offset);
                        field["sourceReference"]["byteLength"] = json!(1);
                        field["compressedTimeOffset"] = json!(message.time_offset());
                        fields.push(field);
                    }
                }
                let mut archived = json!({"index":index,"globalMessageNumber":global,"localMessageNumber":local,
                    "definitionReference":definition_id,"sourceReference":{"messageIndex":index,"globalMessageNumber":global,"byteOffset":offset,"byteLength":length}});
                archived["fields"] = Value::Array(fields);
                emit(Frame::Message(archived,raw_record(&record)))?;
                message_count += 1;
            }
            FitObject::Crc(_) => processor.reset(),
        }
    }
    if headers == 0 { return Err(FitError::InvalidFit); }
    let mut decoder = runs::decoder_metadata();
    decoder["sourceSha256"] = json!(format!("{:x}",Sha256::digest(bytes)));
    decoder["sourceByteLength"] = json!(bytes.len());
    let mut result = json!({"schemaVersion":ARCHIVE_SCHEMA_VERSION,"decoder":decoder});
    result["warnings"] = Value::Array(warnings);
    Ok(result)
}

fn numeric_u8(value:&FitValue)->Option<u8> { json_value(value).as_u64().and_then(|v|u8::try_from(v).ok()) }

fn archived_field(index:usize,global:u16,number:u8,developer:Option<Value>,decoded:Option<&FitDataField>,raw:Option<&FitValue>)->Value {
    let composite = decoded.is_some_and(FitDataField::is_composite);
    let opaque = composite || (developer.is_some() && decoded.and_then(FitDataField::scale).is_none());
    let invalid = |value:&FitValue| matches!(value,FitValue::Invalid)
        || matches!(value,FitValue::String(text) if text.is_empty())
        || (!opaque && matches!(value,FitValue::Byte(255)));
    let mut value = decoded.map(|f|json_value(f.value())).or_else(||raw.map(json_value)).unwrap_or(Value::Null);
    if let Some(raw) = raw {
        if invalid(raw) {
            value = Value::Null;
        } else if let (FitValue::Array(raw_elements),Some(elements)) = (raw,value.as_array_mut()) {
            for (raw,element) in raw_elements.iter().zip(elements) {
                if invalid(raw) { *element = Value::Null; }
            }
        }
    }
    let raw_value = raw.map(json_value).unwrap_or(Value::Null);
    let validity = match raw {
        Some(value) if invalid(value) => "invalid",
        Some(FitValue::Array(values)) if values.iter().all(&invalid) => "invalid",
        Some(FitValue::Array(values)) if values.iter().any(&invalid) => "mixed",
        _ if value.is_null() => "invalid",
        _ => "valid",
    };
    let classification = if developer.is_some() {"unclassified"} else {classification(global,number)};
    let mut reference = json!({"messageIndex":index,"globalMessageNumber":global,"fieldNumber":number});
    if let Some(identity) = &developer { reference["developerIdentity"]=identity.clone(); }
    let mut field = json!({"fieldNumber":number,"name":decoded.map(|f|f.name()),"value":value,"rawValue":raw_value,"unit":decoded.and_then(|f|(!f.units().is_empty()).then_some(f.units())),"baseType":decoded.and_then(|f|f.profile_type()),"validity":validity,"role":"native","developerIdentity":developer,"classification":classification,"sourceReference":reference,"scale":decoded.and_then(|f|f.scale()),"offset":decoded.and_then(|f|f.offset())});
    field["profileType"] = json!(decoded.and_then(|f|f.profile_type()));
    field["isComposite"] = json!(composite);
    if decoded.is_some_and(|f|f.profile_type()==Some("local_date_time")) {
        field["value"] = raw.map(json_value).unwrap_or(Value::Null);
        field["warnings"] = json!(["LOCAL_TIMESTAMP_HAS_NO_ZONE"]);
    }
    if let Some(FitValue::Array(values)) = raw {
        field["elementValidity"] = json!(values.iter().map(|v|if invalid(v){"invalid"}else{"valid"}).collect::<Vec<_>>());
    }
    if decoded.is_some_and(|f|matches!(f.value(),FitValue::String(_))) && raw.is_some_and(|v|!matches!(v,FitValue::String(_))) {
        field["enumCode"] = raw.map(json_value).unwrap_or(Value::Null);
    }
    if field["developerIdentity"].is_null() && decoded.and_then(FitDataField::profile_type).is_none() {
        field["warnings"] = json!(["UNKNOWN_PROFILE_FIELD"]);
    }
    field
}

fn classification(global:u16,number:u8)->&'static str {
    match (global,number) {
        (20,0|1)|(18,3|4)|(19,3..=6)=>"location",
        (0,3)|(23,3|27|29)|(207,0|1|4)=>"deviceIdentifiers",
        (18|19|20|21|23|34|132|216,253)|(18|19,2)|(34,5)|(132,0|1|9|10)|(2,1|2|5|39)=>"time",
        (20,2..=9|13|29|30|39..=43|53|73|78)|(18,7..=9|14|16|18|20|59|92|124)
            |(19,7..=9|13|15|17|19|52|80|110)|(78,0)|(7,1..=3)|(8|9,1)
            |(10,1..=3)|(53|131,0)|(216,2..=9|11..=13|15)=>"metric",
        (0,0|1|2)|(18,5|6)|(21,0|1)|(7,5|7)|(8|9|10|53|131,254)|(12,0|1)
            |(216,0|1|10|14)|(2,0|4)|(23,0|1|2|4|5|10|11|18|19|20|21|22|25)=>"structure",
        _=>"unclassified",
    }
}
