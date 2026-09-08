//! Public-only, deliberately narrow descriptor import. No secret-key parser is used.
use crate::{Error, ImportPreview, Network, Policy, Result};
use bdk_wallet::{
    KeychainKind, Wallet,
    bitcoin::{
        NetworkKind,
        bip32::ChildNumber,
        hashes::{Hash, sha256},
    },
    miniscript::{
        Descriptor, ForEachKey,
        descriptor::{DescriptorPublicKey, Wildcard, WshInner},
    },
};
use serde_json::Value;
use std::{collections::BTreeSet, str::FromStr};

pub const MAX_DESCRIPTOR_BYTES: usize = 32_768;

type PublicDescriptor = Descriptor<DescriptorPublicKey>;

fn parse_public(s: &str) -> Result<PublicDescriptor> {
    let s = s.trim();
    if s.len() > MAX_DESCRIPTOR_BYTES || !s.is_ascii() || s.chars().any(char::is_whitespace) {
        return Err(Error::Descriptor);
    }
    // Require a checksum; miniscript verifies its contents against the original expression.
    let (_, checksum) = s.split_once('#').ok_or(Error::Descriptor)?;
    if checksum.len() != 8 || s.matches('#').count() != 1 {
        return Err(Error::Descriptor);
    }
    PublicDescriptor::from_str(s).map_err(|_| Error::Descriptor)
}
fn body(d: &PublicDescriptor) -> String {
    format!("{d:#}")
}

fn policy(d: &PublicDescriptor) -> Result<Policy> {
    match d {
        Descriptor::Wpkh(_) => Ok(Policy::SingleSig),
        Descriptor::Wsh(wsh) if matches!(wsh.as_inner(), WshInner::SortedMulti(keys) if keys.k() == 2 && keys.n() == 3) => {
            Ok(Policy::TwoOfThree)
        }
        _ => Err(Error::UnsupportedPolicy),
    }
}

/// sortedmulti sorts the derived script keys. Its source key order must not create
/// a second wallet identity (and a second set of labels, freezes and reservations).
pub(crate) fn canonical_descriptor(d: &PublicDescriptor) -> Result<PublicDescriptor> {
    if let Descriptor::Wsh(wsh) = d
        && let WshInner::SortedMulti(multi) = wsh.as_inner()
    {
        let mut keys = multi.pks().to_vec();
        keys.sort();
        return PublicDescriptor::new_wsh_sortedmulti(multi.k(), keys)
            .map_err(|_| Error::Descriptor);
    }
    Ok(d.clone())
}

/// Validate account keys and return branch-independent identities in stable order.
fn identities(d: &PublicDescriptor, branch: u32, network: Network) -> Result<Vec<String>> {
    let mut identities = Vec::new();
    let mut key_material = BTreeSet::new();
    let mut fingerprints = BTreeSet::new();
    let mut error = None;
    d.for_each_key(|key| {
        let DescriptorPublicKey::XPub(x) = key else {
            error = Some(Error::UnsupportedKeys);
            return false;
        };
        let Some((fingerprint, origin_path)) = &x.origin else {
            error = Some(Error::UnsupportedKeys);
            return false;
        };
        if x.xkey.network != NetworkKind::from(network.bitcoin()) {
            error = Some(Error::NetworkMismatch);
            return false;
        }
        let path: Vec<_> = x.derivation_path.as_ref().to_vec();
        if x.wildcard != Wildcard::Unhardened
            || path != vec![ChildNumber::Normal { index: branch }]
            || origin_path.as_ref().is_empty()
            || x.xkey.depth as usize != origin_path.as_ref().len()
        {
            error = Some(Error::UnsupportedKeys);
            return false;
        }
        // Distinct origin strings alone are not enough: reject repeated account key material.
        let material = format!("{}:{}", x.xkey.public_key, x.xkey.chain_code);
        if !key_material.insert(material) || !fingerprints.insert(*fingerprint) {
            error = Some(Error::InvalidInput("duplicate key or master fingerprint"));
            return false;
        }
        identities.push(format!("[{fingerprint}/{origin_path}]{}", x.xkey));
        true
    });
    if let Some(error) = error {
        return Err(error);
    }
    identities.sort();
    Ok(identities)
}

