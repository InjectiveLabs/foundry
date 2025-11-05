//! Injective bank precompile implementation.
//!
//! This implements a complete bank precompile with state management for testing token operations.
//! 
//! The bank precompile is deployed at address 0x64 and supports:
//! - mint(address recipient, uint256 amount) - mint tokens to an address
//! - burn(address account, uint256 amount) - burn tokens from an address
//! - balanceOf(address token, address account) - get balance of an account
//! - transfer(address from, address to, uint256 amount) - transfer tokens between addresses
//! - totalSupply(address token) - get total supply of a token
//! - metadata(address token) - get token metadata (name, symbol, decimals)
//! - setMetadata(string name, string symbol, uint8 decimals) - set token metadata

use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;

use alloy_evm::precompiles::{DynPrecompile, PrecompileInput};
use alloy_primitives::{Address, U256, address, Bytes};
use alloy_sol_types::{SolCall, SolValue, SolType, sol};
use revm::precompile::{PrecompileError, PrecompileId, PrecompileOutput, PrecompileResult};

thread_local! {
    /// Global storage for bank state
    /// Maps (token, account) -> balance
    static BALANCES: RefCell<HashMap<(Address, Address), U256>> = RefCell::new(HashMap::new());
    /// Maps token -> total supply
    static SUPPLIES: RefCell<HashMap<Address, U256>> = RefCell::new(HashMap::new());
    /// Maps token -> (name, symbol, decimals)
    static METADATA: RefCell<HashMap<Address, (String, String, u8)>> = RefCell::new(HashMap::new());
}

// Define the bank module interface using Alloy's sol! macro
sol! {
    interface IBankModule {
        function mint(address recipient, uint256 amount) external payable returns (bool);
        function burn(address account, uint256 amount) external payable returns (bool);
        function balanceOf(address token, address account) external view returns (uint256);
        function transfer(address from, address to, uint256 amount) external payable returns (bool);
        function totalSupply(address token) external view returns (uint256);
        function metadata(address token) external view returns (string memory name, string memory symbol, uint8 decimals);
        function setMetadata(string memory name, string memory symbol, uint8 decimals) external payable returns (bool);
    }
}

/// Label of the Injective bank precompile to display in traces.
pub const INJECTIVE_BANK_LABEL: &str = "INJECTIVE_BANK_PRECOMPILE";

/// Address of the Injective bank precompile.
pub const INJECTIVE_BANK_ADDRESS: Address = address!("0x0000000000000000000000000000000000000064");

/// ID for the [Injective bank precompile](INJECTIVE_BANK_ADDRESS).
pub static PRECOMPILE_ID_INJECTIVE_BANK: PrecompileId =
    PrecompileId::Custom(Cow::Borrowed("injective bank"));

/// Gas costs for different operations
const MINT_GAS_COST: u64 = 200000;
const BURN_GAS_COST: u64 = 200000;
const BALANCE_OF_GAS_COST: u64 = 10000;
const TRANSFER_GAS_COST: u64 = 150000;
const TOTAL_SUPPLY_GAS_COST: u64 = 10000;
const METADATA_GAS_COST: u64 = 10000;
const SET_METADATA_GAS_COST: u64 = 150000;

/// Returns the Injective bank precompile.
pub fn precompile() -> DynPrecompile {
    DynPrecompile::new_stateful(PRECOMPILE_ID_INJECTIVE_BANK.clone(), injective_bank_precompile)
}

/// Injective bank precompile implementation with full state management.
///
/// This implementation uses thread-local HashMaps to maintain token balances, supplies, and metadata.
pub fn injective_bank_precompile(input: PrecompileInput<'_>) -> PrecompileResult {
    // Check minimum input length (must have at least 4 bytes for method signature)
    if input.data.len() < 4 {
        return Err(PrecompileError::Other("Input too short".into()));
    }

    // Parse method signature and route to appropriate handler
    let selector = &input.data[0..4];
    
    // Try to decode as each call type
    if selector == IBankModule::mintCall::SELECTOR {
        return handle_mint(input);
    } else if selector == IBankModule::burnCall::SELECTOR {
        return handle_burn(input);
    } else if selector == IBankModule::balanceOfCall::SELECTOR {
        return handle_balance_of(input);
    } else if selector == IBankModule::transferCall::SELECTOR {
        return handle_transfer(input);
    } else if selector == IBankModule::totalSupplyCall::SELECTOR {
        return handle_total_supply(input);
    } else if selector == IBankModule::metadataCall::SELECTOR {
        return handle_metadata(input);
    } else if selector == IBankModule::setMetadataCall::SELECTOR {
        return handle_set_metadata(input);
    }
    
    Err(PrecompileError::Other(format!(
        "Unknown method selector: {:02x}{:02x}{:02x}{:02x}",
        selector[0], selector[1], selector[2], selector[3]
    )))
}

