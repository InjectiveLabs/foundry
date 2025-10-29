//! Injective bank precompile implementation.
//!
//! This is a barebones precompile implementation for testing token deployments.
//! Currently only supports the setMetadata function to enable FiatToken deployment.
//! 
//! The bank precompile is deployed at address 0x64 and supports:
//! - setMetadata(string name, string symbol, uint8 decimals) - set token metadata

use std::borrow::Cow;

use alloy_evm::precompiles::{DynPrecompile, PrecompileInput};
use alloy_primitives::{Address, address, Bytes};
use revm::precompile::{PrecompileError, PrecompileId, PrecompileOutput, PrecompileResult};

/// Label of the Injective bank precompile to display in traces.
pub const INJECTIVE_BANK_LABEL: &str = "INJECTIVE_BANK_PRECOMPILE";

/// Address of the Injective bank precompile.
pub const INJECTIVE_BANK_ADDRESS: Address = address!("0x0000000000000000000000000000000000000064");

/// ID for the [Injective bank precompile](INJECTIVE_BANK_ADDRESS).
pub static PRECOMPILE_ID_INJECTIVE_BANK: PrecompileId =
    PrecompileId::Custom(Cow::Borrowed("injective bank"));

/// Gas cost for setMetadata operation.
const SET_METADATA_GAS_COST: u64 = 150000;

/// Method signature for the bank precompile (first 4 bytes of keccak256(signature))
const SET_METADATA_METHOD_SIGNATURE: [u8; 4] = [0x37, 0xd2, 0xc2, 0xf4]; // setMetadata(string,string,uint8)

/// Returns the Injective bank precompile.
pub fn precompile() -> DynPrecompile {
    DynPrecompile::new_stateful(PRECOMPILE_ID_INJECTIVE_BANK.clone(), injective_bank_precompile)
}

/// Injective bank precompile implementation.
///
/// This is a barebones implementation for testing token deployments.
/// Currently only validates method signature and returns success.
/// A full implementation would integrate with the Cosmos SDK bank module.
pub fn injective_bank_precompile(input: PrecompileInput<'_>) -> PrecompileResult {
    // Check minimum input length (must have at least 4 bytes for method signature)
    if input.data.len() < 4 {
        return Err(PrecompileError::Other("Input too short".into()));
    }

    // Parse method signature
    let method_sig: [u8; 4] = input.data[0..4].try_into().map_err(|_| {
        PrecompileError::Other("Invalid method signature length".into())
    })?;
    let method_data = &input.data[4..];

    match method_sig {
        SET_METADATA_METHOD_SIGNATURE => set_metadata(input.gas, method_data),
        _ => Err(PrecompileError::Other("Unknown method".into())),
    }
}

/// Set metadata of a token.
/// Input: name (string), symbol (string), decimals (uint8)
/// Output: bool (true for success)
/// 
/// This is a barebones implementation that always returns success (true).
/// A full implementation would store metadata in the Cosmos SDK bank module.
fn set_metadata(
    gas: u64,
    _input_data: &[u8],
) -> PrecompileResult {
    if gas < SET_METADATA_GAS_COST {
        return Err(PrecompileError::OutOfGas);
    }

    // Return true to indicate successful metadata setting
    let mut output = [0u8; 32];
    output[31] = 1; // Return true (1) for success
    Ok(PrecompileOutput::new(SET_METADATA_GAS_COST, Bytes::from(output.to_vec())))
}