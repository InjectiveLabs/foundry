//! # foundry-evm-precompiles
//!
//! Foundry EVM network custom precompiles.

use crate::celo::transfer::{
    CELO_TRANSFER_ADDRESS, CELO_TRANSFER_LABEL, PRECOMPILE_ID_CELO_TRANSFER,
};

use crate::injective::bank::{
    INJECTIVE_BANK_ADDRESS, INJECTIVE_BANK_LABEL, PRECOMPILE_ID_INJECTIVE_BANK,
};

use alloy_chains::NamedChain;
use alloy_evm::precompiles::PrecompilesMap;
use alloy_primitives::{Address, map::AddressHashMap};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod celo;
pub mod injective;

#[derive(Clone, Debug, Default, Parser, Copy, Serialize, Deserialize)]
pub struct NetworkConfigs {
    /// Enable Optimism network features.
    #[arg(help_heading = "Networks", long, visible_alias = "optimism", conflicts_with_all = ["celo", "injective"])]
    // Skipped from configs (forge) as there is no feature to be added yet.
    #[serde(skip)]
    pub optimism: bool,
    /// Enable Celo network features.
    #[arg(help_heading = "Networks", long, conflicts_with_all = ["optimism", "injective"])]
    #[serde(default)]
    pub celo: bool,
    /// Enable Injective network features.
    #[arg(help_heading = "Networks", long, conflicts_with_all = ["optimism", "celo"])]
    #[serde(default)]
    pub injective: bool,
    /// Whether to bypass prevrandao.
    #[arg(skip)]
    #[serde(default)]
    bypass_prevrandao: bool,
}

impl NetworkConfigs {
    pub fn celo(mut self, celo: bool) -> Self {
        self.celo = celo;
        self
    }

    pub fn with_optimism() -> Self {
        Self { optimism: true, ..Default::default() }
    }

    pub fn with_celo() -> Self {
        Self { celo: true, ..Default::default() }
    }

    pub fn with_injective() -> Self {
        Self { injective: true, ..Default::default() }
    }

    pub fn is_optimism(&self) -> bool {
        self.optimism
    }

    pub fn bypass_prevrandao(&self, chain_id: u64) -> bool {
        self.bypass_prevrandao
    }

    pub fn is_celo(&self) -> bool {
        self.celo
    }

    pub fn is_injective(&self) -> bool {
        self.injective
    }

    pub fn with_chain_id(mut self, chain_id: u64) -> Self {
        if let Ok(NamedChain::Celo | NamedChain::CeloSepolia) = NamedChain::try_from(chain_id) {
            self.celo = true;
        }

        // Injective chain IDs
        match chain_id {
            1776 => {
                self.injective = true;
            }
            _ => {}
        }

        self
    }

    /// Inject precompiles for configured networks.
    pub fn inject_precompiles(self, precompiles: &mut PrecompilesMap) {
        if self.celo {
            precompiles.apply_precompile(&CELO_TRANSFER_ADDRESS, move |_| {
                Some(celo::transfer::precompile())
            });
        }

        if self.injective {
            precompiles.apply_precompile(&INJECTIVE_BANK_ADDRESS, move |_| {
                Some(injective::bank::precompile())
            });
        }
    }

    /// Returns precompiles label for configured networks, to be used in traces.
    pub fn precompiles_label(self) -> AddressHashMap<String> {
        let mut labels = AddressHashMap::default();
        if self.celo {
            labels.insert(CELO_TRANSFER_ADDRESS, CELO_TRANSFER_LABEL.to_string());
        }
        if self.injective {
            labels.insert(INJECTIVE_BANK_ADDRESS, INJECTIVE_BANK_LABEL.to_string());
        }
        labels
    }

    /// Returns precompiles for configured networks.
    pub fn precompiles(self) -> BTreeMap<String, Address> {
        let mut precompiles = BTreeMap::new();
        if self.celo {
            precompiles
                .insert(PRECOMPILE_ID_CELO_TRANSFER.name().to_string(), CELO_TRANSFER_ADDRESS);
        }
        if self.injective {
            precompiles
                .insert(PRECOMPILE_ID_INJECTIVE_BANK.name().to_string(), INJECTIVE_BANK_ADDRESS);
        }
        precompiles
    }
}