// Storage helper functions

/// Get balance from storage
fn get_balance(token: Address, account: Address) -> U256 {
    BALANCES.with(|b| b.borrow().get(&(token, account)).copied().unwrap_or(U256::ZERO))
}

/// Set balance in storage
fn set_balance(token: Address, account: Address, amount: U256) {
    BALANCES.with(|b| b.borrow_mut().insert((token, account), amount));
}

/// Get total supply from storage
fn get_supply(token: Address) -> U256 {
    SUPPLIES.with(|s| s.borrow().get(&token).copied().unwrap_or(U256::ZERO))
}

/// Set total supply in storage
fn set_supply(token: Address, amount: U256) {
    SUPPLIES.with(|s| s.borrow_mut().insert(token, amount));
}

/// Get metadata from storage
fn get_metadata(token: Address) -> (String, String, u8) {
    METADATA.with(|m| {
        m.borrow()
            .get(&token)
            .cloned()
            .unwrap_or_else(|| (String::new(), String::new(), 0))
    })
}

/// Set metadata in storage
fn set_metadata_storage(token: Address, name: String, symbol: String, decimals: u8) {
    METADATA.with(|m| m.borrow_mut().insert(token, (name, symbol, decimals)));
}

/// Mint tokens to an address.
fn handle_mint(input: PrecompileInput<'_>) -> PrecompileResult {
    if input.gas < MINT_GAS_COST {
        return Err(PrecompileError::OutOfGas);
    }

    // Decode call data using Alloy's generated types
    let call = IBankModule::mintCall::abi_decode(&input.data)
        .map_err(|e| PrecompileError::Other(format!("Failed to decode mint call: {e}")))?;

    let token = input.caller;
    let recipient = call.recipient;
    let amount = call.amount;

    // Read current balance
    let current_balance = get_balance(token, recipient);
    
    // Check for overflow
    let new_balance = current_balance.checked_add(amount)
        .ok_or_else(|| PrecompileError::Other("Balance overflow".into()))?;
    
    // Update balance
    set_balance(token, recipient, new_balance);

    // Update total supply
    let current_supply = get_supply(token);
    let new_supply = current_supply.checked_add(amount)
        .ok_or_else(|| PrecompileError::Other("Supply overflow".into()))?;
    set_supply(token, new_supply);

    // Return encoded result
    Ok(PrecompileOutput::new(MINT_GAS_COST, Bytes::from(true.abi_encode())))
}

/// Burn tokens from an address.
fn handle_burn(input: PrecompileInput<'_>) -> PrecompileResult {
    if input.gas < BURN_GAS_COST {
        return Err(PrecompileError::OutOfGas);
    }

    // Decode call data
    let call = IBankModule::burnCall::abi_decode(&input.data)
        .map_err(|e| PrecompileError::Other(format!("Failed to decode burn call: {e}")))?;

    let token = input.caller;
    let account = call.account;
    let amount = call.amount;

    // Read current balance
    let current_balance = get_balance(token, account);
    
    // Check sufficient balance
    if current_balance < amount {
        return Err(PrecompileError::Other("Insufficient balance to burn".into()));
    }
    
    // Update balance
    let new_balance = current_balance - amount;
    set_balance(token, account, new_balance);

    // Update total supply
    let current_supply = get_supply(token);
    let new_supply = current_supply - amount;
    set_supply(token, new_supply);

    // Return encoded result
    Ok(PrecompileOutput::new(BURN_GAS_COST, Bytes::from(true.abi_encode())))
}

/// Get balance of an account for a specific token.
fn handle_balance_of(input: PrecompileInput<'_>) -> PrecompileResult {
    if input.gas < BALANCE_OF_GAS_COST {
        return Err(PrecompileError::OutOfGas);
    }

    // Decode call data
    let call = IBankModule::balanceOfCall::abi_decode(&input.data)
        .map_err(|e| PrecompileError::Other(format!("Failed to decode balanceOf call: {e}")))?;

    let token = call.token;
    let account = call.account;

    // Read balance
    let balance = get_balance(token, account);

    // Return encoded result
    Ok(PrecompileOutput::new(BALANCE_OF_GAS_COST, Bytes::from(balance.abi_encode())))
}

