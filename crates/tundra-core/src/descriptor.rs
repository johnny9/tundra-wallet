//! Public-only, deliberately narrow descriptor import. No secret-key parser is used.
use std::{collections::BTreeSet, str::FromStr};
use bdk_wallet::{bitcoin::{bip32::ChildNumber, hashes::{sha256, Hash}, NetworkKind},
    miniscript::{descriptor::{DescriptorPublicKey, Wildcard}, Descriptor, ForEachKey}, KeychainKind, Wallet};
use serde_json::Value;
use crate::{Error, ImportPreview, Network, Policy, Result};

pub const MAX_DESCRIPTOR_BYTES: usize = 32_768;

type PublicDescriptor = Descriptor<DescriptorPublicKey>;

fn parse_public(s: &str) -> Result<PublicDescriptor> {
    let s = s.trim();
    if s.len() > MAX_DESCRIPTOR_BYTES || !s.is_ascii() || s.chars().any(char::is_whitespace) {
        return Err(Error::Descriptor);
    }
    // Require a checksum; miniscript verifies its contents against the original expression.
    let (_, checksum) = s.split_once('#').ok_or(Error::Descriptor)?;
    if checksum.len() != 8 || s.matches('#').count() != 1 { return Err(Error::Descriptor); }
    PublicDescriptor::from_str(s).map_err(|_| Error::Descriptor)
}
fn body(d: &PublicDescriptor) -> String { format!("{d:#}") }

fn policy(d: &PublicDescriptor) -> Result<Policy> {
    match d {
        Descriptor::Wpkh(_) => Ok(Policy::SingleSig),
        Descriptor::Wsh(_) if body(d).starts_with("wsh(sortedmulti(2,") => {
            let mut count = 0;
            d.for_each_key(|_| { count += 1; true });
            if count == 3 { Ok(Policy::TwoOfThree) } else { Err(Error::UnsupportedPolicy) }
        }
        _ => Err(Error::UnsupportedPolicy),
    }
}

/// Validate account keys and return branch-independent identities in stable order.
fn identities(d: &PublicDescriptor, branch: u32, network: Network) -> Result<Vec<String>> {
    let mut identities = Vec::new();
    let mut key_material = BTreeSet::new();
    let mut fingerprints = BTreeSet::new();
    let mut error = None;
    d.for_each_key(|key| {
        let DescriptorPublicKey::XPub(x) = key else { error = Some(Error::UnsupportedKeys); return false; };
        let Some((fingerprint, origin_path)) = &x.origin else { error = Some(Error::UnsupportedKeys); return false; };
        if x.xkey.network != NetworkKind::from(network.bitcoin()) {
            error = Some(Error::NetworkMismatch); return false;
        }
        let path: Vec<_> = x.derivation_path.as_ref().to_vec();
        if x.wildcard != Wildcard::Unhardened || path != vec![ChildNumber::Normal { index: branch }]
            || origin_path.as_ref().is_empty() || x.xkey.depth as usize != origin_path.as_ref().len() {
            error = Some(Error::UnsupportedKeys); return false;
        }
        // Distinct origin strings alone are not enough: reject repeated account key material.
        let material = format!("{}:{}", x.xkey.public_key, x.xkey.chain_code);
        if !key_material.insert(material) || !fingerprints.insert(*fingerprint) {
            error = Some(Error::InvalidInput("duplicate key or master fingerprint")); return false;
        }
        identities.push(format!("[{fingerprint}/{origin_path}]{}", x.xkey));
        true
    });
    if let Some(error) = error { return Err(error); }
    identities.sort();
    Ok(identities)
}

fn input_descriptors(input: &str, network: Network) -> Result<Vec<String>> {
    if input.len() > MAX_DESCRIPTOR_BYTES { return Err(Error::InvalidInput("descriptor file is too large")); }
    let input = input.trim().trim_start_matches('\u{feff}');
    if !input.starts_with('{') {
        let lines: Vec<_> = input.lines().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).collect();
        if (1..=2).contains(&lines.len()) { return Ok(lines); }
        return Err(Error::InvalidInput("provide one multipath descriptor or a receive/change pair"));
    }
    let v: Value = serde_json::from_str(input).map_err(|_| Error::InvalidInput("descriptor JSON"))?;
    if let Some(n) = v.get("network") {
        if n.as_str() != Some(network.key()) { return Err(Error::NetworkMismatch); }
    }
    if let Some(d) = v.get("descriptor").and_then(Value::as_str) { return Ok(vec![d.into()]); }
    if let (Some(r),Some(c)) = (v.get("receive_descriptor").and_then(Value::as_str),v.get("change_descriptor").and_then(Value::as_str)) {
        return Ok(vec![r.into(),c.into()]);
    }
    if let Some(list) = v.get("descriptors").and_then(Value::as_array) {
        if list.len()!=2 { return Err(Error::BranchMismatch); }
        let mut receive=None; let mut change=None;
        for row in list {
            let d = row.get("desc").and_then(Value::as_str).ok_or(Error::Descriptor)?.to_owned();
            match row.get("internal").and_then(Value::as_bool) {
                Some(false) if receive.is_none() => receive=Some(d),
                Some(true) if change.is_none() => change=Some(d),
                _ => return Err(Error::BranchMismatch),
            }
        }
        return Ok(vec![receive.ok_or(Error::BranchMismatch)?,change.ok_or(Error::BranchMismatch)?]);
    }
    Err(Error::InvalidInput("unsupported descriptor JSON format"))
}

