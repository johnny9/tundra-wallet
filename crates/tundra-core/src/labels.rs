//! BIP 329 tx/addr/output subset. Unknown types are skipped; null is not omission.
use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::{Error, Result};

pub const MAX_LABEL_FILE:usize=2*1024*1024;
pub const MAX_LABELS:usize=5000;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelRecord {
    #[serde(rename="type")] pub kind:String,
    #[serde(rename="ref")] pub reference:String,
    #[serde(skip_serializing_if="Option::is_none")] pub label:Option<String>,
    #[serde(skip_serializing_if="Option::is_none")] pub spendable:Option<bool>,
    #[serde(skip_serializing_if="Option::is_none")] pub origin:Option<String>,
}
#[derive(Debug,Clone,Serialize,Deserialize)]
pub struct LabelPreview { pub matched:u32,pub skipped:u32,pub changed:u32 }

pub fn validate_label(s:&str)->Result<()> {
    if s.chars().count()>255 || s.chars().any(char::is_control) {
        return Err(Error::InvalidInput("labels must be at most 255 characters without control characters"));
    }
    Ok(())
}
pub fn parse_labels(input:&str)->Result<(Vec<LabelRecord>,u32)> {
    if input.len()>MAX_LABEL_FILE { return Err(Error::InvalidInput("label file is too large")); }
    let mut records=Vec::new();let mut skipped=0;let mut seen=BTreeSet::new();
    for (index,line) in input.lines().filter(|line| !line.trim().is_empty()).enumerate() {
        if index>=MAX_LABELS { return Err(Error::InvalidInput("too many label records")); }
        let v:Value=serde_json::from_str(line).map_err(|_|Error::InvalidInput("invalid label JSON line"))?;
        let kind=v.get("type").and_then(Value::as_str).ok_or(Error::InvalidInput("label type"))?;
        if !["tx","addr","output"].contains(&kind) { skipped+=1;continue; }
        let reference=v.get("ref").and_then(Value::as_str).ok_or(Error::InvalidInput("label reference"))?;
        if reference.is_empty()||reference.len()>200 { return Err(Error::InvalidInput("label reference")); }
        let label=match v.get("label") {
            None=>None,Some(Value::String(s))=>{validate_label(s)?;Some(s.clone())},_=>return Err(Error::InvalidInput("label must be text")),
        };
        let spendable=match v.get("spendable") {
            None=>None,Some(Value::Bool(b)) if kind=="output"=>Some(*b),_=>return Err(Error::InvalidInput("spendable must be a boolean on outputs")),
        };
        let origin=match v.get("origin") {
            None=>None,Some(Value::String(s)) if s.len()<=1024=>Some(s.clone()),_=>return Err(Error::InvalidInput("label origin")),
        };
        if !seen.insert((kind.to_owned(),reference.to_owned())) { return Err(Error::InvalidInput("duplicate label reference")); }
        records.push(LabelRecord{kind:kind.into(),reference:reference.into(),label,spendable,origin});
    }
    Ok((records,skipped))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn omitted_fields_preserve_metadata(){let(r,_)=parse_labels(r#"{"type":"output","ref":"a:0"}"#).unwrap();assert!(r[0].label.is_none());assert!(r[0].spendable.is_none());}
    #[test] fn rejects_string_boolean(){assert!(parse_labels(r#"{"type":"output","ref":"a:0","spendable":"false"}"#).is_err());}
    #[test] fn rejects_null(){assert!(parse_labels(r#"{"type":"tx","ref":"a","label":null}"#).is_err());}
    #[test] fn unknown_types_skipped(){let(r,n)=parse_labels(r#"{"type":"future","ref":"x"}"#).unwrap();assert!(r.is_empty());assert_eq!(n,1);}
    #[test] fn preserves_unicode(){assert!(validate_label("Épargne 家族").is_ok());}
    #[test] fn label_size_is_characters(){assert!(validate_label(&"é".repeat(255)).is_ok());assert!(validate_label(&"é".repeat(256)).is_err());}
    #[test] fn duplicate_records_rejected(){let r="{\"type\":\"tx\",\"ref\":\"a\"}";assert!(parse_labels(&format!("{r}\n{r}")).is_err());}
    #[test] fn cannot_spend_a_transaction(){assert!(parse_labels(r#"{"type":"tx","ref":"x","spendable":true}"#).is_err());}
}