/// Transfer tokens between addresses.
fn handle_transfer(input: PrecompileInput<'_>) -> PrecompileResult {
    if input.gas < TRANSFER_GAS_COST {
        return Err(PrecompileError::OutOfGas);
    }

    // Decode call data
    let call = IBankModule::transferCall::abi_decode(&input.data)
        .map_err(|e| PrecompileError::Other(format!("Failed to decode transfer call: {e}")))?;

    let token = input.caller;
    let from = call.from;
    let to = call.to;
    let amount = call.amount;

    // Read from balance
    let from_balance = get_balance(token, from);
    
    // Check sufficient balance
    if from_balance < amount {
        return Err(PrecompileError::Other("Insufficient balance for transfer".into()));
    }
    
    // Update from balance
    let new_from_balance = from_balance - amount;
    set_balance(token, from, new_from_balance);

    // Read to balance
    let to_balance = get_balance(token, to);
    
    // Check for overflow
    let new_to_balance = to_balance.checked_add(amount)
        .ok_or_else(|| PrecompileError::Other("Balance overflow in recipient".into()))?;
    
    // Update to balance
    set_balance(token, to, new_to_balance);

    // Return encoded result
    Ok(PrecompileOutput::new(TRANSFER_GAS_COST, Bytes::from(true.abi_encode())))
}

/// Get total supply of a token.
fn handle_total_supply(input: PrecompileInput<'_>) -> PrecompileResult {
    if input.gas < TOTAL_SUPPLY_GAS_COST {
        return Err(PrecompileError::OutOfGas);
    }

    // Decode call data
    let call = IBankModule::totalSupplyCall::abi_decode(&input.data)
        .map_err(|e| PrecompileError::Other(format!("Failed to decode totalSupply call: {e}")))?;

    let token = call.token;

    // Read total supply
    let supply = get_supply(token);

    // Return encoded result
    Ok(PrecompileOutput::new(TOTAL_SUPPLY_GAS_COST, Bytes::from(supply.abi_encode())))
}

/// Get metadata of a token.
fn handle_metadata(input: PrecompileInput<'_>) -> PrecompileResult {
    if input.gas < METADATA_GAS_COST {
        return Err(PrecompileError::OutOfGas);
    }

    // Decode call data
    let call = IBankModule::metadataCall::abi_decode(&input.data)
        .map_err(|e| PrecompileError::Other(format!("Failed to decode metadata call: {e}")))?;

    let token = call.token;

    // Read metadata from storage
    let (name, symbol, decimals) = get_metadata(token);

    // Manually encode the return tuple (string, string, uint8) for ABI compatibility
    // This is the proper ABI encoding for a tuple with two dynamic types (strings) and one static type (uint8)
    let mut result = Vec::new();
    
    // Offsets for the two strings (both are dynamic)
    // Offset to first string (name) - starts after the 3 header words (96 bytes)
    result.extend_from_slice(&U256::from(96).to_be_bytes::<32>());
    
    // Calculate offset to second string (symbol)
    let name_bytes = name.as_bytes();
    let name_padded_len = ((name_bytes.len() + 31) / 32) * 32;
    let symbol_offset = 96 + 32 + name_padded_len; // header (96) + name length word (32) + name content
    result.extend_from_slice(&U256::from(symbol_offset).to_be_bytes::<32>());
    
    // Decimals value (static, directly encoded)
    let mut decimals_bytes = [0u8; 32];
    decimals_bytes[31] = decimals;
    result.extend_from_slice(&decimals_bytes);
    
    // Name string: length + content + padding
    result.extend_from_slice(&U256::from(name_bytes.len()).to_be_bytes::<32>());
    result.extend_from_slice(name_bytes);
    if name_bytes.len() % 32 != 0 {
        result.resize(result.len() + (32 - name_bytes.len() % 32), 0);
    }
    
    // Symbol string: length + content + padding
    let symbol_bytes = symbol.as_bytes();
    result.extend_from_slice(&U256::from(symbol_bytes.len()).to_be_bytes::<32>());
    result.extend_from_slice(symbol_bytes);
    if symbol_bytes.len() % 32 != 0 {
        result.resize(result.len() + (32 - symbol_bytes.len() % 32), 0);
    }
    
    Ok(PrecompileOutput::new(METADATA_GAS_COST, Bytes::from(result)))
}

/// Set metadata of a token.
fn handle_set_metadata(input: PrecompileInput<'_>) -> PrecompileResult {
    if input.gas < SET_METADATA_GAS_COST {
        return Err(PrecompileError::OutOfGas);
    }

    // Decode call data
    let call = IBankModule::setMetadataCall::abi_decode(&input.data)
        .map_err(|e| PrecompileError::Other(format!("Failed to decode setMetadata call: {e}")))?;

    let name = call.name;
    let symbol = call.symbol;
    let decimals = call.decimals;

    // Validate inputs
    if name.len() > 128 {
        return Err(PrecompileError::Other("Name too long (max 128 characters)".into()));
    }
    if symbol.len() > 32 {
        return Err(PrecompileError::Other("Symbol too long (max 32 characters)".into()));
    }

    // Get the token address (caller)
    let token = input.caller;

    // Store metadata
    set_metadata_storage(token, name, symbol, decimals);

    // Return encoded result
    Ok(PrecompileOutput::new(SET_METADATA_GAS_COST, Bytes::from(true.abi_encode())))
}