fn input_descriptors(input: &str, network: Network) -> Result<Vec<String>> {
    if input.len() > MAX_DESCRIPTOR_BYTES {
        return Err(Error::InvalidInput("descriptor file is too large"));
    }
    let input = input.trim().trim_start_matches('\u{feff}');
    if !input.starts_with('{') {
        let lines: Vec<_> = input
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        if (1..=2).contains(&lines.len()) {
            return Ok(lines);
        }
        return Err(Error::InvalidInput(
            "provide one multipath descriptor or a receive/change pair",
        ));
    }
    let v: Value =
        serde_json::from_str(input).map_err(|_| Error::InvalidInput("descriptor JSON"))?;
    if let Some(n) = v.get("network")
        && n.as_str() != Some(network.key())
    {
        return Err(Error::NetworkMismatch);
    }
    if let Some(d) = v.get("descriptor").and_then(Value::as_str) {
        return Ok(vec![d.into()]);
    }
    if let (Some(r), Some(c)) = (
        v.get("receive_descriptor").and_then(Value::as_str),
        v.get("change_descriptor").and_then(Value::as_str),
    ) {
        return Ok(vec![r.into(), c.into()]);
    }
    if let Some(list) = v.get("descriptors").and_then(Value::as_array) {
        if list.len() != 2 {
            return Err(Error::BranchMismatch);
        }
        let mut receive = None;
        let mut change = None;
        for row in list {
            let d = row
                .get("desc")
                .and_then(Value::as_str)
                .ok_or(Error::Descriptor)?
                .to_owned();
            match row.get("internal").and_then(Value::as_bool) {
                Some(false) if receive.is_none() => receive = Some(d),
                Some(true) if change.is_none() => change = Some(d),
                _ => return Err(Error::BranchMismatch),
            }
        }
        return Ok(vec![
            receive.ok_or(Error::BranchMismatch)?,
            change.ok_or(Error::BranchMismatch)?,
        ]);
    }
    Err(Error::InvalidInput("unsupported descriptor JSON format"))
}

pub fn preview_import(input: &str, network: Network) -> Result<ImportPreview> {
    let rows = input_descriptors(input, network)?;
    let (receive, change) = if rows.len() == 1 {
        let d = parse_public(&rows[0])?;
        policy(&d)?;
        let canonical = body(&d);
        // Narrow multipath support is intentional: never guess a missing change branch.
        let mut key_count = 0;
        d.for_each_key(|_| {
            key_count += 1;
            true
        });
        if canonical.matches("/<0;1>/*").count() != key_count || key_count == 0 {
            return Err(Error::BranchMismatch);
        }
        // The original checksum was checked before expansion. Reparse public keys only.
        let r = PublicDescriptor::from_str(&canonical.replace("/<0;1>/*", "/0/*"))
            .map_err(|_| Error::Descriptor)?;
        let c = PublicDescriptor::from_str(&canonical.replace("/<0;1>/*", "/1/*"))
            .map_err(|_| Error::Descriptor)?;
        (r, c)
    } else {
        (parse_public(&rows[0])?, parse_public(&rows[1])?)
    };
    let p = policy(&receive)?;
    if policy(&change)? != p
        || identities(&receive, 0, network)? != identities(&change, 1, network)?
    {
        return Err(Error::BranchMismatch);
    }
    let receive_descriptor = canonical_descriptor(&receive)?.to_string();
    let change_descriptor = canonical_descriptor(&change)?.to_string();
    let wallet = Wallet::create(receive_descriptor.clone(), change_descriptor.clone())
        .network(network.bitcoin())
        .create_wallet_no_persist()
        .map_err(|_| Error::Descriptor)?;
    let id = sha256::Hash::hash(
        format!(
            "tundra:v1:{}:{receive_descriptor}:{change_descriptor}",
            network.key()
        )
        .as_bytes(),
    )
    .to_string();
    Ok(ImportPreview {
        id,
        network,
        policy: p,
        receive_descriptor,
        change_descriptor,
        first_address: wallet
            .peek_address(KeychainKind::External, 0)
            .address
            .to_string(),
    })
}

