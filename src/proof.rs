use crate::fp::Fp;
use crate::keys::PublicKey;

use ethers::{abi::ethabi::ethereum_types::FromStrRadixErr, prelude::*};
use eyre::Result;
use ff::PrimeField;
use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path, process::Command, str::FromStr};
use tempfile::NamedTempFile;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Proof {
    pub a: [U256; 2],
    pub b: [[U256; 2]; 2],
    pub c: [U256; 2],
    pub public: Vec<U256>,
}

pub fn prove<P: AsRef<Path>>(
    params: P,
    index: u32,
    token_address: U256,
    amount: U256,
    new_amount1: U256,
    new_amount2: U256,
    address_1: PublicKey,
    address_2: PublicKey,
    secret: Fp,
    proof: [[Fp; 3]; 16],
) -> Result<Proof> {
    let mut inputs_file = NamedTempFile::new()?;

    println!(
        "proof {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
        index,
        BigUint::from_str(&token_address.to_string()).unwrap(),
        BigUint::from_str(&amount.to_string()).unwrap(),
        BigUint::from_str(&new_amount1.to_string()).unwrap(),
        BigUint::from_str(&new_amount2.to_string()).unwrap(),
        BigUint::from_str(&U256::to_string(&address_1.point.x.into())).unwrap(),
        BigUint::from_str(&U256::to_string(&address_1.point.y.into())).unwrap(),
        BigUint::from_str(&U256::to_string(&address_2.point.x.into())).unwrap(),
        BigUint::from_str(&U256::to_string(&address_2.point.y.into())).unwrap(),
    );

    let json_input = format!(
        "{{ \"index\": \"{:?}\", 
        \"token_address\": \"{:?}\", 
        \"amount\": \"{:?}\", 
        \"new_amount1\": \"{:?}\", 
        \"new_amount2\": \"{:?}\", 
        \"pk_ax1\": \"{:?}\", 
        \"pk_ay1\": \"{:?}\", 
        \"pk_ax2\": \"{:?}\", 
        \"pk_ay2\": \"{:?}\", 
        \"secret\": \"{:?}\", 
        \"proof\": [{}] }}",
        index,
        BigUint::from_str(&token_address.to_string()).unwrap(),
        BigUint::from_str(&amount.to_string()).unwrap(),
        BigUint::from_str(&new_amount1.to_string()).unwrap(),
        BigUint::from_str(&new_amount2.to_string()).unwrap(),
        BigUint::from_str(&U256::to_string(&address_1.point.x.into())).unwrap(),
        BigUint::from_str(&U256::to_string(&address_1.point.y.into())).unwrap(),
        BigUint::from_str(&U256::to_string(&address_2.point.x.into())).unwrap(),
        BigUint::from_str(&U256::to_string(&address_2.point.y.into())).unwrap(),
        BigUint::from_bytes_le(secret.to_repr().as_ref()),
        proof
            .iter()
            .map(|p| format!(
                "[{}]",
                p.iter()
                    .map(|p| format!(
                        "\"{}\"",
                        BigUint::from_bytes_le(p.to_repr().as_ref()).to_string()
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            ))
            .collect::<Vec<_>>()
            .join(",")
    );

    write!(inputs_file, "{}", json_input)?;

    let witness_file = NamedTempFile::new()?;
    let wtns_gen_output = Command::new("contracts/circuits/coin_withdraw_cpp/coin_withdraw")
        .arg(inputs_file.path())
        .arg(witness_file.path())
        .output()?;

    println!(
        "STDOUT: {}",
        String::from_utf8_lossy(&wtns_gen_output.stdout)
    );
    println!(
        "STDERR: {}",
        String::from_utf8_lossy(&wtns_gen_output.stderr)
    );

    assert_eq!(wtns_gen_output.stdout.len(), 0);
    assert_eq!(wtns_gen_output.stderr.len(), 0);

    let proof_file = NamedTempFile::new()?;
    let pub_inp_file = NamedTempFile::new()?;
    let proof_gen_output = Command::new("snarkjs")
        .arg("groth16")
        .arg("prove")
        .arg(params.as_ref().as_os_str())
        .arg(witness_file.path())
        .arg(proof_file.path())
        .arg(pub_inp_file.path())
        .output()?;

    assert_eq!(proof_gen_output.stdout.len(), 0);
    assert_eq!(proof_gen_output.stderr.len(), 0);

    let generatecall_output = Command::new("snarkjs")
        .arg("generatecall")
        .arg(pub_inp_file.path())
        .arg(proof_file.path())
        .output()?;
    let mut calldata = std::str::from_utf8(&generatecall_output.stdout)?.to_string();
    calldata = calldata
        .replace("\"", "")
        .replace("[", "")
        .replace("]", "")
        .replace(" ", "")
        .replace("\n", "");

    let data = calldata
        .split(",")
        .map(|k| U256::from_str_radix(k, 16))
        .collect::<Result<Vec<U256>, FromStrRadixErr>>()?;

    let proof = Proof {
        a: data[0..2].try_into()?,
        b: [data[2..4].try_into()?, data[4..6].try_into()?],
        c: data[6..8].try_into()?,
        public: data[8..].to_vec(),
    };

    Ok(proof)
}