pub fn preview_import(input: &str, network: Network) -> Result<ImportPreview> {
    let rows = input_descriptors(input, network)?;
    let (receive, change) = if rows.len()==1 {
        let d = parse_public(&rows[0])?;
        policy(&d)?;
        let canonical = body(&d);
        // Narrow multipath support is intentional: never guess a missing change branch.
        let mut key_count = 0;
        d.for_each_key(|_| { key_count += 1; true });
        if canonical.matches("/<0;1>/*").count()!=key_count || key_count==0 {
            return Err(Error::BranchMismatch);
        }
        // The original checksum was checked before expansion. Reparse public keys only.
        let r = PublicDescriptor::from_str(&canonical.replace("/<0;1>/*","/0/*")).map_err(|_| Error::Descriptor)?;
        let c = PublicDescriptor::from_str(&canonical.replace("/<0;1>/*","/1/*")).map_err(|_| Error::Descriptor)?;
        (r,c)
    } else { (parse_public(&rows[0])?,parse_public(&rows[1])?) };
    let p = policy(&receive)?;
    if policy(&change)?!=p || identities(&receive,0,network)?!=identities(&change,1,network)? {
        return Err(Error::BranchMismatch);
    }
    let receive_descriptor=receive.to_string();
    let change_descriptor=change.to_string();
    let wallet=Wallet::create(receive_descriptor.clone(),change_descriptor.clone())
        .network(network.bitcoin()).create_wallet_no_persist().map_err(|_| Error::Descriptor)?;
    let id=sha256::Hash::hash(format!("tundra:v1:{}:{receive_descriptor}:{change_descriptor}",network.key()).as_bytes()).to_string();
    Ok(ImportPreview { id,network,policy:p,receive_descriptor,change_descriptor,
        first_address:wallet.peek_address(KeychainKind::External,0).address.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    const SINGLE:&str=include_str!("../../../tests/fixtures/single-sig.txt");
    const MULTI:&str=include_str!("../../../tests/fixtures/two-of-three.txt");
    #[test] fn single_sig() { assert_eq!(preview_import(SINGLE,Network::Signet).unwrap().policy,Policy::SingleSig); }
    #[test] fn two_of_three() { assert_eq!(preview_import(MULTI,Network::Signet).unwrap().policy,Policy::TwoOfThree); }
    #[test] fn checksums_required() { assert!(preview_import(SINGLE.split('#').next().unwrap(),Network::Signet).is_err()); }
    #[test] fn bad_checksum() { assert!(preview_import(&SINGLE.replace("2qugx73x","aaaaaaaa"),Network::Signet).is_err()); }
    #[test] fn no_network_guessing() { assert!(preview_import(SINGLE,Network::Mainnet).is_err()); }
    #[test] fn rejects_private_material() { assert!(preview_import("wpkh(xprv123/0/*)#aaaaaaaa",Network::Signet).is_err()); }
    #[test] fn no_missing_change_guess() { assert!(preview_import(include_str!("../../../tests/fixtures/receive-only.txt"),Network::Signet).is_err()); }
    #[test] fn core_pair_matches_multipath() {
        let pair=include_str!("../../../tests/fixtures/core-style-pair.json");
        assert_eq!(preview_import(pair,Network::Signet).unwrap().id,preview_import(MULTI,Network::Signet).unwrap().id);
    }
    #[test] fn oversized_import() { assert!(preview_import(&"a".repeat(MAX_DESCRIPTOR_BYTES+1),Network::Signet).is_err()); }
    #[test] fn receive_change_are_different() {
        let p=preview_import(SINGLE,Network::Signet).unwrap();
        assert_ne!(p.receive_descriptor,p.change_descriptor);
    }
}