/// BIP 329's abbreviated descriptor contains origins only, not xpubs/child branches.
pub(crate) fn label_origin(descriptor: &PublicDescriptor) -> Result<String> {
    let mut origins = Vec::new();
    descriptor.for_each_key(|key| {
        if let DescriptorPublicKey::XPub(key) = key
            && let Some((fingerprint, path)) = &key.origin
        {
            origins.push(format!("[{fingerprint}/{path}]"));
        }
        true
    });
    origins.sort();
    match policy(descriptor)? {
        Policy::SingleSig if origins.len() == 1 => Ok(format!("wpkh({})", origins[0])),
        Policy::TwoOfThree if origins.len() == 3 => {
            Ok(format!("wsh(sortedmulti(2,{}))", origins.join(",")))
        }
        _ => Err(Error::CorruptState),
    }
}
pub(crate) fn normalize_label_origin(input: &str) -> Option<String> {
    use bdk_wallet::bitcoin::bip32::{DerivationPath, Fingerprint};
    if input.len() > 1024 || input.chars().any(char::is_whitespace) {
        return None;
    }
    let (body, multi) = if let Some(body) = input
        .strip_prefix("wpkh(")
        .and_then(|s| s.strip_suffix(')'))
    {
        (body, false)
    } else {
        (
            input
                .strip_prefix("wsh(sortedmulti(2,")?
                .strip_suffix("))")?,
            true,
        )
    };
    let mut origins = Vec::new();
    for key in body.split(',') {
        let key = key.strip_prefix('[')?.strip_suffix(']')?;
        let (fingerprint, path) = key.split_once('/')?;
        let fingerprint = Fingerprint::from_str(fingerprint).ok()?;
        let path = DerivationPath::from_str(path).ok()?;
        if path.as_ref().is_empty() {
            return None;
        }
        origins.push(format!("[{fingerprint}/{path}]"));
    }
    origins.sort();
    if origins.windows(2).any(|v| v[0] == v[1]) {
        return None;
    }
    match (multi, origins.len()) {
        (false, 1) => Some(format!("wpkh({})", origins[0])),
        (true, 3) => Some(format!("wsh(sortedmulti(2,{}))", origins.join(","))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abbreviated_origins_canonicalize_multisig_key_order_and_hardened_notation() {
        let left = "wsh(sortedmulti(2,[deadbeef/48'/1'/0'/2'],[aaaaaaaa/48'/1'/0'/2'],[bbbbbbbb/48'/1'/0'/2']))";
        let right = "wsh(sortedmulti(2,[BBBBBBBB/48h/1h/0h/2h],[DEADBEEF/48h/1h/0h/2h],[AAAAAAAA/48h/1h/0h/2h]))";
        assert_eq!(normalize_label_origin(left), normalize_label_origin(right));
        assert!(normalize_label_origin(left).is_some());
        for bad in [
            "wpkh([deadbeef])",
            "wpkh([deadbeef/84h/1h/0h]xpub)",
            "wpkh([deadbeef/84h/1h/0h])/0/*",
            "wpkh([deadbeef/84h/1h/0h])#checksum",
            "wsh(sortedmulti(2,[deadbeef/48h/1h/0h/2h],[deadbeef/48h/1h/0h/2h],[aaaaaaaa/48h/1h/0h/2h]))",
        ] {
            assert!(normalize_label_origin(bad).is_none());
        }
    }
    const SINGLE: &str = include_str!("../../../tests/fixtures/single-sig.txt");
    const MULTI: &str = include_str!("../../../tests/fixtures/two-of-three.txt");

    fn checksummed(body: &str) -> String {
        let checksum = bdk_wallet::miniscript::descriptor::checksum::desc_checksum(body).unwrap();
        format!("{body}#{checksum}")
    }

    fn multi_keys() -> Vec<String> {
        MULTI
            .trim()
            .split('#')
            .next()
            .unwrap()
            .strip_prefix("wsh(sortedmulti(2,")
            .unwrap()
            .strip_suffix("))")
            .unwrap()
            .split(',')
            .map(str::to_owned)
            .collect()
    }

    fn multi(keys: &[String]) -> String {
        checksummed(&format!("wsh(sortedmulti(2,{}))", keys.join(",")))
    }

    #[test]
    fn multisig_key_order_does_not_change_wallet_identity() {
        let keys = multi_keys();
        let original = preview_import(MULTI, Network::Signet).unwrap();
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let reordered = multi(&order.map(|i| keys[i].clone()));
            let preview = preview_import(&reordered, Network::Signet).unwrap();
            assert_eq!(preview.first_address, original.first_address);
            assert_eq!(preview.id, original.id);
            assert_eq!(preview.receive_descriptor, original.receive_descriptor);
            assert_eq!(preview.change_descriptor, original.change_descriptor);
        }
    }

    #[test]
    fn receive_and_change_may_list_multisig_keys_in_different_orders() {
        let keys = multi_keys();
        let receive = multi(
            &keys
                .iter()
                .map(|k| k.replace("/<0;1>/*", "/0/*"))
                .collect::<Vec<_>>(),
        );
        let change = multi(
            &keys
                .iter()
                .rev()
                .map(|k| k.replace("/<0;1>/*", "/1/*"))
                .collect::<Vec<_>>(),
        );
        let preview = preview_import(&format!("{receive}\n{change}"), Network::Signet).unwrap();
        assert_eq!(
            preview.id,
            preview_import(MULTI, Network::Signet).unwrap().id
        );
    }

    #[test]
    fn duplicate_account_key_is_rejected_even_with_a_different_origin() {
        for change_origin in [false, true] {
            let mut keys = multi_keys();
            keys[1] = if change_origin {
                keys[0].replace("a1b2c3d4", "1122aabb")
            } else {
                keys[0].clone()
            };
            assert!(preview_import(&multi(&keys), Network::Signet).is_err());
        }
    }

    #[test]
    fn different_accounts_with_the_same_master_fingerprint_are_rejected() {
        let mut keys = multi_keys();
        keys[1] = keys[1].replace("1122aabb", "a1b2c3d4");
        assert!(preview_import(&multi(&keys), Network::Signet).is_err());
    }

    #[test]
    fn public_derivation_requires_origins_and_unhardened_ranges() {
        let original = SINGLE.trim().split('#').next().unwrap();
        let origin_start = original.find('[').unwrap();
        let origin_end = original.find(']').unwrap() + 1;
        let without_origin = format!("{}{}", &original[..origin_start], &original[origin_end..]);
        for invalid in [
            without_origin,
            original.replace("/<0;1>/*", "/<0;1>/*h"),
            original.replace("/<0;1>/*", "/<0h;1h>/*"),
            original.replace("/<0;1>/*", "/<0;1>/0/*"),
            original.replace("/<0;1>/*", "/<1;0>/*"),
            original.replace("/<0;1>/*", "/<0;2>/*"),
        ] {
            assert!(preview_import(&checksummed(&invalid), Network::Signet).is_err());
        }
    }

    #[test]
    fn mismatched_receive_change_accounts_and_policies_are_rejected() {
        let single = preview_import(SINGLE, Network::Signet).unwrap();
        let original = preview_import(MULTI, Network::Signet).unwrap();
        let changed = original
            .change_descriptor
            .split('#')
            .next()
            .unwrap()
            .replace("48'", "49'");
        // Use a same-depth origin change, which must not be accepted as the receive policy.
        assert_ne!(
            changed,
            original.change_descriptor.split('#').next().unwrap()
        );
        for change in [
            single.change_descriptor,
            checksummed(&changed),
            original.receive_descriptor.clone(),
        ] {
            assert!(
                preview_import(
                    &format!("{}\n{change}", original.receive_descriptor),
                    Network::Signet
                )
                .is_err()
            );
        }
    }
    #[test]
    fn single_sig() {
        assert_eq!(
            preview_import(SINGLE, Network::Signet).unwrap().policy,
            Policy::SingleSig
        );
    }
    #[test]
    fn two_of_three() {
        assert_eq!(
            preview_import(MULTI, Network::Signet).unwrap().policy,
            Policy::TwoOfThree
        );
    }
    #[test]
    fn checksums_required() {
        assert!(preview_import(SINGLE.split('#').next().unwrap(), Network::Signet).is_err());
    }
    #[test]
    fn bad_checksum() {
        assert!(preview_import(&SINGLE.replace("2qugx73x", "aaaaaaaa"), Network::Signet).is_err());
    }
    #[test]
    fn no_network_guessing() {
        assert!(preview_import(SINGLE, Network::Mainnet).is_err());
    }
    #[test]
    fn rejects_private_material() {
        assert!(preview_import("wpkh(xprv123/0/*)#aaaaaaaa", Network::Signet).is_err());
    }
    #[test]
    fn no_missing_change_guess() {
        assert!(
            preview_import(
                include_str!("../../../tests/fixtures/receive-only.txt"),
                Network::Signet
            )
            .is_err()
        );
    }
    #[test]
    fn core_pair_matches_multipath() {
        let pair = include_str!("../../../tests/fixtures/core-style-pair.json");
        assert_eq!(
            preview_import(pair, Network::Signet).unwrap().id,
            preview_import(MULTI, Network::Signet).unwrap().id
        );
    }
    #[test]
    fn oversized_import() {
        assert!(preview_import(&"a".repeat(MAX_DESCRIPTOR_BYTES + 1), Network::Signet).is_err());
    }
    #[test]
    fn receive_change_are_different() {
        let p = preview_import(SINGLE, Network::Signet).unwrap();
        assert_ne!(p.receive_descriptor, p.change_descriptor);
    }
}
