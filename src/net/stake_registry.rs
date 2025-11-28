#![cfg(feature = "net")]

//! Durable stake/balance store for fee enforcement and slashing.

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};

/// Account record storing stake and balance.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StakeAccount {
    /// Spendable balance for fees.
    pub balance: u64,
    /// Bonded stake.
    pub stake: u64,
    /// Whether the account is slashed.
    pub slashed: bool,
}

/// Registry keyed by base64 public key.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StakeRegistry {
    accounts: HashMap<String, StakeAccount>,
}

impl StakeRegistry {
    /// Load from JSON; missing file -> empty registry.
    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())
    }

    /// Persist to JSON.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let data = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, data).map_err(|e| e.to_string())
    }

    /// Ensure an account exists and return mutable ref.
    pub fn ensure_account(&mut self, pk: &str) -> &mut StakeAccount {
        self.accounts.entry(pk.to_string()).or_default()
    }

    /// Get account if present.
    pub fn account(&self, pk: &str) -> Option<&StakeAccount> {
        self.accounts.get(pk)
    }

    /// Return stake weight if not slashed.
    pub fn stake_for(&self, pk: &str) -> Option<u64> {
        self.accounts
            .get(pk)
            .filter(|acct| !acct.slashed)
            .map(|acct| acct.stake)
    }

    /// Credit a reward or fee refund to balance.
    pub fn credit_reward(&mut self, pk: &str, amount: u64) {
        let acct = self.ensure_account(pk);
        acct.balance = acct.balance.saturating_add(amount);
    }

    /// Debit fee from balance.
    pub fn debit_fee(&mut self, pk: &str, fee: u64) -> Result<(), String> {
        let acct = self.ensure_account(pk);
        if acct.balance < fee {
            return Err("insufficient balance".into());
        }
        acct.balance -= fee;
        Ok(())
    }

    /// Debit fee from payer and credit operator reward.
    pub fn transfer_fee(&mut self, payer: &str, operator: &str, fee: u64) -> Result<(), String> {
        self.debit_fee(payer, fee)?;
        self.credit_reward(operator, fee);
        Ok(())
    }

    /// Slash stake to zero and mark slashed.
    pub fn slash(&mut self, pk: &str) {
        let acct = self.ensure_account(pk);
        acct.stake = 0;
        acct.slashed = true;
    }
    /// Credit external funds to balance.
    pub fn fund_balance(&mut self, pk: &str, amount: u64) {
        let acct = self.ensure_account(pk);
        acct.balance = acct.balance.saturating_add(amount);
    }

    /// Move balance into bonded stake.
    pub fn bond_from_balance(&mut self, pk: &str, amount: u64) -> Result<(), String> {
        let acct = self.ensure_account(pk);
        if acct.balance < amount {
            return Err("insufficient balance to bond".into());
        }
        acct.balance -= amount;
        acct.stake = acct.stake.saturating_add(amount);
        Ok(())
    }

    /// Unbond stake back to balance.
    pub fn unbond(&mut self, pk: &str, amount: u64) -> Result<(), String> {
        let acct = self.ensure_account(pk);
        if acct.stake < amount {
            return Err("insufficient stake to unbond".into());
        }
        acct.stake -= amount;
        acct.balance = acct.balance.saturating_add(amount);
        Ok(())
    }
}
